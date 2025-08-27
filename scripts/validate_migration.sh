#!/bin/bash
# validate_migration.sh
# Automated validation script for src/ directory migration

set -euo pipefail

echo "🔍 Validating migration progress..."

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Success/failure tracking
CHECKS_PASSED=0
TOTAL_CHECKS=0

check_result() {
    local condition=$1
    local success_msg=$2
    local failure_msg=$3
    
    TOTAL_CHECKS=$((TOTAL_CHECKS + 1))
    
    if [ "$condition" -eq 0 ]; then
        echo -e "${GREEN}✅ $success_msg${NC}"
        CHECKS_PASSED=$((CHECKS_PASSED + 1))
    else
        echo -e "${RED}❌ $failure_msg${NC}"
    fi
}

# Check 1: Remaining src/ content
echo "📁 Checking src/ directory cleanup..."
SRC_FILES=$(find src/ -name "*.rs" 2>/dev/null | wc -l)
if [ "$SRC_FILES" -eq 1 ]; then
    # Only lib.rs should remain
    if [ -f "src/lib.rs" ]; then
        LIB_SIZE=$(wc -l < src/lib.rs)
        if [ "$LIB_SIZE" -lt 10 ]; then
            check_result 0 "src/ directory properly cleaned (only minimal lib.rs remains)" "src/ cleanup incomplete"
        else
            check_result 1 "src/ directory properly cleaned" "src/lib.rs still contains $LIB_SIZE lines (should be minimal)"
        fi
    else
        check_result 0 "src/ directory completely cleaned" "src/ cleanup incomplete"
    fi
elif [ "$SRC_FILES" -eq 0 ]; then
    check_result 0 "src/ directory completely removed" "src/ cleanup incomplete"
else
    check_result 1 "src/ directory properly cleaned" "$SRC_FILES Rust files still in src/"
fi

# Check 2: Empty directories removed
echo "📂 Checking for empty directories..."
EMPTY_DIRS=$(find src/ -type d -empty 2>/dev/null | wc -l)
check_result "$EMPTY_DIRS" "No empty directories found" "$EMPTY_DIRS empty directories still exist"

# Check 3: Python files removed
echo "🐍 Checking for Python files in src/..."
PYTHON_FILES=$(find src/ -name "*.py" 2>/dev/null | wc -l)
check_result "$PYTHON_FILES" "All Python files removed from src/" "$PYTHON_FILES Python files still in src/"

# Check 4: Binary utilities migrated
echo "⚙️  Checking binary utilities migration..."
if [ -d "src/bin" ]; then
    BIN_FILES=$(find src/bin/ -name "*.rs" 2>/dev/null | wc -l)
    check_result "$BIN_FILES" "All binary utilities migrated" "$BIN_FILES binary utilities still in src/bin/"
else
    check_result 0 "Binary utilities directory removed" "Binary utilities migration incomplete"
fi

# Check 5: Workspace compilation
echo "🔧 Checking workspace compilation..."
if cargo check --workspace --quiet 2>/dev/null; then
    check_result 0 "Workspace compiles successfully" "Compilation errors detected"
else
    check_result 1 "Workspace compiles successfully" "Compilation errors detected"
fi

# Check 6: Old imports
echo "📦 Checking for old import statements..."
OLD_IMPORTS=$(rg "use.*neural_trader::" --count 2>/dev/null | tail -n1 || echo "0")
check_result "$OLD_IMPORTS" "All imports updated to microservices" "$OLD_IMPORTS old import statements found"

# Check 7: Microservice independence
echo "🏗️  Checking microservice independence..."
MICROSERVICES=("neural-core" "neural-ml-ops" "neural-trading" "data-staging" "config-store")
INDEPENDENT_SERVICES=0

for service in "${MICROSERVICES[@]}"; do
    if [ -d "$service" ]; then
        if (cd "$service" && cargo check --quiet 2>/dev/null); then
            INDEPENDENT_SERVICES=$((INDEPENDENT_SERVICES + 1))
            echo -e "${GREEN}  ✅ $service compiles independently${NC}"
        else
            echo -e "${RED}  ❌ $service has compilation issues${NC}"
        fi
    else
        echo -e "${YELLOW}  ⚠️  $service directory not found${NC}"
    fi
done

# Check 8: Test compilation
echo "🧪 Checking test compilation..."
if cargo test --workspace --no-run --quiet 2>/dev/null; then
    check_result 0 "All tests compile successfully" "Test compilation errors detected"
else
    check_result 1 "All tests compile successfully" "Test compilation errors detected"
fi

# Check 9: Workspace structure validation
echo "📋 Validating workspace structure..."
if [ -f "Cargo.toml" ] && grep -q "\[workspace\]" Cargo.toml; then
    WORKSPACE_MEMBERS=$(grep -A 10 "members = \[" Cargo.toml | grep -c '".*"' || echo "0")
    if [ "$WORKSPACE_MEMBERS" -ge 4 ]; then
        check_result 0 "Workspace properly configured with $WORKSPACE_MEMBERS members" "Workspace configuration incomplete"
    else
        check_result 1 "Workspace properly configured" "Only $WORKSPACE_MEMBERS workspace members found"
    fi
else
    check_result 1 "Workspace properly configured" "Workspace configuration missing or invalid"
fi

# Summary
echo ""
echo "📊 MIGRATION VALIDATION SUMMARY"
echo "================================"
echo -e "Checks passed: ${GREEN}$CHECKS_PASSED${NC}/$TOTAL_CHECKS"

if [ "$CHECKS_PASSED" -eq "$TOTAL_CHECKS" ]; then
    echo -e "${GREEN}🎉 ALL CHECKS PASSED - MIGRATION COMPLETE!${NC}"
    echo "src/ directory has been successfully migrated to microservices."
    exit 0
elif [ "$CHECKS_PASSED" -gt $((TOTAL_CHECKS / 2)) ]; then
    echo -e "${YELLOW}⚠️  MIGRATION IN PROGRESS${NC}"
    echo "Most checks passed. Continue with remaining migration steps."
    exit 1
else
    echo -e "${RED}🚨 MIGRATION ISSUES DETECTED${NC}"
    echo "Multiple checks failed. Review migration plan and address issues."
    exit 2
fi