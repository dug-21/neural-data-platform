# Phase 3 Implementation Guide - Binary Refactoring

## Overview

This guide provides step-by-step instructions for implementing the Phase 3 architecture refactoring, transforming the current monolithic binary into three focused binaries with clear separation of concerns.

## Prerequisites

1. Current codebase compiles successfully
2. All tests pass
3. Git branch created for Phase 3 work: `v2-phase3-binary-separation`
4. Backup of current state

## Implementation Timeline

```
Week 1: Neural-Core Extraction
Week 2: Neural-ML-Ops Binary
Week 3: Neural-Trading Binary  
Week 4: Integration & Testing
```

## Phase 1: Neural-Core Extraction (Week 1)

### Step 1.1: Create Neural-Core Workspace

```bash
# Create the neural-core directory structure
mkdir -p neural-core/src
mkdir -p neural-core/src/types
mkdir -p neural-core/src/traits  
mkdir -p neural-core/src/utils
mkdir -p neural-core/src/events

# Create Cargo.toml for neural-core
cat > neural-core/Cargo.toml << 'EOF'
[package]
name = "neural-core"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
chrono = { version = "0.4", features = ["serde"] }
redis = { version = "0.26", features = ["tokio-comp"] }
anyhow = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }
tokio = { version = "1.35", features = ["full"] }
thiserror = "1.0"
tracing = "0.1"

[dev-dependencies]
tempfile = "3.14"
EOF
```

### Step 1.2: Extract Core Types

```bash
# Copy and adapt core data types
cp src/data/mod.rs neural-core/src/types/time_series.rs
cp src/types/mod.rs neural-core/src/types/mod.rs 2>/dev/null || touch neural-core/src/types/mod.rs
```

Edit `neural-core/src/types/time_series.rs`:
```rust
//! Core time series data types shared across binaries

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Core time series data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Feature vector for neural models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub symbol: String,
    pub timestamp: DateTime<Utc>,
    pub features: Vec<f64>,
    pub feature_names: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Model metadata for registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub model_id: String,
    pub version: String,
    pub symbol: String,
    pub model_type: String,
    pub config_path: String,
    pub performance_metrics: HashMap<String, f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### Step 1.3: Extract Core Traits

Create `neural-core/src/traits/mod.rs`:
```rust
//! Core traits shared across binaries

use async_trait::async_trait;
use anyhow::Result;
use crate::types::{TimeSeriesData, FeatureVector, ModelMetadata};

/// Neural predictor trait (inference only - training in ML-Ops)
#[async_trait]
pub trait NeuralPredictor: Send + Sync {
    async fn predict(&self, features: &FeatureVector) -> Result<f64>;
    async fn get_confidence(&self) -> f64;
    fn model_id(&self) -> &str;
}

/// Feature extractor trait
#[async_trait]
pub trait FeatureExtractor: Send + Sync {
    async fn extract_features(&self, data: &[TimeSeriesData]) -> Result<FeatureVector>;
    fn get_feature_names(&self) -> Vec<String>;
}

/// Event publisher trait for Redis Streams
#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish_features(&self, features: &FeatureVector) -> Result<()>;
    async fn publish_model_update(&self, metadata: &ModelMetadata) -> Result<()>;
    async fn publish_trading_signal(&self, signal: &serde_json::Value) -> Result<()>;
}

/// Event subscriber trait for Redis Streams
#[async_trait]  
pub trait EventSubscriber: Send + Sync {
    async fn subscribe_features<F>(&self, handler: F) -> Result<()>
    where F: Fn(FeatureVector) -> Result<()> + Send + Sync + 'static;
    
    async fn subscribe_model_updates<F>(&self, handler: F) -> Result<()>
    where F: Fn(ModelMetadata) -> Result<()> + Send + Sync + 'static;
}

