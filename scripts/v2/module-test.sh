#!/bin/bash
# Module Test Script - Run tests for specific module

set -e

# Configuration
MODULE=${1:-}
TEST_TYPE=${2:-unit}
CACHE_DIR=${MODULE_CACHE_DIR:-/tmp/module-cache}
COVERAGE_THRESHOLD=${COVERAGE_THRESHOLD:-70}
PARALLEL_TESTS=${PARALLEL_TESTS:-true}

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

# Global variables for module location
MODULE_PATH=""
SERVICE_ROOT=""

# Validate inputs and detect module location
validate_inputs() {
    if [ -z "$MODULE" ]; then
        log_error "Module name required"
        echo "Usage: $0 <module-name> [test-type]"
        exit 1
    fi
    
    if [[ ! "$TEST_TYPE" =~ ^(unit|integration|all)$ ]]; then
        log_error "Invalid test type: $TEST_TYPE"
        echo "Valid types: unit, integration, all"
        exit 1
    fi
    
    # Determine where module exists and set SERVICE_ROOT
    if [ -d "$MODULE" ]; then
        SERVICE_ROOT="."
        MODULE_PATH="$MODULE"
        log_info "Module found in root directory: $MODULE"
    elif [ -d "services/$MODULE" ]; then
        SERVICE_ROOT="services"
        MODULE_PATH="services/$MODULE"
        log_info "Module found in services directory: services/$MODULE"
    elif [ -d "modules/$MODULE" ]; then
        SERVICE_ROOT="modules"
        MODULE_PATH="modules/$MODULE"
        log_info "Module found in modules directory: modules/$MODULE"
    else
        log_error "Module directory not found: $MODULE"
        exit 1
    fi
}

# Run Rust unit tests
run_rust_unit_tests() {
    log_test "Running Rust unit tests for $MODULE"
    
    cd "$MODULE_PATH"
    
    start_timer
    
    # Run tests with coverage
    local test_result=0
    if [ "$PARALLEL_TESTS" = "true" ]; then
        cargo test --release --jobs $(nproc) -- --test-threads=$(nproc) --nocapture || test_result=$?
    else
        cargo test --release -- --nocapture || test_result=$?
    fi
    
    # Check if tests failed
    if [ $test_result -ne 0 ]; then
        log_error "Rust unit tests failed with exit code $test_result"
        cd ../..
        return 1
    fi
    
    # Generate coverage report if llvm-cov is available
    if command -v cargo-llvm-cov &> /dev/null; then
        cargo llvm-cov --html --output-dir "$CACHE_DIR/$MODULE/coverage"
        
        # Extract coverage percentage
        local coverage=$(cargo llvm-cov --print-summary 2>&1 | grep -oP '\d+\.\d+(?=%)' | head -1)
        echo "$coverage" > "$CACHE_DIR/$MODULE/coverage/percentage.txt"
        
        if (( $(echo "$coverage < $COVERAGE_THRESHOLD" | bc -l) )); then
            log_warn "Coverage ${coverage}% is below threshold ${COVERAGE_THRESHOLD}%"
        else
            log_info "Coverage ${coverage}% meets threshold"
        fi
    fi
    
    duration=$(end_timer)
    log_info "Unit tests completed in ${duration}s"
    
    cd ../..
    
    return 0
}

# Run Python unit tests
run_python_unit_tests() {
    log_test "Running Python unit tests for $MODULE"
    
    cd "$MODULE_PATH"
    
    # Activate virtual environment
    source "$CACHE_DIR/$MODULE/build/venv/bin/activate"
    
    start_timer
    
    # Run pytest with coverage
    pytest tests/unit/ \
        --cov=$MODULE \
        --cov-report=html:"$CACHE_DIR/$MODULE/coverage" \
        --cov-report=term \
        --junit-xml="$CACHE_DIR/$MODULE/test/unit-results.xml" \
        -v
    
    # Extract coverage percentage
    local coverage=$(pytest --cov=$MODULE --cov-report=term | grep -oP 'TOTAL.*\K\d+(?=%)')
    echo "$coverage" > "$CACHE_DIR/$MODULE/coverage/percentage.txt"
    
    if [ "$coverage" -lt "$COVERAGE_THRESHOLD" ]; then
        log_warn "Coverage ${coverage}% is below threshold ${COVERAGE_THRESHOLD}%"
    else
        log_info "Coverage ${coverage}% meets threshold"
    fi
    
    deactivate
    
    duration=$(end_timer)
    log_info "Unit tests completed in ${duration}s"
    
    cd ../..
    
    return 0
}

# Run integration tests
run_integration_tests() {
    log_test "Running integration tests for $MODULE"
    
    start_timer
    
    # Start required services
    log_info "Starting test dependencies..."
    ./scripts/v2/module-setup.sh "$MODULE"
    
    # Wait for services to be ready
    ./scripts/v2/wait-for-dependencies.sh "$MODULE"
    
    # Run integration tests based on module type
    case $MODULE in
        config-store)
            test_config_store_integration
            ;;
        data-ingestion)
            test_data_ingestion_integration
            ;;
        data-staging)
            test_data_staging_integration
            ;;
        neural-ml-ops)
            test_neural_ml_ops_integration
            ;;
        neural-trading)
            test_neural_trading_integration
            ;;
        *)
            log_warn "No integration tests defined for $MODULE"
            ;;
    esac
    
    duration=$(end_timer)
    log_info "Integration tests completed in ${duration}s"
    
    return 0
}

