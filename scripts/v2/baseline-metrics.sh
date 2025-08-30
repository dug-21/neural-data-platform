#!/bin/bash
# Baseline Metrics Collection Script for Performance Tracking

set -e

# Configuration
METRICS_DIR=${METRICS_DIR:-/workspaces/neural-trader/metrics/baseline}
TEST_ITERATIONS=${TEST_ITERATIONS:-10}
WARMUP_ITERATIONS=${WARMUP_ITERATIONS:-3}
DB_URL=${DB_URL:-postgresql://postgres:postgres@localhost:5432/neural_trader_v2}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_step() { echo -e "${BLUE}[STEP]${NC} $1"; }
log_metric() { echo -e "${CYAN}[METRIC]${NC} $1"; }
log_baseline() { echo -e "${MAGENTA}[BASELINE]${NC} $1"; }

# Initialize metrics storage
mkdir -p "$METRICS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BASELINE_FILE="$METRICS_DIR/baseline_${TIMESTAMP}.json"
SUMMARY_FILE="$METRICS_DIR/summary_${TIMESTAMP}.txt"

# Baseline metrics to collect
declare -A baselines
declare -A measurements

# Collect build time baseline
collect_build_baseline() {
    log_step "Collecting build time baseline..."
    
    local build_times=()
    
    for i in $(seq 1 $TEST_ITERATIONS); do
        log_info "Build iteration $i/$TEST_ITERATIONS..."
        
        # Clean build cache
        rm -rf /tmp/cargo-cache/* 2>/dev/null || true
        
        # Measure build time
        local start_time=$(date +%s%3N)
        
        # Build specific module
        cd /workspaces/neural-trader/v2/data-ingestion
        cargo build --release 2>&1 | tail -1
        
        local end_time=$(date +%s%3N)
        local build_time=$((end_time - start_time))
        
        if [ $i -gt $WARMUP_ITERATIONS ]; then
            build_times+=($build_time)
            log_metric "Build time: ${build_time}ms"
        else
            log_info "Warmup build: ${build_time}ms (not counted)"
        fi
    done
    
    # Calculate statistics
    local sum=0
    local min=${build_times[0]}
    local max=${build_times[0]}
    
    for time in "${build_times[@]}"; do
        sum=$((sum + time))
        [ $time -lt $min ] && min=$time
        [ $time -gt $max ] && max=$time
    done
    
    local count=${#build_times[@]}
    local avg=$((sum / count))
    
    baselines[build_avg]=$avg
    baselines[build_min]=$min
    baselines[build_max]=$max
    
    log_baseline "Build: avg=${avg}ms, min=${min}ms, max=${max}ms"
}

# Collect test execution baseline
collect_test_baseline() {
    log_step "Collecting test execution baseline..."
    
    local test_times=()
    local coverage_percentages=()
    
    for i in $(seq 1 $TEST_ITERATIONS); do
        log_info "Test iteration $i/$TEST_ITERATIONS..."
        
        # Run tests with coverage
        local start_time=$(date +%s%3N)
        
        cd /workspaces/neural-trader/v2/data-ingestion
        local output=$(cargo test --release 2>&1)
        
        local end_time=$(date +%s%3N)
        local test_time=$((end_time - start_time))
        
        # Extract coverage if available
        local coverage=$(echo "$output" | grep -oE '[0-9]+\.[0-9]+%' | head -1 | tr -d '%')
        
        if [ $i -gt $WARMUP_ITERATIONS ]; then
            test_times+=($test_time)
            [ -n "$coverage" ] && coverage_percentages+=($coverage)
            log_metric "Test time: ${test_time}ms, Coverage: ${coverage:-N/A}%"
        else
            log_info "Warmup test: ${test_time}ms (not counted)"
        fi
    done
    
    # Calculate test time statistics
    local sum=0
    local min=${test_times[0]}
    local max=${test_times[0]}
    
    for time in "${test_times[@]}"; do
        sum=$((sum + time))
        [ $time -lt $min ] && min=$time
        [ $time -gt $max ] && max=$time
    done
    
    local count=${#test_times[@]}
    local avg=$((sum / count))
    
    baselines[test_avg]=$avg
    baselines[test_min]=$min
    baselines[test_max]=$max
    
    # Calculate coverage statistics
    if [ ${#coverage_percentages[@]} -gt 0 ]; then
        local cov_sum=0
        for cov in "${coverage_percentages[@]}"; do
            cov_sum=$(echo "$cov_sum + $cov" | bc)
        done
        baselines[coverage_avg]=$(echo "scale=2; $cov_sum / ${#coverage_percentages[@]}" | bc)
    fi
    
    log_baseline "Tests: avg=${avg}ms, min=${min}ms, max=${max}ms"
    [ -n "${baselines[coverage_avg]}" ] && log_baseline "Coverage: ${baselines[coverage_avg]}%"
}

# Collect memory usage baseline
collect_memory_baseline() {
    log_step "Collecting memory usage baseline..."
    
    # Start services
    log_info "Starting services for memory baseline..."
    docker-compose -f docker-compose.v2.yml up -d data-ingestion data-staging 2>/dev/null
    
    # Wait for services to stabilize
    sleep 10
    
    local memory_samples=()
    local cpu_samples=()
    
    for i in $(seq 1 $TEST_ITERATIONS); do
        log_info "Memory sample $i/$TEST_ITERATIONS..."
        
        # Get container stats
        local stats=$(docker stats --no-stream --format "json" data-ingestion 2>/dev/null || echo "{}")
        
        if [ "$stats" != "{}" ]; then
            local mem_usage=$(echo "$stats" | jq -r '.MemUsage' | grep -oE '[0-9.]+' | head -1)
            local cpu_usage=$(echo "$stats" | jq -r '.CPUPerc' | tr -d '%')
            
            if [ $i -gt $WARMUP_ITERATIONS ]; then
                [ -n "$mem_usage" ] && memory_samples+=($mem_usage)
                [ -n "$cpu_usage" ] && cpu_samples+=($cpu_usage)
                log_metric "Memory: ${mem_usage}MB, CPU: ${cpu_usage}%"
            fi
        fi
        
        sleep 2
    done
    
    # Calculate memory statistics
    if [ ${#memory_samples[@]} -gt 0 ]; then
        local sum=0
        for mem in "${memory_samples[@]}"; do
            sum=$(echo "$sum + $mem" | bc)
        done
        baselines[memory_avg]=$(echo "scale=2; $sum / ${#memory_samples[@]}" | bc)
    fi
    
    # Calculate CPU statistics
    if [ ${#cpu_samples[@]} -gt 0 ]; then
        local sum=0
        for cpu in "${cpu_samples[@]}"; do
            sum=$(echo "$sum + $cpu" | bc)
        done
        baselines[cpu_avg]=$(echo "scale=2; $sum / ${#cpu_samples[@]}" | bc)
    fi
    
    log_baseline "Memory: avg=${baselines[memory_avg]:-0}MB"
    log_baseline "CPU: avg=${baselines[cpu_avg]:-0}%"
}

# Collect throughput baseline
collect_throughput_baseline() {
    log_step "Collecting throughput baseline..."
    
    local throughput_samples=()
    local latency_samples=()
    
    # Run load test
    log_info "Running throughput test..."
    
    for i in $(seq 1 $TEST_ITERATIONS); do
        log_info "Throughput iteration $i/$TEST_ITERATIONS..."
        
        # Send batch of messages
        local start_time=$(date +%s%3N)
        local messages_sent=0
        
        for j in $(seq 1 100); do
            redis-cli -u redis://localhost:6379 XADD "market-data" "*" \
                symbol "TEST" \
                price "$((100 + RANDOM % 50))" \
                volume "$((1000 + RANDOM % 9000))" \
                timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /dev/null 2>&1
            
            messages_sent=$((messages_sent + 1))
        done
        
        local end_time=$(date +%s%3N)
        local duration=$((end_time - start_time))
        local throughput=$(echo "scale=2; $messages_sent * 1000 / $duration" | bc)
        local avg_latency=$(echo "scale=2; $duration / $messages_sent" | bc)
        
        if [ $i -gt $WARMUP_ITERATIONS ]; then
            throughput_samples+=($throughput)
            latency_samples+=($avg_latency)
            log_metric "Throughput: ${throughput} msg/s, Latency: ${avg_latency}ms"
        else
            log_info "Warmup: ${throughput} msg/s (not counted)"
        fi
    done
    
    # Calculate statistics
    if [ ${#throughput_samples[@]} -gt 0 ]; then
        local sum=0
        for tp in "${throughput_samples[@]}"; do
            sum=$(echo "$sum + $tp" | bc)
        done
        baselines[throughput_avg]=$(echo "scale=2; $sum / ${#throughput_samples[@]}" | bc)
    fi
    
    if [ ${#latency_samples[@]} -gt 0 ]; then
        local sum=0
        for lat in "${latency_samples[@]}"; do
            sum=$(echo "$sum + $lat" | bc)
        done
        baselines[latency_avg]=$(echo "scale=2; $sum / ${#latency_samples[@]}" | bc)
    fi
    
    log_baseline "Throughput: ${baselines[throughput_avg]:-0} msg/s"
    log_baseline "Latency: ${baselines[latency_avg]:-0}ms"
}

# Collect database performance baseline
collect_database_baseline() {
    log_step "Collecting database performance baseline..."
    
    local query_times=()
    local insert_times=()
    
    for i in $(seq 1 $TEST_ITERATIONS); do
        log_info "Database iteration $i/$TEST_ITERATIONS..."
        
        # Test insert performance
        local start_time=$(date +%s%3N)
        
        PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "
            INSERT INTO market.market_data (symbol, price, volume, timestamp)
            SELECT 
                'TEST' || generate_series,
                100 + random() * 50,
                1000 + random() * 9000,
                NOW() - INTERVAL '1 hour' * generate_series
            FROM generate_series(1, 1000);
        " > /dev/null 2>&1
        
        local end_time=$(date +%s%3N)
        local insert_time=$((end_time - start_time))
        
        # Test query performance
        start_time=$(date +%s%3N)
        
        PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -c "
            SELECT 
                time_bucket('1 minute', timestamp) as bucket,
                symbol,
                avg(price) as avg_price,
                sum(volume) as total_volume
            FROM market.market_data
            WHERE timestamp > NOW() - INTERVAL '1 hour'
            GROUP BY bucket, symbol
            ORDER BY bucket DESC
            LIMIT 100;
        " > /dev/null 2>&1
        
        end_time=$(date +%s%3N)
        local query_time=$((end_time - start_time))
        
        if [ $i -gt $WARMUP_ITERATIONS ]; then
            insert_times+=($insert_time)
            query_times+=($query_time)
            log_metric "Insert: ${insert_time}ms, Query: ${query_time}ms"
        else
            log_info "Warmup: Insert=${insert_time}ms, Query=${query_time}ms (not counted)"
        fi
    done
    
    # Calculate statistics
    if [ ${#insert_times[@]} -gt 0 ]; then
        local sum=0
        for time in "${insert_times[@]}"; do
            sum=$((sum + time))
        done
        baselines[db_insert_avg]=$((sum / ${#insert_times[@]}))
    fi
    
    if [ ${#query_times[@]} -gt 0 ]; then
        local sum=0
        for time in "${query_times[@]}"; do
            sum=$((sum + time))
        done
        baselines[db_query_avg]=$((sum / ${#query_times[@]}))
    fi
    
    log_baseline "DB Insert: ${baselines[db_insert_avg]:-0}ms"
    log_baseline "DB Query: ${baselines[db_query_avg]:-0}ms"
}

# Save baseline metrics
save_baselines() {
    log_step "Saving baseline metrics..."
    
    # Create JSON output
    cat > "$BASELINE_FILE" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "system": {
        "os": "$(uname -s)",
        "arch": "$(uname -m)",
        "cores": $(nproc),
        "memory_gb": $(free -g | awk '/^Mem:/{print $2}')
    },
    "baselines": {
        "build": {
            "avg_ms": ${baselines[build_avg]:-0},
            "min_ms": ${baselines[build_min]:-0},
            "max_ms": ${baselines[build_max]:-0}
        },
        "test": {
            "avg_ms": ${baselines[test_avg]:-0},
            "min_ms": ${baselines[test_min]:-0},
            "max_ms": ${baselines[test_max]:-0},
            "coverage_percent": ${baselines[coverage_avg]:-0}
        },
        "memory": {
            "avg_mb": ${baselines[memory_avg]:-0}
        },
        "cpu": {
            "avg_percent": ${baselines[cpu_avg]:-0}
        },
        "throughput": {
            "avg_msg_per_sec": ${baselines[throughput_avg]:-0}
        },
        "latency": {
            "avg_ms": ${baselines[latency_avg]:-0}
        },
        "database": {
            "insert_avg_ms": ${baselines[db_insert_avg]:-0},
            "query_avg_ms": ${baselines[db_query_avg]:-0}
        }
    },
    "thresholds": {
        "build_max_ms": $((${baselines[build_avg]:-180000} * 15 / 10)),
        "test_max_ms": $((${baselines[test_avg]:-60000} * 15 / 10)),
        "memory_max_mb": $(echo "${baselines[memory_avg]:-100} * 2" | bc),
        "cpu_max_percent": 80,
        "throughput_min_msg_per_sec": $(echo "${baselines[throughput_avg]:-100} * 0.7" | bc),
        "latency_max_ms": $(echo "${baselines[latency_avg]:-10} * 2" | bc),
        "db_insert_max_ms": $((${baselines[db_insert_avg]:-1000} * 2)),
        "db_query_max_ms": $((${baselines[db_query_avg]:-500} * 2))
    }
}
EOF
    
    log_info "Baseline metrics saved to: $BASELINE_FILE"
}

# Generate summary report
generate_summary() {
    log_step "Generating summary report..."
    
    cat > "$SUMMARY_FILE" << EOF
========================================
Baseline Metrics Summary
========================================
Generated: $(date)
System: $(uname -s) $(uname -m) | $(nproc) cores | $(free -g | awk '/^Mem:/{print $2}')GB RAM

Build Performance
-----------------
Average: ${baselines[build_avg]:-0}ms
Minimum: ${baselines[build_min]:-0}ms
Maximum: ${baselines[build_max]:-0}ms
Threshold: $((${baselines[build_avg]:-180000} * 15 / 10))ms (150% of avg)

Test Performance
----------------
Average: ${baselines[test_avg]:-0}ms
Minimum: ${baselines[test_min]:-0}ms
Maximum: ${baselines[test_max]:-0}ms
Coverage: ${baselines[coverage_avg]:-0}%
Threshold: $((${baselines[test_avg]:-60000} * 15 / 10))ms (150% of avg)

Resource Usage
--------------
Memory (avg): ${baselines[memory_avg]:-0}MB
CPU (avg): ${baselines[cpu_avg]:-0}%
Memory threshold: $(echo "${baselines[memory_avg]:-100} * 2" | bc)MB (200% of avg)
CPU threshold: 80%

Throughput & Latency
--------------------
Throughput: ${baselines[throughput_avg]:-0} msg/s
Latency: ${baselines[latency_avg]:-0}ms
Min throughput: $(echo "${baselines[throughput_avg]:-100} * 0.7" | bc) msg/s (70% of baseline)
Max latency: $(echo "${baselines[latency_avg]:-10} * 2" | bc)ms (200% of baseline)

Database Performance
--------------------
Insert (avg): ${baselines[db_insert_avg]:-0}ms
Query (avg): ${baselines[db_query_avg]:-0}ms
Insert threshold: $((${baselines[db_insert_avg]:-1000} * 2))ms
Query threshold: $((${baselines[db_query_avg]:-500} * 2))ms

Recommendations
---------------
1. Use these baselines for drift detection
2. Alert when metrics exceed thresholds
3. Re-baseline after major changes
4. Track trends over time

Files Generated
---------------
Baseline: $BASELINE_FILE
Summary: $SUMMARY_FILE

EOF
    
    log_info "Summary report saved to: $SUMMARY_FILE"
    cat "$SUMMARY_FILE"
}

# Main execution
main() {
    log_info "Starting baseline metrics collection..."
    log_info "Iterations: $TEST_ITERATIONS (plus $WARMUP_ITERATIONS warmup)"
    
    # Ensure services are running
    log_step "Preparing environment..."
    docker-compose -f docker-compose.v2.yml up -d > /dev/null 2>&1
    sleep 5
    
    # Collect all baselines
    collect_build_baseline
    collect_test_baseline
    collect_memory_baseline
    collect_throughput_baseline
    collect_database_baseline
    
    # Save and report
    save_baselines
    generate_summary
    
    log_info "✓ Baseline metrics collection completed!"
    log_info "Use $BASELINE_FILE for drift detection"
    
    exit 0
}

main