#!/bin/bash

# Phase 1 Coverage Analysis Script
# Generates comprehensive coverage reports for Python and Rust components

set -e

echo "======================================"
echo "Phase 1 Coverage Analysis"
echo "======================================"

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Create coverage directory
COVERAGE_DIR="coverage_reports/phase1"
mkdir -p $COVERAGE_DIR

# Python Coverage
echo -e "\n${YELLOW}1. Python Coverage Analysis${NC}"
echo "================================"

# Run Python tests with coverage
echo "Running Python tests with coverage..."
cd /workspaces/neural-trader

# Install coverage tools if needed
pip install -q coverage pytest-cov

# Run data ingestion tests with coverage
python -m pytest tests/phase1_integration_test.py \
    --cov=data_ingestion \
    --cov-report=html:$COVERAGE_DIR/python_html \
    --cov-report=json:$COVERAGE_DIR/python_coverage.json \
    --cov-report=term \
    -v

# Analyze Python coverage
echo -e "\n${GREEN}Python Coverage Summary:${NC}"
python -m coverage report --format=total

# Rust Coverage
echo -e "\n${YELLOW}2. Rust Coverage Analysis${NC}"
echo "================================"

# Check if cargo-tarpaulin is installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run Rust tests with coverage
echo "Running Rust tests with coverage..."
cargo tarpaulin \
    --out Html \
    --out Json \
    --output-dir $COVERAGE_DIR \
    --exclude-files "*/tests/*" \
    --exclude-files "*/benches/*" \
    --exclude-files "*/vendor/*" \
    --ignore-panics \
    --timeout 300 \
    --features test-utils \
    -- --test-threads=1

# Combined Coverage Report
echo -e "\n${YELLOW}3. Combined Coverage Analysis${NC}"
echo "================================"

# Create coverage summary
cat > $COVERAGE_DIR/phase1_coverage_summary.md << EOF
# Phase 1 Coverage Report

## Python Coverage

\`\`\`
$(python -m coverage report)
\`\`\`

## Rust Coverage

\`\`\`
$(cargo tarpaulin --print-summary 2>/dev/null || echo "See tarpaulin-report.html for details")
\`\`\`

## Phase 1 Component Coverage

### Data Ingestion
- Providers: $(find data_ingestion/providers -name "*.py" | wc -l) files
- Coverage: See python_coverage.json

### Feature Engineering
- Technical Indicators: src/features/technical_indicators.rs
- Market Microstructure: src/features/market_microstructure.rs
- Coverage: See tarpaulin-report.html

### Neural Models
- FANN Predictor: src/neural/fann_predictor.rs
- Coverage: See tarpaulin-report.html

## Coverage Targets

- Target: 85%+ overall coverage
- Critical paths: 90%+ coverage
- Integration tests: Full end-to-end coverage

EOF

# Check coverage thresholds
echo -e "\n${YELLOW}4. Coverage Validation${NC}"
echo "================================"

# Extract Python coverage percentage
PYTHON_COV=$(python -m coverage report --format=total 2>/dev/null || echo "0")
echo "Python Coverage: ${PYTHON_COV}%"

# Extract Rust coverage (if tarpaulin ran successfully)
if [ -f "$COVERAGE_DIR/tarpaulin-report.json" ]; then
    RUST_COV=$(python3 -c "
import json
with open('$COVERAGE_DIR/tarpaulin-report.json') as f:
    data = json.load(f)
    print(f\"{data.get('coverage', 0):.1f}\")
" 2>/dev/null || echo "0")
    echo "Rust Coverage: ${RUST_COV}%"
else
    RUST_COV=0
    echo "Rust Coverage: Not available (install cargo-tarpaulin)"
fi

# Overall assessment
echo -e "\n${YELLOW}5. Coverage Assessment${NC}"
echo "================================"

# Calculate if we meet the 85% target
MEETS_TARGET=false
if (( $(echo "$PYTHON_COV >= 85" | bc -l) )); then
    echo -e "${GREEN}✓ Python coverage meets target (85%+)${NC}"
    MEETS_TARGET=true
else
    echo -e "${RED}✗ Python coverage below target (<85%)${NC}"
fi

# Generate HTML report index
cat > $COVERAGE_DIR/index.html << EOF
<!DOCTYPE html>
<html>
<head>
    <title>Phase 1 Coverage Report</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .metric { display: inline-block; margin: 10px; padding: 20px; border: 1px solid #ddd; }
        .good { background-color: #d4edda; }
        .warning { background-color: #fff3cd; }
        .bad { background-color: #f8d7da; }
    </style>
</head>
<body>
    <h1>Phase 1 Neural-Expand Coverage Report</h1>
    
    <div class="metrics">
        <div class="metric ${PYTHON_COV >= 85 ? 'good' : 'bad'}">
            <h3>Python Coverage</h3>
            <p>${PYTHON_COV}%</p>
            <a href="python_html/index.html">View Details</a>
        </div>
        
        <div class="metric">
            <h3>Rust Coverage</h3>
            <p>${RUST_COV}%</p>
            <a href="tarpaulin-report.html">View Details</a>
        </div>
    </div>
    
    <h2>Component Coverage</h2>
    <ul>
        <li>Data Ingestion Providers</li>
        <li>Feature Engineering (Elliott Waves, Harmonic Patterns)</li>
        <li>Market Microstructure (Toxicity Metrics)</li>
        <li>Neural Models (LSTM, GRU, Attention)</li>
    </ul>
    
    <h2>Reports</h2>
    <ul>
        <li><a href="phase1_coverage_summary.md">Coverage Summary</a></li>
        <li><a href="python_coverage.json">Python Coverage JSON</a></li>
        <li><a href="tarpaulin-report.json">Rust Coverage JSON</a></li>
    </ul>
</body>
</html>
EOF

echo -e "\n${GREEN}Coverage reports generated in: $COVERAGE_DIR${NC}"
echo "Open $COVERAGE_DIR/index.html to view the coverage dashboard"

# Return success/failure based on coverage
if [ "$MEETS_TARGET" = true ]; then
    echo -e "\n${GREEN}✓ Phase 1 coverage analysis PASSED${NC}"
    exit 0
else
    echo -e "\n${RED}✗ Phase 1 coverage analysis FAILED${NC}"
    echo "Increase test coverage to meet the 85% target"
    exit 1
fi