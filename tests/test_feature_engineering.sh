#!/bin/bash
# Test runner for feature engineering modules

echo "🧪 Running Feature Engineering Tests"
echo "===================================="

# Set test environment
export RUST_BACKTRACE=1
export RUST_LOG=info

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to run tests for a module
run_module_tests() {
    local module=$1
    local description=$2
    
    echo -e "\n${YELLOW}Testing ${description}...${NC}"
    
    if cargo test --package neural-trader --lib features::${module} -- --nocapture --test-threads=4; then
        echo -e "${GREEN}✅ ${description} tests passed${NC}"
        return 0
    else
        echo -e "${RED}❌ ${description} tests failed${NC}"
        return 1
    fi
}

# Track failures
FAILED=0

# Run individual module tests
run_module_tests "technical_indicators_tests" "Technical Indicators (Elliott Wave, Harmonics)" || FAILED=1
run_module_tests "market_microstructure_tests" "Market Microstructure (Order Flow Toxicity)" || FAILED=1
run_module_tests "cross_asset_tests" "Cross-Asset Correlations" || FAILED=1

# Run integration tests
echo -e "\n${YELLOW}Running Integration Tests...${NC}"
if cargo test --package neural-trader --lib features::integration_tests -- --nocapture --test-threads=2; then
    echo -e "${GREEN}✅ Integration tests passed${NC}"
else
    echo -e "${RED}❌ Integration tests failed${NC}"
    FAILED=1
fi

# Run benchmarks if requested
if [ "$1" == "--bench" ]; then
    echo -e "\n${YELLOW}Running Performance Benchmarks...${NC}"
    cargo test --package neural-trader --lib features -- test_performance_benchmark --nocapture
fi

# Generate coverage report if requested
if [ "$1" == "--coverage" ]; then
    echo -e "\n${YELLOW}Generating Coverage Report...${NC}"
    cargo tarpaulin --lib -p neural-trader --exclude-files "*/tests/*" --out Html --output-dir target/coverage
    echo -e "${GREEN}Coverage report generated at: target/coverage/tarpaulin-report.html${NC}"
fi

# Summary
echo -e "\n===================================="
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ All feature engineering tests passed!${NC}"
    
    # Count total tests
    TOTAL_TESTS=$(cargo test --package neural-trader --lib features -- --list 2>/dev/null | grep -c "test features::")
    echo -e "Total tests: ${TOTAL_TESTS}"
    
    exit 0
else
    echo -e "${RED}❌ Some tests failed!${NC}"
    exit 1
fi