# EventBus Integration Plan for Neural-Trader System

## 🎯 Integration Actions Required

### 1. **Add EventBus to Service Dependencies**

Each service needs to import and initialize the EventBus:

```toml
# Add to each service's Cargo.toml
[dependencies]
neural-core = { path = "../neural-core", features = ["eventbus"] }
```

### 2. **Initialize EventBus in Each Service**

#### A. **neural-trading/src/main.rs**
```rust
use neural_core::eventbus::{EventBus, RedisEventBus, Event};

// In main()
let event_bus = Arc::new(RedisEventBus::new(&config.redis_url).await?);

// Replace direct Redis pub/sub with EventBus
let execution_engine = ExecutionEngine::new(
    event_bus.clone(),  // Instead of redis client
    // ... other params
);
```

#### B. **neural-ml-ops/src/main.rs**
```rust
use neural_core::eventbus::{EventBus, RedisEventBus};

// In start command
let event_bus = Arc::new(RedisEventBus::new(&redis_url).await?);

// Replace EventPublisher with EventBus
let coordinator = TrainingCoordinator::new(
    event_bus.clone(),
    // ... other params
);
```

### 3. **Define Event Schemas**

Create Protocol Buffer definitions for each event type:

```protobuf
// neural-core/proto/events.proto

message MarketDataEvent {
    string symbol = 1;
    double price = 2;
    int64 volume = 3;
    int64 timestamp = 4;
}

message TradeExecutionEvent {
    string order_id = 1;
    string symbol = 2;
    double quantity = 3;
    double price = 4;
    enum Side {
        BUY = 0;
        SELL = 1;
    }
    Side side = 5;
}

message ModelTrainingEvent {
    string model_id = 1;
    string status = 2;
    map<string, double> metrics = 3;
}
```

### 4. **Channel Mapping Strategy**

Map existing Redis pub/sub patterns to EventBus channels:

| Current Pattern | EventBus Channel | Purpose |
|-----------------|------------------|---------|
| `market:*` | `stream:symbol:{symbol}` | Market data per symbol |
| `trades:*` | `stream:action:trades` | Trade executions |
| `ml:training:*` | `stream:ml:training` | ML training events |
| `ml:inference:*` | `stream:ml:inference` | ML predictions |
| `risk:*` | `stream:action:risk` | Risk management |
| `portfolio:*` | `stream:portfolio:{id}` | Portfolio updates |

### 5. **Service-Specific Integration Steps**

#### **neural-trading Service**
```rust
// events/consumer.rs - REPLACE with:
pub struct EventConsumer {
    event_bus: Arc<dyn EventBus>,
    subscription: Box<dyn EventSubscriber>,
}

impl EventConsumer {
    pub async fn new(event_bus: Arc<dyn EventBus>) -> Result<Self> {
        let config = SubscriptionConfig {
            group_name: "trading-consumer".to_string(),
            consumer_name: "main".to_string(),
            // ... config
        };
        
        let subscription = event_bus.subscribe(
            &[
                "stream:symbol:*".to_string(),
                "stream:ml:inference".to_string(),
            ],
            config
        ).await?;
        
        Ok(Self { event_bus, subscription })
    }
    
    pub async fn consume(&mut self) -> Result<()> {
        while let Some(envelope) = self.subscription.next().await? {
            // Deserialize protobuf
            match envelope.event.event_type.as_str() {
                "MarketData" => self.handle_market_data(envelope).await?,
                "MLPrediction" => self.handle_prediction(envelope).await?,
                _ => {}
            }
            
            // ACK the message
            self.event_bus.ack(
                &envelope.channel,
                "trading-consumer",
                &envelope.event_id
            ).await?;
        }
        Ok(())
    }
}
```

#### **neural-ml-ops Service**
```rust
// events/publisher.rs - REPLACE with:
pub struct MLEventPublisher {
    event_bus: Arc<dyn EventBus>,
}

impl MLEventPublisher {
    pub async fn publish_training_complete(
        &self,
        model_id: &str,
        metrics: HashMap<String, f64>
    ) -> Result<()> {
        let event_data = ModelTrainingEvent {
            model_id: model_id.to_string(),
            status: "complete".to_string(),
            metrics,
        };
        
        let payload = event_data.encode_to_vec(); // protobuf
        let event = Event::new("ModelTraining".to_string(), payload)
            .with_metadata("model_id".to_string(), model_id.to_string());
            
        self.event_bus.publish("stream:ml:training", event).await?;
        Ok(())
    }
}
```