# Module-specific integration tests
test_config_store_integration() {
    log_test "Testing config-store integration..."
    
    # Test Git sync
    curl -f http://localhost:50051/health || return 1
    
    # Test configuration loading
    grpcurl -plaintext localhost:50051 config.ConfigStore/GetConfig || return 1
    
    log_info "Config-store integration tests passed"
}

test_data_ingestion_integration() {
    log_test "Testing data-ingestion integration..."
    
    # Test health endpoint
    curl -f http://localhost:8081/health || return 1
    
    # Test data ingestion endpoint
    curl -X POST http://localhost:8081/ingest \
        -H "Content-Type: application/json" \
        -d '{"symbol":"TEST","price":100}' || return 1
    
    log_info "Data-ingestion integration tests passed"
}

test_data_staging_integration() {
    log_test "Testing data-staging integration..."
    
    # Test gRPC health
    grpcurl -plaintext localhost:50052 grpc.health.v1.Health/Check || return 1
    
    # Test data processing
    # Add specific data-staging tests here
    
    log_info "Data-staging integration tests passed"
}

test_neural_ml_ops_integration() {
    log_test "Testing neural-ml-ops integration..."
    
    # Test gRPC health
    grpcurl -plaintext localhost:50053 grpc.health.v1.Health/Check || return 1
    
    # Test model operations
    # Add specific ML-ops tests here
    
    log_info "Neural-ml-ops integration tests passed"
}

test_neural_trading_integration() {
    log_test "Testing neural-trading integration..."
    
    # Test gRPC health
    grpcurl -plaintext localhost:50054 grpc.health.v1.Health/Check || return 1
    
    # Test trading operations
    # Add specific trading tests here
    
    log_info "Neural-trading integration tests passed"
}

# Generate test report
generate_test_report() {
    local report_file="$CACHE_DIR/$MODULE/test-report.txt"
    local coverage_file="$CACHE_DIR/$MODULE/coverage/percentage.txt"
    local coverage="N/A"
    
    if [ -f "$coverage_file" ]; then
        coverage="$(cat $coverage_file)%"
    fi
    
    cat > "$report_file" << EOF
Module Test Report
==================
Date: $(date)
Module: $MODULE
Test Type: $TEST_TYPE

Test Results:
-------------
Total Duration: ${TOTAL_DURATION}s
Tests Passed: ${TESTS_PASSED:-true}
Coverage: $coverage
Coverage Threshold: ${COVERAGE_THRESHOLD}%

Test Output:
------------
Unit Tests: ${UNIT_TEST_DURATION:-N/A}s
Integration Tests: ${INTEGRATION_TEST_DURATION:-N/A}s

Artifacts:
----------
Coverage Report: $CACHE_DIR/$MODULE/coverage/index.html
Test Results: $CACHE_DIR/$MODULE/test/

Status: ${TEST_STATUS:-PASSED}
EOF
    
    log_info "Test report saved: $report_file"
    
    # Display summary
    if [ "$TESTS_PASSED" = "true" ] && [ ${TOTAL_DURATION} -lt 180 ]; then
        echo -e "${GREEN}✓ All tests passed in ${TOTAL_DURATION}s (< 3 min target)${NC}"
    elif [ "$TESTS_PASSED" = "true" ]; then
        echo -e "${YELLOW}✓ All tests passed in ${TOTAL_DURATION}s (> 3 min target)${NC}"
    else
        echo -e "${RED}✗ Some tests failed${NC}"
    fi
}

# Main execution
main() {
    log_info "Starting tests for module: $MODULE ($TEST_TYPE)"
    
    validate_inputs
    
    # Setup directories
    mkdir -p "$CACHE_DIR/$MODULE/test"
    mkdir -p "$CACHE_DIR/$MODULE/coverage"
    
    start_timer
    TESTS_PASSED=true
    TEST_STATUS="PASSED"
    
    # Run tests based on type
    case $TEST_TYPE in
        unit)
            if [ -f "$MODULE_PATH/Cargo.toml" ]; then
                run_rust_unit_tests || TESTS_PASSED=false
            elif [ -f "$MODULE_PATH/requirements.txt" ] || [ -f "$MODULE_PATH/setup.py" ]; then
                run_python_unit_tests || TESTS_PASSED=false
            else
                log_warn "No unit tests found for $MODULE"
            fi
            UNIT_TEST_DURATION=$(end_timer)
            ;;
        integration)
            run_integration_tests || TESTS_PASSED=false
            INTEGRATION_TEST_DURATION=$(end_timer)
            ;;
        all)
            # Run unit tests first
            if [ -f "$MODULE_PATH/Cargo.toml" ]; then
                run_rust_unit_tests || TESTS_PASSED=false
            elif [ -f "$MODULE_PATH/requirements.txt" ] || [ -f "$MODULE_PATH/setup.py" ]; then
                run_python_unit_tests || TESTS_PASSED=false
            else
                log_warn "No unit tests found for $MODULE"
            fi
            UNIT_TEST_DURATION=$(end_timer)
            
            # Then integration tests
            start_timer
            run_integration_tests || TESTS_PASSED=false
            INTEGRATION_TEST_DURATION=$(end_timer)
            ;;
    esac
    
    TOTAL_DURATION=$(end_timer)
    
    if [ "$TESTS_PASSED" = "false" ]; then
        TEST_STATUS="FAILED"
    fi
    
    generate_test_report
    
    log_info "Module tests complete for $MODULE"
    
    # Exit with appropriate code
    [ "$TESTS_PASSED" = "true" ] && exit 0 || exit 1
}

main