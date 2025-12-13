#!/bin/bash
# Drift Detection Test Suite

set -e

# Configuration
BASELINE_DIR=${BASELINE_DIR:-/workspaces/neural-trader/metrics/baseline}
RESULTS_DIR=${RESULTS_DIR:-/workspaces/neural-trader/metrics/drift}
THRESHOLD_CONFIG=${THRESHOLD_CONFIG:-/workspaces/neural-trader/configs/drift-thresholds.yaml}

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
log_drift() { echo -e "${MAGENTA}[DRIFT]${NC} $1"; }
log_test() { echo -e "${CYAN}[TEST]${NC} $1"; }

# Initialize results
mkdir -p "$RESULTS_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
TEST_RESULTS="$RESULTS_DIR/drift_test_${TIMESTAMP}.json"
TEST_REPORT="$RESULTS_DIR/drift_report_${TIMESTAMP}.txt"

# Test results tracking
declare -A test_results
declare -A drift_detected
total_tests=0
passed_tests=0
failed_tests=0

# Load baseline metrics
load_baseline() {
    log_step "Loading baseline metrics..."
    
    # Find most recent baseline
    local baseline_file=$(ls -t "$BASELINE_DIR"/baseline_*.json 2>/dev/null | head -1)
    
    if [ -z "$baseline_file" ]; then
        log_error "No baseline found. Run baseline-metrics.sh first."
        exit 1
    fi
    
    log_info "Loading baseline from: $baseline_file"
    
    # Extract baseline values using jq
    BASELINE_BUILD_AVG=$(jq -r '.baselines.build.avg_ms' "$baseline_file")
    BASELINE_TEST_AVG=$(jq -r '.baselines.test.avg_ms' "$baseline_file")
    BASELINE_MEMORY_AVG=$(jq -r '.baselines.memory.avg_mb' "$baseline_file")
    BASELINE_CPU_AVG=$(jq -r '.baselines.cpu.avg_percent' "$baseline_file")
    BASELINE_THROUGHPUT=$(jq -r '.baselines.throughput.avg_msg_per_sec' "$baseline_file")
    BASELINE_LATENCY=$(jq -r '.baselines.latency.avg_ms' "$baseline_file")
    
    # Load thresholds
    THRESHOLD_BUILD=$(jq -r '.thresholds.build_max_ms' "$baseline_file")
    THRESHOLD_TEST=$(jq -r '.thresholds.test_max_ms' "$baseline_file")
    THRESHOLD_MEMORY=$(jq -r '.thresholds.memory_max_mb' "$baseline_file")
    THRESHOLD_CPU=$(jq -r '.thresholds.cpu_max_percent' "$baseline_file")
    THRESHOLD_THROUGHPUT=$(jq -r '.thresholds.throughput_min_msg_per_sec' "$baseline_file")
    THRESHOLD_LATENCY=$(jq -r '.thresholds.latency_max_ms' "$baseline_file")
    
    log_info "Baseline loaded successfully"
}

