# DP-012 London TDD Test Strategy

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Document**: London School TDD Implementation Strategy
**Created**: 2026-01-18

---

## 1. London TDD Overview

### 1.1 Core Principles

London School TDD (also called "Mockist TDD") focuses on:

1. **Outside-In Development**: Start from behavior, work inward
2. **Behavior Verification**: Mock collaborators, verify interactions
3. **Interface Discovery**: Tests drive interface design
4. **Fast, Isolated Tests**: Mocks eliminate external dependencies

### 1.2 Project Context

The NDP already uses London TDD extensively (see `core/src/traits.rs` tests). This strategy extends the existing patterns to dp-012 components.

---

## 2. Mock Definitions

### 2.1 Subscriber Mock

```rust
// core/src/subscribers/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub Subscriber {}

        #[async_trait]
        impl Subscriber for Subscriber {
            fn id(&self) -> &str;
            async fn start(
                &mut self,
                receiver: broadcast::Receiver<Arc<RawDataPoint>>
            ) -> Result<(), SubscriberError>;
            async fn stop(&mut self) -> Result<(), SubscriberError>;
            fn accepts_stream(&self, stream_id: &str) -> bool;
            async fn health_check(&self) -> HealthStatus;
            async fn reconfigure(&mut self, config: serde_json::Value) -> Result<(), SubscriberError>;
        }
    }
}
```

### 2.2 Processor Mock

```rust
// core/src/processors/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub Processor {}

        #[async_trait]
        impl Processor for Processor {
            fn id(&self) -> &str;
            async fn process(
                &self,
                point: &RawDataPoint
            ) -> Result<Vec<ProcessorOutput>, ProcessorError>;
            fn accepts_stream(&self, stream_id: &str) -> bool;
            fn config(&self) -> &ProcessorConfig;
        }
    }
}
```

### 2.3 OutputSink Mock

```rust
// core/src/outputs/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub OutputSink {}

        #[async_trait]
        impl OutputSink for OutputSink {
            fn id(&self) -> &str;
            async fn write(&self, output: ProcessorOutput) -> Result<(), OutputError>;
            async fn flush(&self) -> Result<(), OutputError>;
            async fn health_check(&self) -> HealthStatus;
        }
    }
}
```

### 2.4 TimescaleDb Mock

```rust
// core/src/silver/mod.rs

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub TimescaleDb {}

        #[async_trait]
        impl TimescaleDb for TimescaleDb {
            async fn get_watermark(&self, table: &str) -> Result<Option<DateTime<Utc>>, DbError>;
            async fn upsert_batch(&self, table: &str, rows: Vec<SilverRow>) -> Result<(), DbError>;
            async fn query(&self, sql: &str) -> Result<Vec<Row>, DbError>;
            async fn health_check(&self) -> HealthStatus;
        }
    }
}
```

### 2.5 BronzeReader Mock

```rust
// core/src/silver/mod.rs

#[cfg(test)]
mod tests {
    mock! {
        pub BronzeReader {}

        #[async_trait]
        impl BronzeReader for BronzeReader {
            async fn read_since(
                &self,
                since: DateTime<Utc>
            ) -> Result<Vec<RawDataPoint>, StorageError>;
        }
    }
}
```

### 2.6 MqttClient Mock

```rust
// core/src/outputs/mqtt.rs

#[cfg(test)]
mod tests {
    mock! {
        pub MqttClient {}

        impl MqttClient for MqttClient {
            fn try_publish(
                &self,
                topic: &str,
                qos: QoS,
                retain: bool,
                payload: &[u8]
            ) -> Result<(), MqttError>;
        }
    }
}
```

---

## 3. TDD Cycles by Component

### 3.1 EventBus TDD Cycles

#### Cycle 1: Basic Publish/Subscribe

```rust
#[test]
fn test_event_bus_creation() {
    // RED: EventBus::new doesn't exist
    let bus = EventBus::new(EventBusConfig::default());
    assert_eq!(bus.subscriber_count(), 0);
}

// GREEN: Implement minimal EventBus::new
// REFACTOR: Extract config handling
```

#### Cycle 2: Broadcasting

```rust
#[tokio::test]
async fn test_event_bus_broadcasts_to_subscriber() {
    // RED: publish() doesn't exist
    let bus = EventBus::new(EventBusConfig::default());
    let mut rx = bus.subscribe();

    let point = RawDataPoint::new("test-Http", json!({"value": 1}));
    bus.publish(point.clone()).unwrap();

    let received = rx.recv().await.unwrap();
    assert_eq!(received.source_id, "test-Http");
}

// GREEN: Implement publish() and subscribe()
// REFACTOR: Add Arc wrapper for zero-copy
```

