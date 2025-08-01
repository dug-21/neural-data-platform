# NeuralFix Directory Structure Design

## Overview

This document defines the complete directory structure for the NeuralFix model adapter implementation, ensuring clean separation of concerns and maintainable code organization.

## Proposed Directory Structure

```
src/neuralfix/                          # Root NeuralFix module
├── mod.rs                              # Module exports and public API
├── controller.rs                       # NeuralFixController - main orchestration
├── model_factory.rs                    # ModelFactory - creates and manages adapters
├── ensemble_coordinator.rs             # EnsembleCoordinator - intelligent routing
├── types.rs                           # Core data types (ModelType, ModelConfig, etc.)
├── config.rs                          # Configuration management
├── errors.rs                          # NeuralFix-specific error types
├── performance_tracker.rs             # Performance tracking utilities
│
├── adapters/                          # Model adapter implementations
│   ├── mod.rs                         # Adapter module exports
│   ├── model_adapter.rs               # ModelAdapter trait definition
│   ├── base/                          # Base adapter implementations
│   │   ├── mod.rs
│   │   ├── fann_adapter.rs            # Base FANN adapter
│   │   └── vendor_adapter.rs          # Base vendor adapter
│   ├── fann/                          # FANN-specific adapters
│   │   ├── mod.rs
│   │   ├── mlp_adapter.rs             # MLP adapter
│   │   └── lstm_adapter.rs            # LSTM adapter
│   └── vendor/                        # Vendor model adapters
│       ├── mod.rs
│       ├── nhits_adapter.rs           # NHITS adapter
│       ├── tcn_adapter.rs             # TCN adapter
│       └── deepar_adapter.rs          # DeepAR adapter
│
├── integration/                       # Integration utilities
│   ├── mod.rs
│   ├── data_conversion.rs             # Data format conversion utilities
│   ├── config_migration.rs           # Config migration from existing system
│   └── backward_compatibility.rs     # Backward compatibility layer
│
├── monitoring/                        # Monitoring and health
│   ├── mod.rs
│   ├── health_monitor.rs              # Health monitoring system
│   ├── circuit_breaker.rs             # Circuit breaker implementation
│   └── metrics_collector.rs          # Metrics collection
│
├── storage/                           # Model storage and persistence
│   ├── mod.rs
│   ├── model_storage.rs               # Model file storage
│   ├── checkpoint_manager.rs          # Model checkpoint management
│   └── cache_manager.rs               # Model cache management
│
└── tests/                             # Tests
    ├── mod.rs
    ├── unit/                          # Unit tests
    │   ├── mod.rs
    │   ├── test_adapters.rs           # Adapter unit tests
    │   ├── test_factory.rs            # Factory unit tests
    │   └── test_coordinator.rs        # Coordinator unit tests
    ├── integration/                   # Integration tests
    │   ├── mod.rs
    │   ├── test_end_to_end.rs         # End-to-end tests
    │   ├── test_performance.rs        # Performance tests
    │   └── test_reliability.rs        # Reliability tests
    └── fixtures/                      # Test fixtures
        ├── mod.rs
        ├── sample_data.rs             # Sample time series data
        └── mock_models.rs             # Mock model implementations
```

## File Content Templates

### 1. Root Module (src/neuralfix/mod.rs)

```rust
//! NeuralFix: Advanced Model Configuration System
//!
//! This module provides a unified interface for all neural model types,
//! supporting both FANN-based models and vendor models with intelligent
//! ensemble coordination and performance tracking.

pub mod controller;
pub mod model_factory;
pub mod ensemble_coordinator;
pub mod types;
pub mod config;
pub mod errors;
pub mod performance_tracker;

pub mod adapters;
pub mod integration;
pub mod monitoring;
pub mod storage;

// Re-export main public API
pub use controller::NeuralFixController;
pub use model_factory::ModelFactory;
pub use ensemble_coordinator::EnsembleCoordinator;
pub use types::{ModelType, ModelConfig, ModelInfo, HealthStatus};
pub use adapters::ModelAdapter;
pub use errors::NeuralFixError;

// Configuration exports
pub use config::{NeuralFixConfig, EnsembleConfig, RoutingConfig};

// Integration helpers
pub use integration::{migrate_from_neural_config, BackwardCompatibility};
```

