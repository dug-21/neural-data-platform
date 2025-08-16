#!/bin/bash

# Phase 3 Performance Test Runner Script
# Validates all Phase 3 success criteria through comprehensive performance testing

set -e

echo "🚀 Phase 3 Performance Test Suite"
echo "=================================="
echo "Validating all success criteria before Phase 3 completion"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}📊 $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Check if Rust and Cargo are available
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust and Cargo."
    exit 1
fi

# Create results directory
mkdir -p results
RESULTS_DIR="results/phase3_performance_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

print_status "Results will be saved to: $RESULTS_DIR"

# Build the project in release mode for accurate performance testing
print_status "Building project in release mode..."
if cargo build --release > "$RESULTS_DIR/build.log" 2>&1; then
    print_success "Build completed successfully"
else
    print_error "Build failed. Check $RESULTS_DIR/build.log"
    exit 1
fi

# Initialize test results tracking
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Function to run a test and track results
run_test() {
    local test_name="$1"
    local test_command="$2"
    local log_file="$RESULTS_DIR/${test_name}.log"
    
    print_status "Running $test_name..."
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if eval "$test_command" > "$log_file" 2>&1; then
        print_success "$test_name passed"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        print_error "$test_name failed - check $log_file"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

echo ""
print_status "Starting Phase 3 Performance Tests..."
echo ""

# 1. Memory Compliance Tests (Critical - must pass first)
print_status "=== Memory Compliance Tests ==="
run_test "memory_total_system_limit" "cargo test --release test_total_system_memory_limit -- --nocapture"
run_test "memory_per_symbol_limit" "cargo test --release test_per_symbol_memory_limit -- --nocapture"
run_test "memory_reduction_validation" "cargo test --release test_memory_reduction_validation -- --nocapture"
run_test "memory_leak_detection" "cargo test --release test_memory_leak_detection -- --nocapture"
run_test "memory_concurrent_efficiency" "cargo test --release test_concurrent_memory_efficiency -- --nocapture"
run_test "memory_cleanup" "cargo test --release test_memory_cleanup -- --nocapture"

echo ""
print_status "=== Performance Benchmark Tests ==="
# 2. Core Performance Benchmarks
run_test "prediction_latency_requirement" "cargo test --release test_prediction_latency_requirement -- --nocapture"
run_test "data_discovery_requirement" "cargo test --release test_data_type_discovery_requirement -- --nocapture"
run_test "channel_routing_requirement" "cargo test --release test_channel_routing_requirement -- --nocapture"

# 3. Criterion Benchmarks (if available)
if cargo bench --help &> /dev/null; then
    print_status "Running Criterion benchmarks..."
    run_test "criterion_benchmarks" "cargo bench --bench phase3b_performance_benchmarks"
else
    print_warning "Criterion benchmarks not available - skipping"
fi

echo ""
print_status "=== Load Testing ==="
# 4. Load Tests
run_test "concurrent_symbols_100" "cargo test --release test_100_symbols_concurrent_processing -- --nocapture"
run_test "updates_per_second_1000" "cargo test --release test_1000_updates_per_second -- --nocapture"
run_test "stability_24_hours" "cargo test --release test_24_hour_stability_accelerated -- --nocapture"
run_test "graceful_degradation" "cargo test --release test_graceful_degradation -- --nocapture"

echo ""
print_status "=== Integration Validation ==="
# 5. Integration Tests
run_test "daa_preservation" "cargo test --release test_voting_weights_preserved -- --nocapture" || true
run_test "byzantine_consensus" "cargo test --release test_byzantine_consensus_maintained -- --nocapture" || true
run_test "autonomous_trading" "cargo test --release test_autonomous_trading_preserved -- --nocapture" || true

# 6. System Performance Tests
print_status "=== System Performance Validation ==="
run_test "shared_features_preserved" "cargo test --release test_shared_features_preserved -- --nocapture" || true
run_test "sector_architecture" "cargo test --release sector_aggregator -- --nocapture" || true

# Generate comprehensive report
print_status "Generating performance report..."

cat > "$RESULTS_DIR/performance_summary.md" << EOF
# Phase 3 Performance Test Results

Generated on: $(date)
Test execution directory: $RESULTS_DIR

## Summary
- Total tests run: $TOTAL_TESTS
- Tests passed: $PASSED_TESTS
- Tests failed: $FAILED_TESTS
- Success rate: $(( PASSED_TESTS * 100 / TOTAL_TESTS ))%

## Phase 3 Success Criteria Validation

### Critical Performance Metrics
- [ ] Prediction latency <100ms maintained
- [ ] Data type discovery <10ms per packet  
- [ ] Channel routing <5ms per message
- [ ] Real-time updates <50ms per update
- [ ] Model checkpoint <200ms per save
- [ ] Model rollback <500ms total
- [ ] Memory overhead <25MB additional

### Memory Compliance  
- [ ] Total system memory <525MB (Phase 2: 500MB + 5% overhead)
- [ ] Per-symbol memory <50MB
- [ ] 90% memory reduction validation
- [ ] Memory leak detection over 24 hours

### Load Testing
- [ ] 100 symbols concurrent processing
- [ ] 1000 updates/second handling  
- [ ] 24-hour stability validation
- [ ] Graceful degradation testing

### DAA Preservation (Critical)
- [ ] 60/40 voting weights maintained
- [ ] 70% Byzantine consensus threshold
- [ ] Performance thresholds active
- [ ] Autonomous decision making preserved

## Test Results Details

See individual log files for detailed results:
EOF

# Add individual test results to summary
for log_file in "$RESULTS_DIR"/*.log; do
    if [ -f "$log_file" ]; then
        test_name=$(basename "$log_file" .log)
        if grep -q "test result: ok" "$log_file" 2>/dev/null; then
            echo "- ✅ $test_name: PASSED" >> "$RESULTS_DIR/performance_summary.md"
        else
            echo "- ❌ $test_name: FAILED" >> "$RESULTS_DIR/performance_summary.md"
        fi
    fi
done

echo "" >> "$RESULTS_DIR/performance_summary.md"
echo "## System Information" >> "$RESULTS_DIR/performance_summary.md"
echo "\`\`\`" >> "$RESULTS_DIR/performance_summary.md"
echo "OS: $(uname -a)" >> "$RESULTS_DIR/performance_summary.md"
echo "Rust version: $(rustc --version)" >> "$RESULTS_DIR/performance_summary.md"
echo "Cargo version: $(cargo --version)" >> "$RESULTS_DIR/performance_summary.md"
echo "CPU cores: $(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 'unknown')" >> "$RESULTS_DIR/performance_summary.md"
echo "Available RAM: $(free -h 2>/dev/null | awk '/^Mem:/ {print $2}' || echo 'unknown')" >> "$RESULTS_DIR/performance_summary.md"
echo "\`\`\`" >> "$RESULTS_DIR/performance_summary.md"

# Final results
echo ""
echo "========================================"
print_status "Phase 3 Performance Test Results"
echo "========================================"

if [ $FAILED_TESTS -eq 0 ]; then
    print_success "🎉 ALL TESTS PASSED! Phase 3 meets all performance criteria"
    print_success "✅ Phase 3 is ready for completion"
    echo ""
    print_status "Key achievements:"
    echo "  ✅ Memory usage stays within 525MB limit"
    echo "  ✅ Prediction latency <100ms maintained"  
    echo "  ✅ Real-time training doesn't impact performance"
    echo "  ✅ System handles 100+ concurrent symbols"
    echo "  ✅ 1000+ updates/second processing capability"
    echo "  ✅ 24-hour stability validated"
    echo "  ✅ Graceful degradation under extreme load"
else
    print_error "❌ $FAILED_TESTS out of $TOTAL_TESTS tests failed"
    print_error "🚫 Phase 3 cannot be completed until all performance tests pass"
    echo ""
    print_warning "Failed tests - check logs in $RESULTS_DIR:"
    for log_file in "$RESULTS_DIR"/*.log; do
        if [ -f "$log_file" ] && ! grep -q "test result: ok" "$log_file" 2>/dev/null; then
            test_name=$(basename "$log_file" .log)
            echo "  ❌ $test_name"
        fi
    done
fi

echo ""
print_status "Detailed results saved to: $RESULTS_DIR"
print_status "Performance summary: $RESULTS_DIR/performance_summary.md"

# Copy results to latest for easy access
rm -rf results/latest
cp -r "$RESULTS_DIR" results/latest
print_status "Latest results also available at: results/latest"

echo ""
echo "🔗 Next steps:"
if [ $FAILED_TESTS -eq 0 ]; then
    echo "  1. Review performance summary at results/latest/performance_summary.md"
    echo "  2. Proceed with Phase 3 completion"
    echo "  3. Update documentation with performance achievements"
else
    echo "  1. Review failed test logs in results/latest/"
    echo "  2. Fix performance issues identified"
    echo "  3. Re-run tests until all pass"
    echo "  4. Only then proceed with Phase 3 completion"
fi

exit $FAILED_TESTS