#### Cycle 3: Multiple Subscribers

```rust
#[tokio::test]
async fn test_event_bus_broadcasts_to_all() {
    // RED: Second subscriber doesn't receive
    let bus = EventBus::new(EventBusConfig::default());
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    bus.publish(RawDataPoint::new("test", json!({}))).unwrap();

    assert!(rx1.recv().await.is_ok());
    assert!(rx2.recv().await.is_ok());
}

// GREEN: Ensure broadcast semantics
// REFACTOR: Verify Arc is shared, not cloned
```

#### Cycle 4: Lag Handling

```rust
#[tokio::test]
async fn test_event_bus_handles_lag() {
    let bus = EventBus::new(EventBusConfig { capacity: 5, ..Default::default() });
    let mut slow_rx = bus.subscribe();

    // Fill buffer beyond capacity
    for i in 0..10 {
        bus.publish(RawDataPoint::new(&format!("test-{i}"), json!({}))).unwrap();
    }

    // Slow subscriber should get Lagged error
    match slow_rx.recv().await {
        Err(broadcast::error::RecvError::Lagged(n)) => assert!(n > 0),
        _ => panic!("Expected Lagged error"),
    }
}

// GREEN: Use bounded broadcast channel
// REFACTOR: Add lag metrics
```

### 3.2 SubscriberCoordinator TDD Cycles

#### Cycle 1: Add Subscriber

```rust
#[tokio::test]
async fn test_coordinator_adds_subscriber() {
    let bus = Arc::new(EventBus::new(EventBusConfig::default()));
    let mut coordinator = SubscriberCoordinator::new(bus);

    let mut mock = MockSubscriber::new();
    mock.expect_id().returning(|| "test-sub");

    coordinator.add_subscriber(Box::new(mock));

    assert!(coordinator.has_subscriber("test-sub"));
}
```

#### Cycle 2: Start All

```rust
#[tokio::test]
async fn test_coordinator_starts_all_subscribers() {
    let bus = Arc::new(EventBus::new(EventBusConfig::default()));
    let mut coordinator = SubscriberCoordinator::new(bus);

    let mut mock1 = MockSubscriber::new();
    mock1.expect_id().returning(|| "sub1");
    mock1.expect_start().times(1).returning(|_| Ok(()));

    let mut mock2 = MockSubscriber::new();
    mock2.expect_id().returning(|| "sub2");
    mock2.expect_start().times(1).returning(|_| Ok(()));

    coordinator.add_subscriber(Box::new(mock1));
    coordinator.add_subscriber(Box::new(mock2));

    coordinator.start_all().await.unwrap();

    // Verified by mock expectations
}
```

#### Cycle 3: Stop All

```rust
#[tokio::test]
async fn test_coordinator_stops_all_subscribers() {
    let bus = Arc::new(EventBus::new(EventBusConfig::default()));
    let mut coordinator = SubscriberCoordinator::new(bus);

    let mut mock = MockSubscriber::new();
    mock.expect_id().returning(|| "sub1");
    mock.expect_start().returning(|_| Ok(()));
    mock.expect_stop().times(1).returning(|| Ok(()));

    coordinator.add_subscriber(Box::new(mock));
    coordinator.start_all().await.unwrap();
    coordinator.stop_all().await.unwrap();
}
```

#### Cycle 4: Failure Isolation

```rust
#[tokio::test]
async fn test_coordinator_isolates_failures() {
    let bus = Arc::new(EventBus::new(EventBusConfig::default()));
    let mut coordinator = SubscriberCoordinator::new(bus);

    let mut failing = MockSubscriber::new();
    failing.expect_id().returning(|| "failing");
    failing.expect_start().returning(|_| Err(SubscriberError::StartupFailed("test".into())));

    let mut working = MockSubscriber::new();
    working.expect_id().returning(|| "working");
    working.expect_start().times(1).returning(|_| Ok(()));

    coordinator.add_subscriber(Box::new(failing));
    coordinator.add_subscriber(Box::new(working));

    // Should not fail, just log error for failing subscriber
    let result = coordinator.start_all().await;
    assert!(result.is_ok());
}
```

### 3.3 BronzeSubscriber TDD Cycles

#### Cycle 1: Buffer Points

