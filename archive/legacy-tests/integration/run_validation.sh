#!/bin/bash
set -e

# Config-Store Integration Test Validation Script
# This script validates the test setup and runs the complete test suite

echo "======================================================================="
echo "CONFIG-STORE INTEGRATION TEST VALIDATION AND EXECUTION"
echo "======================================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    local status=$1
    local message=$2
    case $status in
        "INFO")
            echo -e "${GREEN}[INFO]${NC} $message"
            ;;
        "WARN")
            echo -e "${YELLOW}[WARN]${NC} $message"
            ;;
        "ERROR")
            echo -e "${RED}[ERROR]${NC} $message"
            ;;
    esac
}

# Check dependencies
print_status "INFO" "Checking dependencies..."

if ! command -v docker &> /dev/null; then
    print_status "ERROR" "Docker is not installed or not in PATH"
    exit 1
fi

if ! command -v docker-compose &> /dev/null; then
    print_status "ERROR" "Docker Compose is not installed or not in PATH"
    exit 1
fi

if ! command -v python3 &> /dev/null; then
    print_status "ERROR" "Python 3 is not installed or not in PATH"
    exit 1
fi

print_status "INFO" "Dependencies check passed"

# Change to the script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Validate test setup
print_status "INFO" "Validating test setup..."

if [ -f "validate_test_setup.py" ]; then
    python3 validate_test_setup.py --verbose --output-report validation_report.json
    if [ $? -ne 0 ]; then
        print_status "ERROR" "Test setup validation failed"
        exit 1
    fi
    print_status "INFO" "Test setup validation passed"
else
    print_status "WARN" "Test setup validator not found, skipping validation"
fi

# Create results directory
mkdir -p test-results coverage reports

# Set default environment variables for testing
export POSTGRES_HOST=postgres-test
export POSTGRES_PORT=5432
export POSTGRES_DB=neural_trader_test
export POSTGRES_USER=postgres
export POSTGRES_PASSWORD=test_password_123

export REDIS_HOST=redis-test
export REDIS_PORT=6379
export REDIS_PASSWORD=test_redis_pass
export REDIS_DB=15

export CONFIG_STORE_URL=http://config-store-mock:8080
export DATA_INGESTION_URL=http://data-ingestion-test:8000

export TEST_TIMEOUT=300
export COVERAGE_MIN_PERCENTAGE=85

# Parse command line arguments
RUN_SCENARIO=""
PROFILE="integration"
VERBOSE=false
NO_CLEANUP=false
PARALLEL=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --scenario)
            RUN_SCENARIO="$2"
            shift 2
            ;;
        --profile)
            PROFILE="$2"
            shift 2
            ;;
        --verbose|-v)
            VERBOSE=true
            shift
            ;;
        --no-cleanup)
            NO_CLEANUP=true
            shift
            ;;
        --parallel)
            PARALLEL=true
            shift
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --scenario SCENARIO    Run specific test scenario"
            echo "  --profile PROFILE      Docker compose profile to use (default: integration)"
            echo "  --verbose, -v          Enable verbose output"
            echo "  --no-cleanup          Don't cleanup containers after tests"
            echo "  --parallel            Run tests in parallel"
            echo "  --help, -h            Show this help message"
            echo ""
            echo "Available scenarios:"
            echo "  basic_integration     Basic configuration loading tests"
            echo "  fallback_mechanism    Environment variable fallback tests"
            echo "  hot_reloading         Configuration hot reload tests"
            echo "  provider_configuration Provider configuration tests"
            echo "  rate_limiting         Rate limiting configuration tests"
            echo "  database_redis        Database and Redis configuration tests"
            echo "  migration_process     Complete migration process tests"
            echo "  full_suite            All tests with coverage reporting"
            echo ""
            echo "Available profiles:"
            echo "  integration           Basic integration test services"
            echo "  full                  Full test suite with monitoring"
            echo "  fallback              Fallback testing (config-store unavailable)"
            echo "  performance           Performance testing profile"
            exit 0
            ;;
        *)
            print_status "ERROR" "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Cleanup function
cleanup() {
    local exit_code=$?
    if [ "$NO_CLEANUP" = false ]; then
        print_status "INFO" "Cleaning up containers..."
        docker-compose -f ../../../docker-compose.test.yml down -v --remove-orphans 2>/dev/null || true
    else
        print_status "INFO" "Skipping cleanup as requested"
    fi
    exit $exit_code
}

# Set trap for cleanup
trap cleanup EXIT

# Check if docker-compose file exists
COMPOSE_FILE="../../../docker-compose.test.yml"
if [ ! -f "$COMPOSE_FILE" ]; then
    print_status "ERROR" "Docker compose file not found: $COMPOSE_FILE"
    exit 1
fi

# Build and start services
print_status "INFO" "Starting test environment with profile: $PROFILE"
docker-compose -f "$COMPOSE_FILE" --profile "$PROFILE" down -v --remove-orphans
docker-compose -f "$COMPOSE_FILE" --profile "$PROFILE" build

# Start services in detached mode
docker-compose -f "$COMPOSE_FILE" --profile "$PROFILE" up -d

# Wait for services to be healthy
print_status "INFO" "Waiting for services to be ready..."
max_wait=300  # 5 minutes
wait_time=0
interval=10