/// Model registry trait
#[async_trait]
pub trait ModelRegistry: Send + Sync {
    async fn store_model(&self, metadata: &ModelMetadata, model_data: &[u8]) -> Result<String>;
    async fn load_model(&self, model_id: &str) -> Result<Vec<u8>>;
    async fn get_metadata(&self, model_id: &str) -> Result<ModelMetadata>;
    async fn list_models(&self, symbol: Option<&str>) -> Result<Vec<ModelMetadata>>;
}
```

### Step 1.4: Extract Utilities

```bash
cp -r src/utils/ neural-core/src/utils/
```

Edit `neural-core/src/utils/mod.rs` to remove dependencies on trading-specific code.

### Step 1.5: Create Redis Event Client

Create `neural-core/src/events/redis_client.rs`:
```rust
//! Redis Streams client for inter-binary communication

use crate::traits::{EventPublisher, EventSubscriber};
use crate::types::{FeatureVector, ModelMetadata};
use anyhow::Result;
use async_trait::async_trait;
use redis::{AsyncCommands, Client};
use serde_json;
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info};

pub struct RedisEventClient {
    client: Client,
    connection: redis::aio::Connection,
}

impl RedisEventClient {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = Client::open(redis_url)?;
        let connection = client.get_async_connection().await?;
        
        Ok(Self { client, connection })
    }
}

#[async_trait]
impl EventPublisher for RedisEventClient {
    async fn publish_features(&self, features: &FeatureVector) -> Result<()> {
        let stream_key = format!("features:computed:{}", features.symbol);
        let data = serde_json::to_string(features)?;
        
        let mut conn = self.client.get_async_connection().await?;
        let _: String = conn.xadd(
            stream_key,
            "*",
            &[("data", data.as_str())]
        ).await?;
        
        debug!("Published features for symbol: {}", features.symbol);
        Ok(())
    }
    
    async fn publish_model_update(&self, metadata: &ModelMetadata) -> Result<()> {
        let stream_key = "models:updated";
        let data = serde_json::to_string(metadata)?;
        
        let mut conn = self.client.get_async_connection().await?;
        let _: String = conn.xadd(
            stream_key,
            "*", 
            &[("data", data.as_str())]
        ).await?;
        
        info!("Published model update: {}", metadata.model_id);
        Ok(())
    }
    
    async fn publish_trading_signal(&self, signal: &serde_json::Value) -> Result<()> {
        let stream_key = "trading:signals";
        let data = serde_json::to_string(signal)?;
        
        let mut conn = self.client.get_async_connection().await?;
        let _: String = conn.xadd(
            stream_key,
            "*",
            &[("data", data.as_str())]  
        ).await?;
        
        debug!("Published trading signal");
        Ok(())
    }
}

// Subscription implementation would be similar with XREAD commands
```

### Step 1.6: Create Neural-Core Lib

Create `neural-core/src/lib.rs`:
```rust
//! Neural-Core: Shared library for neural trader platform
//! 
//! Contains common types, traits, and utilities shared across
//! neural-ml-ops and neural-trading binaries.

pub mod types;
pub mod traits;  
pub mod utils;
pub mod events;

// Re-export main types
pub use types::{TimeSeriesData, FeatureVector, ModelMetadata};
pub use traits::{NeuralPredictor, FeatureExtractor, EventPublisher, EventSubscriber, ModelRegistry};
pub use events::RedisEventClient;

// Common result type
pub type Result<T> = anyhow::Result<T>;
```

### Step 1.7: Update Workspace Cargo.toml

```toml
[workspace]
members = [
    "neural-core",
    "config-store", 
    "mcp-trading-server",
]
exclude = [
    "vendor",
    "vendor/ruv-fann", 
    "vendor/ruv-fann/ruv-swarm",
    "vendor/ruv-fann/neuro-divergent",
]
resolver = "2"

[workspace.dependencies]
# Core dependencies
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4", "serde"] }

# Neural dependencies  
ruv-fann = { path = "./vendor/ruv-fann", features = ["default"] }
neuro-divergent = { path = "./vendor/ruv-fann/neuro-divergent", features = ["default", "models", "registry"] }

