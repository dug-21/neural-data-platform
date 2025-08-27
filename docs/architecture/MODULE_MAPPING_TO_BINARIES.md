# Neural Trader Phase 3 Architecture - Module to Binary Mapping

## Executive Summary

This document provides a comprehensive mapping of the current `src/` modules to their target binaries in the Phase 3 architecture. Based on analysis of all current modules and the Phase 3 target architecture, this mapping guides the refactoring from a monolithic binary to three distinct binaries with clear separation of concerns.

## Target Architecture Overview

### Three Binary System

```
┌──────────────────────────────────────────────────────────────┐
│                     PHASE 3 ARCHITECTURE                     │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────────────┐  ┌─────────────────────────────┐   │
│  │   neural-ml-ops     │  │     neural-trading          │   │
│  │  (ML Training)      │  │   (Trading Execution)       │   │
│  │                     │  │                             │   │
│  │ • Feature Engine    │  │ • DAA Coordinator           │   │
│  │ • Model Training    │  │ • Strategy Execution        │   │
│  │ • ruv-FANN Train    │  │ • ruv-FANN Inference        │   │
│  │ • Drift Detection   │  │ • Trade Execution           │   │
│  │ • Model Registry    │  │ • Risk Management           │   │
│  └─────────────────────┘  └─────────────────────────────┘   │
│            ↓                           ↓                     │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              neural-core (Shared Library)            │   │
│  │  • Common Types     • Traits      • Utilities       │   │
│  │  • Event Streaming  • Redis Client                  │   │
│  └─────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
```

## Current Module Analysis and Mapping

### 1. NEURAL-CORE (Shared Library) Modules

**Purpose**: Common types, traits, utilities shared across all binaries

| Current Module | Module Purpose | Target Location | Status |
|---|---|---|---|
| `src/types/mod.rs` | Common data types | `neural-core/src/types.rs` | ✅ Move |
| `src/lib.rs` (exports) | Public API surface | `neural-core/src/lib.rs` | ✅ Refactor |
| `src/utils/` | Utility functions | `neural-core/src/utils/` | ✅ Move |
| `src/data/mod.rs` | Time series types | `neural-core/src/data_types.rs` | ✅ Extract types |
| `src/streaming/` | Event bus traits | `neural-core/src/events.rs` | ✅ Extract traits |
| `src/config/` (core) | Base config traits | `neural-core/src/config_traits.rs` | ✅ Extract traits |

**Key Components to Extract**:
- `TimeSeriesData` struct and related types
- `NeuralPredictorTrait` and core traits
- Redis Streams client abstractions
- Common error types and result wrappers
- Utility functions (market hours, symbol loading)

### 2. NEURAL-ML-OPS (Training Binary) Modules

**Purpose**: Feature engineering, model training, drift detection - domain agnostic

| Current Module | Module Purpose | Target Location | Status |
|---|---|---|---|
| `src/features/` | Feature engineering | `neural-ml-ops/src/features/` | ✅ Move entire |
| `src/neural/` (training) | Model training | `neural-ml-ops/src/training/` | ⚠️ Extract training |
| `src/neural/vendor_predictor.rs` (train) | ruv-FANN training | `neural-ml-ops/src/ruv_fann/` | ⚠️ Extract training |
| `src/data/` (pipelines) | Data processing | `neural-ml-ops/src/data_pipeline/` | ✅ Move pipelines |
| `src/adapters/timescale.rs` | Historical data | `neural-ml-ops/src/storage/` | ✅ Move |
| `src/monitoring/model_performance_tracker.rs` | Model metrics | `neural-ml-ops/src/monitoring/` | ✅ Move |
| `src/config/neural.rs` | Training config | `neural-ml-ops/src/config.rs` | ✅ Move |

**Key Components**:
- Complete feature engineering pipeline (Rust-native)
- ruv-FANN model training capabilities
- Model registry and storage integration
- Drift detection and performance monitoring
- Data pipeline consolidation and routing

### 3. NEURAL-TRADING (Domain Binary) Modules

**Purpose**: Trading execution, DAA coordination, strategy management

| Current Module | Module Purpose | Target Location | Status |
|---|---|---|---|
| `src/integration/daa_coordinator.rs` | DAA orchestration | `neural-trading/src/daa.rs` | ✅ Move |
| `src/neural/vendor_predictor.rs` (inference) | ruv-FANN inference | `neural-trading/src/inference.rs` | ⚠️ Extract inference |
| `src/strategies/` | Trading strategies | `neural-trading/src/strategies/` | ✅ Move |
| `src/action_layer/` | Trade execution | `neural-trading/src/execution/` | ✅ Move |
| `src/adapters/redis.rs` | Redis integration | `neural-trading/src/market_data.rs` | ✅ Move |
| `src/daa/` | DAA components | `neural-trading/src/daa/` | ✅ Move |
| `src/orchestration/` | Platform orchestration | `neural-trading/src/orchestration.rs` | ✅ Move |
| `src/main.rs` | Application entry | `neural-trading/src/main.rs` | ⚠️ Refactor for domain |

