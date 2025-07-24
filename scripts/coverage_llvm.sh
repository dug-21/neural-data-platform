#!/bin/bash
# Alternative coverage script using cargo-llvm-cov
# Provides source-based code coverage with LLVM

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}🔬 Neural Trader - LLVM Coverage Analysis${NC}"
echo -e "${CYAN}==========================================${NC}"

# Check if cargo-llvm-cov is installed
if ! command -v cargo-llvm-cov &> /dev/null; then
    echo -e "${YELLOW}📦 Installing cargo-llvm-cov...${NC}"
    cargo install cargo-llvm-cov --locked
fi

# Clean previous coverage data
echo -e "${YELLOW}🧹 Cleaning previous coverage data...${NC}"
cargo llvm-cov clean --workspace

# Function to run LLVM coverage
run_llvm_coverage() {
    local mode=$1
    local description=$2
    local extra_args="${@:3}"
    
    echo -e "\n${BLUE}📊 Running LLVM coverage: ${description}${NC}"
    echo -e "${BLUE}────────────────────────────────────────${NC}"
    
    case $mode in
        "full")
            cargo llvm-cov \
                --all-features \
                --workspace \
                --html \
                --output-dir "$PROJECT_ROOT/target/llvm-cov" \
                --lcov --output-path "$PROJECT_ROOT/target/llvm-cov/lcov.info" \
                $extra_args
            ;;
        "phase1")
            cargo llvm-cov \
                --package autonomous-platform \
                --features neural \
                --html \
                --output-dir "$PROJECT_ROOT/target/llvm-cov/phase1" \
                --lcov --output-path "$PROJECT_ROOT/target/llvm-cov/phase1/lcov.info" \
                $extra_args
            ;;
        "unit")
            cargo llvm-cov \
                --lib \
                --html \
                --output-dir "$PROJECT_ROOT/target/llvm-cov/unit" \
                $extra_args
            ;;
        "integration")
            cargo llvm-cov \
                --test '*' \
                --html \
                --output-dir "$PROJECT_ROOT/target/llvm-cov/integration" \
                $extra_args
            ;;
    esac
    
    # Display coverage summary
    echo -e "${GREEN}✓ Coverage analysis complete${NC}"
}

# Parse command line arguments
MODE="full"
SHOW_MISSING=false
FAIL_UNDER=""

while [[ $# -gt 0 ]]; do
    case $1 in
        --phase1|-p1)
            MODE="phase1"
            shift
            ;;
        --unit|-u)
            MODE="unit"
            shift
            ;;
        --integration|-i)
            MODE="integration"
            shift
            ;;
        --show-missing|-m)
            SHOW_MISSING=true
            shift
            ;;
        --fail-under)
            FAIL_UNDER="$2"
            shift 2
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            exit 1
            ;;
    esac
done

# Build extra arguments
EXTRA_ARGS=""
if [ "$SHOW_MISSING" = true ]; then
    EXTRA_ARGS="$EXTRA_ARGS --show-missing-lines"
fi
if [ -n "$FAIL_UNDER" ]; then
    EXTRA_ARGS="$EXTRA_ARGS --fail-under-lines $FAIL_UNDER"
fi

# Run coverage based on mode
echo -e "${GREEN}🚀 Starting LLVM coverage analysis...${NC}"

case $MODE in
    "full")
        run_llvm_coverage "full" "Full Workspace Coverage" $EXTRA_ARGS
        ;;
    "phase1")
        run_llvm_coverage "phase1" "Phase 1 Neural Components" $EXTRA_ARGS
        ;;
    "unit")
        run_llvm_coverage "unit" "Unit Tests Only" $EXTRA_ARGS
        ;;
    "integration")
        run_llvm_coverage "integration" "Integration Tests Only" $EXTRA_ARGS
        ;;
esac

# Generate detailed report
echo -e "\n${CYAN}📈 Coverage Report Summary${NC}"
echo -e "${CYAN}═════════════════════════${NC}"

# Show coverage statistics
cargo llvm-cov report --summary-only 2>/dev/null || true

# Display report locations
echo -e "\n${GREEN}✅ Coverage reports generated:${NC}"

case $MODE in
    "full")
        echo -e "  • HTML Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/html/index.html${NC}"
        echo -e "  • LCOV Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/lcov.info${NC}"
        ;;
    "phase1")
        echo -e "  • HTML Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/phase1/html/index.html${NC}"
        echo -e "  • LCOV Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/phase1/lcov.info${NC}"
        ;;
    "unit")
        echo -e "  • HTML Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/unit/html/index.html${NC}"
        ;;
    "integration")
        echo -e "  • HTML Report: ${BLUE}$PROJECT_ROOT/target/llvm-cov/integration/html/index.html${NC}"
        ;;
esac

# Compare with tarpaulin if both are available
if [ -f "$PROJECT_ROOT/target/coverage/tarpaulin-report.json" ] && [ -f "$PROJECT_ROOT/target/llvm-cov/lcov.info" ]; then
    echo -e "\n${CYAN}📊 Coverage Tool Comparison${NC}"
    echo -e "${CYAN}══════════════════════════${NC}"
    
    if command -v jq &> /dev/null; then
        TARPAULIN_COV=$(jq -r '.coverage' "$PROJECT_ROOT/target/coverage/tarpaulin-report.json" 2>/dev/null || echo "N/A")
        echo -e "  • Tarpaulin: ${YELLOW}${TARPAULIN_COV}%${NC}"
    fi
    
    echo -e "  • LLVM-Cov:  ${YELLOW}(see summary above)${NC}"
fi

echo -e "\n${GREEN}✨ LLVM coverage analysis complete!${NC}"

# Usage help
if [ "$1" == "--help" ] || [ "$1" == "-h" ]; then
    echo -e "\n${CYAN}Usage:${NC}"
    echo -e "  $0 [options]"
    echo -e "\n${CYAN}Options:${NC}"
    echo -e "  --phase1, -p1         Run Phase 1 coverage only"
    echo -e "  --unit, -u           Run unit test coverage only"
    echo -e "  --integration, -i    Run integration test coverage only"
    echo -e "  --show-missing, -m   Show missing line numbers"
    echo -e "  --fail-under <N>     Fail if coverage is below N%"
    echo -e "  --help, -h           Show this help message"
fi