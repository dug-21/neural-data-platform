#!/bin/bash
# Test runner for Historical Data Backfill System

set -e

echo "🧪 Running Historical Data Backfill Tests"
echo "========================================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test categories
UNIT_TESTS=true
INTEGRATION_TESTS=true
PERFORMANCE_TESTS=false
E2E_TESTS=false

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --unit-only)
            INTEGRATION_TESTS=false
            ;;
        --integration-only)
            UNIT_TESTS=false
            ;;
        --performance)
            PERFORMANCE_TESTS=true
            ;;
        --e2e)
            E2E_TESTS=true
            ;;
        --all)
            PERFORMANCE_TESTS=true
            E2E_TESTS=true
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--unit-only|--integration-only|--performance|--e2e|--all]"
            exit 1
            ;;
    esac
    shift
done

# Create test report directory
REPORT_DIR="test-reports"
mkdir -p $REPORT_DIR

# Function to run tests
run_test_suite() {
    local suite_name=$1
    local test_path=$2
    local pytest_args=$3
    
    echo -e "\n${YELLOW}Running $suite_name...${NC}"
    
    if pytest $test_path $pytest_args \
        --cov=data_ingestion.providers.historical_backfill \
        --cov-report=html:$REPORT_DIR/coverage-$suite_name \
        --cov-report=term \
        --junit-xml=$REPORT_DIR/junit-$suite_name.xml \
        --html=$REPORT_DIR/report-$suite_name.html \
        --self-contained-html; then
        echo -e "${GREEN}✓ $suite_name passed${NC}"
        return 0
    else
        echo -e "${RED}✗ $suite_name failed${NC}"
        return 1
    fi
}

# Track overall status
OVERALL_STATUS=0

# Run unit tests
if [ "$UNIT_TESTS" = true ]; then
    if ! run_test_suite "Unit Tests" "tests/unit/" "-v"; then
        OVERALL_STATUS=1
    fi
fi

# Run integration tests
if [ "$INTEGRATION_TESTS" = true ]; then
    # Start test containers
    echo -e "\n${YELLOW}Starting test containers...${NC}"
    docker-compose -f docker-compose.test.yml up -d
    
    # Wait for services to be ready
    echo "Waiting for services..."
    sleep 10
    
    if ! run_test_suite "Integration Tests" "tests/integration/" "-v -m integration"; then
        OVERALL_STATUS=1
    fi
    
    # Stop test containers
    echo -e "\n${YELLOW}Stopping test containers...${NC}"
    docker-compose -f docker-compose.test.yml down
fi

# Run performance tests (optional)
if [ "$PERFORMANCE_TESTS" = true ]; then
    if ! run_test_suite "Performance Tests" "tests/performance/" "-v -m performance"; then
        OVERALL_STATUS=1
    fi
fi

# Run E2E tests (optional)
if [ "$E2E_TESTS" = true ]; then
    if ! run_test_suite "End-to-End Tests" "tests/e2e/" "-v -m slow"; then
        OVERALL_STATUS=1
    fi
fi

# Generate combined coverage report
if [ -f ".coverage" ]; then
    echo -e "\n${YELLOW}Generating combined coverage report...${NC}"
    coverage html -d $REPORT_DIR/coverage-combined
    coverage report
fi

# Summary
echo -e "\n========================================"
echo "Test Summary"
echo "========================================"

if [ $OVERALL_STATUS -eq 0 ]; then
    echo -e "${GREEN}✓ All tests passed!${NC}"
else
    echo -e "${RED}✗ Some tests failed!${NC}"
fi

echo -e "\nTest reports available in: ${YELLOW}$REPORT_DIR/${NC}"
echo "  - Coverage: $REPORT_DIR/coverage-combined/index.html"
echo "  - HTML Reports: $REPORT_DIR/report-*.html"
echo "  - JUnit XML: $REPORT_DIR/junit-*.xml"

# Check coverage threshold
COVERAGE_THRESHOLD=80
ACTUAL_COVERAGE=$(coverage report | grep TOTAL | awk '{print $4}' | sed 's/%//')

if [ ! -z "$ACTUAL_COVERAGE" ]; then
    echo -e "\nCoverage: ${YELLOW}${ACTUAL_COVERAGE}%${NC} (threshold: $COVERAGE_THRESHOLD%)"
    
    if (( $(echo "$ACTUAL_COVERAGE < $COVERAGE_THRESHOLD" | bc -l) )); then
        echo -e "${RED}✗ Coverage below threshold!${NC}"
        OVERALL_STATUS=1
    else
        echo -e "${GREEN}✓ Coverage meets threshold${NC}"
    fi
fi

exit $OVERALL_STATUS