**Key Components**:
- DAA Coordinator for autonomous decision making
- Embedded ruv-FANN models for fast inference
- Strategy execution and coordination
- Order management and execution
- Real-time market data processing

### 4. MODULES TO BE DEPRECATED/REMOVED

| Current Module | Reason for Deprecation | Replacement |
|---|---|---|
| `src/bin/` (various) | Old test binaries | Remove or convert to tests |
| `src/mcp_server.rs` | MCP in wrong place | Move to separate service |
| `src/neural/predictor_modular_backup/` | Legacy backup code | Remove |
| `src/adapters/data_converter.rs` | Deprecated converter | Remove (noted in mod.rs) |
| Legacy config files | Monolithic config | Use modular config system |

### 5. NEW MODULES TO BE CREATED

| New Module | Purpose | Target Binary | Priority |
|---|---|---|---|
| `neural-core/src/events.rs` | Redis Streams abstraction | Shared | High |
| `neural-core/src/ruv_fann.rs` | ruv-FANN base integration | Shared | High |
| `neural-ml-ops/src/drift_detector.rs` | Model drift detection | ML Ops | Medium |
| `neural-ml-ops/src/model_registry.rs` | Model storage/retrieval | ML Ops | High |
| `neural-trading/src/market.rs` | Market data processing | Trading | High |
| `neural-trading/src/risk.rs` | Risk management | Trading | High |

## Detailed Module Breakdown

### A. Neural Module Split Strategy

The current `src/neural/` module needs careful splitting:

```
src/neural/mod.rs → Split into:
├── neural-core/src/neural_traits.rs     # NeuralPredictorTrait
├── neural-ml-ops/src/training/          # Training components
│   ├── vendor_trainer.rs                # ruv-FANN training
│   ├── batch_optimizer.rs               # Training optimization
│   ├── performance_optimizer.rs         # Training performance
│   └── training_coordinator.rs          # Training coordination
└── neural-trading/src/inference/        # Inference components
    ├── vendor_predictor.rs              # ruv-FANN inference
    ├── model_factory.rs                 # Model loading
    ├── emergency_model.rs               # Fallback models
    └── memory_optimized_predictor.rs    # Memory optimization
```

### B. Integration Module Refactoring

The `src/integration/` module maps as follows:

```
src/integration/mod.rs → Split into:
├── neural-ml-ops/src/data_access.rs     # Training data access
├── neural-ml-ops/src/training_data_service.rs  # Training service
├── neural-trading/src/daa_coordinator.rs       # DAA coordination
└── neural-trading/src/autonomous_decisions.rs  # Decision making
```

### C. Adapters Module Distribution

```
src/adapters/mod.rs → Split into:
├── neural-core/src/adapters/            # Core adapter traits
├── neural-ml-ops/src/storage/           # Storage adapters
│   ├── timescale.rs                     # Historical data
│   └── model_storage.rs                 # Model storage
└── neural-trading/src/adapters/         # Trading adapters
    ├── redis.rs                         # Real-time data
    ├── vendor_bridge.rs                 # Vendor integration
    └── health_monitor.rs                # Health monitoring
```

## Binary Communication Architecture

### Event Streams (Redis)

```yaml
ML_Ops_Outputs:
  - stream: "features:computed"
    format: "FeatureVector + metadata"
    consumers: ["neural-trading"]
    
  - stream: "models:updated" 
    format: "Model metadata + config-store path"
    consumers: ["neural-trading"]
    
  - stream: "drift:detected"
    format: "Drift metrics + recommendations"
    consumers: ["neural-trading", "monitoring"]

Trading_Outputs:
  - stream: "trades:executed"
    format: "Trade execution details"
    consumers: ["neural-ml-ops", "analytics"]
    
  - stream: "performance:metrics"
    format: "Trading performance data"
    consumers: ["neural-ml-ops"]
    
  - stream: "feedback:learning"
    format: "Model feedback for retraining"
    consumers: ["neural-ml-ops"]
```

## Workspace Structure

### Cargo.toml (Workspace Root)

```toml
[workspace]
members = [
    "neural-core",
    "neural-ml-ops", 
    "neural-trading",
    "config-store"
]
resolver = "2"

[workspace.dependencies]
# Shared dependencies across workspace
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
tracing = "0.1"

# ruv-FANN dependencies
ruv-fann = { path = "./vendor/ruv-fann" }
neuro-divergent = { path = "./vendor/ruv-fann/neuro-divergent" }

# Redis and data
redis = { version = "0.26", features = ["tokio-comp"] }
sqlx = { version = "0.6", features = ["runtime-tokio-native-tls", "postgres"] }
```

