# SPARC Refinement: EventBus Component
## Test-Driven Development and Implementation Plan

*Document Version*: 1.0  
*Created*: 2025-01-26  
*Status*: Active Refinement  
*Component*: EventBus Abstraction Layer

## 1. TDD Implementation Strategy

### 1.1 Test Categories and Priority

```rust
// Test implementation order following TDD principles
pub enum TestPriority {
    P0_Critical,    // Core trait compliance
    P1_Essential,   // Basic functionality
    P2_Important,   // Integration patterns
    P3_Nice,        // Performance and edge cases
}
```

### 1.2 Test-First Development Phases

#### Phase 1: Core Trait Tests (Day 1 Morning)
Write tests BEFORE implementation:

```rust
// tests/trait_compliance.rs

#[cfg(test)]
mod trait_compliance_tests {
    use super::*;
    use eventbus::traits::{EventBus, Event, EventId};
    
    #[tokio::test]
    async fn test_publish_returns_event_id() {
        // GIVEN an EventBus implementation
        let event_bus = create_test_event_bus();
        
        // WHEN publishing an event
        let event = create_test_event();
        let result = event_bus.publish("stream:symbol:AAPL", event).await;
        
        // THEN it should return a valid EventId
        assert!(result.is_ok());
        let event_id = result.unwrap();
        assert!(!event_id.to_string().is_empty());
    }
    
    #[tokio::test]
    async fn test_subscribe_returns_subscriber() {
        // GIVEN an EventBus with a channel
        let event_bus = create_test_event_bus();
        let channels = vec!["stream:symbol:AAPL".to_string()];
        
        // WHEN subscribing to channels
        let config = SubscriptionConfig::default();
        let result = event_bus.subscribe(&channels, config).await;
        
        // THEN it should return a valid subscriber
        assert!(result.is_ok());
        let mut subscriber = result.unwrap();
        
        // AND subscriber should implement EventSubscriber trait
        let next = subscriber.next().await;
        assert!(next.is_ok());
    }
    
    #[tokio::test]
    async fn test_ack_succeeds_for_valid_event() {
        // GIVEN a consumed event
        let event_bus = create_test_event_bus();
        let event_id = EventId::from("test-123");
        
        // WHEN acknowledging the event
        let result = event_bus.ack(
            "stream:symbol:AAPL",
            "test-group",
            &event_id
        ).await;
        
        // THEN acknowledgment should succeed
        assert!(result.is_ok());
    }
}
```

#### Phase 2: Channel Validation Tests (Day 1 Afternoon)

```rust
// tests/channel_validation.rs

#[cfg(test)]
mod channel_validation_tests {
    use super::*;
    
    #[test]
    fn test_valid_channel_formats() {
        let valid_channels = vec![
            "stream:symbol:AAPL",
            "stream:sector:technology",
            "stream:portfolio:decisions",
            "stream:ml:training_requests",
        ];
        
        for channel in valid_channels {
            assert!(validate_channel_name(channel));
        }
    }
    
    #[test]
    fn test_invalid_channel_formats() {
        let invalid_channels = vec![
            "market:AAPL",  // Old format
            "symbol:AAPL",  // Missing stream prefix
            "stream:unknown:test",  // Invalid domain
            "stream:symbol:",  // Missing identifier
        ];
        
        for channel in invalid_channels {
            assert!(!validate_channel_name(channel));
        }
    }
    
    #[test]
    fn test_channel_migration() {
        assert_eq!(
            migrate_channel_name("market:AAPL"),
            "stream:symbol:AAPL"
        );
        assert_eq!(
            migrate_channel_name("sector_technology"),
            "stream:sector:technology"
        );
    }
}
```

#### Phase 3: InMemory Implementation Tests (Day 2 Morning)

