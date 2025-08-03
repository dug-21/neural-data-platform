#!/bin/bash

# Phase 3 Acceptance Test Runner
# =============================
# Comprehensive test suite for validating Phase 3 neural trading system
# is ready for production deployment.

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test configuration
REDIS_PORT=${REDIS_PORT:-6379}
TEST_TIMEOUT=${TEST_TIMEOUT:-300}  # 5 minutes per test suite
MEMORY_LIMIT=${MEMORY_LIMIT:-1073741824}  # 1GB memory limit for tests

echo -e "${BLUE}🚀 Phase 3 Acceptance Test Suite${NC}"
echo -e "${BLUE}=================================${NC}"
echo ""

# Check prerequisites
echo -e "${YELLOW}📋 Checking prerequisites...${NC}"

# Check Redis
if ! redis-cli -p $REDIS_PORT ping > /dev/null 2>&1; then
    echo -e "${RED}❌ Redis not running on port $REDIS_PORT${NC}"
    echo "Please start Redis: redis-server --port $REDIS_PORT"
    exit 1
fi
echo -e "${GREEN}✅ Redis running on port $REDIS_PORT${NC}"

# Check Rust toolchain
if ! cargo --version > /dev/null 2>&1; then
    echo -e "${RED}❌ Cargo not found${NC}"
    echo "Please install Rust toolchain"
    exit 1
fi
echo -e "${GREEN}✅ Rust toolchain available${NC}"

# Check available memory
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    AVAILABLE_MEMORY=$(awk '/MemAvailable/ {print $2 * 1024}' /proc/meminfo)
    if [ $AVAILABLE_MEMORY -lt $MEMORY_LIMIT ]; then
        echo -e "${YELLOW}⚠️  Low memory: ${AVAILABLE_MEMORY} bytes available, ${MEMORY_LIMIT} recommended${NC}"
    else
        echo -e "${GREEN}✅ Sufficient memory available${NC}"
    fi
elif [[ "$OSTYPE" == "darwin"* ]]; then
    echo -e "${GREEN}✅ macOS detected - memory check skipped${NC}"
fi

echo ""

# Build the project first
echo -e "${YELLOW}🔨 Building project...${NC}"
if ! cargo build --release --quiet; then
    echo -e "${RED}❌ Build failed${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Build successful${NC}"
echo ""

# Function to run a test suite with timeout and error handling
run_test_suite() {
    local test_name="$1"
    local test_command="$2"
    local description="$3"
    
    echo -e "${BLUE}📋 Running: $description${NC}"
    echo -e "${BLUE}Command: $test_command${NC}"
    echo ""
    
    local start_time=$(date +%s)
    
    if timeout ${TEST_TIMEOUT} bash -c "$test_command"; then
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo ""
        echo -e "${GREEN}✅ $test_name: PASSED (${duration}s)${NC}"
        return 0
    else
        local exit_code=$?
        local end_time=$(date +%s)
        local duration=$((end_time - start_time))
        echo ""
        if [ $exit_code -eq 124 ]; then
            echo -e "${RED}⏰ $test_name: TIMEOUT after ${TEST_TIMEOUT}s${NC}"
        else
            echo -e "${RED}❌ $test_name: FAILED (${duration}s, exit code: $exit_code)${NC}"
        fi
        return $exit_code
    fi
}

# Track test results
TESTS_PASSED=0
TESTS_FAILED=0
FAILED_TESTS=()

