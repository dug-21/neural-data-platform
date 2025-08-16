#!/bin/bash

# Phase 1.4 Integration Testing Script
# Comprehensive test execution for neural trader integration testing
# 
# This script executes all integration tests in the correct order with
# proper environment setup and result collection.

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEST_RESULTS_DIR="$PROJECT_ROOT/test-results/integration"
LOG_LEVEL="${LOG_LEVEL:-info}"
PARALLEL_JOBS="${PARALLEL_JOBS:-4}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Test configuration
declare -A TEST_SUITES=(
    ["health_monitoring"]="tests/integration/health_monitoring_integration_test.rs"
    ["end_to_end_workflow"]="tests/integration/end_to_end_workflow_test.rs"
    ["multi_symbol_load"]="tests/integration/multi_symbol_load_test.rs"
    ["neuralfix_integration"]="src/neural/tests/test_neuralfix_integration.rs"
    ["performance_benchmarks"]="src/neural/tests/test_neuralfix_performance.rs"
    ["reliability_tests"]="src/neural/tests/test_neuralfix_reliability.rs"
)

# Test execution results
declare -A TEST_RESULTS=()
declare -A TEST_DURATIONS=()

# Setup functions
setup_test_environment() {
    log_info "Setting up test environment..."
    
    # Create test results directory
    mkdir -p "$TEST_RESULTS_DIR"
    
    # Set environment variables for testing
    export RUST_LOG="${LOG_LEVEL}"
    export RUST_BACKTRACE=1
    export TEST_MODE=integration
    export NEURAL_TRADER_CONFIG_PATH="$PROJECT_ROOT/config/test.toml"
    
    # Setup test database if needed
    if command -v docker &> /dev/null; then
        log_info "Setting up test database with Docker..."
        docker-compose -f "$PROJECT_ROOT/docker/test/docker-compose.test.yml" up -d timescaledb redis
        sleep 10 # Wait for services to be ready
    else
        log_warning "Docker not available, using mock services"
    fi
    
    # Build project in test mode
    log_info "Building project for integration tests..."
    cd "$PROJECT_ROOT"
    cargo build --tests --features test-utils
    
    log_success "Test environment setup complete"
}

cleanup_test_environment() {
    log_info "Cleaning up test environment..."
    
    # Stop test services
    if command -v docker &> /dev/null; then
        docker-compose -f "$PROJECT_ROOT/docker/test/docker-compose.test.yml" down
    fi
    
    # Clean up temporary files
    find "$PROJECT_ROOT" -name "*.tmp" -delete 2>/dev/null || true
    
    log_success "Test environment cleanup complete"
}

# Test execution functions
run_test_suite() {
    local suite_name="$1"
    local test_file="$2"
    local start_time
    local end_time
    local duration
    
    log_info "Running test suite: $suite_name"
    start_time=$(date +%s)
    
    # Create suite-specific results directory
    local suite_results_dir="$TEST_RESULTS_DIR/$suite_name"
    mkdir -p "$suite_results_dir"
    
    # Run the test suite with appropriate flags
    local test_cmd="cargo test --test $(basename "$test_file" .rs) --features test-utils"
    
    # Add suite-specific configurations
    case "$suite_name" in
        "health_monitoring")
            test_cmd="$test_cmd -- --test-threads=1"
            ;;
        "multi_symbol_load")
            test_cmd="$test_cmd -- --test-threads=1 --nocapture"
            export LOAD_TEST_DURATION=60 # Shorter duration for CI
            ;;
        "performance_benchmarks")
            test_cmd="$test_cmd -- --test-threads=1 --ignored"
            ;;
    esac
    
    # Execute test with output capture
    local output_file="$suite_results_dir/output.log"
    local junit_file="$suite_results_dir/junit.xml"
    
    if eval "$test_cmd" > "$output_file" 2>&1; then
        TEST_RESULTS["$suite_name"]="PASSED"
        log_success "Test suite $suite_name completed successfully"
    else
        TEST_RESULTS["$suite_name"]="FAILED"
        log_error "Test suite $suite_name failed"
        
        # Show last few lines of output for quick debugging
        log_error "Last 10 lines of output:"
        tail -10 "$output_file" | sed 's/^/  /'
    fi
    
    end_time=$(date +%s)
    duration=$((end_time - start_time))
    TEST_DURATIONS["$suite_name"]="$duration"
    
    log_info "Test suite $suite_name completed in ${duration}s"
}

