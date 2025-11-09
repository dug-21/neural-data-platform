#!/bin/bash
# Fix Data-Ingestion to Data-Staging Connection Script

set -e

# Configuration
REDIS_URL=${REDIS_URL:-redis://localhost:6379}
GRPC_HEALTH_CHECK_TIMEOUT=${GRPC_HEALTH_CHECK_TIMEOUT:-5}
RETRY_COUNT=${RETRY_COUNT:-3}
RETRY_DELAY=${RETRY_DELAY:-2}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }

# Check service health
check_service_health() {
    local service=$1
    local port=$2
    
    log_step "Checking $service health on port $port..."
    
    if nc -z localhost $port 2>/dev/null; then
        log_info "✓ $service is running on port $port"
        return 0
    else
        log_error "✗ $service is not accessible on port $port"
        return 1
    fi
}

# Check Redis stream connectivity
check_redis_stream() {
    local stream=$1
    
    log_step "Checking Redis stream: $stream"
    
    if redis-cli -u "$REDIS_URL" XINFO STREAM "$stream" > /dev/null 2>&1; then
        log_info "✓ Stream $stream exists"
        
        # Get stream length
        local length=$(redis-cli -u "$REDIS_URL" XLEN "$stream" 2>/dev/null)
        log_info "  Stream length: $length messages"
        
        # Check consumer groups
        local groups=$(redis-cli -u "$REDIS_URL" XINFO GROUPS "$stream" 2>/dev/null | grep -c "name" || echo "0")
        log_info "  Consumer groups: $groups"
        
        return 0
    else
        log_warn "Stream $stream does not exist, creating..."
        
        # Create stream with initial message
        redis-cli -u "$REDIS_URL" XADD "$stream" "*" init "true" > /dev/null 2>&1
        
        # Create consumer group
        redis-cli -u "$REDIS_URL" XGROUP CREATE "$stream" "data-staging" 0 MKSTREAM > /dev/null 2>&1 || true
        
        log_info "✓ Stream $stream created with consumer group"
        return 0
    fi
}

# Fix gRPC connectivity
fix_grpc_connection() {
    local service=$1
    local port=$2
    
    log_step "Testing gRPC connection for $service on port $port..."
    
    # Check if grpcurl is available
    if ! command -v grpcurl > /dev/null 2>&1; then
        log_warn "grpcurl not installed, installing..."
        
        # Install grpcurl
        if [[ "$OSTYPE" == "linux-gnu"* ]]; then
            curl -sSL https://github.com/fullstorydev/grpcurl/releases/download/v1.8.7/grpcurl_1.8.7_linux_x86_64.tar.gz | tar xz -C /tmp
            sudo mv /tmp/grpcurl /usr/local/bin/
        elif [[ "$OSTYPE" == "darwin"* ]]; then
            brew install grpcurl
        fi
    fi
    
    # Test gRPC health check
    if grpcurl -plaintext localhost:$port grpc.health.v1.Health/Check > /dev/null 2>&1; then
        log_info "✓ gRPC health check passed for $service"
        return 0
    else
        log_error "✗ gRPC health check failed for $service"
        
        # Try reflection to list services
        log_info "Attempting service reflection..."
        grpcurl -plaintext localhost:$port list 2>&1 | head -5 || true
        
        return 1
    fi
}

# Fix network configuration
fix_network_config() {
    log_step "Checking Docker network configuration..."
    
    # Check if services are on the same network
    local network="neural-trader-v2"
    
    if docker network ls | grep -q "$network"; then
        log_info "✓ Network $network exists"
        
        # List connected containers
        local containers=$(docker network inspect "$network" -f '{{range .Containers}}{{.Name}} {{end}}' 2>/dev/null || echo "none")
        log_info "  Connected containers: $containers"
    else
        log_warn "Network $network does not exist, creating..."
        docker network create "$network"
        log_info "✓ Network created"
    fi
    
    # Ensure services are connected
    for service in data-ingestion data-staging; do
        if docker ps --filter "name=$service" --format "{{.Names}}" | grep -q "$service"; then
            docker network connect "$network" "$service" 2>/dev/null || true
            log_info "✓ Connected $service to network"
        fi
    done
}

