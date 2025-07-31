# Neural Model Migration Plan: Custom → Neuro-Divergent

## Executive Summary
This document outlines the complete migration strategy to replace ALL custom neural network implementations in neural-trader with the vendored neuro-divergent library from ruv-FANN.

## Current State Analysis

### Files to be Modified/Deleted:
1. **src/neural/mod.rs** - Complete replacement required
   - Contains placeholder implementations for NHITS, TCN, DeepAR, MLP
   - All model structs and trait implementations must be replaced
   - NeuralPredictor struct needs complete refactoring

2. **src/strategies/neural_enhanced.rs** - Update imports and API calls
3. **src/mcp_server.rs** - Update neural predictor usage
4. **src/mcp/trading_tools.rs** - Update neural prediction methods

## Migration Strategy

### Phase 1: Replace Core Neural Module

#### Step 1.1: Update Cargo.toml dependencies
```toml
# Remove any direct neural network dependencies
# Add path dependency to vendored library
neuro-divergent-models = { path = "vendor/ruv-fann/neuro-divergent/neuro-divergent-models" }
```

#### Step 1.2: Replace src/neural/mod.rs entirely
The new implementation will:
- Import from `neuro_divergent_models` instead of custom implementations
- Use `NeuralForecast` as the main predictor interface
- Map existing model names to neuro-divergent equivalents:
  - NHITS → `neuro_divergent_models::advanced::NHITS`
  - TCN → `neuro_divergent_models::specialized::TCN`
  - DeepAR → `neuro_divergent_models::specialized::DeepAR`
  - MLP → `neuro_divergent_models::basic::MLP`

### Phase 2: API Compatibility Layer

#### Key API Changes:

1. **TimeSeriesData → TimeSeriesInput**
   ```rust
   // Old
   pub struct TimeSeriesData {
       timestamp: DateTime<Utc>,
       close: f64,
       // ...
   }
   
   // New
   use neuro_divergent_models::foundation::TimeSeriesInput;
   // Convert vector of prices to TimeSeriesInput
   ```

2. **PredictionResult → ForecastOutput**
   ```rust
   // Old
   pub struct PredictionResult {
       timestamp: DateTime<Utc>,
       value: f64,
       confidence: f64,
       interval_low: f64,
       interval_high: f64,
       model_name: String,
   }
   
   // New
   use neuro_divergent_models::foundation::ForecastOutput;
   // Map ForecastOutput fields to PredictionResult for compatibility
   ```

3. **Model Creation**
   ```rust
   // Old
   let model: Box<dyn NeuralModel> = Box::new(NHITSModel::new(&config)?);
   
   // New
   use neuro_divergent_models::{models::NHITS, advanced::nhits::NHITSConfig};
   let config = NHITSConfig::default()
       .with_horizon(horizon)
       .with_input_size(input_size);
   let model = NHITS::new(config)?;
   ```

### Phase 3: Update Dependencies

#### Files requiring import updates:
1. **src/strategies/neural_enhanced.rs**
   - Change: `use crate::neural::{NeuralPredictor, PredictionResult};`
   - To: Use new neural module exports

2. **src/mcp_server.rs**
   - Update neural predictor initialization
   - Adapt prediction method calls

3. **src/mcp/trading_tools.rs**
   - Update neural prediction tool implementation

### Phase 4: Implementation Details

#### New src/neural/mod.rs structure:
```rust
//! Neural Network Integration Module using Neuro-Divergent
//! 
//! Provides neural network prediction capabilities via ruv-FANN's neuro-divergent library

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

// Import from vendored neuro-divergent library
use neuro_divergent_models::{
    NeuralForecast,
    foundation::{BaseModel, TimeSeriesInput, ForecastOutput},
    models::{LSTM, RNN, GRU},
    config::{LSTMConfig, RNNConfig, GRUConfig},
};

// Import specific model implementations
use neuro_divergent_models::advanced::nhits::{NHITS, NHITSConfig};
use neuro_divergent_models::specialized::{
    tcn::{TCN, TCNConfig},
    deepar::{DeepAR, DeepARConfig},
};
use neuro_divergent_models::basic::mlp::{MLP, MLPConfig};

// Re-export for backward compatibility
pub use neuro_divergent_models::foundation::ForecastOutput as PredictionResult;
```

### Phase 5: Testing & Validation

1. **Unit Tests**: Update all neural module tests
2. **Integration Tests**: Verify strategy implementations work
3. **Performance Tests**: Compare prediction accuracy and speed
4. **Memory Tests**: Ensure no memory leaks with new library

## Benefits of Migration

1. **Production-Ready Models**: Battle-tested implementations
2. **Better Performance**: Optimized with ruv-FANN backend
3. **More Features**: 
   - Probabilistic forecasting
   - Multi-step ahead predictions
   - Ensemble methods
   - Advanced architectures (27+ models available)
4. **Maintained Codebase**: Active development and bug fixes
5. **Type Safety**: Rust's type system ensures correctness

## Risk Mitigation

1. **Backward Compatibility**: Maintain API wrapper for smooth transition
2. **Gradual Rollout**: Test each model replacement individually
3. **Fallback Option**: Keep old code in separate branch temporarily
4. **Performance Monitoring**: Track metrics before/after migration

## Estimated Timeline

- Phase 1: 2-3 hours (Core module replacement)
- Phase 2: 1-2 hours (API compatibility)
- Phase 3: 1 hour (Update dependencies)
- Phase 4: 2-3 hours (Implementation and testing)
- Phase 5: 2-3 hours (Comprehensive testing)

**Total: 8-12 hours for complete migration**

## Files to Delete After Migration

1. All placeholder model implementations in src/neural/mod.rs:
   - `struct NHITSModel`
   - `struct TCNModel`
   - `struct DeepARModel`
   - `struct MLPModel`
   - `trait NeuralModel`

## Next Steps

1. Begin with Phase 1 - Update Cargo.toml
2. Create new neural module implementation
3. Test with simple MLP model first
4. Gradually migrate other models
5. Update all dependent modules
6. Run comprehensive test suite