# Test Suite 1: Autonomous Trading Preservation
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📋 Phase 1: Autonomous Trading Preservation Tests${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

if run_test_suite "Autonomous Trading Preservation" \
    "cargo test --release --test autonomous_trading_preserved_test -- --nocapture" \
    "Validating autonomous trading capabilities are preserved and enhanced"; then
    ((TESTS_PASSED++))
else
    ((TESTS_FAILED++))
    FAILED_TESTS+=("Autonomous Trading Preservation")
fi

echo ""

# Test Suite 2: Capability Enhancement
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📋 Phase 2: Capability Enhancement Tests${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

if run_test_suite "Capability Enhancement" \
    "cargo test --release --test capability_enhancement_test -- --nocapture" \
    "Validating Phase 3 enhancements provide measurable improvements"; then
    ((TESTS_PASSED++))
else
    ((TESTS_FAILED++))
    FAILED_TESTS+=("Capability Enhancement")
fi

echo ""

# Test Suite 3: End-to-End Integration
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📋 Phase 3: End-to-End Integration Tests${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

if run_test_suite "End-to-End Integration" \
    "cargo test --release --test phase3_end_to_end_test -- --nocapture" \
    "Validating complete system integration and production readiness"; then
    ((TESTS_PASSED++))
else
    ((TESTS_FAILED++))
    FAILED_TESTS+=("End-to-End Integration")
fi

echo ""

# Optional: Run all acceptance tests together
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📋 Complete Acceptance Suite${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

if run_test_suite "Complete Acceptance Suite" \
    "cargo test --release --package neural-trader --test acceptance -- --nocapture" \
    "Running all acceptance tests together to validate system coherence"; then
    ((TESTS_PASSED++))
else
    ((TESTS_FAILED++))
    FAILED_TESTS+=("Complete Acceptance Suite")
fi

echo ""

# Performance and Memory Validation
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"
echo -e "${YELLOW}📋 Performance and Memory Validation${NC}"
echo -e "${YELLOW}═══════════════════════════════════════════════════${NC}"

# Check for memory leaks
echo -e "${BLUE}🔍 Checking for memory leaks...${NC}"
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    MEMORY_BEFORE=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
    
    # Run a subset of tests to check memory usage
    cargo test --release --test acceptance test_complete_acceptance_suite > /dev/null 2>&1 || true
    
    sleep 2  # Allow memory to settle
    MEMORY_AFTER=$(awk '/MemAvailable/ {print $2}' /proc/meminfo)
    
    MEMORY_DIFF=$((MEMORY_BEFORE - MEMORY_AFTER))
    if [ $MEMORY_DIFF -gt 102400 ]; then  # >100MB difference
        echo -e "${YELLOW}⚠️  Potential memory leak detected: ${MEMORY_DIFF}KB difference${NC}"
    else
        echo -e "${GREEN}✅ No significant memory leaks detected${NC}"
    fi
else
    echo -e "${YELLOW}⚠️  Memory leak detection not available on this platform${NC}"
fi

# Final Results Summary
echo ""
echo -e "${BLUE}🏁 Acceptance Test Suite Results${NC}"
echo -e "${BLUE}=================================${NC}"
echo ""

echo -e "Tests Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests Failed: ${RED}$TESTS_FAILED${NC}"
echo -e "Total Tests: $((TESTS_PASSED + TESTS_FAILED))"

if [ $TESTS_FAILED -eq 0 ]; then
    echo ""
    echo -e "${GREEN}🎉 ALL ACCEPTANCE TESTS PASSED!${NC}"
    echo -e "${GREEN}🚀 PHASE 3 IS READY FOR PRODUCTION DEPLOYMENT${NC}"
    echo ""
    echo -e "${GREEN}The neural trading system has successfully validated:${NC}"
    echo -e "${GREEN}✅ Autonomous trading capabilities preserved and enhanced${NC}"
    echo -e "${GREEN}✅ Measurable capability improvements (≥2% accuracy, ≥5% confidence)${NC}"
    echo -e "${GREEN}✅ Complete system integration and production readiness${NC}"
    echo -e "${GREEN}✅ Performance targets met under realistic load conditions${NC}"
    echo -e "${GREEN}✅ System integrity and reliability demonstrated${NC}"
    echo ""
    
    # Generate deployment approval report
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    cat > "PHASE3_DEPLOYMENT_APPROVAL_$(date +%Y%m%d_%H%M%S).txt" << EOF
PHASE 3 NEURAL TRADING SYSTEM - DEPLOYMENT APPROVAL
===================================================

Timestamp: $TIMESTAMP
Test Suite Version: Phase 3 Acceptance Tests
Test Environment: $(uname -a)
Rust Version: $(rustc --version)

ACCEPTANCE TEST RESULTS:
✅ Autonomous Trading Preservation: PASSED
✅ Capability Enhancement: PASSED  
✅ End-to-End Integration: PASSED
✅ Complete Acceptance Suite: PASSED

VALIDATION CRITERIA MET:
✅ 60/40 voting ratio preserved exactly
✅ 70% consensus threshold maintained
✅ Autonomous trading without human intervention
✅ ≥2% accuracy improvement from real-time training
✅ ≥5% confidence enhancement from data type discovery
✅ Memory usage within 525MB budget
✅ Prediction latency <100ms under normal load
✅ System integrity and fault tolerance validated
✅ Production readiness requirements satisfied

APPROVAL STATUS: ✅ APPROVED FOR PRODUCTION DEPLOYMENT

This report certifies that the Phase 3 neural trading system has
successfully passed all acceptance criteria and is ready for
production deployment.

Generated by: Phase 3 Acceptance Test Suite
EOF
    
    echo -e "${GREEN}📋 Deployment approval report generated: PHASE3_DEPLOYMENT_APPROVAL_$(date +%Y%m%d_%H%M%S).txt${NC}"
    exit 0
else
    echo ""
    echo -e "${RED}❌ ACCEPTANCE TESTS FAILED${NC}"
    echo -e "${RED}🚫 PHASE 3 IS NOT READY FOR PRODUCTION DEPLOYMENT${NC}"
    echo ""
    echo -e "${RED}Failed test suites:${NC}"
    for test in "${FAILED_TESTS[@]}"; do
        echo -e "${RED}  - $test${NC}"
    done
    echo ""
    echo -e "${YELLOW}Please address the failures before proceeding to production deployment.${NC}"
    echo -e "${YELLOW}Check the test output above for specific failure details.${NC}"
    
    # Generate failure report
    TIMESTAMP=$(date '+%Y-%m-%d %H:%M:%S')
    cat > "PHASE3_TEST_FAILURES_$(date +%Y%m%d_%H%M%S).txt" << EOF
PHASE 3 NEURAL TRADING SYSTEM - TEST FAILURE REPORT
===================================================

Timestamp: $TIMESTAMP
Test Suite Version: Phase 3 Acceptance Tests
Test Environment: $(uname -a)

FAILED TESTS:
EOF
    for test in "${FAILED_TESTS[@]}"; do
        echo "❌ $test" >> "PHASE3_TEST_FAILURES_$(date +%Y%m%d_%H%M%S).txt"
    done
    
    cat >> "PHASE3_TEST_FAILURES_$(date +%Y%m%d_%H%M%S).txt" << EOF

DEPLOYMENT STATUS: ❌ NOT APPROVED

The Phase 3 neural trading system has failed acceptance testing
and is NOT ready for production deployment. Please review and
fix the failed tests before rerunning the acceptance suite.

Generated by: Phase 3 Acceptance Test Suite
EOF
    
    echo -e "${RED}📋 Failure report generated: PHASE3_TEST_FAILURES_$(date +%Y%m%d_%H%M%S).txt${NC}"
    exit 1
fi