# Test data flow
test_data_flow() {
    log_step "Testing data flow from ingestion to staging..."
    
    # Send test message through Redis stream
    local test_id="test-$(date +%s)"
    local stream="market-data"
    
    log_info "Sending test message with ID: $test_id"
    
    # Create test message
    redis-cli -u "$REDIS_URL" XADD "$stream" "*" \
        test_id "$test_id" \
        symbol "TEST" \
        price "100.00" \
        volume "1000" \
        timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /dev/null
    
    log_info "Test message sent to stream: $stream"
    
    # Wait for processing
    sleep 2
    
    # Check if staging received the message
    log_info "Checking if data-staging processed the message..."
    
    # Check consumer group pending messages
    local pending=$(redis-cli -u "$REDIS_URL" XPENDING "$stream" "data-staging" 2>/dev/null | head -1 | awk '{print $1}')
    
    if [ "$pending" = "0" ] || [ -z "$pending" ]; then
        log_info "✓ No pending messages - data was processed"
        return 0
    else
        log_warn "⚠ $pending pending messages in consumer group"
        
        # Try to manually acknowledge
        log_info "Attempting to clear pending messages..."
        redis-cli -u "$REDIS_URL" XAUTOCLAIM "$stream" "data-staging" "staging-1" 0 "*" COUNT 100 > /dev/null 2>&1 || true
        
        return 1
    fi
}

# Fix service dependencies
fix_dependencies() {
    log_step "Checking service dependencies..."
    
    # Check if TimescaleDB is accessible
    if PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "SELECT 1" > /dev/null 2>&1; then
        log_info "✓ TimescaleDB is accessible"
    else
        log_error "✗ TimescaleDB is not accessible"
        
        # Try to restart database
        log_info "Attempting to restart database..."
        docker-compose -f docker-compose.v2.yml restart timescaledb
        sleep 5
    fi
    
    # Check if Redis is accessible
    if redis-cli -u "$REDIS_URL" ping > /dev/null 2>&1; then
        log_info "✓ Redis is accessible"
    else
        log_error "✗ Redis is not accessible"
        
        # Try to restart Redis
        log_info "Attempting to restart Redis..."
        docker-compose -f docker-compose.v2.yml restart redis
        sleep 3
    fi
}

# Apply fixes
apply_fixes() {
    log_step "Applying connection fixes..."
    
    # 1. Fix network configuration
    fix_network_config
    
    # 2. Fix service dependencies
    fix_dependencies
    
    # 3. Create/verify Redis streams
    check_redis_stream "market-data"
    check_redis_stream "processed-data"
    check_redis_stream "signals"
    
    # 4. Test gRPC connections
    fix_grpc_connection "data-ingestion" 50051
    fix_grpc_connection "data-staging" 50052
    
    # 5. Test data flow
    test_data_flow
}

# Generate connection report
generate_report() {
    local report_file="/tmp/connection-fix-report.txt"
    
    cat > "$report_file" << EOF
Connection Fix Report
=====================
Date: $(date)

Service Status:
---------------
EOF
    
    # Check each service
    for service_port in "config-store:50050" "data-ingestion:50051" "data-staging:50052" "neural-ml-ops:50053" "neural-trading:50054"; do
        IFS=':' read -r service port <<< "$service_port"
        if check_service_health "$service" "$port" > /dev/null 2>&1; then
            echo "✓ $service (port $port): CONNECTED" >> "$report_file"
        else
            echo "✗ $service (port $port): DISCONNECTED" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" << EOF

Redis Streams:
--------------
EOF
    
    # Check Redis streams
    for stream in "market-data" "processed-data" "signals"; do
        if redis-cli -u "$REDIS_URL" EXISTS "$stream" > /dev/null 2>&1; then
            local length=$(redis-cli -u "$REDIS_URL" XLEN "$stream" 2>/dev/null)
            echo "✓ $stream: $length messages" >> "$report_file"
        else
            echo "✗ $stream: NOT FOUND" >> "$report_file"
        fi
    done
    
    cat >> "$report_file" << EOF

Network Configuration:
----------------------
$(docker network ls | grep neural-trader || echo "No neural-trader network found")

Recommendations:
----------------
1. Ensure all services are running with: make v2-up
2. Verify Redis streams are being consumed
3. Check service logs for errors
4. Monitor gRPC health endpoints

EOF
    
    log_info "Report saved to: $report_file"
    cat "$report_file"
}

# Main execution
main() {
    log_info "Starting connection fix process..."
    
    # Check initial status
    log_step "Initial connection status:"
    check_service_health "data-ingestion" 50051 || true
    check_service_health "data-staging" 50052 || true
    
    # Apply fixes
    apply_fixes
    
    # Verify fixes
    log_step "Verifying connection fixes..."
    
    local success=true
    
    # Final checks
    if ! check_service_health "data-ingestion" 50051; then
        success=false
    fi
    
    if ! check_service_health "data-staging" 50052; then
        success=false
    fi
    
    if ! test_data_flow; then
        success=false
    fi
    
    # Generate report
    generate_report
    
    if [ "$success" = true ]; then
        log_info "✓ All connection issues resolved!"
        exit 0
    else
        log_error "✗ Some connection issues remain. Check the report for details."
        exit 1
    fi
}

main