### 6. **Migration Script**

Create a migration script to transition existing services:

```bash
#!/bin/bash
# migrate-to-eventbus.sh

# 1. Update Cargo.toml files
for service in neural-trading neural-ml-ops mcp-trading-server; do
    echo "Updating $service/Cargo.toml"
    # Add neural-core dependency
done

# 2. Generate protobuf code
cd neural-core
cargo build --features proto

# 3. Run tests
cargo test --all-features
```

### 7. **Configuration Updates**

Update service configs to use EventBus:

```toml
# config.toml
[eventbus]
backend = "redis"  # or "kafka" for high scale
redis_url = "redis://localhost:6379"

[channels]
market_data = "stream:symbol:*"
ml_training = "stream:ml:training"
trade_execution = "stream:action:trades"

[consumer_groups]
trading = "trading-consumer-group"
ml_ops = "ml-ops-consumer-group"
risk = "risk-consumer-group"
```

### 8. **Monitoring & Observability**

Add EventBus metrics to existing monitoring:

```rust
// Add to each service
use neural_core::eventbus::metrics::EventBusMetrics;

let metrics = EventBusMetrics::new(&event_bus);
prometheus::register(Box::new(metrics))?;
```

### 9. **Testing Strategy**

Create integration tests:

```rust
#[tokio::test]
async fn test_cross_service_communication() {
    // Start EventBus
    let event_bus = InMemoryEventBus::new();
    
    // Simulate neural-ml-ops publishing
    let ml_service = MLEventPublisher::new(event_bus.clone());
    ml_service.publish_training_complete("model_v1", metrics).await?;
    
    // Verify neural-trading receives it
    let mut trading_consumer = EventConsumer::new(event_bus.clone()).await?;
    let event = trading_consumer.subscription.next().await?;
    assert_eq!(event.event_type, "ModelTraining");
}
```

### 10. **Rollout Plan**

**Phase 1: Development Environment (Week 1)**
- [ ] Update all service dependencies
- [ ] Create protobuf schemas
- [ ] Implement EventBus wrappers in each service
- [ ] Write integration tests

**Phase 2: Testing (Week 2)**
- [ ] Run integration tests
- [ ] Performance benchmarking
- [ ] Load testing with multiple services

**Phase 3: Staging Deployment (Week 3)**
- [ ] Deploy to staging with feature flags
- [ ] Monitor metrics and performance
- [ ] Validate message delivery guarantees

**Phase 4: Production Rollout (Week 4)**
- [ ] Gradual rollout with canary deployment
- [ ] Monitor error rates and latency
- [ ] Full cutover from direct Redis pub/sub

## 🚀 Quick Start Commands

```bash
# 1. Build EventBus
cd neural-core
cargo build --features eventbus

# 2. Run tests
cargo test eventbus

# 3. Start Redis (required for integration)
docker run -d -p 6379:6379 redis:7-alpine

# 4. Run example integration
cargo run --example eventbus_integration_demo

# 5. Update services
./scripts/migrate-to-eventbus.sh
```

## 📊 Success Metrics

- **Zero message loss** during migration
- **P99 latency < 50ms** for event delivery
- **Support 10K+ messages/second** per channel
- **Backwards compatibility** during migration
- **No service downtime** during rollout

## 🔄 Rollback Plan

If issues arise:
1. Feature flags disable EventBus, fallback to direct Redis
2. Consumer groups preserve message offsets
3. Replay events from persistent storage
4. Monitor and fix issues before retry

## 📝 Next Steps

1. **Immediate**: Add EventBus to neural-trading's Cargo.toml
2. **Today**: Create protobuf schemas for all event types
3. **This Week**: Implement EventBus in one service as POC
4. **Next Sprint**: Full integration across all services