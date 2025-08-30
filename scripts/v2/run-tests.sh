#!/bin/bash
# Test runner script for module and platform testing

set -e

# Configuration
TEST_TYPE=${1:-all}
MODULE=${2:-}
COVERAGE_THRESHOLD=${COVERAGE_THRESHOLD:-70}
REPORT_PATH=${REPORT_PATH:-/reports}

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_test() { echo -e "${BLUE}[TEST]${NC} $1"; }

# Timer functions
start_timer() {
    START_TIME=$(date +%s)
}

end_timer() {
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo $DURATION
}

# Test execution functions
run_unit_tests() {
    local module=$1
    log_test "Running unit tests for ${module:-all modules}..."
    
    start_timer
    
    if [ -n "$module" ]; then
        # Module-specific tests
        cd /tests
        python -m pytest tests/unit/test_${module}.py \
            --cov=${module} \
            --cov-report=html:${REPORT_PATH}/coverage-${module}.html \
            --cov-report=term \
            --junit-xml=${REPORT_PATH}/unit-${module}.xml
    else
        # All unit tests
        python -m pytest tests/unit/ \
            --cov=. \
            --cov-report=html:${REPORT_PATH}/coverage-unit.html \
            --cov-report=term \
            --junit-xml=${REPORT_PATH}/unit-all.xml
    fi
    
    duration=$(end_timer)
    log_info "Unit tests completed in ${duration} seconds"
}

run_integration_tests() {
    local module=$1
    log_test "Running integration tests for ${module:-all modules}..."
    
    start_timer
    
    if [ -n "$module" ]; then
        python -m pytest tests/integration/test_${module}_integration.py \
            --junit-xml=${REPORT_PATH}/integration-${module}.xml
    else
        python -m pytest tests/integration/ \
            --junit-xml=${REPORT_PATH}/integration-all.xml
    fi
    
    duration=$(end_timer)
    log_info "Integration tests completed in ${duration} seconds"
}

run_e2e_tests() {
    log_test "Running end-to-end tests..."
    
    start_timer
    
    python -m pytest tests/e2e/ \
        --junit-xml=${REPORT_PATH}/e2e.xml
    
    duration=$(end_timer)
    log_info "E2E tests completed in ${duration} seconds"
}

run_performance_tests() {
    log_test "Running performance tests..."
    
    start_timer
    
    python -m pytest tests/performance/ \
        --benchmark-only \
        --benchmark-json=${REPORT_PATH}/benchmark.json
    
    duration=$(end_timer)
    log_info "Performance tests completed in ${duration} seconds"
}

run_module_pipeline() {
    local module=$1
    
    if [ -z "$module" ]; then
        log_error "Module name required for module pipeline"
        exit 1
    fi
    
    log_info "Starting module pipeline for: $module"
    start_timer
    
    # Run tests in sequence
    run_unit_tests $module
    run_integration_tests $module
    
    # Check coverage
    coverage=$(python -c "import json; print(json.load(open('${REPORT_PATH}/coverage-${module}.json'))['totals']['percent_covered'])")
    if (( $(echo "$coverage < $COVERAGE_THRESHOLD" | bc -l) )); then
        log_warn "Coverage ${coverage}% is below threshold ${COVERAGE_THRESHOLD}%"
    else
        log_info "Coverage ${coverage}% meets threshold"
    fi
    
    total_duration=$(end_timer)
    log_info "Module pipeline completed in ${total_duration} seconds"
    
    if [ $total_duration -gt 180 ]; then
        log_warn "Module pipeline exceeded 3-minute target (${total_duration}s > 180s)"
    else
        log_info "Module pipeline met 3-minute target!"
    fi
}

run_platform_pipeline() {
    log_info "Starting platform pipeline..."
    start_timer
    
    # Run all test types
    run_unit_tests
    run_integration_tests
    run_e2e_tests
    run_performance_tests
    
    total_duration=$(end_timer)
    log_info "Platform pipeline completed in ${total_duration} seconds"
    
    if [ $total_duration -gt 960 ]; then
        log_warn "Platform pipeline exceeded 16-minute target (${total_duration}s > 960s)"
    else
        log_info "Platform pipeline met 16-minute target!"
    fi
}

# Generate test report
generate_report() {
    log_info "Generating test report..."
    
    cat > ${REPORT_PATH}/summary.txt << EOF
Test Execution Summary
======================
Date: $(date)
Type: $TEST_TYPE
Module: ${MODULE:-all}

Results:
--------
EOF
    
    # Add test results
    if [ -f "${REPORT_PATH}/unit-all.xml" ]; then
        echo "Unit Tests: $(grep -c 'testcase' ${REPORT_PATH}/unit-all.xml || echo 0) tests" >> ${REPORT_PATH}/summary.txt
    fi
    
    if [ -f "${REPORT_PATH}/integration-all.xml" ]; then
        echo "Integration Tests: $(grep -c 'testcase' ${REPORT_PATH}/integration-all.xml || echo 0) tests" >> ${REPORT_PATH}/summary.txt
    fi
    
    log_info "Report generated at ${REPORT_PATH}/summary.txt"
}

# Main execution
main() {
    mkdir -p $REPORT_PATH
    
    case $TEST_TYPE in
        unit)
            run_unit_tests $MODULE
            ;;
        integration)
            run_integration_tests $MODULE
            ;;
        e2e)
            run_e2e_tests
            ;;
        performance)
            run_performance_tests
            ;;
        module)
            run_module_pipeline $MODULE
            ;;
        platform)
            run_platform_pipeline
            ;;
        all)
            run_platform_pipeline
            ;;
        *)
            log_error "Unknown test type: $TEST_TYPE"
            echo "Usage: $0 [unit|integration|e2e|performance|module|platform|all] [module_name]"
            exit 1
            ;;
    esac
    
    generate_report
    log_info "Test execution complete!"
}

main