```rust
#[tokio::test]
async fn test_bronze_buffers_points() {
    let mock_store = Arc::new(MockRawStore::new());
    let mut subscriber = BronzeSubscriber::new(
        mock_store,
        BronzeConfig { batch_size: 10, ..Default::default() }
    );

    let point = Arc::new(RawDataPoint::new("test", json!({})));
    subscriber.buffer_point(point);

    assert_eq!(subscriber.buffer_len(), 1);
}
```

#### Cycle 2: Batch Flush

```rust
#[tokio::test]
async fn test_bronze_flushes_on_batch_size() {
    let mut mock_store = MockRawStore::new();
    mock_store.expect_write_raw_batch()
        .times(1)
        .withf(|points| points.len() == 3)
        .returning(|_| Ok(()));

    let mut subscriber = BronzeSubscriber::new(
        Arc::new(mock_store),
        BronzeConfig { batch_size: 3, ..Default::default() }
    );

    for _ in 0..3 {
        subscriber.buffer_point(Arc::new(RawDataPoint::new("test", json!({}))));
    }

    subscriber.maybe_flush().await.unwrap();
}
```

#### Cycle 3: Timeout Flush

```rust
#[tokio::test]
async fn test_bronze_flushes_on_timeout() {
    let mut mock_store = MockRawStore::new();
    mock_store.expect_write_raw_batch()
        .times(1)
        .returning(|_| Ok(()));

    let mut subscriber = BronzeSubscriber::new(
        Arc::new(mock_store),
        BronzeConfig { batch_size: 100, batch_timeout_secs: 1 }
    );

    subscriber.buffer_point(Arc::new(RawDataPoint::new("test", json!({}))));

    tokio::time::sleep(Duration::from_secs(2)).await;
    subscriber.flush_on_timeout().await.unwrap();
}
```

#### Cycle 4: Stream Filter

```rust
#[test]
fn test_bronze_filters_streams() {
    let subscriber = BronzeSubscriber::new(
        Arc::new(MockRawStore::new()),
        BronzeConfig {
            stream_filter: Some(vec!["air-quality".into()]),
            ..Default::default()
        }
    );

    assert!(subscriber.accepts_stream("air-quality"));
    assert!(!subscriber.accepts_stream("outdoor-weather"));
}
```

### 3.4 SilverSubscriber TDD Cycles

#### Cycle 1: Transform Single Point

```rust
#[test]
fn test_silver_transforms_point() {
    let config = SilverEtlConfig {
        field_mappings: vec![
            SilverFieldMapping {
                source_path: "raw_payload.pm02Compensated".into(),
                target_column: "pm25".into(),
                column_type: "double precision".into(),
                ..Default::default()
            }
        ],
        ..Default::default()
    };

    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 12.5}));
    let row = transform_to_silver(&point, &config).unwrap();

    assert_eq!(row.get("pm25"), Some(&SqlValue::DoublePrecision(12.5)));
}
```

#### Cycle 2: Catch-up from Bronze

```rust
#[tokio::test]
async fn test_silver_catches_up() {
    let mut mock_db = MockTimescaleDb::new();
    mock_db.expect_get_watermark()
        .returning(|_| Ok(Some(DateTime::parse_from_rfc3339("2026-01-17T00:00:00Z").unwrap().into())));

    let mut mock_bronze = MockBronzeReader::new();
    mock_bronze.expect_read_since()
        .returning(|_| Ok(vec![
            RawDataPoint::new("test", json!({"pm02Compensated": 12.5}))
        ]));

    mock_db.expect_upsert_batch()
        .times(1)
        .returning(|_, _| Ok(()));

    let subscriber = SilverSubscriber::new(
        Arc::new(mock_db),
        Arc::new(mock_bronze),
        config
    );

    subscriber.catch_up().await.unwrap();
}
```

#### Cycle 3: UPSERT Semantics

```rust
#[tokio::test]
async fn test_silver_upserts() {
    let mut mock_db = MockTimescaleDb::new();

    // Expect two calls, both succeed (UPSERT handles duplicates)
    mock_db.expect_upsert_batch()
        .times(2)
        .returning(|_, _| Ok(()));

    let subscriber = SilverSubscriber::new(Arc::new(mock_db), ...);

    let row = SilverRow { ... };
    subscriber.write_row(row.clone()).await.unwrap();
    subscriber.write_row(row).await.unwrap();  // Same row, should still succeed
}
```

#### Cycle 4: DQ Evaluation

