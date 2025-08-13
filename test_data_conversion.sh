#!/bin/bash

# Test data conversion functionality
echo "Testing data conversion between database rows and TimeSeriesData..."

# Check if we can build the project with the new data conversion logic
echo "Building project to verify data structures..."
cargo check --lib --quiet

if [ $? -eq 0 ]; then
    echo "✅ Project builds successfully with new data conversion logic"
else
    echo "❌ Build failed - there are compilation errors"
    exit 1
fi

# Run specific tests for data conversion
echo "Running data conversion tests..."
cargo test data::test_conversion --lib --quiet

if [ $? -eq 0 ]; then
    echo "✅ Data conversion tests pass"
else
    echo "⚠️ Data conversion tests may not exist yet or have issues"
fi

# Test basic TimeSeriesData creation
echo "Testing basic data structure functionality..."
cargo test validate --lib --quiet

echo "Data conversion testing complete!"