```rust
// tests/inmemory_tests.rs

#[cfg(test)]
mod inmemory_implementation_tests {
    use super::*;
    use eventbus::implementations::inmemory::InMemoryEventBus;
    
    #[tokio::test]
    async fn test_inmemory_publish_subscribe_flow() {
        // GIVEN an InMemoryEventBus
        let event_bus = InMemoryEventBus::new();
        
        // WHEN publishing an event
        let event = Event {
            event_type: "MarketData".to_string(),
            payload: vec![1, 2, 3],
            metadata: HashMap::new(),
            timestamp: 123456789,
        };
        
        let event_id = event_bus
            .publish("stream:symbol:AAPL", event.clone())
            .await
            .unwrap();
        
        // AND subscribing to the channel
        let mut subscriber = event_bus
            .subscribe(
                &["stream:symbol:AAPL".to_string()],
                SubscriptionConfig::default()
            )
            .await
            .unwrap();
        
        // THEN the subscriber should receive the event
        let envelope = subscriber.next().await.unwrap().unwrap();
        assert_eq!(envelope.event_id, event_id);
        assert_eq!(envelope.event.event_type, "MarketData");
    }
    
    #[tokio::test]
    async fn test_inmemory_consumer_groups() {
        let event_bus = InMemoryEventBus::new();
        
        // Create two subscribers in same group
        let mut sub1 = create_subscriber(&event_bus, "group1").await;
        let mut sub2 = create_subscriber(&event_bus, "group1").await;
        
        // Publish two events
        publish_test_event(&event_bus, "event1").await;
        publish_test_event(&event_bus, "event2").await;
        
        // Each subscriber should get one event (load balancing)
        let env1 = sub1.next().await.unwrap();
        let env2 = sub2.next().await.unwrap();
        
        assert!(env1.is_some());
        assert!(env2.is_some());
        assert_ne!(env1.unwrap().event_id, env2.unwrap().event_id);
    }
}
```

#### Phase 4: Redis Implementation Tests (Day 2 Afternoon)

```rust
// tests/redis_tests.rs

#[cfg(test)]
mod redis_implementation_tests {
    use super::*;
    use eventbus::implementations::redis::RedisEventBus;
    use testcontainers::clients::Cli;
    use testcontainers::images::redis::Redis;
    
    #[tokio::test]
    async fn test_redis_with_testcontainer() {
        // GIVEN a Redis container
        let docker = Cli::default();
        let redis_container = docker.run(Redis::default());
        let redis_url = format!("redis://127.0.0.1:{}", redis_container.get_port(6379));
        
        // AND a RedisEventBus
        let event_bus = RedisEventBus::new(&redis_url).await.unwrap();
        
        // WHEN performing pub/sub operations
        test_eventbus_operations(event_bus).await;
    }
    
    #[tokio::test]
    async fn test_redis_channel_migration() {
        let redis_bus = create_test_redis_bus().await;
        
        // Test that old channel names are migrated
        let event = create_test_event();
        
        // Publish to old format
        redis_bus.publish("market:AAPL", event.clone()).await.unwrap();
        
        // Subscribe with new format should receive it
        let mut subscriber = redis_bus
            .subscribe(&["stream:symbol:AAPL".to_string()], config)
            .await
            .unwrap();
            
        let envelope = subscriber.next().await.unwrap().unwrap();
        assert_eq!(envelope.channel, "stream:symbol:AAPL");
    }
}
```

#### Phase 5: Recording Implementation Tests (Day 3 Morning)

```rust
// tests/recording_tests.rs

#[cfg(test)]
mod recording_implementation_tests {
    use super::*;
    use eventbus::implementations::recording::RecordingEventBus;
    
    #[tokio::test]
    async fn test_recording_captures_all_operations() {
        // GIVEN a RecordingEventBus wrapping InMemory
        let inner = InMemoryEventBus::new();
        let recording_bus = RecordingEventBus::new(Box::new(inner));
        
        // WHEN performing operations
        let event1 = create_test_event("event1");
        let event2 = create_test_event("event2");
        
        recording_bus.publish("stream:symbol:AAPL", event1).await.unwrap();
        recording_bus.publish("stream:symbol:MSFT", event2).await.unwrap();
        
        // THEN all operations should be recorded
        let recorded = recording_bus.get_recorded_publishes();
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].channel, "stream:symbol:AAPL");
        assert_eq!(recorded[1].channel, "stream:symbol:MSFT");
    }
    
    #[tokio::test]
    async fn test_recording_assertions() {
        let recording_bus = create_test_recording_bus();
        
        // Perform operations
        publish_market_data(&recording_bus, "AAPL", 150.0).await;
        
        // Use assertion helpers
        assert!(recording_bus.assert_event_published(
            "stream:symbol:AAPL",
            "MarketData"
        ));
        
        assert!(!recording_bus.assert_event_published(
            "stream:symbol:GOOGL",
            "MarketData"
        ));
    }
}
```