# Data dependencies
redis = { version = "0.26", features = ["tokio-comp", "connection-manager"] }
sqlx = { version = "0.6", features = ["runtime-tokio-native-tls", "postgres", "macros", "chrono"] }
```

## Phase 2: Neural-ML-Ops Binary (Week 2)

### Step 2.1: Create ML-Ops Structure

```bash
mkdir -p neural-ml-ops/src
mkdir -p neural-ml-ops/src/features
mkdir -p neural-ml-ops/src/training
mkdir -p neural-ml-ops/src/storage
mkdir -p neural-ml-ops/src/monitoring

cat > neural-ml-ops/Cargo.toml << 'EOF'
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
redis = { workspace = true }

# ML and feature engineering
ndarray = { version = "0.15", features = ["rayon"] }
polars = { version = "0.35", features = ["lazy", "temporal", "csv"] }
rayon = "1.8"
statrs = "0.16"

# Common
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
EOF
```

### Step 2.2: Move Feature Engineering

```bash
# Move entire features module
cp -r src/features/ neural-ml-ops/src/features/

# Move feature-related adapters
mkdir -p neural-ml-ops/src/storage
cp src/adapters/timescale.rs neural-ml-ops/src/storage/
```

### Step 2.3: Extract Training Components from Neural Module

Create `neural-ml-ops/src/training/mod.rs`:
```rust
//! Model training components for neural-ml-ops binary

pub mod ruv_fann_trainer;
pub mod training_coordinator; 
pub mod batch_optimizer;
pub mod performance_tracker;
pub mod drift_detector;

use neural_core::{Result, FeatureVector, ModelMetadata};
use async_trait::async_trait;

#[async_trait]
pub trait ModelTrainer: Send + Sync {
    async fn train(&self, features: &[FeatureVector], targets: &[f64]) -> Result<ModelMetadata>;
    async fn evaluate(&self, features: &[FeatureVector], targets: &[f64]) -> Result<f64>;
    fn model_type(&self) -> &str;
}

pub use ruv_fann_trainer::RuvFannTrainer;
```

### Step 2.4: Create ML-Ops Main

Create `neural-ml-ops/src/main.rs`:
```rust
//! Neural-ML-Ops Binary
//! 
//! Handles feature engineering, model training, and drift detection.
//! Domain-agnostic ML operations.

use neural_core::{RedisEventClient, TimeSeriesData, FeatureVector};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{info, error};

mod features;
mod training;
mod storage;
mod monitoring;

use features::FeaturePipeline;
use training::RuvFannTrainer;
use storage::ModelRegistry;

#[tokio::main]
async fn main() -> neural_core::Result<()> {
    // Initialize logging
    tracing_subscriber::init();
    info!("Starting Neural-ML-Ops Binary");
    
    // Initialize Redis event client
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let event_client = Arc::new(RedisEventClient::new(&redis_url).await?);
    
    // Initialize feature pipeline
    let feature_pipeline = Arc::new(FeaturePipeline::new());
    
    // Initialize trainer
    let trainer = Arc::new(RuvFannTrainer::new());
    
    // Start feature processing loop
    let feature_processor = FeatureProcessor::new(
        event_client.clone(),
        feature_pipeline,
        trainer,
    );
    
    feature_processor.run().await?;
    
    Ok(())
}

struct FeatureProcessor {
    event_client: Arc<RedisEventClient>,
    feature_pipeline: Arc<FeaturePipeline>,
    trainer: Arc<RuvFannTrainer>,
}

impl FeatureProcessor {
    fn new(
        event_client: Arc<RedisEventClient>,
        feature_pipeline: Arc<FeaturePipeline>,
        trainer: Arc<RuvFannTrainer>,
    ) -> Self {
        Self {
            event_client,
            feature_pipeline,
            trainer,
        }
    }
    
