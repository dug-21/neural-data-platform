#!/bin/bash
# Script to run tests with coverage measurement for DAA integration

set -e

echo "🧪 Running DAA Integration Tests with Coverage..."

# Install cargo-tarpaulin if not already installed
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "📦 Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Clean previous coverage data
rm -rf target/tarpaulin

# Run tests with coverage
echo "🔍 Measuring test coverage for DAA integration..."

# Run unit/integration tests
cargo tarpaulin \
    --test daa_unit_integration_test \
    --exclude-files "vendor/*" \
    --exclude-files "tests/*" \
    --exclude-files "src/bin/*" \
    --ignore-panics \
    --timeout 300 \
    --out Lcov \
    --output-dir target/tarpaulin \
    -- --test-threads=1

# Generate summary
echo ""
echo "📊 Coverage Summary:"
cargo tarpaulin \
    --test daa_unit_integration_test \
    --exclude-files "vendor/*" \
    --exclude-files "tests/*" \
    --exclude-files "src/bin/*" \
    --print-summary \
    -- --test-threads=1

echo ""
echo "✅ Coverage report generated at: target/tarpaulin/lcov.info"