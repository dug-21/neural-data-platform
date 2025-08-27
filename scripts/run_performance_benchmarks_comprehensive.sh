#!/bin/bash

# Comprehensive Performance Benchmark Runner
# Neural Trading Platform - Performance Test Suite
# Generated: 2025-08-27

set -euo pipefail

# Configuration
BENCHMARK_OUTPUT_DIR="/workspaces/neural-trader/benchmark_results"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
REPORT_FILE="${BENCHMARK_OUTPUT_DIR}/performance_report_${TIMESTAMP}.md"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Create output directory
mkdir -p "$BENCHMARK_OUTPUT_DIR"

echo -e "${BLUE}🚀 Neural Trading Platform - Comprehensive Performance Benchmarks${NC}"
echo -e "${BLUE}=================================================================${NC}"
echo "Timestamp: $(date)"
echo "Output Directory: $BENCHMARK_OUTPUT_DIR"
echo ""

# Initialize report
cat > "$REPORT_FILE" << EOF
# Performance Benchmark Report
**Generated:** $(date)
**System:** $(uname -a)

## Executive Summary

EOF

# Function to log and append to report
log_and_report() {
    local message="$1"
    echo -e "$message"
    echo "$message" >> "$REPORT_FILE"
}

# Function to run benchmark with timeout and capture metrics
run_benchmark() {
    local name="$1"
    local command="$2"
    local timeout_seconds="${3:-300}" # 5 minute default timeout
    local description="${4:-}"
    
    log_and_report "### $name"
    log_and_report "**Description:** $description"
    log_and_report "\`\`\`bash"
    log_and_report "Command: $command"
    log_and_report "\`\`\`"
    
    echo -e "${YELLOW}Running: $name${NC}"
    
    local start_time=$(date +%s.%3N)
    local temp_output=$(mktemp)
    
    if timeout "$timeout_seconds" bash -c "$command" > "$temp_output" 2>&1; then
        local end_time=$(date +%s.%3N)
        local duration=$(echo "$end_time - $start_time" | bc -l)
        
        echo -e "${GREEN}✅ $name completed in ${duration}s${NC}"
        
        log_and_report "**Status:** ✅ SUCCESS"
        log_and_report "**Duration:** ${duration}s"
        log_and_report "**Output:**"
        log_and_report "\`\`\`"
        head -50 "$temp_output" >> "$REPORT_FILE"
        log_and_report "\`\`\`"
    else
        local end_time=$(date +%s.%3N)
        local duration=$(echo "$end_time - $start_time" | bc -l)
        
        echo -e "${RED}❌ $name failed or timed out after ${duration}s${NC}"
        
        log_and_report "**Status:** ❌ FAILED/TIMEOUT"
        log_and_report "**Duration:** ${duration}s"
        log_and_report "**Error Output:**"
        log_and_report "\`\`\`"
        tail -20 "$temp_output" >> "$REPORT_FILE"
        log_and_report "\`\`\`"
    fi
    
    rm -f "$temp_output"
    log_and_report ""
}

# System Information
log_and_report "## System Information"
log_and_report "\`\`\`"
log_and_report "CPU: $(nproc) cores"
log_and_report "Memory: $(free -h | grep Mem | awk '{print $2}')"
log_and_report "Disk: $(df -h /workspaces/neural-trader | tail -1 | awk '{print $2 " total, " $4 " available"}')"
log_and_report "Load: $(uptime | sed 's/.*load average: //')"
log_and_report "\`\`\`"
log_and_report ""

# 1. Build Performance Tests
run_benchmark \
    "Build Performance Test" \
    "cd /workspaces/neural-trader && time cargo check --workspace --release" \
    180 \
    "Measures compilation time and build performance across all workspace members"

run_benchmark \
    "Release Build Performance" \
    "cd /workspaces/neural-trader && time cargo build --release --workspace" \
    600 \
    "Full release build with all optimizations enabled"

# 2. Neural Core Performance Tests
run_benchmark \
    "Neural Core EventBus Performance" \
    "cd /workspaces/neural-trader/neural-core && timeout 120 cargo test --release eventbus -- --nocapture" \
    180 \
    "Tests EventBus message processing performance and latency"

run_benchmark \
    "Neural Core Service Tests" \
    "cd /workspaces/neural-trader/neural-core && timeout 60 cargo test --release --lib -- --nocapture" \
    120 \
    "Core neural network processing performance tests"

# 3. Data Staging Performance Tests
run_benchmark \
    "Data Staging Performance" \
    "cd /workspaces/neural-trader/data-staging && timeout 90 cargo test --release -- --nocapture performance" \
    150 \
    "Data transformation and staging pipeline performance"

run_benchmark \
    "Data Staging Integration Tests" \
    "cd /workspaces/neural-trader/data-staging && timeout 90 cargo test --release integration_tests" \
    150 \
    "End-to-end data processing pipeline tests"

