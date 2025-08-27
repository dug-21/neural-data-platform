# Neural-Trader V2: Technical Migration Guide
## Step-by-Step Implementation Instructions

*Document Version*: 1.0  
*Created*: 2025-01-25  
*Purpose*: Detailed technical guide for executing the revised implementation plan

## Prerequisites

Before starting migration:
1. Ensure all tests pass: `cargo test --all`
2. Create full backup: `git checkout -b v2-migration-backup`
3. Document current metrics for comparison
4. Set up parallel monitoring for old/new systems

## Week 1: Foundation Completion

### Day 1-2: EventBus Abstraction Implementation

#### Step 1: Create EventBus Trait
```bash
mkdir -p eventbus/src
cd eventbus
cargo init --lib
```

#### Step 2: Implement Core Trait
```rust
// eventbus/src/lib.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: String,
    pub stream: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub timestamp: i64,
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, stream: &str, event: Event) -> Result<String, EventBusError>;
    async fn subscribe(&self, stream: &str, group: &str) -> Result<Box<dyn EventSubscriber>, EventBusError>;
    async fn ack(&self, stream: &str, group: &str, id: &str) -> Result<(), EventBusError>;
    async fn nack(&self, stream: &str, group: &str, id: &str) -> Result<(), EventBusError>;
}

#[async_trait]
pub trait EventSubscriber: Send + Sync {
    async fn next(&mut self) -> Result<Option<Event>, EventBusError>;
    async fn close(&mut self) -> Result<(), EventBusError>;
}
```

#### Step 3: Create InMemory Implementation
```rust
// eventbus/src/inmemory.rs
use std::collections::{HashMap, VecDeque};
use tokio::sync::RwLock;

pub struct InMemoryEventBus {
    streams: Arc<RwLock<HashMap<String, VecDeque<Event>>>>,
    groups: Arc<RwLock<HashMap<String, ConsumerGroup>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            streams: Arc::new(RwLock::new(HashMap::new())),
            groups: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    // Implementation for testing
}
```

#### Step 4: Migrate Redis Implementation
```rust
// eventbus/src/redis.rs
use redis::aio::ConnectionManager;

pub struct RedisEventBus {
    conn: ConnectionManager,
    key_prefix: String,
}

#[async_trait]
impl EventBus for RedisEventBus {
    async fn publish(&self, stream: &str, event: Event) -> Result<String, EventBusError> {
        // Migrate existing Redis adapter code
        // Fix channel naming: stream:symbol:AAPL instead of market:AAPL
        let key = format!("{}:{}", self.key_prefix, stream);
        // ... rest of implementation
    }
}
```

#### Step 5: Create Recording Implementation
```rust
// eventbus/src/recording.rs
pub struct RecordingEventBus {
    inner: Box<dyn EventBus>,
    recorded_events: Arc<RwLock<Vec<Event>>>,
}

impl RecordingEventBus {
    pub async fn get_recorded_events(&self) -> Vec<Event> {
        self.recorded_events.read().await.clone()
    }
    
    pub async fn clear_recordings(&self) {
        self.recorded_events.write().await.clear();
    }
}
```

### Day 3-4: Critical System Migration - DAA Coordinator

#### Step 1: Analyze Legacy Implementation
```bash
# Read and understand the legacy DAA coordinator
cat src/integration/daa_coordinator.rs | head -100
# Identify core logic to preserve
grep -n "pub fn\|pub async fn" src/integration/daa_coordinator.rs
```

