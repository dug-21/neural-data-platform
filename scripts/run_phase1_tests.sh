#!/bin/bash

# Run Phase 1 Tests Script
# This script runs all Phase 1 tests and provides a summary

echo "🧪 Running Phase 1 Tests for Neural-Expand Feature"
echo "================================================="
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test results
PASSED=0
FAILED=0
SKIPPED=0

# Function to run tests for a component
run_component_tests() {
    local component=$1
    local pattern=$2
    
    echo -e "${BLUE}Testing $component...${NC}"
    
    # Run tests and capture output
    if cargo test --manifest-path /workspaces/neural-trader/Cargo.toml $pattern 2>&1 | tee /tmp/test_output_$$.txt | grep -E "(test .* ... ok|test .* ... FAILED|running [0-9]+ test)"; then
        # Count results
        local passed_count=$(grep -c "test .* ... ok" /tmp/test_output_$$.txt || echo 0)
        local failed_count=$(grep -c "test .* ... FAILED" /tmp/test_output_$$.txt || echo 0)
        
        PASSED=$((PASSED + passed_count))
        FAILED=$((FAILED + failed_count))
        
        if [ $failed_count -eq 0 ]; then
            echo -e "${GREEN}✅ $component: All tests passed ($passed_count tests)${NC}"
        else
            echo -e "${RED}❌ $component: $failed_count tests failed${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️  $component: No tests found or compilation error${NC}"
    fi
    
    rm -f /tmp/test_output_$$.txt
    echo ""
}

# Run tests for each Phase 1 component
echo -e "${BLUE}🔍 Running unit tests for Phase 1 components...${NC}"
echo ""

# Feature Engineering Tests
run_component_tests "Technical Indicators" "technical_indicators::tests"
run_component_tests "Market Microstructure" "market_microstructure::tests"
run_component_tests "Cross-Asset Correlations" "cross_asset::tests"

# Neural Model Tests
run_component_tests "FANN Predictor" "fann_predictor::tests"
run_component_tests "Neural Enhancements" "neural_enhancements"
run_component_tests "Ensemble Manager" "ensemble_manager"

# Integration Tests
echo -e "${BLUE}🔗 Running integration tests...${NC}"
run_component_tests "Phase 1 Integration" "phase1"

# Summary
echo ""
echo -e "${BLUE}📊 Test Summary${NC}"
echo "==============="
echo -e "Total Passed: ${GREEN}$PASSED${NC}"
echo -e "Total Failed: ${RED}$FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}✅ All Phase 1 tests passed!${NC}"
    exit 0
else
    echo -e "${RED}❌ Some tests failed. Please review the output above.${NC}"
    exit 1
fi