### 2. Adapter Module (src/neuralfix/adapters/mod.rs)

```rust
//! Model Adapter Implementations
//!
//! This module contains all model adapter implementations following the
//! adapter pattern to provide a uniform interface across different model types.

pub mod model_adapter;
pub mod base;
pub mod fann;
pub mod vendor;

// Re-export the main trait
pub use model_adapter::ModelAdapter;

// Re-export adapter implementations
pub use fann::{MLPAdapter, LSTMAdapter};
pub use vendor::{NHITSAdapter, TCNAdapter, DeepARAdapter};

// Re-export base adapters for extension
pub use base::{FannModelAdapter, VendorModelAdapter};
```

### 3. FANN Adapters Module (src/neuralfix/adapters/fann/mod.rs)

```rust
//! FANN Model Adapters
//!
//! Adapters for FANN-based neural network models, providing integration
//! with the existing FannPredictor system.

mod mlp_adapter;
mod lstm_adapter;

pub use mlp_adapter::MLPAdapter;
pub use lstm_adapter::LSTMAdapter;

// Common utilities for FANN adapters
pub(crate) mod utils;
```

### 4. Vendor Adapters Module (src/neuralfix/adapters/vendor/mod.rs)

```rust
//! Vendor Model Adapters
//!
//! Adapters for external vendor models (NHITS, TCN, DeepAR) with simulation
//! fallbacks until real implementations are integrated.

mod nhits_adapter;
mod tcn_adapter;
mod deepar_adapter;

pub use nhits_adapter::NHITSAdapter;
pub use tcn_adapter::TCNAdapter;
pub use deepar_adapter::DeepARAdapter;

// Common utilities for vendor adapters
pub(crate) mod vendor_utils;
pub(crate) mod simulation_fallback;
```

## Integration Points with Existing Code

### 1. Enhanced Neural Adapter Integration

```rust
// Location: src/adapters/enhanced_neural_adapter.rs
impl EnhancedNeuralAdapter {
    pub fn with_neuralfix(&mut self, controller: Arc<NeuralFixController>) -> Result<()> {
        self.neuralfix_controller = Some(controller);
        info!("NeuralFix integration enabled for EnhancedNeuralAdapter");
        Ok(())
    }
}
```

### 2. Neural Module Integration

```rust
// Location: src/neural/mod.rs (addition)
#[cfg(feature = "neuralfix")]
pub mod neuralfix {
    pub use crate::neuralfix::*;
}
```

### 3. Configuration Integration

```rust
// Location: src/config/enhanced_neural_config.rs (addition)
impl EnhancedNeuralConfig {
    pub fn to_neuralfix_config(&self) -> crate::neuralfix::NeuralFixConfig {
        crate::neuralfix::integration::migrate_from_neural_config(self)
    }
}
```

## Implementation Strategy

### Phase 1: Core Infrastructure
**Files to Create:**
- `src/neuralfix/mod.rs` - Module foundation
- `src/neuralfix/types.rs` - Core data types
- `src/neuralfix/errors.rs` - Error definitions
- `src/neuralfix/adapters/model_adapter.rs` - Adapter trait
- `src/neuralfix/model_factory.rs` - Basic factory structure

### Phase 2: FANN Adapters
**Files to Create:**
- `src/neuralfix/adapters/base/fann_adapter.rs` - Base FANN adapter
- `src/neuralfix/adapters/fann/mlp_adapter.rs` - MLP adapter
- `src/neuralfix/adapters/fann/lstm_adapter.rs` - LSTM adapter
- `src/neuralfix/integration/data_conversion.rs` - Data conversion utilities

### Phase 3: Vendor Adapters (Simulation)
**Files to Create:**
- `src/neuralfix/adapters/base/vendor_adapter.rs` - Base vendor adapter
- `src/neuralfix/adapters/vendor/nhits_adapter.rs` - NHITS adapter with simulation
- `src/neuralfix/adapters/vendor/tcn_adapter.rs` - TCN adapter with simulation
- `src/neuralfix/adapters/vendor/deepar_adapter.rs` - DeepAR adapter with simulation