#### Step 2: Enhance V2 Stub
```rust
// neural-trading/src/daa/coordinator.rs

// PRESERVE these critical functions from legacy:
impl DaaCoordinator {
    // Migrate from src/integration/daa_coordinator.rs:150-300
    pub async fn process_market_data(&self, data: MarketData) -> Result<()> {
        // Critical autonomous decision logic
    }
    
    // Migrate from src/integration/daa_coordinator.rs:400-600
    pub async fn evaluate_strategies(&self) -> Vec<StrategySignal> {
        // Multi-strategy evaluation with consensus
    }
    
    // Migrate from src/integration/daa_coordinator.rs:700-900
    pub async fn execute_autonomous_decision(&self, signals: Vec<StrategySignal>) -> Decision {
        // Autonomous execution logic with risk management
    }
    
    // Migrate from src/integration/daa_coordinator.rs:1000-1200
    pub async fn update_performance_metrics(&self, outcome: TradingOutcome) {
        // Performance tracking for autonomous learning
    }
}
```

#### Step 3: Preserve Redis Communication
```rust
// neural-trading/src/events/redis_channels.rs

// Migrate from src/adapters/redis_sector_channels.rs
pub struct SectorChannelManager {
    redis_client: redis::Client,
    channels: HashMap<String, String>,
}

impl SectorChannelManager {
    pub fn new() -> Self {
        let mut channels = HashMap::new();
        // Fix channel naming during migration
        channels.insert("AAPL".to_string(), "stream:symbol:AAPL".to_string());
        channels.insert("XLK".to_string(), "stream:sector:technology".to_string());
        // ... more channels
        Self { redis_client, channels }
    }
    
    pub async fn subscribe_to_sector(&self, sector: &str) -> Result<Subscriber> {
        let channel = self.channels.get(sector)
            .ok_or_else(|| anyhow!("Unknown sector: {}", sector))?;
        // Subscribe with correct channel format
    }
}
```

### Day 5: Performance Tracking Migration

#### Step 1: Migrate Core Metrics
```rust
// neural-ml-ops/src/training/metrics.rs

// Migrate from src/monitoring/model_performance_tracker.rs
pub struct ModelPerformanceTracker {
    metrics: Arc<RwLock<HashMap<String, PerformanceMetrics>>>,
    history: Arc<RwLock<VecDeque<PerformanceSnapshot>>>,
}

impl ModelPerformanceTracker {
    // Preserve these critical functions:
    pub async fn track_prediction(&self, model_id: &str, prediction: Prediction, actual: Option<f64>) {
        // Channel-specific performance tracking
    }
    
    pub async fn get_model_performance(&self, model_id: &str) -> PerformanceMetrics {
        // Used by DAA for training decisions
    }
    
    pub async fn trigger_retraining(&self, model_id: &str) -> bool {
        // Autonomous retraining logic
    }
}
```

## Week 2: Neural Engine Replacement

### Day 1: Vendor Model Factory
```rust
// neural-ml-ops/src/models/vendor_factory.rs
use vendor::ruv_fann::neuro_divergent::{
    BaseModel, ModelType,
    models::{LSTM, GRU, Transformer, NBEATS, ...}
};

pub struct VendorModelFactory {
    model_registry: HashMap<String, Box<dyn BaseModel<f32>>>,
}

impl VendorModelFactory {
    pub fn create_model(&self, config: ModelConfig) -> Result<Box<dyn BaseModel<f32>>> {
        match config.model_type {
            ModelType::LSTM => Ok(Box::new(LSTM::new(config.params)?)),
            ModelType::GRU => Ok(Box::new(GRU::new(config.params)?)),
            ModelType::Transformer => Ok(Box::new(Transformer::new(config.params)?)),
            ModelType::NBEATS => Ok(Box::new(NBEATS::new(config.params)?)),
            // ... all 27+ models
        }
    }
}
```

### Day 2-3: DAA Integration Preservation
```rust
// neural-ml-ops/src/models/daa_adapter.rs

/// Adapter to preserve DAA interfaces during neural engine replacement
pub struct DaaCompatiblePredictor {
    vendor_model: Box<dyn BaseModel<f32>>,
    performance_tracker: Arc<ModelPerformanceTracker>,
}

impl DaaCompatiblePredictor {
    pub async fn predict(&self, features: &Features) -> Result<Prediction> {
        // Convert features to vendor format
        let vendor_input = self.convert_features(features)?;
        
        // Get prediction from vendor model
        let output = self.vendor_model.forward(&vendor_input)?;
        
        // Track performance for DAA
        self.performance_tracker.track_prediction(
            &self.model_id,
            output.clone(),
            None
        ).await;
        
        // Convert back to DAA format
        Ok(self.convert_prediction(output))
    }
}
```