```rust
#[test]
fn test_silver_evaluates_dq_rules() {
    let config = SilverEtlConfig {
        field_mappings: vec![
            SilverFieldMapping {
                source_path: "raw_payload.pm02Compensated".into(),
                target_column: "pm25".into(),
                dq_rules: vec![
                    DqRule::RangeCheck { min: 0.0, max: 1000.0, action: DqAction::Flag }
                ],
                ..Default::default()
            }
        ],
        ..Default::default()
    };

    // Value out of range
    let point = RawDataPoint::new("test", json!({"pm02Compensated": 1500.0}));
    let row = transform_to_silver(&point, &config).unwrap();

    let flags = row.dq_flags.unwrap();
    assert!(flags.iter().any(|f| f.contains("range_check")));
}
```

### 3.5 ThresholdProcessor TDD Cycles

#### Cycle 1: Rule Evaluation

```rust
#[tokio::test]
async fn test_threshold_evaluates_rule() {
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            name: "test_rule".into(),
            field: "raw_payload.value".into(),
            condition: "> 100".into(),
            severity: Severity::Warning,
            ..Default::default()
        }
    ]);

    let point = RawDataPoint::new("test", json!({"value": 150}));
    let outputs = processor.process(&point).await.unwrap();

    assert_eq!(outputs.len(), 1);
    match &outputs[0] {
        ProcessorOutput::Alert(alert) => {
            assert_eq!(alert.rule_name, "test_rule");
        }
        _ => panic!("Expected Alert"),
    }
}
```

#### Cycle 2: Cooldown

```rust
#[tokio::test]
async fn test_threshold_respects_cooldown() {
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            name: "test_rule".into(),
            cooldown_secs: 300,
            ..Default::default()
        }
    ]);

    let point = RawDataPoint::new("test", json!({"value": 150}));

    // First trigger - alert
    let outputs1 = processor.process(&point).await.unwrap();
    assert_eq!(outputs1.len(), 1);

    // Second trigger within cooldown - no alert
    let outputs2 = processor.process(&point).await.unwrap();
    assert_eq!(outputs2.len(), 0);
}
```

#### Cycle 3: Stream Filter

```rust
#[test]
fn test_threshold_filters_streams() {
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            stream_filter: Some(vec!["air-quality".into()]),
            ..Default::default()
        }
    ]);

    assert!(processor.accepts_stream("air-quality"));
    assert!(!processor.accepts_stream("outdoor-weather"));
}
```

### 3.6 EventNotifier TDD Cycles

#### Cycle 1: Publish Notification

```rust
#[tokio::test]
async fn test_notifier_publishes() {
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish()
        .times(1)
        .withf(|topic, _, _, _| topic == "ndp/events/air-quality")
        .returning(|_, _, _, _| Ok(()));

    let notifier = EventNotifier::new(Arc::new(mock_mqtt), true);

    let point = RawDataPoint::new("air-quality-Mqtt", json!({}))
        .with_ndp_id("sensor-001");

    notifier.notify(&point).await.unwrap();
}
```

#### Cycle 2: Minimal Payload

```rust
#[tokio::test]
async fn test_notifier_sends_minimal_payload() {
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish()
        .withf(|_, _, _, payload| {
            let json: Value = serde_json::from_slice(payload).unwrap();
            json.get("stream_id").is_some() &&
            json.get("ndp_id").is_some() &&
            json.get("timestamp").is_some() &&
            json.get("raw_payload").is_none()  // Must not include raw data
        })
        .returning(|_, _, _, _| Ok(()));

    let notifier = EventNotifier::new(Arc::new(mock_mqtt), true);
    notifier.notify(&RawDataPoint::new("test", json!({"secret": "data"}))).await.unwrap();
}
```

#### Cycle 3: Disabled No-op

```rust
#[tokio::test]
async fn test_notifier_disabled_noop() {
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish().never();  // Should never be called

    let notifier = EventNotifier::new(Arc::new(mock_mqtt), false);  // Disabled

    notifier.notify(&RawDataPoint::new("test", json!({}))).await.unwrap();
}
```

#### Cycle 4: Fire-and-Forget

```rust
#[tokio::test]
async fn test_notifier_ignores_errors() {
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish()
        .returning(|_, _, _, _| Err(MqttError::ConnectionFailed));

    let notifier = EventNotifier::new(Arc::new(mock_mqtt), true);

    // Should not propagate error
    let result = notifier.notify(&RawDataPoint::new("test", json!({}))).await;
    assert!(result.is_ok());
}
```