## 2. Implementation Refinement

### 2.1 Core Trait Implementation

```rust
// eventbus/src/lib.rs

pub mod traits;
pub mod implementations;
pub mod controllers;
pub mod metrics;

pub use traits::{EventBus, EventSubscriber, Event, EventId, EventEnvelope};
pub use implementations::{
    inmemory::InMemoryEventBus,
    redis::RedisEventBus,
    recording::RecordingEventBus,
};

/// Factory function for creating EventBus instances
pub async fn create_event_bus(config: EventBusConfig) -> Result<Arc<dyn EventBus>> {
    match config.backend {
        Backend::Redis(redis_config) => {
            let redis_bus = RedisEventBus::new(redis_config).await?;
            Ok(Arc::new(redis_bus))
        }
        Backend::InMemory => {
            Ok(Arc::new(InMemoryEventBus::new()))
        }
        Backend::Recording(inner_backend) => {
            let inner = create_event_bus(*inner_backend).await?;
            Ok(Arc::new(RecordingEventBus::new(inner)))
        }
    }
}
```

### 2.2 Backpressure Controller Refinement

```rust
// eventbus/src/controllers/backpressure.rs

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct BackpressureController {
    limits: Arc<RwLock<HashMap<String, ChannelLimits>>>,
    metrics: Arc<RwLock<HashMap<String, ChannelMetrics>>>,
    throttle_states: Arc<RwLock<HashMap<String, ThrottleState>>>,
}

impl BackpressureController {
    pub async fn check_and_apply(&self, channel: &str) -> Result<PressureStatus> {
        let limits = self.get_limits(channel).await;
        let metrics = self.measure_metrics(channel).await?;
        
        let pressure = self.calculate_pressure(&limits, &metrics);
        
        match pressure {
            p if p >= limits.critical_threshold => {
                self.apply_critical_throttling(channel).await?;
                Ok(PressureStatus::Critical)
            }
            p if p >= limits.warning_threshold => {
                self.apply_warning_throttling(channel).await?;
                Ok(PressureStatus::Warning)
            }
            _ => {
                self.clear_throttling(channel).await?;
                Ok(PressureStatus::Normal)
            }
        }
    }
    
    fn calculate_pressure(&self, limits: &ChannelLimits, metrics: &ChannelMetrics) -> f64 {
        let message_pressure = metrics.pending_messages as f64 / limits.max_pending as f64;
        let memory_pressure = metrics.memory_mb as f64 / limits.max_memory_mb as f64;
        let lag_pressure = metrics.consumer_lag_ms as f64 / limits.max_lag_ms as f64;
        
        message_pressure.max(memory_pressure).max(lag_pressure)
    }
}
```

### 2.3 Dead Letter Queue Refinement

