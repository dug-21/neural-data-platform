#!/bin/bash
# Script to run neural trader performance benchmarks

set -e

echo "🚀 Neural Trader Performance Benchmarking Suite"
echo "=============================================="
echo ""

# Color codes for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found. Please install Rust.${NC}"
    exit 1
fi

# Function to run a specific benchmark
run_benchmark() {
    local bench_name=$1
    local description=$2
    
    echo -e "${BLUE}Running: ${description}${NC}"
    echo "----------------------------------------"
    
    if cargo bench --bench neural_trader_bench -- "$bench_name" --warm-up-time 1 --measurement-time 5; then
        echo -e "${GREEN}✓ ${description} completed successfully${NC}"
    else
        echo -e "${RED}✗ ${description} failed${NC}"
        return 1
    fi
    echo ""
}

# Main benchmarking flow
echo -e "${YELLOW}Starting performance benchmarks...${NC}"
echo ""

# Create results directory
RESULTS_DIR="benchmark_results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Run individual benchmark groups
echo "1. Neural Predictions Comparison (Placeholder vs FANN)"
run_benchmark "neural_predictions_comparison" "Neural Predictions Comparison"

echo "2. DAA Decision Latency (<1ms target)"
run_benchmark "daa_decision_latency" "DAA Decision Latency"

echo "3. Ensemble Performance"
run_benchmark "ensemble_performance" "Ensemble Prediction Performance"

echo "4. Memory Usage Analysis"
run_benchmark "memory_usage" "Memory Usage and Optimization"

echo "5. Neural Trading Strategy"
run_benchmark "neural_trading_strategy" "Full Trading Strategy Performance"

echo "6. Latency Distribution Analysis"
run_benchmark "latency_distribution" "Latency Percentiles Analysis"

# Run all benchmarks for complete report
echo -e "${YELLOW}Running complete benchmark suite...${NC}"
cargo bench --bench neural_trader_bench

# Copy results
echo -e "${BLUE}Copying benchmark results...${NC}"
cp -r target/criterion/* "$RESULTS_DIR/" 2>/dev/null || true

# Generate summary report
echo -e "${GREEN}Benchmark Summary${NC}" > "$RESULTS_DIR/summary.txt"
echo "==================" >> "$RESULTS_DIR/summary.txt"
echo "" >> "$RESULTS_DIR/summary.txt"
echo "Date: $(date)" >> "$RESULTS_DIR/summary.txt"
echo "Results directory: $RESULTS_DIR" >> "$RESULTS_DIR/summary.txt"
echo "" >> "$RESULTS_DIR/summary.txt"
echo "Key Performance Targets:" >> "$RESULTS_DIR/summary.txt"
echo "- DAA Decision Latency: <1ms (p95)" >> "$RESULTS_DIR/summary.txt"
echo "- Neural Prediction: <10ms (single)" >> "$RESULTS_DIR/summary.txt"
echo "- Ensemble Prediction: <25ms" >> "$RESULTS_DIR/summary.txt"
echo "- Memory per Model: <50MB" >> "$RESULTS_DIR/summary.txt"

# Final message
echo ""
echo -e "${GREEN}===============================================${NC}"
echo -e "${GREEN}✓ Performance benchmarks completed!${NC}"
echo -e "${GREEN}===============================================${NC}"
echo ""
echo "Results saved to: $RESULTS_DIR/"
echo "HTML reports available at: target/criterion/report/index.html"
echo ""
echo "To view the HTML report:"
echo "  open target/criterion/report/index.html"
echo ""
echo "To compare with baseline:"
echo "  cargo bench --bench neural_trader_bench -- --baseline placeholder"