run_parallel_tests() {
    log_info "Running integration tests in parallel (max $PARALLEL_JOBS jobs)..."
    
    local pids=()
    local job_count=0
    
    for suite_name in "${!TEST_SUITES[@]}"; do
        local test_file="${TEST_SUITES[$suite_name]}"
        
        # Check if test file exists
        if [[ ! -f "$PROJECT_ROOT/$test_file" ]]; then
            log_warning "Test file not found: $test_file (skipping $suite_name)"
            TEST_RESULTS["$suite_name"]="SKIPPED"
            continue
        fi
        
        # Wait if we've reached max jobs
        if (( job_count >= PARALLEL_JOBS )); then
            wait "${pids[0]}"
            pids=("${pids[@]:1}") # Remove first element
            ((job_count--))
        fi
        
        # Start test in background
        (run_test_suite "$suite_name" "$test_file") &
        local pid=$!
        pids+=("$pid")
        ((job_count++))
        
        log_info "Started test suite $suite_name (PID: $pid)"
    done
    
    # Wait for all remaining jobs
    for pid in "${pids[@]}"; do
        wait "$pid"
    done
    
    log_success "All parallel tests completed"
}

run_sequential_critical_tests() {
    log_info "Running critical tests sequentially..."
    
    # These tests must run sequentially to avoid resource conflicts
    local critical_tests=(
        "health_monitoring"
        "end_to_end_workflow"
    )
    
    for suite_name in "${critical_tests[@]}"; do
        if [[ -n "${TEST_SUITES[$suite_name]:-}" ]]; then
            run_test_suite "$suite_name" "${TEST_SUITES[$suite_name]}"
        fi
    done
}

# Reporting functions
generate_test_report() {
    local report_file="$TEST_RESULTS_DIR/integration_test_report.md"
    local html_report="$TEST_RESULTS_DIR/integration_test_report.html"
    
    log_info "Generating test report..."
    
    cat > "$report_file" << EOF
# Phase 1.4 Integration Test Report

**Execution Date:** $(date)
**Test Environment:** Integration Testing
**Total Test Suites:** ${#TEST_SUITES[@]}

## Test Results Summary

| Test Suite | Status | Duration (s) | Notes |
|------------|--------|--------------|-------|
EOF
    
    local total_passed=0
    local total_failed=0
    local total_skipped=0
    
    for suite_name in "${!TEST_SUITES[@]}"; do
        local status="${TEST_RESULTS[$suite_name]:-UNKNOWN}"
        local duration="${TEST_DURATIONS[$suite_name]:-0}"
        local notes=""
        
        case "$status" in
            "PASSED") ((total_passed++)) ;;
            "FAILED") 
                ((total_failed++))
                notes="❌ Check logs for details"
                ;;
            "SKIPPED") 
                ((total_skipped++))
                notes="⚠️ Test file not found"
                ;;
        esac
        
        echo "| $suite_name | $status | $duration | $notes |" >> "$report_file"
    done
    
    cat >> "$report_file" << EOF

## Summary Statistics

- **Passed:** $total_passed
- **Failed:** $total_failed
- **Skipped:** $total_skipped
- **Success Rate:** $(( total_passed * 100 / (total_passed + total_failed + 1) ))%

## Test Suite Details

EOF
    
    # Add detailed results for each suite
    for suite_name in "${!TEST_SUITES[@]}"; do
        local status="${TEST_RESULTS[$suite_name]:-UNKNOWN}"
        local output_file="$TEST_RESULTS_DIR/$suite_name/output.log"
        
        cat >> "$report_file" << EOF
### $suite_name

**Status:** $status
**Duration:** ${TEST_DURATIONS[$suite_name]:-0}s
**Test File:** ${TEST_SUITES[$suite_name]}

EOF
        
        if [[ -f "$output_file" ]]; then
            echo "**Output Summary:**" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            tail -20 "$output_file" >> "$report_file"
            echo "\`\`\`" >> "$report_file"
            echo "" >> "$report_file"
        fi
    done
    
    log_success "Test report generated: $report_file"
    
    # Generate HTML report if pandoc is available
    if command -v pandoc &> /dev/null; then
        pandoc "$report_file" -o "$html_report"
        log_success "HTML report generated: $html_report"
    fi
}

print_summary() {
    log_info "Integration Test Execution Summary"
    echo "=================================="
    
    local total_passed=0
    local total_failed=0
    local total_skipped=0
    local total_duration=0
    
    for suite_name in "${!TEST_SUITES[@]}"; do
        local status="${TEST_RESULTS[$suite_name]:-UNKNOWN}"
        local duration="${TEST_DURATIONS[$suite_name]:-0}"
        
        printf "%-25s: %s (%ss)\n" "$suite_name" "$status" "$duration"
        
        case "$status" in
            "PASSED") ((total_passed++)) ;;
            "FAILED") ((total_failed++)) ;;
            "SKIPPED") ((total_skipped++)) ;;
        esac
        
        total_duration=$((total_duration + duration))
    done
    
    echo "=================================="
    echo "Total Passed:  $total_passed"
    echo "Total Failed:  $total_failed"
    echo "Total Skipped: $total_skipped"
    echo "Total Time:    ${total_duration}s"
    echo "Success Rate:  $(( total_passed * 100 / (total_passed + total_failed + 1) ))%"
    
    if (( total_failed > 0 )); then
        log_error "Integration tests failed! Check logs for details."
        return 1
    else
        log_success "All integration tests passed!"
        return 0
    fi
}