```rust
// eventbus/src/controllers/dlq.rs

use exponential_backoff::Backoff;

pub struct DeadLetterQueue {
    config: DLQConfig,
    retry_tracker: Arc<RwLock<HashMap<EventId, RetryInfo>>>,
    event_bus: Arc<dyn EventBus>,
}

impl DeadLetterQueue {
    pub async fn handle_failure(
        &self,
        channel: &str,
        event_id: &EventId,
        event: &Event,
        error: &EventBusError,
    ) -> Result<MessageDisposition> {
        let mut retry_info = self.get_or_create_retry_info(event_id).await;
        
        if self.should_retry(&retry_info, error) {
            let delay = self.calculate_backoff(retry_info.attempt);
            retry_info.attempt += 1;
            retry_info.last_error = error.to_string();
            
            self.schedule_retry(channel, event_id, event, delay).await?;
            self.update_retry_info(event_id, retry_info).await;
            
            Ok(MessageDisposition::Retry { attempt: retry_info.attempt, delay })
        } else {
            self.move_to_dlq(channel, event_id, event, &retry_info).await?;
            self.remove_retry_info(event_id).await;
            
            Ok(MessageDisposition::DeadLetter {
                reason: format!("Max retries ({}) exceeded", self.config.max_retries),
                final_error: error.clone(),
            })
        }
    }
    
    fn should_retry(&self, retry_info: &RetryInfo, error: &EventBusError) -> bool {
        retry_info.attempt < self.config.max_retries && 
        matches!(error, 
            EventBusError::Temporary(_) | 
            EventBusError::Timeout(_) |
            EventBusError::RateLimit(_)
        )
    }
    
    fn calculate_backoff(&self, attempt: usize) -> Duration {
        let mut backoff = Backoff::new(
            self.config.base_delay_ms,
            self.config.max_delay_ms,
            self.config.multiplier,
        );
        
        for _ in 0..attempt {
            backoff.next();
        }
        
        backoff.current_interval()
    }
}
```

## 3. Performance Optimization Refinements

### 3.1 Batching Optimization

```rust
// eventbus/src/controllers/batching.rs

pub struct OptimizedBatcher {
    configs: HashMap<String, BatchConfig>,
    pending: Arc<RwLock<HashMap<String, PendingBatch>>>,
    flush_scheduler: Arc<FlushScheduler>,
}

impl OptimizedBatcher {
    pub async fn add_event(
        &self,
        channel: &str,
        event: Event,
    ) -> Result<BatchDisposition> {
        let config = self.get_config(channel);
        
        let mut pending = self.pending.write().await;
        let batch = pending.entry(channel.to_string())
            .or_insert_with(|| PendingBatch::new(config.clone()));
        
        batch.add(event);
        
        if batch.should_flush() {
            let events = batch.take_events();
            drop(pending);  // Release lock early
            
            Ok(BatchDisposition::FlushNow(events))
        } else {
            if !self.flush_scheduler.is_scheduled(channel) {
                self.flush_scheduler.schedule(channel, config.max_wait_ms);
            }
            Ok(BatchDisposition::Pending)
        }
    }
}
```

### 3.2 Channel Pool Optimization

```rust
// eventbus/src/implementations/redis/connection.rs

pub struct ConnectionPool {
    connections: Vec<Arc<Mutex<redis::aio::Connection>>>,
    round_robin: AtomicUsize,
}

impl ConnectionPool {
    pub async fn get_connection(&self) -> Arc<Mutex<redis::aio::Connection>> {
        let index = self.round_robin.fetch_add(1, Ordering::Relaxed) % self.connections.len();
        self.connections[index].clone()
    }
    
    pub async fn execute_with_retry<T, F>(&self, f: F) -> Result<T>
    where
        F: Fn(Arc<Mutex<redis::aio::Connection>>) -> Future<Output = Result<T>>,
    {
        let mut attempts = 0;
        loop {
            let conn = self.get_connection().await;
            
            match f(conn).await {
                Ok(result) => return Ok(result),
                Err(e) if attempts < 3 && is_retriable(&e) => {
                    attempts += 1;
                    tokio::time::sleep(Duration::from_millis(100 * attempts)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

## 4. Integration Test Refinements

### 4.1 Multi-Service Integration Test

```rust
// tests/integration/multi_service.rs

