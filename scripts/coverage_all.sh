#!/bin/bash
# Comprehensive test coverage script for neural-trader
# Targets 85% coverage for new code

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🧪 Neural Trader - Comprehensive Test Coverage${NC}"
echo -e "${BLUE}================================================${NC}"

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}📦 Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin
fi

# Clean previous coverage data
echo -e "${YELLOW}🧹 Cleaning previous coverage data...${NC}"
rm -rf "$PROJECT_ROOT/target/coverage"
rm -rf "$PROJECT_ROOT/target/tarpaulin"

# Function to run coverage with specific configuration
run_coverage() {
    local config_name=$1
    local description=$2
    
    echo -e "\n${BLUE}📊 Running coverage: ${description}${NC}"
    echo -e "${BLUE}================================${NC}"
    
    if [ "$config_name" == "default" ]; then
        cargo tarpaulin --config "$PROJECT_ROOT/tarpaulin.toml"
    else
        cargo tarpaulin --config "$PROJECT_ROOT/tarpaulin.toml" --profile "$config_name"
    fi
}

# Run different coverage profiles
echo -e "${GREEN}🚀 Starting coverage analysis...${NC}\n"

# Full coverage
run_coverage "default" "Full Project Coverage"

# Phase 1 specific coverage
if [ "$1" == "--phase1" ] || [ "$1" == "-p1" ]; then
    echo -e "\n${YELLOW}🎯 Running Phase 1 specific coverage...${NC}"
    run_coverage "phase1-neural" "Phase 1 - Neural Components"
    run_coverage "phase1-features" "Phase 1 - Feature Engineering"
    run_coverage "phase1-backtesting" "Phase 1 - Backtesting Framework"
fi

# Generate coverage report summary
echo -e "\n${BLUE}📈 Coverage Summary${NC}"
echo -e "${BLUE}==================${NC}"

# Check if lcov is installed for detailed reporting
if command -v lcov &> /dev/null; then
    echo -e "${GREEN}📊 Generating detailed coverage report...${NC}"
    lcov --summary "$PROJECT_ROOT/target/coverage/lcov.info" 2>/dev/null || true
fi

# Display coverage results
if [ -f "$PROJECT_ROOT/target/coverage/tarpaulin-report.json" ]; then
    echo -e "\n${GREEN}✅ Coverage reports generated:${NC}"
    echo -e "  • HTML Report: ${BLUE}$PROJECT_ROOT/target/coverage/tarpaulin-report.html${NC}"
    echo -e "  • LCOV Report: ${BLUE}$PROJECT_ROOT/target/coverage/lcov.info${NC}"
    echo -e "  • JSON Report: ${BLUE}$PROJECT_ROOT/target/coverage/tarpaulin-report.json${NC}"
    
    # Extract coverage percentage from JSON
    if command -v jq &> /dev/null; then
        COVERAGE=$(jq -r '.coverage' "$PROJECT_ROOT/target/coverage/tarpaulin-report.json" 2>/dev/null || echo "N/A")
        if [ "$COVERAGE" != "N/A" ]; then
            echo -e "\n${BLUE}📊 Total Coverage: ${GREEN}${COVERAGE}%${NC}"
            
            # Check if we meet the 85% target
            if (( $(echo "$COVERAGE >= 85" | bc -l) )); then
                echo -e "${GREEN}✅ Coverage target (85%) achieved!${NC}"
            else
                echo -e "${YELLOW}⚠️  Coverage below target (85%). Current: ${COVERAGE}%${NC}"
            fi
        fi
    fi
else
    echo -e "${RED}❌ Coverage report generation failed${NC}"
    exit 1
fi

echo -e "\n${GREEN}✨ Coverage analysis complete!${NC}"