while [ $wait_time -lt $max_wait ]; do
    if docker-compose -f "$COMPOSE_FILE" exec -T integration-test-runner python /app/health_check.py; then
        print_status "INFO" "All services are healthy"
        break
    fi
    
    if [ $wait_time -eq 0 ]; then
        print_status "INFO" "Services starting up, waiting for health checks..."
    fi
    
    sleep $interval
    wait_time=$((wait_time + interval))
    
    if [ $wait_time -ge $max_wait ]; then
        print_status "ERROR" "Services failed to become healthy within $max_wait seconds"
        print_status "INFO" "Service logs:"
        docker-compose -f "$COMPOSE_FILE" logs --tail=50
        exit 1
    fi
done

# Run tests
if [ -n "$RUN_SCENARIO" ]; then
    print_status "INFO" "Running specific test scenario: $RUN_SCENARIO"
    
    if [ -f "run_tests.py" ]; then
        TEST_ARGS=""
        if [ "$VERBOSE" = true ]; then
            TEST_ARGS="$TEST_ARGS --verbose"
        fi
        if [ "$NO_CLEANUP" = true ]; then
            TEST_ARGS="$TEST_ARGS --no-cleanup"
        fi
        if [ "$PARALLEL" = true ]; then
            TEST_ARGS="$TEST_ARGS --parallel"
        fi
        
        python3 run_tests.py --scenario "$RUN_SCENARIO" --output-dir ./test-results $TEST_ARGS
        test_exit_code=$?
    else
        # Fallback to direct docker-compose execution
        PYTEST_ARGS="/app/tests/test_data_ingestion_config.py -v --tb=short --junit-xml=/app/test-results/${RUN_SCENARIO}.xml"
        
        docker-compose -f "$COMPOSE_FILE" exec -T integration-test-runner \
            python -m pytest $PYTEST_ARGS
        test_exit_code=$?
    fi
else
    print_status "INFO" "Running full test suite"
    
    if [ -f "run_tests.py" ]; then
        TEST_ARGS=""
        if [ "$VERBOSE" = true ]; then
            TEST_ARGS="$TEST_ARGS --verbose"
        fi
        if [ "$NO_CLEANUP" = true ]; then
            TEST_ARGS="$TEST_ARGS --no-cleanup"
        fi
        if [ "$PARALLEL" = true ]; then
            TEST_ARGS="$TEST_ARGS --parallel"
        fi
        
        python3 run_tests.py --scenario full_suite --output-dir ./test-results $TEST_ARGS
        test_exit_code=$?
    else
        # Fallback to direct docker-compose execution
        docker-compose -f "$COMPOSE_FILE" exec -T integration-test-runner /app/run_integration_tests.sh
        test_exit_code=$?
    fi
fi

# Collect results
print_status "INFO" "Collecting test results..."

# Copy results from container
docker-compose -f "$COMPOSE_FILE" cp integration-test-runner:/app/test-results ./test-results/ 2>/dev/null || true
docker-compose -f "$COMPOSE_FILE" cp integration-test-runner:/app/coverage ./coverage/ 2>/dev/null || true
docker-compose -f "$COMPOSE_FILE" cp integration-test-runner:/app/reports ./reports/ 2>/dev/null || true

# Display results summary
if [ -f "./test-results/summary.json" ]; then
    print_status "INFO" "Test execution summary:"
    python3 -c "
import json
try:
    with open('./test-results/summary.json', 'r') as f:
        summary = json.load(f)
    print(f\"Duration: {summary.get('test_run', {}).get('total_duration', 'N/A'):.2f} seconds\")
    print(f\"Scenarios: {summary.get('test_run', {}).get('total_scenarios', 'N/A')}\")
    print(f\"Passed: {summary.get('summary', {}).get('passed', 'N/A')}\")
    print(f\"Failed: {summary.get('summary', {}).get('failed', 'N/A')}\")
    print(f\"Errors: {summary.get('summary', {}).get('error', 'N/A')}\")
    print(f\"Overall: {summary.get('overall_status', 'UNKNOWN')}\")
except Exception as e:
    print(f\"Could not read test summary: {e}\")
"
fi

# Show coverage if available
if [ -f "./coverage/coverage.xml" ]; then
    print_status "INFO" "Coverage report available at: ./coverage/html/index.html"
fi

# Final status
if [ $test_exit_code -eq 0 ]; then
    print_status "INFO" "All tests completed successfully!"
    echo ""
    echo "======================================================================="
    echo "INTEGRATION TEST EXECUTION COMPLETED - SUCCESS"
    echo "======================================================================="
    echo ""
    echo "Results available in:"
    echo "  - Test results: ./test-results/"
    echo "  - Coverage reports: ./coverage/"
    echo "  - Custom reports: ./reports/"
    echo ""
else
    print_status "ERROR" "Some tests failed!"
    echo ""
    echo "======================================================================="
    echo "INTEGRATION TEST EXECUTION COMPLETED - FAILURES DETECTED"
    echo "======================================================================="
    echo ""
    echo "Check the following for details:"
    echo "  - Test results: ./test-results/"
    echo "  - Service logs: docker-compose -f $COMPOSE_FILE logs"
    echo ""
fi

exit $test_exit_code