#[tokio::test]
async fn test_neural_mlops_to_neural_trading_flow() {
    // Setup test environment
    let event_bus = create_test_event_bus().await;
    
    // Simulate neural-ml-ops publishing training complete
    let training_event = Event {
        event_type: "TrainingComplete".to_string(),
        payload: serialize_protobuf(&TrainingResult {
            model_id: "model-123".to_string(),
            accuracy: 0.95,
            ready_for_deployment: true,
        }),
        metadata: hashmap!{
            "model_type" => "LSTM",
            "training_time_ms" => "3600000",
        },
        timestamp: Utc::now().timestamp(),
    };
    
    event_bus.publish("stream:ml:training_results", training_event).await.unwrap();
    
    // Simulate neural-trading subscribing and receiving
    let mut trading_subscriber = event_bus.subscribe(
        &["stream:ml:training_results".to_string()],
        SubscriptionConfig {
            group_name: "trading-service".to_string(),
            consumer_name: "trading-1".to_string(),
            ..Default::default()
        }
    ).await.unwrap();
    
    let envelope = trading_subscriber.next().await.unwrap().unwrap();
    assert_eq!(envelope.event.event_type, "TrainingComplete");
    
    // Verify model deployment trigger
    let deploy_event = Event {
        event_type: "ModelDeploymentRequest".to_string(),
        payload: serialize_protobuf(&DeploymentRequest {
            model_id: "model-123".to_string(),
            target_service: "neural-trading".to_string(),
        }),
        ..Default::default()
    };
    
    event_bus.publish("stream:ml:model_updates", deploy_event).await.unwrap();
}
```

### 4.2 DAA Coordinator Integration Test

```rust
// tests/integration/daa_coordinator.rs

#[tokio::test]
async fn test_daa_coordinator_communication() {
    let event_bus = create_test_event_bus().await;
    
    // Simulate market data flow
    publish_market_data_stream(&event_bus, "AAPL", vec![150.0, 151.0, 149.5]).await;
    
    // Simulate DAA decision
    let decision_event = Event {
        event_type: "AutonomousDecision".to_string(),
        payload: serialize_protobuf(&TradingDecision {
            action: "BUY".to_string(),
            symbol: "AAPL".to_string(),
            quantity: 100.0,
            confidence: 0.85,
            reasoning: "Bullish momentum detected".to_string(),
        }),
        ..Default::default()
    };
    
    event_bus.publish("stream:portfolio:decisions", decision_event).await.unwrap();
    
    // Verify execution trigger
    let mut action_subscriber = event_bus.subscribe(
        &["stream:action:trade_executions".to_string()],
        SubscriptionConfig::default()
    ).await.unwrap();
    
    // Publish execution request
    let execution = Event {
        event_type: "ExecutionRequest".to_string(),
        ..Default::default()
    };
    
    event_bus.publish("stream:action:trade_executions", execution).await.unwrap();
    
    let envelope = action_subscriber.next().await.unwrap().unwrap();
    assert_eq!(envelope.event.event_type, "ExecutionRequest");
}
```

## 5. Performance Benchmarks

### 5.1 Throughput Benchmarks

```rust
// benches/throughput.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_publish_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("publish_throughput");
    
    for size in [100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.to_async(&runtime).iter(|| async {
                    let event_bus = create_benchmark_event_bus().await;
                    
                    for i in 0..size {
                        let event = create_test_event(i);
                        event_bus.publish("stream:symbol:TEST", event).await.unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_consume_throughput(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("consume_10k_messages", |b| {
        b.to_async(&runtime).iter(|| async {
            let event_bus = setup_with_messages(10000).await;
            let mut subscriber = create_subscriber(&event_bus).await;
            
            let mut count = 0;
            while let Some(_) = subscriber.next().await.unwrap() {
                count += 1;
                if count >= 10000 { break; }
            }
        });
    });
}

criterion_group!(benches, benchmark_publish_throughput, benchmark_consume_throughput);
criterion_main!(benches);
```

## 6. Error Handling Refinements

### 6.1 Comprehensive Error Types

```rust
// eventbus/src/error.rs

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EventBusError {
    #[error("Channel validation failed: {0}")]
    InvalidChannel(String),
    
    #[error("Backend connection error: {0}")]
    ConnectionError(#[from] redis::RedisError),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] prost::DecodeError),
    
    #[error("Backpressure limit exceeded for channel {channel}")]
    Throttled { channel: String },
    
    #[error("Consumer group error: {0}")]
    ConsumerGroupError(String),
    
    #[error("Timeout waiting for messages")]
    Timeout,
    
    #[error("Channel does not exist: {0}")]
    ChannelNotFound(String),
    
    #[error("Invalid configuration: {0}")]
    ConfigurationError(String),
}

pub type Result<T> = std::result::Result<T, EventBusError>;
```

---

*This refinement document provides the complete TDD approach and implementation details for the EventBus component, ensuring high quality and reliability.*