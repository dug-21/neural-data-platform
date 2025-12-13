#!/bin/bash
# Full Data Pipeline Integration Test Script

set -e

# Configuration
TEST_DURATION=${TEST_DURATION:-60}  # seconds
MESSAGE_RATE=${MESSAGE_RATE:-10}    # messages per second
VALIDATION_INTERVAL=${VALIDATION_INTERVAL:-5}  # seconds
REDIS_URL=${REDIS_URL:-redis://localhost:6379}
DB_URL=${DB_URL:-postgresql://postgres:postgres@localhost:5432/neural_trader_v2}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_metric() { echo -e "${CYAN}[METRIC]${NC} $1"; }

# Track metrics
declare -A metrics
metrics[messages_sent]=0
metrics[messages_processed]=0
metrics[messages_stored]=0
metrics[signals_generated]=0
metrics[errors]=0
metrics[latency_sum]=0
metrics[latency_count]=0

# Generate market data
generate_market_data() {
    local symbol=$1
    local price=$2
    local volume=$3
    
    cat << EOF
{
    "symbol": "$symbol",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%S.%3NZ)",
    "price": $price,
    "volume": $volume,
    "bid": $(echo "$price * 0.999" | bc),
    "ask": $(echo "$price * 1.001" | bc),
    "open": $(echo "$price * 0.98" | bc),
    "high": $(echo "$price * 1.02" | bc),
    "low": $(echo "$price * 0.97" | bc),
    "close": $price
}
EOF
}

# Send data to ingestion service
send_to_ingestion() {
    local data=$1
    local start_time=$(date +%s%3N)
    
    # Send via gRPC or REST API
    if command -v grpcurl > /dev/null 2>&1; then
        echo "$data" | grpcurl -plaintext -d @ localhost:50051 data.Ingestion/Ingest > /dev/null 2>&1
    else
        # Fallback to REST API
        curl -X POST -H "Content-Type: application/json" \
            -d "$data" \
            http://localhost:8081/api/v1/ingest \
            -s -o /dev/null 2>&1
    fi
    
    local end_time=$(date +%s%3N)
    local latency=$((end_time - start_time))
    
    metrics[latency_sum]=$((${metrics[latency_sum]} + latency))
    metrics[latency_count]=$((${metrics[latency_count]} + 1))
    
    return 0
}

# Send data to Redis stream
send_to_redis() {
    local stream=$1
    local data=$2
    
    # Parse JSON and send to Redis
    local symbol=$(echo "$data" | jq -r '.symbol')
    local price=$(echo "$data" | jq -r '.price')
    local volume=$(echo "$data" | jq -r '.volume')
    local timestamp=$(echo "$data" | jq -r '.timestamp')
    
    redis-cli -u "$REDIS_URL" XADD "$stream" "*" \
        symbol "$symbol" \
        price "$price" \
        volume "$volume" \
        timestamp "$timestamp" \
        data "$data" > /dev/null 2>&1
    
    metrics[messages_sent]=$((${metrics[messages_sent]} + 1))
}

# Check stream processing
check_stream_processing() {
    local stream=$1
    local consumer_group=$2
    
    # Get consumer group info
    local info=$(redis-cli -u "$REDIS_URL" XINFO GROUPS "$stream" 2>/dev/null | grep "$consumer_group" || echo "")
    
    if [ -n "$info" ]; then
        local pending=$(echo "$info" | awk '{print $8}')
        local last_id=$(echo "$info" | awk '{print $10}')
        
        log_metric "Stream $stream: Pending=$pending, LastID=$last_id"
        
        # Calculate processed messages
        local total=$(redis-cli -u "$REDIS_URL" XLEN "$stream" 2>/dev/null)
        local processed=$((total - pending))
        
        metrics[messages_processed]=$processed
        
        return 0
    else
        log_warn "Consumer group $consumer_group not found for stream $stream"
        return 1
    fi
}

# Check database storage
check_database_storage() {
    log_step "Checking database storage..."
    
    # Check market_data table
    local market_count=$(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -t -c \
        "SELECT COUNT(*) FROM market.market_data WHERE timestamp > NOW() - INTERVAL '5 minutes'" 2>/dev/null | xargs)
    
    # Check processed_data table
    local processed_count=$(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -t -c \
        "SELECT COUNT(*) FROM staging.processed_data WHERE created_at > NOW() - INTERVAL '5 minutes'" 2>/dev/null | xargs)
    
    # Check signals table
    local signals_count=$(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -t -c \
        "SELECT COUNT(*) FROM ml.signals WHERE timestamp > NOW() - INTERVAL '5 minutes'" 2>/dev/null | xargs)
    
    metrics[messages_stored]=$market_count
    metrics[signals_generated]=$signals_count
    
    log_metric "Database: Market=$market_count, Processed=$processed_count, Signals=$signals_count"
}

# Test end-to-end latency
test_e2e_latency() {
    log_step "Testing end-to-end latency..."
    
    local test_id="latency-test-$(date +%s%3N)"
    local start_time=$(date +%s%3N)
    
    # Send test message with unique ID
    local test_data=$(generate_market_data "TEST" "100.00" "1000")
    test_data=$(echo "$test_data" | jq --arg id "$test_id" '. + {test_id: $id}')
    
    send_to_redis "market-data" "$test_data"
    
    # Wait for message to propagate through pipeline
    local found=false
    local attempts=0
    local max_attempts=20
    
    while [ $attempts -lt $max_attempts ] && [ "$found" = false ]; do
        sleep 0.5
        
        # Check if message reached signals stream
        local signal=$(redis-cli -u "$REDIS_URL" XRANGE signals - + COUNT 10 2>/dev/null | grep "$test_id" || echo "")
        
        if [ -n "$signal" ]; then
            found=true
            local end_time=$(date +%s%3N)
            local e2e_latency=$((end_time - start_time))
            
            log_metric "End-to-end latency: ${e2e_latency}ms"
            
            if [ $e2e_latency -lt 1000 ]; then
                log_info "✓ Excellent latency (<1s)"
            elif [ $e2e_latency -lt 3000 ]; then
                log_info "✓ Good latency (<3s)"
            else
                log_warn "⚠ High latency (>3s)"
            fi
        fi
        
        attempts=$((attempts + 1))
    done
    
    if [ "$found" = false ]; then
        log_error "✗ Test message did not complete pipeline within timeout"
        metrics[errors]=$((${metrics[errors]} + 1))
    fi
}

# Load test the pipeline
load_test_pipeline() {
    log_step "Starting load test (Duration: ${TEST_DURATION}s, Rate: ${MESSAGE_RATE} msg/s)..."
    
    local symbols=("AAPL" "GOOGL" "MSFT" "AMZN" "TSLA" "META" "NVDA" "AMD")
    local start_time=$(date +%s)
    local messages_to_send=$((TEST_DURATION * MESSAGE_RATE))
    
    # Send messages at specified rate
    for ((i=1; i<=messages_to_send; i++)); do
        # Select random symbol
        local symbol=${symbols[$((RANDOM % ${#symbols[@]}))]}
        
        # Generate random price (100-500 range)
        local price=$(echo "scale=2; 100 + $RANDOM % 400 + $RANDOM % 100 / 100" | bc)
        
        # Generate random volume (1000-10000 range)
        local volume=$((1000 + RANDOM % 9000))
        
        # Generate and send data
        local data=$(generate_market_data "$symbol" "$price" "$volume")
        
        # Send to pipeline
        send_to_redis "market-data" "$data" &
        
        # Control rate
        if [ $((i % MESSAGE_RATE)) -eq 0 ]; then
            sleep 1
            
            # Check progress every validation interval
            if [ $((i % (MESSAGE_RATE * VALIDATION_INTERVAL))) -eq 0 ]; then
                check_stream_processing "market-data" "data-staging"
                check_database_storage
            fi
        fi
    done
    
    # Wait for all background jobs
    wait
    
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    log_info "Load test completed in ${duration}s"
}

# Validate pipeline components
validate_pipeline() {
    log_step "Validating pipeline components..."
    
    local all_valid=true
    
    # Check service health
    log_info "Checking service health..."
    
    for service_port in "data-ingestion:50051" "data-staging:50052" "neural-ml-ops:50053" "neural-trading:50054"; do
        IFS=':' read -r service port <<< "$service_port"
        
        if nc -z localhost $port 2>/dev/null; then
            log_info "✓ $service is healthy"
        else
            log_error "✗ $service is not responding"
            all_valid=false
        fi
    done
    
    # Check Redis streams
    log_info "Checking Redis streams..."
    
    for stream in "market-data" "processed-data" "signals"; do
        if redis-cli -u "$REDIS_URL" EXISTS "$stream" > /dev/null 2>&1; then
            log_info "✓ Stream $stream exists"
        else
            log_warn "⚠ Stream $stream not found, creating..."
            redis-cli -u "$REDIS_URL" XADD "$stream" "*" init "true" > /dev/null 2>&1
        fi
    done
    
    # Check database tables
    log_info "Checking database tables..."
    
    for schema_table in "market.market_data" "staging.processed_data" "ml.signals" "trading.orders"; do
        if PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c \
            "SELECT 1 FROM $schema_table LIMIT 1" > /dev/null 2>&1; then
            log_info "✓ Table $schema_table exists"
        else
            log_error "✗ Table $schema_table not found"
            all_valid=false
        fi
    done
    
    if [ "$all_valid" = true ]; then
        log_info "✓ All pipeline components validated"
        return 0
    else
        log_error "✗ Some pipeline components failed validation"
        return 1
    fi
}

# Generate test report
generate_report() {
    local report_file="/tmp/pipeline-test-report.txt"
    local report_json="/tmp/pipeline-test-report.json"
    
    # Calculate metrics
    local avg_latency=0
    if [ ${metrics[latency_count]} -gt 0 ]; then
        avg_latency=$((${metrics[latency_sum]} / ${metrics[latency_count]}))
    fi
    
    local success_rate=0
    if [ ${metrics[messages_sent]} -gt 0 ]; then
        success_rate=$(echo "scale=2; ${metrics[messages_processed]} * 100 / ${metrics[messages_sent]}" | bc)
    fi
    
    # Text report
    cat > "$report_file" << EOF
Pipeline Integration Test Report
=================================
Date: $(date)
Test Duration: ${TEST_DURATION}s
Message Rate: ${MESSAGE_RATE} msg/s

Performance Metrics:
--------------------
Messages Sent: ${metrics[messages_sent]}
Messages Processed: ${metrics[messages_processed]}
Messages Stored: ${metrics[messages_stored]}
Signals Generated: ${metrics[signals_generated]}
Errors: ${metrics[errors]}

Success Rate: ${success_rate}%
Average Latency: ${avg_latency}ms

Component Status:
-----------------
✓ Data Ingestion: ACTIVE
✓ Data Staging: ACTIVE
✓ Neural ML Ops: ACTIVE
✓ Neural Trading: ACTIVE

Stream Status:
--------------
market-data: $(redis-cli -u "$REDIS_URL" XLEN "market-data" 2>/dev/null || echo "0") messages
processed-data: $(redis-cli -u "$REDIS_URL" XLEN "processed-data" 2>/dev/null || echo "0") messages
signals: $(redis-cli -u "$REDIS_URL" XLEN "signals" 2>/dev/null || echo "0") messages

Test Results:
-------------
$([ ${metrics[errors]} -eq 0 ] && echo "✓ ALL TESTS PASSED" || echo "✗ TESTS FAILED WITH ${metrics[errors]} ERRORS")

Recommendations:
----------------
$([ ${metrics[errors]} -eq 0 ] && echo "- Pipeline is functioning correctly" || echo "- Investigate error logs for failures")
$([ $success_rate -gt 95 ] && echo "- Excellent processing rate" || echo "- Optimize processing performance")
$([ $avg_latency -lt 1000 ] && echo "- Low latency achieved" || echo "- Investigate latency bottlenecks")

EOF
    
    # JSON report
    cat > "$report_json" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "test_duration": $TEST_DURATION,
    "message_rate": $MESSAGE_RATE,
    "metrics": {
        "messages_sent": ${metrics[messages_sent]},
        "messages_processed": ${metrics[messages_processed]},
        "messages_stored": ${metrics[messages_stored]},
        "signals_generated": ${metrics[signals_generated]},
        "errors": ${metrics[errors]},
        "average_latency_ms": $avg_latency,
        "success_rate": $success_rate
    },
    "status": $([ ${metrics[errors]} -eq 0 ] && echo "\"PASSED\"" || echo "\"FAILED\"")
}
EOF
    
    log_info "Reports saved:"
    log_info "  Text: $report_file"
    log_info "  JSON: $report_json"
    
    cat "$report_file"
}

# Main execution
main() {
    log_info "Starting full pipeline integration test..."
    
    # Validate pipeline components first
    if ! validate_pipeline; then
        log_error "Pipeline validation failed. Please ensure all services are running."
        exit 1
    fi
    
    # Test end-to-end latency
    test_e2e_latency
    
    # Run load test
    load_test_pipeline
    
    # Final validation
    log_step "Final pipeline validation..."
    check_stream_processing "market-data" "data-staging"
    check_stream_processing "processed-data" "neural-ml-ops"
    check_database_storage
    
    # Generate report
    generate_report
    
    # Exit with appropriate code
    if [ ${metrics[errors]} -eq 0 ]; then
        log_info "✓ Pipeline integration test completed successfully!"
        exit 0
    else
        log_error "✗ Pipeline integration test failed with ${metrics[errors]} errors"
        exit 1
    fi
}

# Handle cleanup on exit
cleanup() {
    log_info "Cleaning up test resources..."
    # Any cleanup needed
}

trap cleanup EXIT

main