    async fn run(&self) -> neural_core::Result<()> {
        info!("Starting ML-Ops feature processing loop");
        
        // Subscribe to raw market data and process features
        loop {
            // TODO: Implement market data subscription
            // Process features and publish to trading binary
            
            sleep(Duration::from_secs(1)).await;
        }
    }
}
```

## Phase 3: Neural-Trading Binary (Week 3)

### Step 3.1: Create Trading Structure

```bash
mkdir -p neural-trading/src
mkdir -p neural-trading/src/daa
mkdir -p neural-trading/src/inference
mkdir -p neural-trading/src/execution
mkdir -p neural-trading/src/strategies

cat > neural-trading/Cargo.toml << 'EOF'
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

# Trading specific
axum = { version = "0.7", features = ["http2", "query", "tracing"] }
tower = { version = "0.5" }
uuid = { workspace = true }

# Common
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
EOF
```

### Step 3.2: Move DAA Components

```bash
# Move DAA coordinator and related components
cp -r src/integration/daa_coordinator* neural-trading/src/daa/
cp -r src/daa/ neural-trading/src/daa/
cp -r src/strategies/ neural-trading/src/strategies/
cp -r src/action_layer/ neural-trading/src/execution/
```

### Step 3.3: Extract Inference Components

Create `neural-trading/src/inference/mod.rs`:
```rust
//! Neural inference components for real-time trading

pub mod vendor_predictor;
pub mod model_cache;
pub mod emergency_fallback;

use neural_core::{Result, NeuralPredictor, FeatureVector};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Embedded inference engine with model caching
pub struct InferenceEngine {
    models: Arc<RwLock<HashMap<String, Box<dyn NeuralPredictor>>>>,
    fallback_model: Box<dyn NeuralPredictor>,
}

impl InferenceEngine {
    pub async fn new() -> Result<Self> {
        // Initialize with emergency fallback model
        let fallback_model = Box::new(emergency_fallback::EmergencyPredictor::new());
        
        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            fallback_model,
        })
    }
    
    pub async fn predict(&self, symbol: &str, features: &FeatureVector) -> Result<f64> {
        let models = self.models.read().await;
        
        if let Some(model) = models.get(symbol) {
            model.predict(features).await
        } else {
            // Use fallback model
            self.fallback_model.predict(features).await
        }
    }
    
    pub async fn load_model(&self, symbol: String, model: Box<dyn NeuralPredictor>) {
        let mut models = self.models.write().await;
        models.insert(symbol, model);
    }
}
```

### Step 3.4: Create Trading Main

Create `neural-trading/src/main.rs`:
```rust
//! Neural-Trading Binary
//! 
//! Handles trading execution, DAA coordination, and strategy management.

use neural_core::{RedisEventClient, EventSubscriber, FeatureVector};
use std::sync::Arc;
use tracing::{info, error};

mod daa;
mod inference;
mod execution;
mod strategies;

use daa::DAACoordinator;
use inference::InferenceEngine;
use execution::ExecutionEngine;

#[tokio::main]
async fn main() -> neural_core::Result<()> {
    // Initialize logging
    tracing_subscriber::init();
    info!("Starting Neural-Trading Binary");
    
    // Initialize components
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    let event_client = Arc::new(RedisEventClient::new(&redis_url).await?);
    
    let inference_engine = Arc::new(InferenceEngine::new().await?);
    let execution_engine = Arc::new(ExecutionEngine::new().await?);
    let daa_coordinator = Arc::new(DAACoordinator::new().await?);
    
    // Start trading loop
    let trading_loop = TradingLoop::new(
        event_client,
        inference_engine,
        execution_engine,
        daa_coordinator,
    );
    
    trading_loop.run().await?;
    
    Ok(())
}

struct TradingLoop {
    event_client: Arc<RedisEventClient>,
    inference_engine: Arc<InferenceEngine>,
    execution_engine: Arc<ExecutionEngine>,
    daa_coordinator: Arc<DAACoordinator>,
}

impl TradingLoop {
    fn new(
        event_client: Arc<RedisEventClient>,
        inference_engine: Arc<InferenceEngine>,
        execution_engine: Arc<ExecutionEngine>,
        daa_coordinator: Arc<DAACoordinator>,
    ) -> Self {
        Self {
            event_client,
            inference_engine,
            execution_engine,
            daa_coordinator,
        }
    }
    