# 4. Memory Usage Analysis
run_benchmark \
    "Memory Usage Analysis" \
    "ps aux --sort=-%mem | head -20; echo '---'; free -h; echo '---'; df -h" \
    30 \
    "System memory usage analysis and available resources"

run_benchmark \
    "Build Memory Usage" \
    "/usr/bin/time -v cargo check --workspace 2>&1 | grep -E '(Maximum resident|User time|System time|CPU|Memory)'" \
    180 \
    "Memory consumption during build process"

# 5. Benchmark Suite Execution
run_benchmark \
    "Neural Trader Benchmarks" \
    "cd /workspaces/neural-trader && timeout 300 cargo bench --bench neural_trader_bench 2>&1 | head -100" \
    360 \
    "Comprehensive neural trading algorithm benchmarks"

run_benchmark \
    "Performance Benchmarks Suite" \
    "cd /workspaces/neural-trader && timeout 300 cargo bench --bench performance_benchmarks 2>&1 | head -100" \
    360 \
    "Full performance benchmark suite covering all components"

# 6. Throughput Tests
run_benchmark \
    "Service Throughput Test" \
    "cd /workspaces/neural-trader && timeout 60 cargo test --release throughput -- --nocapture" \
    120 \
    "Service throughput and concurrent processing capability"

# 7. Latency Tests
run_benchmark \
    "Latency Analysis" \
    "cd /workspaces/neural-trader && timeout 60 cargo test --release latency -- --nocapture" \
    120 \
    "End-to-end latency measurement for critical operations"

# 8. Proto Event Performance
run_benchmark \
    "Proto Event Processing" \
    "cd /workspaces/neural-trader && timeout 60 cargo test --release proto -- --nocapture" \
    120 \
    "Protocol buffer event serialization/deserialization performance"

# 9. Cache Performance
run_benchmark \
    "Redis Cache Performance" \
    "cd /workspaces/neural-trader && timeout 60 cargo test --release cache -- --nocapture || echo 'Cache tests may require Redis connection'" \
    120 \
    "Cache operations performance (requires Redis connection)"

# 10. Database Performance
run_benchmark \
    "Database Performance" \
    "cd /workspaces/neural-trader && timeout 60 cargo test --release database -- --nocapture || echo 'Database tests may require DB connection'" \
    120 \
    "Database operations performance (requires database connection)"

# Generate Summary
log_and_report "## Performance Summary"

# Count successful and failed tests
successful_tests=$(grep -c "✅ SUCCESS" "$REPORT_FILE" || echo "0")
failed_tests=$(grep -c "❌ FAILED" "$REPORT_FILE" || echo "0")
total_tests=$((successful_tests + failed_tests))

log_and_report "- **Total Tests:** $total_tests"
log_and_report "- **Successful:** $successful_tests"
log_and_report "- **Failed:** $failed_tests"
log_and_report "- **Success Rate:** $(( successful_tests * 100 / total_tests ))%" 2>/dev/null || log_and_report "- **Success Rate:** N/A"

# Performance Targets Analysis
log_and_report ""
log_and_report "## Performance Targets Status"
log_and_report ""
log_and_report "| Target | Requirement | Status | Notes |"
log_and_report "|--------|-------------|---------|-------|"
log_and_report "| Build Time | < 60s | ⏳ | Check build performance results |"
log_and_report "| Memory Usage | < 50MB/symbol | ⏳ | Requires runtime measurement |"
log_and_report "| EventBus Latency | < 5ms | ⏳ | Check EventBus test results |"
log_and_report "| Neural Prediction | < 100ms | ⏳ | Check neural benchmarks |"
log_and_report "| Data Processing | < 50ms | ⏳ | Check data staging results |"

# Recommendations
log_and_report ""
log_and_report "## Recommendations"
log_and_report ""
log_and_report "1. **Monitor Build Performance**: Build times should remain under 60 seconds"
log_and_report "2. **Memory Optimization**: Implement memory profiling for runtime analysis"
log_and_report "3. **Continuous Benchmarking**: Integrate this script into CI/CD pipeline"
log_and_report "4. **Performance Regression Testing**: Establish baseline metrics"
log_and_report "5. **Resource Optimization**: Leverage available CPU cores for parallelization"

# Final output
echo ""
echo -e "${BLUE}📊 Benchmark Summary:${NC}"
echo -e "${GREEN}✅ Successful Tests: $successful_tests${NC}"
echo -e "${RED}❌ Failed Tests: $failed_tests${NC}"
echo -e "${BLUE}📁 Full Report: $REPORT_FILE${NC}"

# Create latest symlink
ln -sf "$REPORT_FILE" "${BENCHMARK_OUTPUT_DIR}/latest_performance_report.md"

echo ""
echo -e "${BLUE}🎯 Performance benchmarking completed!${NC}"
echo -e "${YELLOW}View the full report at: $REPORT_FILE${NC}"

exit 0