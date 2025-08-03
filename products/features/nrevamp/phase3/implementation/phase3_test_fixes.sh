#!/bin/bash
# Phase 3 Test Fixes Script - Comprehensive batch fixes for compilation errors

echo "🔧 Starting comprehensive Phase 3 test fixes..."

# Fix 1: Replace ALL FannPredictor references with NeuralPredictor
echo "📝 Fixing FannPredictor imports and references..."
find tests -name "*.rs" -type f -exec sed -i '' 's/use autonomous_platform::neural::fann::FannPredictor;/use autonomous_platform::neural::predictor::NeuralPredictor;/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/use autonomous_platform::neural::fann_predictor::FannPredictor;/use autonomous_platform::neural::predictor::NeuralPredictor;/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/use crate::neural::fann_predictor::FannPredictor;/use autonomous_platform::neural::predictor::NeuralPredictor;/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/fann_predictor::FannPredictor/predictor::NeuralPredictor/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/FannPredictor::new(/NeuralPredictor::new(/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/_predictor: &FannPredictor/_predictor: \&NeuralPredictor/g' {} \;

# Fix 2: Replace FannModelConfig with appropriate type
echo "🔧 Fixing FannModelConfig references..."
find tests -name "*.rs" -type f -exec sed -i '' 's/use autonomous_platform::neural::fann_predictor::{FannPredictor, FannModelConfig};/use autonomous_platform::neural::predictor::NeuralPredictor;/g' {} \;

# Fix 3: Add .await to NeuralPredictor::new() calls
echo "⏳ Adding .await to async calls..."
find tests -name "*.rs" -type f -exec sed -i '' 's/NeuralPredictor::new(config)\.unwrap()/NeuralPredictor::new(config).await.unwrap()/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/NeuralPredictor::new(neural_config)\.unwrap()/NeuralPredictor::new(neural_config).await.unwrap()/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/NeuralPredictor::new(neural_config\.clone())\.unwrap()/NeuralPredictor::new(neural_config.clone()).await.unwrap()/g' {} \;

# Fix 4: Fix volume field from f64 to Vec<f64>
echo "📊 Fixing TimeSeriesData volume fields..."
find tests -name "*.rs" -type f -exec sed -i '' 's/volume: \([0-9.]*\),$/volume: vec![\1],/g' {} \;
find tests -name "*.rs" -type f -exec sed -i '' 's/volume: \([0-9.]*\) \+ /volume: vec![\1 + /g' {} \;

# Fix 5: Fix DaaCoordinator initialization with market_hours
echo "🏢 Fixing DaaCoordinator initialization..."
find tests -name "*.rs" -type f -exec sed -i '' 's/DaaCoordinator::new(\([^,]*\), \([^,]*\), \([^)]*\))/DaaCoordinator::new(\1, \2, \3, create_test_market_hours())/g' {} \;

# Fix 6: Import create_test_market_hours where needed
echo "📦 Adding test helper imports..."
find tests -name "*.rs" -type f -exec grep -l "create_test_market_hours()" {} \; | while read file; do
    if ! grep -q "use.*test_utils" "$file"; then
        sed -i '' '/^use autonomous_platform/a\
use crate::helpers::test_utils::create_test_market_hours;' "$file"
    fi
done

echo "✅ Comprehensive batch fixes completed!"
echo ""
echo "📋 Remaining manual fixes needed:"
echo "1. Add missing NeuralConfig fields (input_size, output_size, hidden_layers, learning_rate)"
echo "2. Complete TimeSeriesData fields (volume_value, values, intervals, timestamps, metadata_map)"
echo "3. Fix any remaining async/await context issues"
echo "4. Handle Result<> unwrapping in async tests"