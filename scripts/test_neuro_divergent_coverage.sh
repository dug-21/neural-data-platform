#!/bin/bash
# Test runner for neuro-divergent integration with coverage reporting

set -e

echo "🧪 Running Neuro-Divergent Integration Tests with Coverage"
echo "========================================================="

# Clean previous coverage data
echo "🧹 Cleaning previous coverage data..."
cargo clean -p autonomous-platform
rm -rf target/coverage

# Install cargo-tarpaulin if not already installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "📦 Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run tests with coverage for neuro-divergent components
echo "🔍 Running tests with coverage analysis..."

# Run specific test modules with coverage
cargo tarpaulin \
    --out Html \
    --output-dir target/coverage \
    --ignore-panics \
    --ignore-tests \
    --exclude-files "vendor/*" \
    --exclude-files "tests/*" \
    --exclude-files "target/*" \
    --timeout 300 \
    --run-types Tests \
    --packages autonomous-platform \
    -- \
    neuro_divergent_adapter_test \
    neuro_divergent_adapter_comprehensive_test \
    fann_predictor_integration_test \
    neuro_divergent_error_handling_test

# Generate detailed report
echo "📊 Generating detailed coverage report..."
cargo tarpaulin \
    --print-summary \
    --print-source-files \
    --ignore-panics \
    --ignore-tests \
    --exclude-files "vendor/*" \
    --exclude-files "tests/*" \
    --exclude-files "target/*" \
    --packages autonomous-platform \
    -- \
    neuro_divergent

# Check coverage threshold
COVERAGE=$(cargo tarpaulin --print-summary --packages autonomous-platform -- neuro_divergent | grep "Coverage" | awk '{print $2}' | sed 's/%//')

echo ""
echo "📈 Coverage Summary:"
echo "==================="
echo "Total Coverage: ${COVERAGE}%"

# Check if we meet the 85% threshold
if (( $(echo "$COVERAGE >= 85" | bc -l) )); then
    echo "✅ Coverage threshold met (≥85%)"
else
    echo "❌ Coverage below threshold (<85%)"
    echo "   Please add more tests to improve coverage"
fi

echo ""
echo "📁 Coverage report generated at: target/coverage/tarpaulin-report.html"
echo "   Open in browser: file://$(pwd)/target/coverage/tarpaulin-report.html"

# Run individual test categories and show results
echo ""
echo "🔧 Running Test Categories:"
echo "=========================="

echo ""
echo "1️⃣ Model Creation Tests..."
cargo test --lib neuro_divergent_adapter_comprehensive_test::adapter_conversion_tests -- --nocapture

echo ""
echo "2️⃣ Prediction Tests..."
cargo test --lib neuro_divergent_adapter_comprehensive_test::prediction_conversion_tests -- --nocapture

echo ""
echo "3️⃣ Error Handling Tests..."
cargo test --lib neuro_divergent_error_handling_test -- --nocapture

echo ""
echo "4️⃣ Type Conversion Tests..."
cargo test --lib neuro_divergent_adapter_comprehensive_test::model_input_preparation_tests -- --nocapture

echo ""
echo "5️⃣ Feature Flag Behavior Tests..."
cargo test --lib neuro_divergent_adapter_comprehensive_test::feature_flag_tests -- --nocapture

echo ""
echo "✅ All test categories completed!"
echo ""
echo "📊 Test Summary:"
cargo test --lib neuro_divergent --no-run 2>&1 | grep -E "(test result:|passed|failed)"

# Generate test documentation
echo ""
echo "📚 Generating test documentation..."
cargo doc --no-deps --document-private-items --open \
    -p autonomous-platform \
    --features "neuro-divergent-advanced"

echo ""
echo "🎉 Testing complete! Check the coverage report for detailed analysis."