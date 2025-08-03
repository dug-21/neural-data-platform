#!/bin/bash
# Phase 3 Test Execution Script
# 
# Runs comprehensive Phase 3 test suite with proper environment setup

set -e

echo "🚀 Phase 3 Neural Trader Test Suite"
echo "===================================="

# Setup test environment
export RUST_LOG=info
export RUST_BACKTRACE=1
export NEURAL_TRADER_TEST_MODE=phase3

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test categories
echo -e "${BLUE}📋 Test Categories:${NC}"
echo "├── Core Integration Tests"
echo "├── DAA Preservation Tests" 
echo "├── Memory Budget Compliance"
echo "├── Performance Benchmarks"
echo "└── Comprehensive Integration"
echo ""

# Function to run test category
run_test_category() {
    local category=$1
    local pattern=$2
    echo -e "${YELLOW}🧪 Running $category...${NC}"
    
    if cargo test --test "*" --features "test" -- "$pattern" --nocapture; then
        echo -e "${GREEN}✅ $category PASSED${NC}"
        return 0
    else
        echo -e "${RED}❌ $category FAILED${NC}"
        return 1
    fi
}

# Test execution
echo -e "${BLUE}🎯 Starting Test Execution${NC}"
echo ""

# Core tests
echo "═══════════════════════════════════════"
echo "1️⃣  CORE INTEGRATION TESTS"
echo "═══════════════════════════════════════"
run_test_category "Core Integration" "phase3::core" || exit 1
echo ""

# DAA tests  
echo "═══════════════════════════════════════"
echo "2️⃣  DAA PRESERVATION TESTS"
echo "═══════════════════════════════════════"
run_test_category "DAA Preservation" "phase3::daa" || exit 1
echo ""

# Memory tests
echo "═══════════════════════════════════════"
echo "3️⃣  MEMORY BUDGET COMPLIANCE"
echo "═══════════════════════════════════════"
run_test_category "Memory Budget" "phase3::memory" || exit 1
echo ""

# Performance tests
echo "═══════════════════════════════════════"
echo "4️⃣  PERFORMANCE BENCHMARKS"
echo "═══════════════════════════════════════"
run_test_category "Performance" "phase3::performance" || exit 1
echo ""

# Comprehensive integration test
echo "═══════════════════════════════════════"
echo "5️⃣  COMPREHENSIVE INTEGRATION"
echo "═══════════════════════════════════════"
run_test_category "Comprehensive Integration" "phase3_comprehensive_integration" || exit 1
echo ""

# Summary
echo "═══════════════════════════════════════"
echo -e "${GREEN}🎉 ALL PHASE 3 TESTS PASSED!${NC}"
echo "═══════════════════════════════════════"
echo ""
echo "✅ Core integration with async NeuralPredictor"
echo "✅ DAA functionality preservation"
echo "✅ Memory budget compliance verified"
echo "✅ Performance benchmarks met"
echo "✅ End-to-end system validation"
echo ""
echo -e "${GREEN}🚀 Phase 3 system ready for production!${NC}"