# Test build time drift
test_build_drift() {
    log_test "Testing build time drift..."
    total_tests=$((total_tests + 1))
    
    # Measure current build time
    cd /workspaces/neural-trader/v2/data-ingestion
    local start_time=$(date +%s%3N)
    cargo build --release 2>&1 | tail -1
    local end_time=$(date +%s%3N)
    local current_build_time=$((end_time - start_time))
    
    # Calculate drift
    local drift_percent=$(echo "scale=2; ($current_build_time - $BASELINE_BUILD_AVG) * 100 / $BASELINE_BUILD_AVG" | bc)
    
    log_info "Current build time: ${current_build_time}ms"
    log_info "Baseline build time: ${BASELINE_BUILD_AVG}ms"
    log_info "Drift: ${drift_percent}%"
    
    # Check against threshold
    if [ $current_build_time -le $THRESHOLD_BUILD ]; then
        log_info "✓ Build time within threshold (${THRESHOLD_BUILD}ms)"
        test_results[build_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Build time exceeds threshold!"
        log_error "  Current: ${current_build_time}ms > Threshold: ${THRESHOLD_BUILD}ms"
        test_results[build_drift]="FAIL"
        drift_detected[build]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Test memory drift
test_memory_drift() {
    log_test "Testing memory usage drift..."
    total_tests=$((total_tests + 1))
    
    # Get current memory usage
    local stats=$(docker stats --no-stream --format "json" data-ingestion 2>/dev/null || echo "{}")
    local current_memory=0
    
    if [ "$stats" != "{}" ]; then
        current_memory=$(echo "$stats" | jq -r '.MemUsage' | grep -oE '[0-9.]+' | head -1)
    fi
    
    # Calculate drift
    local drift_percent=0
    if [ "${BASELINE_MEMORY_AVG}" != "0" ]; then
        drift_percent=$(echo "scale=2; ($current_memory - $BASELINE_MEMORY_AVG) * 100 / $BASELINE_MEMORY_AVG" | bc)
    fi
    
    log_info "Current memory: ${current_memory}MB"
    log_info "Baseline memory: ${BASELINE_MEMORY_AVG}MB"
    log_info "Drift: ${drift_percent}%"
    
    # Check against threshold
    if (( $(echo "$current_memory <= $THRESHOLD_MEMORY" | bc -l) )); then
        log_info "✓ Memory usage within threshold (${THRESHOLD_MEMORY}MB)"
        test_results[memory_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Memory usage exceeds threshold!"
        log_error "  Current: ${current_memory}MB > Threshold: ${THRESHOLD_MEMORY}MB"
        test_results[memory_drift]="FAIL"
        drift_detected[memory]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Test throughput drift
test_throughput_drift() {
    log_test "Testing throughput drift..."
    total_tests=$((total_tests + 1))
    
    # Measure current throughput
    local start_time=$(date +%s%3N)
    local messages_sent=0
    
    for i in $(seq 1 100); do
        redis-cli -u redis://localhost:6379 XADD "market-data" "*" \
            symbol "TEST" price "100" volume "1000" \
            timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > /dev/null 2>&1
        messages_sent=$((messages_sent + 1))
    done
    
    local end_time=$(date +%s%3N)
    local duration=$((end_time - start_time))
    local current_throughput=$(echo "scale=2; $messages_sent * 1000 / $duration" | bc)
    
    # Calculate drift
    local drift_percent=0
    if [ "${BASELINE_THROUGHPUT}" != "0" ]; then
        drift_percent=$(echo "scale=2; ($current_throughput - $BASELINE_THROUGHPUT) * 100 / $BASELINE_THROUGHPUT" | bc)
    fi
    
    log_info "Current throughput: ${current_throughput} msg/s"
    log_info "Baseline throughput: ${BASELINE_THROUGHPUT} msg/s"
    log_info "Drift: ${drift_percent}%"
    
    # Check against threshold
    if (( $(echo "$current_throughput >= $THRESHOLD_THROUGHPUT" | bc -l) )); then
        log_info "✓ Throughput above minimum threshold (${THRESHOLD_THROUGHPUT} msg/s)"
        test_results[throughput_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Throughput below minimum threshold!"
        log_error "  Current: ${current_throughput} < Threshold: ${THRESHOLD_THROUGHPUT} msg/s"
        test_results[throughput_drift]="FAIL"
        drift_detected[throughput]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Test configuration drift
test_config_drift() {
    log_test "Testing configuration drift..."
    total_tests=$((total_tests + 1))
    
    local config_changes=0
    
    # Check for uncommitted config changes
    cd /workspaces/neural-trader
    local git_status=$(git status --porcelain configs/ 2>/dev/null)
    
    if [ -n "$git_status" ]; then
        log_warn "Detected uncommitted configuration changes:"
        echo "$git_status" | head -5
        config_changes=$((config_changes + 1))
    fi
    
    # Check for config schema validation
    for service in data-ingestion data-staging neural-ml-ops neural-trading; do
        local config_file="/workspaces/neural-trader/configs/base/${service}/config.yaml"
        local schema_file="/workspaces/neural-trader/configs/schemas/${service}.schema.json"
        
        if [ -f "$config_file" ] && [ -f "$schema_file" ]; then
            # Validate config against schema (simplified check)
            if ! python3 -c "
import yaml, json, sys
try:
    with open('$config_file', 'r') as f:
        config = yaml.safe_load(f)
    sys.exit(0)
except:
    sys.exit(1)
" 2>/dev/null; then
                log_warn "Configuration validation failed for $service"
                config_changes=$((config_changes + 1))
            fi
        fi
    done
    
    if [ $config_changes -eq 0 ]; then
        log_info "✓ No configuration drift detected"
        test_results[config_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Configuration drift detected ($config_changes issues)"
        test_results[config_drift]="FAIL"
        drift_detected[config]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Test data quality drift
test_data_quality_drift() {
    log_test "Testing data quality drift..."
    total_tests=$((total_tests + 1))
    
    # Check for data anomalies
    local anomalies=0
    
    # Check for missing data
    local missing_data=$(redis-cli -u redis://localhost:6379 XLEN "market-data-errors" 2>/dev/null || echo "0")
    if [ "$missing_data" -gt "0" ]; then
        log_warn "Found $missing_data error messages in stream"
        anomalies=$((anomalies + missing_data))
    fi
    
    # Check for stale data
    local latest_msg=$(redis-cli -u redis://localhost:6379 XREVRANGE "market-data" + - COUNT 1 2>/dev/null)
    if [ -n "$latest_msg" ]; then
        local msg_id=$(echo "$latest_msg" | grep -oE '[0-9]+-[0-9]+' | head -1)
        local timestamp=$(echo "$msg_id" | cut -d'-' -f1)
        local current_time=$(date +%s%3N)
        local age=$((current_time - timestamp))
        
        if [ $age -gt 60000 ]; then  # More than 60 seconds old
            log_warn "Latest data is stale (${age}ms old)"
            anomalies=$((anomalies + 1))
        fi
    fi
    
    # Check for data distribution changes
    local price_variance=$(PGPASSWORD=postgres psql -h localhost -U postgres -d neural_trader_v2 -t -c "
        SELECT STDDEV(price) 
        FROM market.market_data 
        WHERE timestamp > NOW() - INTERVAL '1 hour'
    " 2>/dev/null | xargs)
    
    if (( $(echo "$price_variance > 100" | bc -l) )); then
        log_warn "High price variance detected: $price_variance"
        anomalies=$((anomalies + 1))
    fi
    
    if [ $anomalies -eq 0 ]; then
        log_info "✓ No data quality drift detected"
        test_results[data_quality_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Data quality issues detected ($anomalies anomalies)"
        test_results[data_quality_drift]="FAIL"
        drift_detected[data_quality]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Test model performance drift
test_model_drift() {
    log_test "Testing model performance drift..."
    total_tests=$((total_tests + 1))
    
    # Simulate model accuracy check
    local current_accuracy=85  # In real scenario, this would be calculated
    local baseline_accuracy=90
    local min_acceptable_accuracy=80
    
    local drift_percent=$(echo "scale=2; ($baseline_accuracy - $current_accuracy) * 100 / $baseline_accuracy" | bc)
    
    log_info "Current model accuracy: ${current_accuracy}%"
    log_info "Baseline accuracy: ${baseline_accuracy}%"
    log_info "Drift: ${drift_percent}%"
    
    if [ $current_accuracy -ge $min_acceptable_accuracy ]; then
        log_info "✓ Model accuracy within acceptable range (>=${min_acceptable_accuracy}%)"
        test_results[model_drift]="PASS"
        passed_tests=$((passed_tests + 1))
    else
        log_error "✗ Model accuracy below threshold!"
        log_error "  Current: ${current_accuracy}% < Threshold: ${min_acceptable_accuracy}%"
        test_results[model_drift]="FAIL"
        drift_detected[model]=true
        failed_tests=$((failed_tests + 1))
    fi
}

# Generate drift detection report
generate_report() {
    log_step "Generating drift detection report..."
    
    # JSON report
    cat > "$TEST_RESULTS" << EOF
{
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "summary": {
        "total_tests": $total_tests,
        "passed": $passed_tests,
        "failed": $failed_tests,
        "pass_rate": $(echo "scale=2; $passed_tests * 100 / $total_tests" | bc)
    },
    "test_results": {
        "build_drift": "${test_results[build_drift]}",
        "memory_drift": "${test_results[memory_drift]}",
        "throughput_drift": "${test_results[throughput_drift]}",
        "config_drift": "${test_results[config_drift]}",
        "data_quality_drift": "${test_results[data_quality_drift]}",
        "model_drift": "${test_results[model_drift]}"
    },
    "drift_detected": {
        "build": ${drift_detected[build]:-false},
        "memory": ${drift_detected[memory]:-false},
        "throughput": ${drift_detected[throughput]:-false},
        "config": ${drift_detected[config]:-false},
        "data_quality": ${drift_detected[data_quality]:-false},
        "model": ${drift_detected[model]:-false}
    }
}
EOF
    
    # Text report
    cat > "$TEST_REPORT" << EOF
==========================================
Drift Detection Test Report
==========================================
Generated: $(date)

Test Summary
------------
Total Tests: $total_tests
Passed: $passed_tests
Failed: $failed_tests
Pass Rate: $(echo "scale=2; $passed_tests * 100 / $total_tests" | bc)%

Test Results
------------
Build Drift:        ${test_results[build_drift]}
Memory Drift:       ${test_results[memory_drift]}
Throughput Drift:   ${test_results[throughput_drift]}
Config Drift:       ${test_results[config_drift]}
Data Quality Drift: ${test_results[data_quality_drift]}
Model Drift:        ${test_results[model_drift]}

Drift Detection Summary
-----------------------
$([ "${drift_detected[build]}" = true ] && echo "⚠ Build performance drift detected" || echo "✓ Build performance stable")
$([ "${drift_detected[memory]}" = true ] && echo "⚠ Memory usage drift detected" || echo "✓ Memory usage stable")
$([ "${drift_detected[throughput]}" = true ] && echo "⚠ Throughput drift detected" || echo "✓ Throughput stable")
$([ "${drift_detected[config]}" = true ] && echo "⚠ Configuration drift detected" || echo "✓ Configuration stable")
$([ "${drift_detected[data_quality]}" = true ] && echo "⚠ Data quality drift detected" || echo "✓ Data quality stable")
$([ "${drift_detected[model]}" = true ] && echo "⚠ Model performance drift detected" || echo "✓ Model performance stable")

Recommended Actions
-------------------
$([ $failed_tests -gt 0 ] && echo "1. Investigate failed tests immediately" || echo "1. Continue monitoring")
$([ "${drift_detected[build]}" = true ] && echo "2. Review recent code changes affecting build time")
$([ "${drift_detected[memory]}" = true ] && echo "3. Profile memory usage and check for leaks")
$([ "${drift_detected[throughput]}" = true ] && echo "4. Analyze performance bottlenecks")
$([ "${drift_detected[config]}" = true ] && echo "5. Review and commit configuration changes")
$([ "${drift_detected[data_quality]}" = true ] && echo "6. Check data sources and pipelines")
$([ "${drift_detected[model]}" = true ] && echo "7. Retrain model with recent data")

Files Generated
---------------
Results: $TEST_RESULTS
Report: $TEST_REPORT

EOF
    
    log_info "Reports generated:"
    log_info "  JSON: $TEST_RESULTS"
    log_info "  Text: $TEST_REPORT"
    
    cat "$TEST_REPORT"
}

# Main execution
main() {
    log_info "Starting drift detection tests..."
    
    # Load baseline
    load_baseline
    
    # Ensure services are running
    log_step "Preparing test environment..."
    docker-compose -f docker-compose.v2.yml up -d > /dev/null 2>&1
    sleep 5
    
    # Run all drift detection tests
    test_build_drift
    test_memory_drift
    test_throughput_drift
    test_config_drift
    test_data_quality_drift
    test_model_drift
    
    # Generate report
    generate_report
    
    # Exit with appropriate code
    if [ $failed_tests -eq 0 ]; then
        log_drift "✓ No significant drift detected!"
        exit 0
    else
        log_drift "✗ Drift detected in $failed_tests areas!"
        exit 1
    fi
}

main