## Week 3-4: Domain Binary Separation

### Day 1: Create Domain Binaries

#### neural-ml-ops Binary
```rust
// neural-ml-ops/src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration
    let config = load_config().await?;
    
    // Initialize EventBus
    let event_bus = Arc::new(RedisEventBus::new(&config.redis).await?);
    
    // Initialize model registry
    let model_registry = ModelRegistry::new(&config.models).await?;
    
    // Initialize training coordinator
    let training_coordinator = TrainingCoordinator::new(
        model_registry.clone(),
        event_bus.clone(),
    );
    
    // Start gRPC service
    let grpc_server = MLOpsGrpcServer::new(
        training_coordinator,
        model_registry,
    );
    
    grpc_server.serve(config.grpc_port).await?;
    Ok(())
}
```

#### neural-trading Binary
```rust
// neural-trading/src/main.rs
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize configuration
    let config = load_config().await?;
    
    // Initialize EventBus
    let event_bus = Arc::new(RedisEventBus::new(&config.redis).await?);
    
    // Initialize DAA Coordinator
    let daa_coordinator = DaaCoordinator::new(&config.daa).await?;
    
    // Initialize execution engine
    let execution_engine = ExecutionEngine::new(
        daa_coordinator.clone(),
        event_bus.clone(),
    );
    
    // Start event consumers
    let market_consumer = MarketDataConsumer::new(event_bus.clone());
    let ml_consumer = MLEventsConsumer::new(event_bus.clone());
    
    // Start gRPC service
    let grpc_server = TradingGrpcServer::new(
        execution_engine,
        daa_coordinator,
    );
    
    tokio::select! {
        _ = market_consumer.run() => {},
        _ = ml_consumer.run() => {},
        _ = grpc_server.serve(config.grpc_port) => {},
    }
    
    Ok(())
}
```

### Day 2-3: Service Boundary Enforcement

#### Step 1: Remove Direct Dependencies
```bash
# Check for direct imports between services
grep -r "use neural_ml_ops" neural-trading/
grep -r "use neural_trading" neural-ml-ops/

# Replace with gRPC clients
```

#### Step 2: Implement gRPC Clients
```rust
// neural-trading/src/clients/ml_ops_client.rs
use neural_core::grpc::ml_ops_client::MLOpsClient;

pub struct MLOpsGrpcClient {
    client: MLOpsClient<tonic::transport::Channel>,
}

impl MLOpsGrpcClient {
    pub async fn get_prediction(&self, features: Features) -> Result<Prediction> {
        let request = tonic::Request::new(PredictionRequest {
            features: features.into(),
        });
        
        let response = self.client.predict(request).await?;
        Ok(response.into_inner().into())
    }
}
```

## Week 5: Integration Testing

### Day 1-2: End-to-End Test Suite
```rust
// tests/integration/e2e_trading_flow.rs

#[tokio::test]
async fn test_complete_trading_flow() {
    // Start all services
    let _ml_ops = start_ml_ops_service().await;
    let _trading = start_trading_service().await;
    let _data_ingestion = start_data_ingestion_service().await;
    
    // Initialize test EventBus
    let event_bus = Arc::new(RecordingEventBus::new(
        RedisEventBus::new(&test_config()).await.unwrap()
    ));
    
    // Simulate market data
    let market_data = create_test_market_data();
    event_bus.publish("stream:symbol:AAPL", market_data.into()).await.unwrap();
    
    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify DAA decision was made
    let recorded = event_bus.get_recorded_events().await;
    let decision_event = recorded.iter()
        .find(|e| e.stream == "stream:action:execute")
        .expect("DAA should make a decision");
    
    // Verify decision quality
    let decision: TradingDecision = serde_json::from_value(decision_event.payload.clone()).unwrap();
    assert!(decision.confidence > 0.7);
    assert!(decision.risk_score < 0.3);
}
```