### Neural-Core Cargo.toml

```toml
[package]
name = "neural-core"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core dependencies only
serde = { workspace = true }
chrono = { version = "0.4", features = ["serde"] }
redis = { workspace = true }
anyhow = { workspace = true }
uuid = { version = "1.0", features = ["v4", "serde"] }

# No domain-specific dependencies
# No training or trading logic
```

### Neural-ML-Ops Cargo.toml

```toml
[package]
name = "neural-ml-ops"
version = "0.1.0" 
edition = "2021"

[[bin]]
name = "neural-ml-ops"
path = "src/main.rs"

[dependencies]
neural-core = { path = "../neural-core" }
ruv-fann = { workspace = true }
neuro-divergent = { workspace = true }
sqlx = { workspace = true }

# Feature engineering dependencies
ndarray = "0.15"
polars = { version = "0.35", features = ["lazy"] }
rayon = "1.8"

# No trading or strategy dependencies
```

### Neural-Trading Cargo.toml

```toml
[package]
name = "neural-trading"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "neural-trading" 
path = "src/main.rs"

[dependencies]
neural-core = { path = "../neural-core" }
ruv-fann = { workspace = true }
redis = { workspace = true }

# Trading specific dependencies
uuid = { workspace = true }
axum = "0.7"
tower = "0.5"

# No training or ML ops dependencies
```

## Migration Strategy

### Phase 1: Extract Neural-Core (Week 1)
1. Create `neural-core` crate
2. Extract common types and traits
3. Update imports in existing codebase
4. Ensure compilation

### Phase 2: Create ML-Ops Binary (Week 2)
1. Create `neural-ml-ops` crate
2. Move feature engineering modules
3. Extract training components from neural module
4. Setup Redis event publishing
5. Test training pipeline

### Phase 3: Create Trading Binary (Week 3) 
1. Create `neural-trading` crate
2. Move DAA coordinator and strategies
3. Extract inference components from neural module
4. Setup Redis event consumption
5. Test trading execution

### Phase 4: Integration & Testing (Week 4)
1. End-to-end Redis Streams communication
2. Model training → deployment → inference flow
3. Performance optimization
4. Documentation and cleanup

## Dependencies Analysis

### Current Dependencies by Module

| Module Category | External Dependencies | Target Binary |
|---|---|---|---|
| Neural Training | ruv-fann, neuro-divergent, ndarray | neural-ml-ops |
| Neural Inference | ruv-fann (inference only) | neural-trading |
| Feature Engineering | polars, ndarray, statrs | neural-ml-ops |
| Trading Execution | axum, tower, uuid | neural-trading |
| Data Storage | sqlx, redis | Both (different usage) |
| Common Utilities | chrono, serde, tokio | neural-core |

### Dependency Separation Rules

1. **ML-Ops**: Can have training-heavy dependencies (ndarray, polars)
2. **Trading**: Should be lightweight, fast startup, real-time focused
3. **Core**: Minimal dependencies, only essential traits and types

## Success Criteria

### Technical Validation
- [ ] Each binary compiles independently
- [ ] Clean dependency separation (no circular dependencies)
- [ ] Redis Streams communication working
- [ ] End-to-end model training → inference pipeline
- [ ] Performance benchmarks meet targets

### Architectural Validation  
- [ ] ML-Ops binary is domain-agnostic
- [ ] Trading binary has embedded DAA coordinator
- [ ] Core library has no business logic
- [ ] Clear separation of concerns maintained

### Performance Targets
- [ ] ML-Ops: Training throughput maintained or improved
- [ ] Trading: <5ms inference latency
- [ ] Core: Minimal runtime overhead
- [ ] Communication: <10ms Redis Streams latency

## Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Complex neural module split | High | Careful analysis of training vs inference code |
| Redis communication overhead | Medium | Local Redis instance, optimized serialization |
| Dependency version conflicts | Medium | Workspace-level dependency management |
| Integration testing complexity | Medium | Comprehensive end-to-end test suite |
| Performance regression | High | Continuous benchmarking during migration |

## Conclusion

This mapping provides a clear path from the current monolithic structure to the Phase 3 three-binary architecture. The key insight is that the current codebase already has good modular structure - we primarily need to:

1. **Extract common code** into neural-core
2. **Split neural module** by training vs inference concerns  
3. **Separate domain logic** from ML operations
4. **Establish Redis communication** between binaries

The resulting architecture will be more maintainable, scalable, and aligned with the quality-first principles of the V2 system design.