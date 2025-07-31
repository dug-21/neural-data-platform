#!/bin/bash
# Phase 3b Performance Validation Script
# Runs performance benchmarks and validates requirements

set -e

echo "=== Phase 3b Performance Validation ==="
echo "Requirements:"
echo "  - Event emission latency <1ms"
echo "  - Decision making latency <10ms"
echo "  - Zero memory overhead"
echo ""

# Create results directory
mkdir -p target

# Run the benchmarks
echo "Running performance benchmarks..."
cargo bench --bench phase3b_performance_benchmarks -- --verbose

# Check if results file was created
if [ -f "target/phase3b_benchmark_results.json" ]; then
    echo ""
    echo "=== Benchmark Results ==="
    cat target/phase3b_benchmark_results.json | jq '.'
    
    # Check if all tests passed
    FAILED=$(cat target/phase3b_benchmark_results.json | jq '.passed' | grep -c "false" || true)
    
    if [ "$FAILED" -eq "0" ]; then
        echo ""
        echo "✅ All performance requirements PASSED!"
    else
        echo ""
        echo "❌ Some performance requirements FAILED!"
        echo "Failed tests:"
        cat target/phase3b_benchmark_results.json | jq 'select(.passed == false) | .test_name'
        exit 1
    fi
else
    echo "⚠️  No benchmark results file found. Benchmarks may have failed to run."
fi

echo ""
echo "=== Detailed Latency Analysis ==="
echo "Check target/criterion/ for detailed benchmark reports"