### Day 3-4: Performance Benchmarks
```rust
// tests/benchmarks/prediction_latency.rs

#[bench]
fn bench_vendor_model_prediction(b: &mut Bencher) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let model = runtime.block_on(create_vendor_model());
    
    b.iter(|| {
        runtime.block_on(async {
            let features = create_test_features();
            let _ = model.predict(&features).await;
        });
    });
}

#[bench]
fn bench_daa_decision_latency(b: &mut Bencher) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let daa = runtime.block_on(create_daa_coordinator());
    
    b.iter(|| {
        runtime.block_on(async {
            let signals = create_test_signals();
            let _ = daa.execute_autonomous_decision(signals).await;
        });
    });
}
```

## Week 6: Cleanup and Documentation

### Day 1-2: Legacy Code Removal
```bash
# Create deprecation branch for safety
git checkout -b legacy-deprecation

# Remove deprecated directories
rm -rf src/neural/fann/
rm -rf src/templates/
rm -rf src/backtesting/

# Remove over-engineered components
rm -rf src/utils/market_hours/complex_*.rs
rm -rf src/models/sectors/over_engineered_*.rs

# Keep only verified migrations
ls src/integration/  # Should only have verified stubs
ls src/adapters/     # Should be empty after migration
```

### Day 3-4: Documentation Update
```bash
# Update architecture docs
cat > docs/architecture/V2_ARCHITECTURE.md << EOF
# Neural-Trader V2 Architecture

## System Overview
The Neural-Trader V2 uses a microservices architecture with three core services:
1. neural-ml-ops: Model training and management
2. neural-trading: Trading execution with DAA
3. data-ingestion: Market data collection

## Communication
All services communicate via:
- EventBus (Redis Streams) for events
- gRPC for synchronous calls

## Key Components
...
EOF

# Update README
cat > README.md << EOF
# Neural-Trader V2

## Quick Start
\`\`\`bash
# Start all services
docker-compose up

# Run tests
cargo test --all

# Run specific service
cargo run --bin neural-trading
\`\`\`
...
EOF
```

## Validation Checklist

### After Each Phase
- [ ] All existing tests still pass
- [ ] New functionality has tests
- [ ] DAA decisions still being made
- [ ] Performance metrics maintained
- [ ] No regression in prediction quality

### Final Validation
- [ ] Full system runs without legacy code
- [ ] All 3 binaries start successfully
- [ ] End-to-end tests pass
- [ ] Performance benchmarks meet targets
- [ ] Documentation is complete

## Rollback Procedures

### If Migration Fails
```bash
# Immediate rollback
git checkout main
cargo build --all
docker-compose restart

# Investigate failure
grep ERROR logs/migration.log
cargo test --all -- --nocapture

# Fix and retry
git checkout v2-migration
# Apply fixes
cargo test --all
```

## Support Resources

- Architecture Diagrams: `product/features/v2Planning/mvp/architecture/`
- Integration Mandate: `product/INTEGRATION_FIRST_MANDATE.md`
- Original V2 Plan: `product/features/v2Planning/mvp/V2-Implementation-Plan.md`
- Swarm Analysis: `product/features/v2Planning/phase-next/CURRENT_STATE_ASSESSMENT.md`

## Success Criteria

The migration is complete when:
1. All 3 domain binaries run independently
2. Legacy src/ directory is removed (90% deletion)
3. All integration tests pass
4. DAA makes profitable autonomous decisions
5. Performance meets or exceeds V1 baseline

---
*This guide provides step-by-step implementation details for the V2 migration. Follow each phase sequentially and validate thoroughly before proceeding.*