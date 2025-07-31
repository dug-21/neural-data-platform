# TDD Test Strategy for Phase 1: Mock Removal

## Overview

This document outlines the TDD London School approach for testing the removal of mock adapters and implementation of feature flag behavior in the neural-trader system.

## Test Strategy Principles

### London School TDD Approach
1. **Outside-In Testing**: Start from the API level and work inward
2. **Mock All Dependencies**: Use mocks/stubs for external dependencies
3. **Test Behavior, Not Implementation**: Focus on what the system does, not how
4. **Fail First**: Write tests that fail initially, then make them pass

## Test Categories

### 1. Mock Removal Verification Tests
- Verify that mock adapters are NOT used in production
- Ensure real FANN models are initialized correctly
- Test that feature flags properly control adapter behavior

### 2. Feature Flag Behavior Tests
- Test `use_real_models` flag behavior
- Test health monitoring enable/disable
- Test fallback system enable/disable
- Test circuit breaker enable/disable

### 3. Enhanced Neural Adapter Tests
- Test adapter initialization with various configurations
- Test prediction flow without mocks
- Test error handling and fallback mechanisms
- Test performance monitoring and metrics collection

## Test Implementation Plan

### Phase 1: Mock Removal Tests (`test_mock_removal.rs`)
1. **Test Mock Adapter is Not Used**
   - Verify no mock adapter initialization
   - Ensure predictions use real FANN models
   - Test configuration validation

2. **Test Real Model Usage**
   - Verify FANN predictor initialization
   - Test model loading and configuration
   - Verify prediction results are from real models

### Phase 2: Feature Flag Tests (`test_feature_flags.rs`)
1. **Test Feature Flag Parsing**
   - Test environment variable reading
   - Test configuration file parsing
   - Test default values

2. **Test Feature Flag Effects**
   - Test model selection based on flags
   - Test monitoring activation
   - Test fallback behavior

### Phase 3: Integration Tests
1. **End-to-End Prediction Tests**
   - Test complete prediction flow
   - Verify no mock data in results
   - Test performance metrics

## Mock Strategy

### Dependencies to Mock
1. **External Services**
   - Database connections (TimescaleDB, Redis)
   - Network calls
   - File system operations

2. **Time-based Operations**
   - System clock for timestamps
   - Timeouts and delays

3. **Random Operations**
   - Model initialization randomness
   - Jitter in retry logic

### Dependencies NOT to Mock
1. **FANN Neural Networks**
   - Real FANN predictor must be used
   - No mock predictions allowed

2. **Core Business Logic**
   - Adapter selection logic
   - Feature flag evaluation
   - Error handling flows

## Test Data Strategy

### Test Data Generators
```rust
// Generate realistic time series data for testing
fn generate_test_time_series(
    symbol: &str,
    points: usize,
    base_price: f64,
    volatility: f64
) -> Vec<TimeSeriesData>

// Generate configuration for testing
fn generate_test_config(
    use_real_models: bool,
    enable_features: Vec<String>
) -> EnhancedNeuralConfig
```

### Test Fixtures
- Pre-calculated expected prediction results
- Known-good configuration files
- Error scenario data sets

## Assertion Strategy

### Key Assertions
1. **No Mock Usage**
   ```rust
   assert!(!adapter.is_using_mock());
   assert!(adapter.is_using_fann_predictor());
   ```

2. **Feature Flag Behavior**
   ```rust
   assert_eq!(config.use_real_models, expected_value);
   assert_eq!(adapter.health_monitor.is_some(), config.enable_health_monitoring);
   ```

3. **Prediction Quality**
   ```rust
   assert!(result.confidence > 0.0);
   assert_eq!(result.model_used, "FANN_MLP");
   ```

## Error Testing Strategy

### Error Scenarios to Test
1. **Configuration Errors**
   - Invalid feature flag values
   - Missing required configuration
   - Conflicting settings

2. **Runtime Errors**
   - Model initialization failures
   - Prediction timeouts
   - Resource exhaustion

3. **Fallback Scenarios**
   - Primary model failure
   - Health check failures
   - Circuit breaker activation

## Performance Testing

### Performance Assertions
1. **Latency Requirements**
   ```rust
   assert!(prediction_time < Duration::from_millis(100));
   ```

2. **Resource Usage**
   ```rust
   assert!(memory_usage < 1_000_000_000); // 1GB
   ```

3. **Throughput**
   ```rust
   assert!(predictions_per_second > 100);
   ```

## Test Execution Order

1. **Unit Tests First**
   - Configuration parsing
   - Feature flag evaluation
   - Individual component behavior

2. **Integration Tests**
   - Component interaction
   - Data flow validation
   - Error propagation

3. **System Tests**
   - End-to-end scenarios
   - Performance validation
   - Reliability testing

## Success Criteria

1. **100% Code Coverage** for:
   - Mock removal code paths
   - Feature flag evaluation
   - Error handling branches

2. **All Tests Pass** with:
   - No flaky tests
   - Consistent results
   - Fast execution (< 1s for unit tests)

3. **No Mock Adapters** in:
   - Production code paths
   - Default configurations
   - Integration test scenarios