---

## 4. Integration Test Strategy

### 4.1 In-Memory Test Infrastructure

```rust
/// In-memory RawStore for integration tests
pub struct InMemoryRawStore {
    data: Arc<RwLock<Vec<RawDataPoint>>>,
}

impl InMemoryRawStore {
    pub fn new() -> Self {
        Self { data: Arc::new(RwLock::new(vec![])) }
    }

    pub async fn get_all(&self) -> Vec<RawDataPoint> {
        self.data.read().await.clone()
    }
}

#[async_trait]
impl RawStore for InMemoryRawStore {
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()> {
        self.data.write().await.extend(points);
        Ok(())
    }
    // ...
}

/// In-memory TimescaleDb for integration tests
pub struct InMemoryTimescaleDb {
    tables: Arc<RwLock<HashMap<String, Vec<SilverRow>>>>,
    watermarks: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}
```

### 4.2 End-to-End Test

```rust
#[tokio::test]
async fn test_end_to_end_data_flow() {
    // Setup in-memory infrastructure
    let bronze_store = Arc::new(InMemoryRawStore::new());
    let silver_db = Arc::new(InMemoryTimescaleDb::new());

    // Create event bus and coordinator
    let event_bus = Arc::new(EventBus::new(EventBusConfig::default()));
    let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

    // Add subscribers
    coordinator.add_subscriber(Box::new(BronzeSubscriber::new(bronze_store.clone())));
    coordinator.add_subscriber(Box::new(SilverSubscriber::new(silver_db.clone())));

    // Start processing
    coordinator.start_all().await.unwrap();

    // Publish data
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 12.5}))
        .with_ndp_id("sensor-001");
    event_bus.publish(point).unwrap();

    // Allow processing
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify Bronze
    let bronze_data = bronze_store.get_all().await;
    assert_eq!(bronze_data.len(), 1);

    // Verify Silver
    let silver_data = silver_db.query("air_quality_observations").await;
    assert_eq!(silver_data.len(), 1);

    // Cleanup
    coordinator.stop_all().await.unwrap();
}
```

---

## 5. Test Organization

### 5.1 Directory Structure

```
core/src/
├── event_bus/
│   ├── mod.rs              # EventBus struct + tests
│   └── tests/
│       ├── unit.rs         # Unit tests with mocks
│       └── integration.rs  # Integration tests
├── subscribers/
│   ├── mod.rs              # Trait + coordinator
│   ├── bronze.rs           # BronzeSubscriber + tests
│   ├── silver.rs           # SilverSubscriber + tests
│   ├── processor.rs        # ProcessorSubscriber + tests
│   ├── event_notifier.rs   # EventNotifier + tests
│   └── tests/
│       └── integration.rs
├── silver/
│   ├── mod.rs
│   ├── transform.rs        # Transform functions + tests
│   └── dq_evaluator.rs     # DQ evaluation + tests
├── processors/
│   ├── mod.rs              # Trait + registry
│   └── threshold.rs        # ThresholdProcessor + tests
└── outputs/
    ├── mod.rs              # Trait
    ├── mqtt.rs             # MqttOutputSink + tests
    └── timescale.rs        # TimescaleOutputSink + tests
```

### 5.2 Test Naming Convention

```rust
// Unit tests: test_{function}_{scenario}
#[test]
fn test_transform_to_silver_extracts_field() { }

#[test]
fn test_transform_to_silver_handles_missing_field() { }

// Behavior tests: test_{component}_{behavior}
#[tokio::test]
async fn test_bronze_subscriber_batches_writes() { }

#[tokio::test]
async fn test_silver_subscriber_catches_up_on_startup() { }

// Integration tests: test_{scenario}_end_to_end
#[tokio::test]
async fn test_data_flows_from_source_to_silver_end_to_end() { }
```

---

## 6. Coverage Goals

| Component | Line Coverage | Branch Coverage |
|-----------|---------------|-----------------|
| EventBus | > 90% | > 85% |
| Subscriber trait | > 95% | > 90% |
| BronzeSubscriber | > 90% | > 85% |
| SilverSubscriber | > 90% | > 85% |
| SilverTransform | > 95% | > 90% |
| ThresholdProcessor | > 90% | > 85% |
| EventNotifier | > 90% | > 85% |
| SubscriberCoordinator | > 85% | > 80% |

---

*TDD Strategy defined: 2026-01-18*
