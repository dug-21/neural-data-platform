#!/bin/bash
# Phase 1 specific test coverage script
# Focuses on neural architecture, feature engineering, and backtesting

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

echo -e "${PURPLE}🎯 Neural Trader - Phase 1 Coverage Analysis${NC}"
echo -e "${PURPLE}============================================${NC}"

# Check dependencies
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo -e "${YELLOW}📦 Installing cargo-tarpaulin...${NC}"
    cargo install cargo-tarpaulin
fi

# Clean previous coverage data
echo -e "${YELLOW}🧹 Cleaning previous coverage data...${NC}"
rm -rf "$PROJECT_ROOT/target/coverage/phase1"
mkdir -p "$PROJECT_ROOT/target/coverage/phase1"

# Function to run coverage for specific component
run_component_coverage() {
    local component=$1
    local test_pattern=$2
    local output_dir="$PROJECT_ROOT/target/coverage/phase1/$component"
    
    echo -e "\n${BLUE}📊 Analyzing coverage for: ${GREEN}$component${NC}"
    echo -e "${BLUE}─────────────────────────────────────${NC}"
    
    mkdir -p "$output_dir"
    
    # Run tarpaulin with specific test pattern
    cargo tarpaulin \
        --test-threads 1 \
        --exclude-files "vendor/*" \
        --exclude-files "tests/*" \
        --exclude-files "benches/*" \
        --exclude-files "examples/*" \
        --exclude-files "src/bin/*" \
        --exclude-files "src/daa/*" \
        --exclude-files "src/agents/*" \
        --ignore-panics \
        --timeout 300 \
        --out Html \
        --out Lcov \
        --out Json \
        --output-dir "$output_dir" \
        -- --test "$test_pattern" 2>/dev/null || true
    
    # Extract and display coverage percentage
    if [ -f "$output_dir/tarpaulin-report.json" ] && command -v jq &> /dev/null; then
        local coverage=$(jq -r '.coverage' "$output_dir/tarpaulin-report.json" 2>/dev/null || echo "0")
        echo -e "${GREEN}✓ $component coverage: ${YELLOW}${coverage}%${NC}"
        
        # Store result for summary
        echo "$component:$coverage" >> "$PROJECT_ROOT/target/coverage/phase1/summary.txt"
    fi
}

# Phase 1 Components Coverage
echo -e "\n${PURPLE}🚀 Starting Phase 1 coverage analysis...${NC}"

# 1. Neural Architecture Components
echo -e "\n${PURPLE}1️⃣ Neural Architecture Components${NC}"
run_component_coverage "neural-predictor" "*neural*"
run_component_coverage "fann-integration" "*fann*"

# 2. Feature Engineering Components
echo -e "\n${PURPLE}2️⃣ Feature Engineering Components${NC}"
run_component_coverage "technical-indicators" "*technical*"
run_component_coverage "market-microstructure" "*microstructure*"
run_component_coverage "regime-detection" "*regime*"
run_component_coverage "cross-asset" "*cross_asset*"

# 3. Backtesting Framework
echo -e "\n${PURPLE}3️⃣ Backtesting Framework${NC}"
run_component_coverage "walk-forward" "*walk_forward*"
run_component_coverage "monte-carlo" "*monte_carlo*"
run_component_coverage "ab-testing" "*ab_testing*"
run_component_coverage "backtesting-engine" "*engine*"

# 4. Integration Tests
echo -e "\n${PURPLE}4️⃣ Integration Tests${NC}"
run_component_coverage "phase1-integration" "*phase1*"

# Generate Phase 1 Summary Report
echo -e "\n${PURPLE}📈 Phase 1 Coverage Summary${NC}"
echo -e "${PURPLE}══════════════════════════${NC}"

if [ -f "$PROJECT_ROOT/target/coverage/phase1/summary.txt" ]; then
    total_coverage=0
    count=0
    
    while IFS=':' read -r component coverage; do
        echo -e "${BLUE}• $component: ${YELLOW}${coverage}%${NC}"
        total_coverage=$(echo "$total_coverage + $coverage" | bc)
        count=$((count + 1))
    done < "$PROJECT_ROOT/target/coverage/phase1/summary.txt"
    
    if [ $count -gt 0 ]; then
        avg_coverage=$(echo "scale=2; $total_coverage / $count" | bc)
        echo -e "\n${PURPLE}📊 Average Phase 1 Coverage: ${GREEN}${avg_coverage}%${NC}"
        
        # Check against 85% target
        if (( $(echo "$avg_coverage >= 85" | bc -l) )); then
            echo -e "${GREEN}✅ Phase 1 coverage target (85%) achieved!${NC}"
        else
            gap=$(echo "scale=2; 85 - $avg_coverage" | bc)
            echo -e "${YELLOW}⚠️  Phase 1 coverage below target. Gap: ${gap}%${NC}"
            echo -e "${YELLOW}   Focus on improving test coverage for components below 85%${NC}"
        fi
    fi
fi

# Generate combined Phase 1 report
echo -e "\n${BLUE}📄 Generating combined Phase 1 coverage report...${NC}"
cargo tarpaulin \
    --config "$PROJECT_ROOT/tarpaulin.toml" \
    --profile phase1-neural \
    --out Html \
    --output-dir "$PROJECT_ROOT/target/coverage/phase1" \
    2>/dev/null || true

echo -e "\n${GREEN}✅ Phase 1 coverage reports generated:${NC}"
echo -e "  • Combined HTML: ${BLUE}$PROJECT_ROOT/target/coverage/phase1/tarpaulin-report.html${NC}"
echo -e "  • Component reports: ${BLUE}$PROJECT_ROOT/target/coverage/phase1/*/tarpaulin-report.html${NC}"

echo -e "\n${GREEN}✨ Phase 1 coverage analysis complete!${NC}"

# Cleanup
rm -f "$PROJECT_ROOT/target/coverage/phase1/summary.txt"