    async fn run(&self) -> neural_core::Result<()> {
        info!("Starting trading loop with DAA coordination");
        
        // Subscribe to feature updates from ML-Ops
        let inference_engine = self.inference_engine.clone();
        let daa_coordinator = self.daa_coordinator.clone();
        let execution_engine = self.execution_engine.clone();
        
        self.event_client.subscribe_features(move |features: FeatureVector| {
            let inference_engine = inference_engine.clone();
            let daa_coordinator = daa_coordinator.clone();
            let execution_engine = execution_engine.clone();
            
            tokio::spawn(async move {
                // 1. Get prediction from inference engine
                let prediction = inference_engine.predict(&features.symbol, &features).await?;
                
                // 2. DAA coordination for trading decision
                let decision = daa_coordinator.make_decision(&features, prediction).await?;
                
                // 3. Execute decision if warranted
                if decision.should_execute() {
                    execution_engine.execute_decision(&decision).await?;
                }
                
                Ok::<_, neural_core::Error>(())
            });
            
            Ok(())
        }).await?;
        
        info!("Trading loop started, waiting for events...");
        
        // Keep running
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}
```

## Phase 4: Integration & Testing (Week 4)

### Step 4.1: Update Original Cargo.toml

```toml
[workspace]
members = [
    "neural-core",
    "neural-ml-ops", 
    "neural-trading",
    "config-store",
    "mcp-trading-server",
]

# Remove the old package section since we now have separate binaries
# [package] - REMOVE

# Keep existing dependencies for workspace
# ...existing workspace.dependencies...
```

### Step 4.2: Create Integration Tests

Create `tests/integration_test.rs`:
```rust
//! Integration tests for the three-binary architecture

use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_full_pipeline() {
    // Start Redis (assuming it's running)
    
    // Start ML-Ops binary in background
    let ml_ops = Command::new("cargo")
        .args(&["run", "--bin", "neural-ml-ops"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start neural-ml-ops");
    
    // Start Trading binary in background  
    let trading = Command::new("cargo")
        .args(&["run", "--bin", "neural-trading"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to start neural-trading");
    
    // Wait for initialization
    sleep(Duration::from_secs(10)).await;
    
    // Send test market data to ML-Ops
    // TODO: Implement test data injection
    
    // Verify feature processing
    // TODO: Check Redis streams for features
    
    // Verify trading decisions
    // TODO: Check Redis streams for trading signals
    
    // Cleanup
    // Kill processes
}

#[tokio::test]  
async fn test_redis_communication() {
    use neural_core::{RedisEventClient, FeatureVector, EventPublisher};
    
    let client = RedisEventClient::new("redis://localhost:6379").await.unwrap();
    
    let test_features = FeatureVector {
        symbol: "TEST".to_string(),
        timestamp: chrono::Utc::now(),
        features: vec![1.0, 2.0, 3.0],
        feature_names: vec!["f1".to_string(), "f2".to_string(), "f3".to_string()],
        metadata: std::collections::HashMap::new(),
    };
    
    // Test publishing
    client.publish_features(&test_features).await.unwrap();
    
    // TODO: Test subscription
}
```

### Step 4.3: Performance Benchmarks

Create `benches/binary_performance.rs`:
```rust
//! Performance benchmarks for binary communication

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use neural_core::{RedisEventClient, FeatureVector, EventPublisher};
use tokio::runtime::Runtime;

fn benchmark_feature_publishing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let client = rt.block_on(async {
        RedisEventClient::new("redis://localhost:6379").await.unwrap()
    });
    
    let features = FeatureVector {
        symbol: "BENCH".to_string(),
        timestamp: chrono::Utc::now(),
        features: vec![1.0; 100],
        feature_names: (0..100).map(|i| format!("f{}", i)).collect(),
        metadata: std::collections::HashMap::new(),
    };
    
    c.bench_function("feature_publishing", |b| {
        b.to_async(&rt).iter(|| async {
            black_box(client.publish_features(&features).await.unwrap());
        });
    });
}

criterion_group!(benches, benchmark_feature_publishing);
criterion_main!(benches);
```

### Step 4.4: Update Documentation

Update `README.md`:
```markdown
# Neural Trader V2 - Three Binary Architecture

## Architecture

The system consists of three specialized binaries:

- **neural-core**: Shared library with common types and utilities
- **neural-ml-ops**: ML training, feature engineering, and drift detection  
- **neural-trading**: Trading execution with DAA coordination

## Running the System

1. Start Redis:
   ```bash
   docker run -p 6379:6379 redis:alpine
   ```

2. Start ML-Ops binary:
   ```bash
   cargo run --bin neural-ml-ops
   ```

3. Start Trading binary:
   ```bash  
   cargo run --bin neural-trading
   ```

## Communication

Binaries communicate via Redis Streams:
- `features:computed:*` - Feature data from ML-Ops to Trading
- `models:updated` - Model updates from ML-Ops to Trading
- `trading:signals` - Trading decisions from Trading to ML-Ops

## Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_test

# Benchmarks
cargo bench
```
```

### Step 4.5: Validate Migration

Create validation checklist:

```bash
#!/bin/bash
# validate_migration.sh

echo "=== Neural Trader Migration Validation ==="

echo "1. Checking workspace compilation..."
cargo check --workspace || exit 1

echo "2. Running tests..."
cargo test --workspace || exit 1

echo "3. Testing neural-core..."
cd neural-core && cargo test && cd ..

echo "4. Testing neural-ml-ops..."
cd neural-ml-ops && cargo check && cd ..

echo "5. Testing neural-trading..." 
cd neural-trading && cargo check && cd ..

echo "6. Checking Redis connectivity..."
redis-cli ping || echo "Warning: Redis not available"

echo "7. Running benchmarks..."
cargo bench

echo "=== Migration validation complete ==="
```

## Success Criteria

### Functional Requirements
- [ ] All three binaries compile successfully
- [ ] Neural-core library works as shared dependency
- [ ] Redis Streams communication established
- [ ] ML-Ops can process features and train models
- [ ] Trading binary can receive features and make decisions
- [ ] Integration tests pass

### Performance Requirements  
- [ ] ML-Ops startup time < 30 seconds
- [ ] Trading binary startup time < 10 seconds
- [ ] Feature processing latency < 100ms
- [ ] Inference latency < 5ms
- [ ] Redis communication latency < 10ms

### Architecture Requirements
- [ ] Clean separation of concerns
- [ ] No circular dependencies
- [ ] ML-Ops is domain-agnostic
- [ ] Trading has embedded DAA coordinator
- [ ] Proper error handling and logging

## Common Issues and Solutions

### Issue 1: Compilation Errors
```
Solution: Ensure all imports updated to use neural-core types
Example: Change `use crate::data::TimeSeriesData` to `use neural_core::TimeSeriesData`
```

### Issue 2: Redis Connection Issues
```
Solution: Verify Redis is running and accessible
Check Redis URL environment variable
Test with: redis-cli ping
```

### Issue 3: Missing Dependencies
```
Solution: Add missing dependencies to appropriate Cargo.toml
ML-Ops: Add ML/feature engineering dependencies
Trading: Add trading/execution dependencies
Core: Keep minimal, shared dependencies only
```

### Issue 4: Performance Degradation
```
Solution: Profile the Redis communication overhead
Optimize serialization (consider bincode vs JSON)
Use local Redis instance
Implement connection pooling
```

## Next Steps After Migration

1. **Optimize Performance**: Profile and optimize hotpaths
2. **Add Monitoring**: Comprehensive metrics and alerting  
3. **Scale Testing**: Load testing with realistic data volumes
4. **Production Deployment**: Kubernetes manifests and CI/CD
5. **Documentation**: API documentation and operational guides

This completes the Phase 3 implementation guide. The migration transforms the monolithic architecture into a clean, scalable, three-binary system with proper separation of concerns.