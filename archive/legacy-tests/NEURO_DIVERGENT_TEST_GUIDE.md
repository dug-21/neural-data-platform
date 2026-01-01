# Neuro-Divergent Integration Test Guide

## Overview

This guide documents the comprehensive test suite for the neuro-divergent integration in the neural-trader platform. The tests ensure robust functionality, error handling, and performance of the integration between FANN neural networks and neuro-divergent models.

## Test Structure

### 1. Core Test Files

#### `neuro_divergent_adapter_comprehensive_test.rs`
Comprehensive unit tests covering all aspects of the NeuroDivergentAdapter:

- **Adapter Conversion Tests**: Tests for DataFrame conversions
  - `test_to_neuro_divergent_df_basic`: Basic DataFrame creation
  - `test_to_neuro_divergent_df_empty_data`: Empty data error handling
  - `test_from_neuro_divergent_df_basic`: DataFrame to TimeSeriesData conversion
  - `test_from_neuro_divergent_df_missing_columns`: Missing column handling

- **Model Input Preparation Tests**: Tests for neural network input preparation
  - `test_prepare_model_input_basic`: Standard input preparation
  - `test_prepare_model_input_insufficient_data`: Insufficient data handling
  - `test_prepare_model_input_edge_cases`: Boundary condition testing

- **Prediction Conversion Tests**: Tests for prediction result handling
  - `test_predictions_to_timeseries_basic`: Basic prediction conversion
  - `test_predictions_to_timeseries_empty`: Empty prediction handling

- **Performance Tests**: Large dataset and performance validation
  - `test_large_dataset_conversion`: 10,000+ data point handling
  - `test_model_input_preparation_performance`: Performance benchmarks

#### `fann_predictor_integration_test.rs`
Integration tests between FannPredictor and NeuroDivergentAdapter:

- **Model Configuration Tests**: FANN model setup validation
  - `test_fann_model_config_default`: Default configuration
  - `test_fann_model_config_custom`: Custom configurations

- **Data Conversion Integration**: FANN-compatible data format tests
  - `test_timeseries_to_fann_input`: Input format validation
  - `test_fann_output_to_predictions`: Output conversion

- **Ensemble Integration**: Multi-model ensemble testing
  - `test_ensemble_weight_calculation`: Weight normalization
  - `test_ensemble_prediction_aggregation`: Prediction combining

#### `neuro_divergent_error_handling_test.rs`
Comprehensive error handling and edge case tests:

- **Adapter Error Tests**: Error condition validation
  - `test_empty_data_error`: Empty data handling
  - `test_missing_required_columns`: Missing column errors
  - `test_invalid_timestamp_format`: Timestamp parsing errors

- **Boundary Condition Tests**: Edge case handling
  - `test_single_data_point`: Single point processing
  - `test_exact_minimum_data_points`: Minimum data validation
  - `test_maximum_indicators`: Large indicator count

- **NaN/Infinity Tests**: Special value handling
  - `test_nan_values_handling`: NaN processing
  - `test_infinity_values`: Infinity value support

### 2. Mock Objects

The test suite includes comprehensive mocks for vendor models:

```rust
// Mock FANN Network
MockFannNetwork {
    fn run(&self, input: &[f32]) -> Vec<f32>;
    fn train_on_data(&mut self, data: &[(Vec<f32>, Vec<f32>)]) -> f32;
}

// Mock Neuro-Divergent Model
MockNeuroDivergentModel {
    fn predict(&self, input: &[f64], horizon: usize) -> Result<Vec<f64>>;
    fn train(&mut self, data: &[Vec<f64>], targets: &[f64]) -> Result<f64>;
}
```

### 3. Test Categories

#### Model Creation Tests
Tests that verify proper initialization and configuration of neural models:
- Architecture validation
- Parameter initialization
- Model type selection

#### Prediction Tests
Tests that validate prediction workflows:
- Input preparation
- Prediction execution
- Output conversion
- Result validation

#### Error Handling Tests
Tests that ensure robust error handling:
- Invalid input detection
- Recovery mechanisms
- Error message clarity
- Graceful degradation

#### Type Conversion Tests
Tests that verify data type conversions:
- TimeSeriesData ↔ DataFrame
- f32 ↔ f64 conversions
- Array ↔ Vector conversions

#### Feature Flag Behavior Tests
Tests that validate conditional compilation:
- GPU acceleration flags
- Advanced model features
- Optional dependencies

## Running Tests

### Run All Neuro-Divergent Tests
```bash
cargo test neuro_divergent
```

### Run Specific Test Categories
```bash
# Adapter conversion tests
cargo test neuro_divergent_adapter_comprehensive_test::adapter_conversion_tests

# Error handling tests
cargo test neuro_divergent_error_handling_test

# Integration tests
cargo test fann_predictor_integration_test
```

### Run with Coverage
```bash
./scripts/test_neuro_divergent_coverage.sh
```

### Run with Verbose Output
```bash
cargo test neuro_divergent -- --nocapture --test-threads=1
```

## Coverage Requirements

The test suite aims for **85% code coverage** across:

1. **NeuroDivergentAdapter**: All public methods
2. **Data Converters**: All conversion functions
3. **FannPredictor Integration**: Integration points
4. **Error Paths**: All error conditions
5. **Edge Cases**: Boundary conditions

## Test Data Helpers

### Creating Test TimeSeries
```rust
fn create_test_timeseries(count: usize, symbol: &str) -> Vec<TimeSeriesData> {
    // Generates realistic time series data with indicators
}
```

### Creating Mock Models
```rust
let mock_model = MockNeuroDivergentModel::new(vec![predictions]);
mock_model.expect_predict()
    .times(1)
    .returning(|_, _| Ok(vec![100.0, 101.0, 102.0]));
```

## Performance Benchmarks

Expected performance metrics:
- DataFrame conversion: <1ms for 1000 points
- Model input preparation: <5ms for 5000 points
- Prediction conversion: <0.1ms per prediction
- Large dataset handling: <1s for 10,000 points

## Common Issues and Solutions

### Issue: Compilation Errors with Vendor Dependencies
**Solution**: Ensure vendor modules are properly excluded in Cargo.toml workspace

### Issue: Test Timeout on Large Datasets
**Solution**: Increase test timeout or reduce dataset size for CI environments

### Issue: Feature Flag Tests Not Running
**Solution**: Enable features explicitly: `cargo test --features neuro-divergent-advanced`

## Adding New Tests

When adding new tests:

1. **Follow Naming Convention**: `test_<component>_<scenario>`
2. **Use Test Helpers**: Leverage existing helper functions
3. **Document Edge Cases**: Add comments for non-obvious test cases
4. **Verify Coverage**: Run coverage report after adding tests
5. **Update This Guide**: Document new test categories

## Continuous Integration

The test suite is designed for CI/CD pipelines:

```yaml
test-neuro-divergent:
  script:
    - cargo test neuro_divergent --release
    - ./scripts/test_neuro_divergent_coverage.sh
  coverage: '/Total Coverage: (\d+\.\d+)%/'
```

## Debugging Tests

### Enable Debug Logging
```bash
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Run Single Test
```bash
cargo test test_exact_name -- --exact
```

### Generate Test Documentation
```bash
cargo doc --no-deps --document-private-items --open
```