### Phase 4: Ensemble and Integration
**Files to Create:**
- `src/neuralfix/ensemble_coordinator.rs` - Ensemble logic
- `src/neuralfix/controller.rs` - Main controller
- `src/neuralfix/integration/config_migration.rs` - Config migration
- `src/neuralfix/integration/backward_compatibility.rs` - Compatibility layer

### Phase 5: Monitoring and Testing
**Files to Create:**
- `src/neuralfix/monitoring/health_monitor.rs` - Health monitoring
- `src/neuralfix/monitoring/circuit_breaker.rs` - Circuit breaker
- `src/neuralfix/tests/integration/test_end_to_end.rs` - E2E tests
- All unit test files

## File Size Guidelines

### Small Files (< 200 lines)
- `mod.rs` files - module organization only
- `types.rs` - type definitions only
- `errors.rs` - error definitions only
- Individual adapter files - focused implementations

### Medium Files (200-500 lines)
- `model_factory.rs` - factory logic with multiple methods
- `ensemble_coordinator.rs` - ensemble logic
- `controller.rs` - main orchestration logic
- Base adapter implementations

### Large Files (> 500 lines)
- Integration test files - comprehensive test coverage
- `data_conversion.rs` - extensive conversion utilities
- Performance test files - multiple benchmark scenarios

## Dependencies and Imports

### Internal Dependencies
```rust
// NeuralFix modules import from each other
use crate::neuralfix::{ModelType, ModelConfig, NeuralFixError};
use crate::data::TimeSeriesData;
use crate::neural::{PredictionResult, NeuralPredictorTrait};
use crate::neural::fann::predictor::FannPredictor;
```

### External Dependencies
```rust
// Standard async/threading
use async_trait::async_trait;
use tokio::sync::{RwLock, Mutex};
use std::sync::Arc;

// Serialization
use serde::{Serialize, Deserialize};

// Error handling
use anyhow::{Result, Context};
use thiserror::Error;

// Logging
use tracing::{debug, info, warn, error};

// Time handling
use chrono::{DateTime, Utc};
```

## Module Visibility Rules

### Public API (`pub`)
- Main controller and factory
- Adapter trait and implementations  
- Core types and configurations
- Error types
- Integration utilities

### Crate-internal (`pub(crate)`)
- Base adapter implementations
- Monitoring utilities
- Performance tracking internals
- Test utilities

### Module-internal (`pub(super)` / private)
- Implementation details
- Helper functions
- Internal state management
- Private data structures

## Documentation Standards

### File-level Documentation
```rust
//! Module Purpose
//!
//! Detailed description of what this module does, its role in the system,
//! and key concepts. Include examples for complex modules.
//!
//! # Examples
//!
//! ```rust
//! // Basic usage example
//! ```
```

### Type Documentation
```rust
/// Brief description of the type
///
/// More detailed explanation if needed, including:
/// - When to use this type
/// - Important constraints or requirements
/// - Related types or concepts
///
/// # Examples
///
/// ```rust
/// // Usage example
/// ```
#[derive(Debug, Clone)]
pub struct MyType {
    /// Field documentation
    pub field: String,
}
```

## Testing Strategy per Directory

### Unit Tests (`tests/unit/`)
- Test individual components in isolation
- Mock external dependencies
- Focus on edge cases and error conditions
- High code coverage (>90%)

### Integration Tests (`tests/integration/`)
- Test component interactions
- Real database/external service connections
- End-to-end workflow validation
- Performance benchmarks

### Test Fixtures (`tests/fixtures/`)
- Reusable test data
- Mock implementations
- Test configuration templates
- Sample model files

## Conclusion

This directory structure provides:

1. **Clear Separation**: Each concern has its own module
2. **Scalability**: Easy to add new model types or features
3. **Maintainability**: Related code is co-located
4. **Testability**: Comprehensive test organization
5. **Integration**: Clear integration points with existing code

The structure supports the phased implementation approach while maintaining clean architecture principles and ensuring easy navigation for developers.