# Performance monitoring
monitor_system_resources() {
    log_info "Starting system resource monitoring..."
    
    local monitor_file="$TEST_RESULTS_DIR/system_resources.log"
    
    # Start resource monitoring in background
    {
        echo "timestamp,cpu_percent,memory_mb,disk_io_mb"
        while true; do
            if command -v top &> /dev/null; then
                # Simple resource monitoring
                local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
                local cpu_percent=$(top -l 1 -n 0 | grep "CPU usage" | awk '{print $3}' | sed 's/%//' | head -1)
                local memory_mb=$(ps -o rss= -p $$ | awk '{print $1/1024}')
                local disk_io="0" # Simplified
                
                echo "$timestamp,$cpu_percent,$memory_mb,$disk_io"
            fi
            sleep 30
        done
    } > "$monitor_file" &
    
    local monitor_pid=$!
    echo "$monitor_pid" > "$TEST_RESULTS_DIR/monitor.pid"
    
    log_info "Resource monitoring started (PID: $monitor_pid)"
}

stop_monitoring() {
    if [[ -f "$TEST_RESULTS_DIR/monitor.pid" ]]; then
        local monitor_pid=$(cat "$TEST_RESULTS_DIR/monitor.pid")
        if kill -0 "$monitor_pid" 2>/dev/null; then
            kill "$monitor_pid"
            log_info "Resource monitoring stopped"
        fi
        rm -f "$TEST_RESULTS_DIR/monitor.pid"
    fi
}

# Main execution
main() {
    local start_time
    local end_time
    local total_duration
    
    log_info "Starting Phase 1.4 Integration Testing"
    log_info "Project root: $PROJECT_ROOT"
    log_info "Results directory: $TEST_RESULTS_DIR"
    
    start_time=$(date +%s)
    
    # Trap for cleanup
    trap 'cleanup_test_environment; stop_monitoring; exit 1' ERR INT TERM
    
    # Setup
    setup_test_environment
    monitor_system_resources
    
    # Execute tests
    if [[ "${SEQUENTIAL:-false}" == "true" ]]; then
        log_info "Running all tests sequentially..."
        for suite_name in "${!TEST_SUITES[@]}"; do
            run_test_suite "$suite_name" "${TEST_SUITES[$suite_name]}"
        done
    else
        # Run critical tests sequentially first
        run_sequential_critical_tests
        
        # Then run remaining tests in parallel
        local remaining_tests=()
        for suite_name in "${!TEST_SUITES[@]}"; do
            if [[ "$suite_name" != "health_monitoring" && "$suite_name" != "end_to_end_workflow" ]]; then
                remaining_tests+=("$suite_name")
            fi
        done
        
        if (( ${#remaining_tests[@]} > 0 )); then
            # Create temporary test suites map for parallel execution
            declare -A PARALLEL_TEST_SUITES=()
            for suite_name in "${remaining_tests[@]}"; do
                PARALLEL_TEST_SUITES["$suite_name"]="${TEST_SUITES[$suite_name]}"
            done
            
            # Override TEST_SUITES temporarily
            local original_test_suites
            declare -A original_test_suites
            for k in "${!TEST_SUITES[@]}"; do
                original_test_suites["$k"]="${TEST_SUITES[$k]}"
            done
            
            TEST_SUITES=()
            for k in "${!PARALLEL_TEST_SUITES[@]}"; do
                TEST_SUITES["$k"]="${PARALLEL_TEST_SUITES[$k]}"
            done
            
            run_parallel_tests
            
            # Restore original test suites
            TEST_SUITES=()
            for k in "${!original_test_suites[@]}"; do
                TEST_SUITES["$k"]="${original_test_suites[$k]}"
            done
        fi
    fi
    
    # Generate reports
    generate_test_report
    
    # Calculate total duration
    end_time=$(date +%s)
    total_duration=$((end_time - start_time))
    
    log_info "Total execution time: ${total_duration}s"
    
    # Cleanup
    stop_monitoring
    cleanup_test_environment
    
    # Print summary and exit with appropriate code
    if print_summary; then
        log_success "Phase 1.4 Integration Testing completed successfully!"
        exit 0
    else
        log_error "Phase 1.4 Integration Testing failed!"
        exit 1
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --sequential)
            SEQUENTIAL=true
            shift
            ;;
        --parallel-jobs)
            PARALLEL_JOBS="$2"
            shift 2
            ;;
        --log-level)
            LOG_LEVEL="$2"
            shift 2
            ;;
        --help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --sequential        Run all tests sequentially instead of parallel"
            echo "  --parallel-jobs N   Maximum number of parallel jobs (default: 4)"
            echo "  --log-level LEVEL   Log level: error, warn, info, debug (default: info)"
            echo "  --help              Show this help message"
            echo ""
            echo "Environment variables:"
            echo "  LOAD_TEST_DURATION  Duration for load tests in seconds (default: 300)"
            echo "  TEST_MODE           Test mode: integration, unit, all (default: integration)"
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# Execute main function
main "$@"