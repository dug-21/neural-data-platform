# DP-012 SPARC Specification (SPARC-S)

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Phase**: Specification
**Created**: 2026-01-18
**Status**: In Progress

---

## 1. Executive Summary

This specification defines the requirements and acceptance criteria for transforming the Neural Data Platform's ingestion architecture from a single-consumer mpsc channel to a **multi-consumer broadcast event bus** with streaming subscribers.

### 1.1 Design Principle: Same Behavior, Faster

**CRITICAL**: This feature does NOT change what the system does - it changes how fast it does it.

| Aspect | Current | Target | Change |
|--------|---------|--------|--------|
| Bronze writes | Working correctly | Working correctly | No change |
| Silver writes | Working correctly (batch) | Working correctly (streaming) | Latency only |
| Configuration | Config-driven | Config-driven | Reuse existing |
| Data accuracy | Correct | Correct | No change |

The existing `silver_etl` configuration is **complete and working**. We are porting the transform concepts to Rust for streaming, not redesigning the transforms.

---

## 2. Functional Requirements

### 2.1 Event Bus (FR-EB)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-EB-001 | Event bus SHALL broadcast `Arc<RawDataPoint>` to multiple subscribers | P0 | Multiple subscribers receive same event |
| FR-EB-002 | Event bus SHALL use `tokio::broadcast` channel | P0 | In-process, zero-copy broadcasting |
| FR-EB-003 | Event bus SHALL be configurable via platform.yaml | P0 | `event_bus.capacity` configurable |
| FR-EB-004 | Event bus SHALL emit lag warnings when subscribers fall behind | P1 | Log warning when lagged > threshold |
| FR-EB-005 | Event bus SHALL continue operating when individual subscribers fail | P0 | Bus operates independently of subscriber health |

### 2.2 Subscriber Trait (FR-SUB)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-SUB-001 | Subscriber trait SHALL define `id()`, `start()`, `stop()`, `accepts_stream()` | P0 | Trait compiles and is implementable |
| FR-SUB-002 | Subscribers SHALL be independently startable/stoppable | P0 | One subscriber can restart without affecting others |
| FR-SUB-003 | Subscribers SHALL filter events by stream_id | P0 | Subscriber only processes configured streams |
| FR-SUB-004 | Subscribers SHALL handle broadcast lag gracefully | P0 | Log warning, continue processing |
| FR-SUB-005 | Subscriber coordinator SHALL manage all subscriber lifecycles | P0 | Start/stop all subscribers via coordinator |

### 2.3 Bronze Subscriber (FR-BRONZE)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-BRONZE-001 | Bronze subscriber SHALL write RawDataPoint to Parquet | P0 | Same output as current RawStorageWriter |
| FR-BRONZE-002 | Bronze subscriber SHALL batch writes (configurable size/timeout) | P0 | batch_size and batch_timeout_secs work |
| FR-BRONZE-003 | Bronze subscriber SHALL use existing ParquetStore | P0 | No changes to Bronze storage format |
| FR-BRONZE-004 | Bronze subscriber SHALL support WAL for durability | P0 | wal_enabled configuration respected |
| FR-BRONZE-005 | Bronze subscriber SHALL partition by day | P0 | Same partitioning as current |

### 2.4 Silver Subscriber (FR-SILVER)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-SILVER-001 | Silver subscriber SHALL transform RawDataPoint using existing SilverEtlConfig | P0 | Same field mappings, same output |
| FR-SILVER-002 | Silver subscriber SHALL catch up from Bronze on startup | P0 | No data loss on restart |
| FR-SILVER-003 | Silver subscriber SHALL UPSERT to TimescaleDB | P0 | Duplicates handled gracefully |
| FR-SILVER-004 | Silver subscriber SHALL evaluate DQ rules | P0 | dq_flags populated correctly |
| FR-SILVER-005 | Silver subscriber SHALL batch writes for efficiency | P1 | Configurable batch_size |
| FR-SILVER-006 | Silver subscriber data SHALL match batch ETL output | P0 | Byte-for-byte same results |

### 2.5 Threshold Processor (FR-THRESH)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-THRESH-001 | Threshold processor SHALL evaluate field conditions | P0 | > < = operators work |
| FR-THRESH-002 | Threshold processor SHALL support cooldown periods | P1 | No duplicate alerts within cooldown |
| FR-THRESH-003 | Threshold processor SHALL output to MQTT | P0 | Alerts published to topic |
| FR-THRESH-004 | Threshold processor SHALL output to TimescaleDB | P1 | Alerts stored in silver.alerts |
| FR-THRESH-005 | Threshold processor SHALL be config-driven | P0 | Rules defined in YAML |

### 2.6 Event Notifier (FR-NOTIFY)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-NOTIFY-001 | Event notifier SHALL publish to MQTT on each event | P0 | Message published |
| FR-NOTIFY-002 | Event notifier SHALL use QoS 0 (fire-and-forget) | P0 | Never blocks on ACK |
| FR-NOTIFY-003 | Event notifier SHALL be toggleable via env var | P0 | EVENT_NOTIFIER_ENABLED works |
| FR-NOTIFY-004 | Event notifier SHALL NOT include raw_payload | P0 | Only IDs and timestamp |
| FR-NOTIFY-005 | Event notifier failure SHALL NOT affect other subscribers | P0 | MQTT down doesn't block Bronze/Silver |

---

## 3. Non-Functional Requirements

### 3.1 Latency (NFR-LAT)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-LAT-001 | Event bus broadcast latency | < 1ms | Timestamp delta source → subscriber receive |
| NFR-LAT-002 | Bronze write latency (from event) | < 2 seconds | Event timestamp → file write |
| NFR-LAT-003 | Silver write latency (from event) | < 5 seconds | Event timestamp → DB commit |
| NFR-LAT-004 | Threshold alert latency | < 100ms | Event → MQTT publish |
| NFR-LAT-005 | Event notifier latency | < 10ms | Event → MQTT publish |

### 3.2 Throughput (NFR-THR)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-THR-001 | Event bus throughput | > 1,000 events/sec | Sustained load test |
| NFR-THR-002 | Bronze write throughput | > 500 events/sec | Points written to Parquet |
| NFR-THR-003 | Silver write throughput | > 500 events/sec | Points inserted to TimescaleDB |

### 3.3 Reliability (NFR-REL)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-REL-001 | Bronze data durability | 100% (WAL) | No data loss on crash |
| NFR-REL-002 | Silver recovery on restart | 100% catch-up | Verify watermark after restart |
| NFR-REL-003 | Subscriber isolation | Full | Kill one, others continue |
| NFR-REL-004 | Graceful degradation | Continue on lag | Log warning, don't crash |

### 3.4 Resource Usage (NFR-RES)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-RES-001 | Memory overhead | < 100MB above baseline | Monitor during load |
| NFR-RES-002 | CPU usage | < 50% additional | Monitor during load |
| NFR-RES-003 | Event bus buffer | 10,000 events | Configurable capacity |

---

## 4. Interface Contracts

### 4.1 Core Traits

#### 4.1.1 Subscriber Trait

```rust
/// Subscriber trait for event bus consumers
///
/// # Design Principles
/// - Subscribers are independent: one failure doesn't affect others
/// - Config-driven: enable/disable via YAML
/// - Stream filtering: only process configured streams
#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Unique identifier for this subscriber
    fn id(&self) -> &str;

    /// Start consuming from the event bus
    ///
    /// This method should:
    /// 1. Perform any startup catch-up (e.g., Silver reads Bronze)
    /// 2. Enter receive loop for broadcast channel
    /// 3. Handle lag gracefully (log warning, continue)
    async fn start(
        &mut self,
        receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError>;

    /// Stop consuming gracefully
    ///
    /// Should flush any buffered data before returning
    async fn stop(&mut self) -> Result<(), SubscriberError>;

    /// Check if this subscriber processes a given stream
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Health check for monitoring
    async fn health_check(&self) -> HealthStatus;
}
```

#### 4.1.2 Processor Trait

```rust
/// Processor trait for real-time data processing
///
/// Processors receive events and produce outputs (alerts, metrics, etc.)
/// They do NOT persist data - that's the subscriber's job
#[async_trait]
pub trait Processor: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Process a single data point
    ///
    /// Returns optional outputs (alerts, metrics, etc.)
    async fn process(
        &self,
        point: &RawDataPoint
    ) -> Result<Vec<ProcessorOutput>, ProcessorError>;

    /// Check if this processor handles a given stream
    fn accepts_stream(&self, stream_id: &str) -> bool;
}

/// Output from a processor
pub enum ProcessorOutput {
    Alert(Alert),
    Metric(Metric),
    Event(Event),
}
```

#### 4.1.3 OutputSink Trait

```rust
/// Output sink for processor results
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Write an output to this sink
    async fn write(&self, output: ProcessorOutput) -> Result<(), OutputError>;

    /// Flush any buffered outputs
    async fn flush(&self) -> Result<(), OutputError>;
}
```

### 4.2 Event Bus API

```rust
/// Event bus for broadcasting data to subscribers
pub struct EventBus {
    sender: broadcast::Sender<Arc<RawDataPoint>>,
    capacity: usize,
    lag_threshold: usize,
}

impl EventBus {
    /// Create a new event bus with given capacity
    pub fn new(capacity: usize) -> Self;

    /// Publish a data point to all subscribers
    ///
    /// Wraps in Arc for zero-copy broadcast
    pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError>;

    /// Subscribe to the event bus
    ///
    /// Returns a receiver that will get all published events
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<RawDataPoint>>;

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize;
}
```

### 4.3 Configuration Schema

```rust
/// Platform-level event bus configuration
#[derive(Debug, Clone, Deserialize)]
pub struct EventBusConfig {
    /// Broadcast channel capacity (default: 10000)
    pub capacity: usize,

    /// Lag threshold for warnings (default: 1000)
    pub lag_warning_threshold: usize,
}

/// Subscriber configuration
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriberConfig {
    /// Unique identifier
    pub id: String,

    /// Subscriber type (storage, timescale, processor, notifier)
    #[serde(rename = "type")]
    pub subscriber_type: SubscriberType,

    /// Enable/disable this subscriber
    pub enabled: bool,

    /// Type-specific configuration
    pub config: serde_json::Value,

    /// Optional processor reference (for processor type)
    pub processor_id: Option<String>,
}
```

---

## 5. London TDD Test Scenarios

### 5.1 Event Bus Tests

```rust
// ========== EVENT BUS BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_event_bus_broadcasts_to_multiple_subscribers() {
    // GIVEN an event bus with 2 subscribers
    let event_bus = EventBus::new(100);
    let mut rx1 = event_bus.subscribe();
    let mut rx2 = event_bus.subscribe();

    // WHEN a point is published
    let point = RawDataPoint::new("test-Http", json!({"value": 42}));
    event_bus.publish(point.clone()).unwrap();

    // THEN both subscribers receive it
    let received1 = rx1.recv().await.unwrap();
    let received2 = rx2.recv().await.unwrap();

    assert_eq!(received1.source_id, "test-Http");
    assert_eq!(received2.source_id, "test-Http");
}

#[tokio::test]
async fn test_event_bus_handles_slow_subscriber_gracefully() {
    // GIVEN an event bus with small capacity
    let event_bus = EventBus::new(5);
    let mut slow_rx = event_bus.subscribe();

    // WHEN many points are published faster than consumed
    for i in 0..10 {
        let point = RawDataPoint::new("test-Http", json!({"i": i}));
        event_bus.publish(point).unwrap();
    }

    // THEN slow subscriber gets Lagged error but bus continues
    match slow_rx.recv().await {
        Err(broadcast::error::RecvError::Lagged(n)) => {
            assert!(n > 0); // Some messages were missed
        }
        _ => {} // First few messages may succeed
    }
}

#[tokio::test]
async fn test_event_bus_continues_when_subscriber_drops() {
    // GIVEN an event bus with a subscriber
    let event_bus = EventBus::new(100);
    let rx = event_bus.subscribe();

    // WHEN subscriber is dropped
    drop(rx);

    // THEN publishing still works
    let point = RawDataPoint::new("test-Http", json!({"value": 1}));
    let result = event_bus.publish(point);
    assert!(result.is_ok());
}
```

### 5.2 Subscriber Trait Tests

```rust
// ========== SUBSCRIBER MOCK DEFINITIONS ==========

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
    }
}

// ========== SUBSCRIBER BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_subscriber_filters_by_stream() {
    // GIVEN a subscriber that only accepts "air-quality"
    let mut mock = MockSubscriber::new();
    mock.expect_accepts_stream()
        .with(eq("air-quality"))
        .returning(|_| true);
    mock.expect_accepts_stream()
        .with(eq("outdoor-weather"))
        .returning(|_| false);

    // THEN it accepts air-quality
    assert!(mock.accepts_stream("air-quality"));

    // AND rejects outdoor-weather
    assert!(!mock.accepts_stream("outdoor-weather"));
}

#[tokio::test]
async fn test_subscriber_coordinator_starts_all() {
    // GIVEN a coordinator with multiple subscribers
    let mut coordinator = SubscriberCoordinator::new(event_bus);

    let mut sub1 = MockSubscriber::new();
    sub1.expect_id().returning(|| "bronze");
    sub1.expect_start().times(1).returning(|_| Ok(()));

    let mut sub2 = MockSubscriber::new();
    sub2.expect_id().returning(|| "silver");
    sub2.expect_start().times(1).returning(|_| Ok(()));

    coordinator.add_subscriber(Box::new(sub1));
    coordinator.add_subscriber(Box::new(sub2));

    // WHEN start_all is called
    let result = coordinator.start_all().await;

    // THEN both subscribers are started
    assert!(result.is_ok());
}
```

### 5.3 Bronze Subscriber Tests

```rust
// ========== BRONZE SUBSCRIBER BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_bronze_subscriber_batches_writes() {
    // GIVEN a bronze subscriber with batch_size=3
    let mut mock_store = MockRawStore::new();
    mock_store.expect_write_raw_batch()
        .times(1)
        .withf(|points| points.len() == 3)
        .returning(|_| Ok(()));

    let mut subscriber = BronzeSubscriber::new(
        Arc::new(mock_store),
        BronzeConfig { batch_size: 3, batch_timeout_secs: 60 }
    );

    // WHEN 3 points are received
    for i in 0..3 {
        subscriber.buffer_point(Arc::new(
            RawDataPoint::new("test-Http", json!({"i": i}))
        ));
    }

    // THEN batch write is triggered
    subscriber.flush().await.unwrap();
}

#[tokio::test]
async fn test_bronze_subscriber_flushes_on_timeout() {
    // GIVEN a bronze subscriber with batch_timeout_secs=1
    let mut mock_store = MockRawStore::new();
    mock_store.expect_write_raw_batch()
        .times(1)
        .returning(|_| Ok(()));

    let subscriber = BronzeSubscriber::new(
        Arc::new(mock_store),
        BronzeConfig { batch_size: 100, batch_timeout_secs: 1 }
    );

    // WHEN 1 point is received and timeout passes
    subscriber.buffer_point(Arc::new(
        RawDataPoint::new("test-Http", json!({"value": 1}))
    ));

    tokio::time::sleep(Duration::from_secs(2)).await;

    // THEN flush is triggered by timeout
    // (verified by mock expectation)
}
```

### 5.4 Silver Subscriber Tests

```rust
// ========== SILVER SUBSCRIBER BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_silver_subscriber_catches_up_on_startup() {
    // GIVEN Silver watermark is behind Bronze
    let mut mock_db = MockTimescaleDb::new();
    mock_db.expect_get_watermark()
        .returning(|| Ok(Some(DateTime::parse_from_rfc3339("2026-01-17T00:00:00Z").unwrap())));

    let mut mock_bronze = MockBronzeReader::new();
    mock_bronze.expect_read_since()
        .with(eq(DateTime::parse_from_rfc3339("2026-01-17T00:00:00Z").unwrap()))
        .returning(|_| Ok(vec![
            RawDataPoint::new("test-Http", json!({"pm25": 12.5}))
        ]));

    mock_db.expect_upsert_batch()
        .times(1)
        .returning(|_| Ok(()));

    // WHEN subscriber starts
    let subscriber = SilverSubscriber::new(mock_db, mock_bronze, config);
    subscriber.catch_up().await.unwrap();

    // THEN it reads Bronze since watermark and upserts to Silver
    // (verified by mock expectations)
}

#[tokio::test]
async fn test_silver_subscriber_applies_field_mappings() {
    // GIVEN a SilverEtlConfig with pm02Compensated -> pm25 mapping
    let config = SilverEtlConfig {
        field_mappings: vec![
            SilverFieldMapping {
                source_path: "raw_payload.pm02Compensated".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double precision".to_string(),
                ..Default::default()
            }
        ],
        ..Default::default()
    };

    // WHEN transforming a RawDataPoint
    let point = RawDataPoint::new("air-quality-Mqtt", json!({
        "pm02Compensated": 12.5,
        "rco2": 450
    }));

    let result = transform_to_silver(&point, &config).unwrap();

    // THEN pm25 field is populated correctly
    assert_eq!(result.get("pm25"), Some(&Value::from(12.5)));
}

#[tokio::test]
async fn test_silver_subscriber_upsert_handles_duplicates() {
    // GIVEN Silver already has a row with this key
    let mut mock_db = MockTimescaleDb::new();
    mock_db.expect_upsert_batch()
        .times(2)  // Called twice with same data
        .returning(|_| Ok(()));  // Both succeed

    // WHEN same data is inserted twice
    let row = SilverRow { observation_time: now, ndp_id: "sensor-1".into(), pm25: 12.5 };
    mock_db.upsert_batch(vec![row.clone()]).await.unwrap();
    mock_db.upsert_batch(vec![row]).await.unwrap();

    // THEN no error (UPSERT semantics)
    // Verified by mock returning Ok(()) twice
}

#[tokio::test]
async fn test_silver_subscriber_evaluates_dq_rules() {
    // GIVEN a config with range check on pm25
    let config = SilverEtlConfig {
        field_mappings: vec![
            SilverFieldMapping {
                source_path: "raw_payload.pm02Compensated".to_string(),
                target_column: "pm25".to_string(),
                dq_rules: vec![
                    DqRule::RangeCheck { min: 0.0, max: 1000.0, action: DqAction::Flag }
                ],
                ..Default::default()
            }
        ],
        ..Default::default()
    };

    // WHEN value is out of range
    let point = RawDataPoint::new("test-Mqtt", json!({"pm02Compensated": 1500.0}));
    let result = transform_to_silver(&point, &config).unwrap();

    // THEN dq_flags contains the violation
    let flags = result.get("dq_flags").unwrap();
    assert!(flags.to_string().contains("range_check"));
}
```

### 5.5 Threshold Processor Tests

```rust
// ========== THRESHOLD PROCESSOR BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_threshold_processor_triggers_alert() {
    // GIVEN a threshold rule for pm25 > 35.4
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            name: "pm25_unhealthy".to_string(),
            field: "raw_payload.pm02Compensated".to_string(),
            condition: "> 35.4".to_string(),
            severity: Severity::Warning,
            ..Default::default()
        }
    ]);

    // WHEN pm25 exceeds threshold
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 40.0}));
    let outputs = processor.process(&point).await.unwrap();

    // THEN alert is generated
    assert_eq!(outputs.len(), 1);
    match &outputs[0] {
        ProcessorOutput::Alert(alert) => {
            assert_eq!(alert.rule_name, "pm25_unhealthy");
            assert_eq!(alert.severity, Severity::Warning);
        }
        _ => panic!("Expected Alert output"),
    }
}

#[tokio::test]
async fn test_threshold_processor_respects_cooldown() {
    // GIVEN a threshold with 300s cooldown
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            name: "pm25_unhealthy".to_string(),
            field: "raw_payload.pm02Compensated".to_string(),
            condition: "> 35.4".to_string(),
            cooldown_secs: 300,
            ..Default::default()
        }
    ]);

    // WHEN threshold is exceeded twice within cooldown
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 40.0}));
    let outputs1 = processor.process(&point).await.unwrap();
    let outputs2 = processor.process(&point).await.unwrap();

    // THEN only first alert is generated
    assert_eq!(outputs1.len(), 1);
    assert_eq!(outputs2.len(), 0);  // Suppressed by cooldown
}

#[tokio::test]
async fn test_threshold_processor_filters_by_stream() {
    // GIVEN a threshold that only applies to air-quality
    let processor = ThresholdProcessor::new(vec![
        ThresholdRule {
            stream_filter: Some(vec!["air-quality".to_string()]),
            ..Default::default()
        }
    ]);

    // THEN it accepts air-quality
    assert!(processor.accepts_stream("air-quality"));

    // AND rejects outdoor-weather
    assert!(!processor.accepts_stream("outdoor-weather"));
}
```

### 5.6 Event Notifier Tests

```rust
// ========== EVENT NOTIFIER BEHAVIOR TESTS ==========

#[tokio::test]
async fn test_event_notifier_publishes_minimal_payload() {
    // GIVEN an enabled event notifier
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish()
        .withf(|topic, qos, _, payload| {
            // Verify payload structure
            let json: Value = serde_json::from_slice(payload).unwrap();
            json.get("stream_id").is_some() &&
            json.get("ndp_id").is_some() &&
            json.get("timestamp").is_some() &&
            json.get("raw_payload").is_none()  // CRITICAL: no raw data
        })
        .returning(|_, _, _, _| Ok(()));

    let notifier = EventNotifier::new(mock_mqtt, true);

    // WHEN a point is processed
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm25": 12.5}))
        .with_ndp_id("sensor-001");
    notifier.notify(&point).await.unwrap();

    // THEN minimal payload is published
    // (verified by mock expectation)
}

#[tokio::test]
async fn test_event_notifier_disabled_noop() {
    // GIVEN a disabled event notifier
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish().never();  // Should never be called

    let notifier = EventNotifier::new(mock_mqtt, false);

    // WHEN a point is processed
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm25": 12.5}));
    let result = notifier.notify(&point).await;

    // THEN nothing is published and no error
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_event_notifier_continues_on_mqtt_failure() {
    // GIVEN MQTT is unavailable
    let mut mock_mqtt = MockMqttClient::new();
    mock_mqtt.expect_try_publish()
        .returning(|_, _, _, _| Err(MqttError::ConnectionFailed));

    let notifier = EventNotifier::new(mock_mqtt, true);

    // WHEN a point is processed
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm25": 12.5}));
    let result = notifier.notify(&point).await;

    // THEN no error propagates (fire-and-forget)
    assert!(result.is_ok());
}
```

### 5.7 Integration Tests

```rust
// ========== END-TO-END INTEGRATION TESTS ==========

#[tokio::test]
async fn test_event_flows_from_source_to_bronze_and_silver() {
    // GIVEN complete pipeline with event bus
    let event_bus = EventBus::new(100);
    let bronze_store = Arc::new(InMemoryRawStore::new());
    let silver_db = Arc::new(InMemoryTimescaleDb::new());

    let mut coordinator = SubscriberCoordinator::new(event_bus.clone());
    coordinator.add_subscriber(Box::new(BronzeSubscriber::new(bronze_store.clone())));
    coordinator.add_subscriber(Box::new(SilverSubscriber::new(silver_db.clone())));
    coordinator.start_all().await.unwrap();

    // WHEN a point is published
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 12.5}))
        .with_ndp_id("sensor-001");
    event_bus.publish(point).unwrap();

    // Allow processing time
    tokio::time::sleep(Duration::from_millis(100)).await;

    // THEN Bronze has the raw data
    let bronze_data = bronze_store.query_raw(now - 1min, now, None).await.unwrap();
    assert_eq!(bronze_data.len(), 1);

    // AND Silver has the transformed data
    let silver_data = silver_db.query("SELECT * FROM silver.air_quality_observations").await.unwrap();
    assert_eq!(silver_data.len(), 1);
    assert_eq!(silver_data[0].pm25, 12.5);
}

#[tokio::test]
async fn test_silver_catches_up_after_restart() {
    // GIVEN Bronze has data that Silver doesn't
    let bronze_store = Arc::new(InMemoryRawStore::new());
    bronze_store.write_raw(
        RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 15.0}))
            .with_timestamp(DateTime::parse_from_rfc3339("2026-01-17T12:00:00Z").unwrap())
    ).await.unwrap();

    let silver_db = Arc::new(InMemoryTimescaleDb::new());
    // Silver watermark is at 2026-01-17T00:00:00Z (behind Bronze)

    // WHEN Silver subscriber starts
    let subscriber = SilverSubscriber::new(silver_db.clone(), bronze_store.clone());
    subscriber.catch_up().await.unwrap();

    // THEN Silver catches up from Bronze
    let silver_data = silver_db.query("SELECT * FROM silver.air_quality_observations").await.unwrap();
    assert_eq!(silver_data.len(), 1);
    assert_eq!(silver_data[0].pm25, 15.0);
}

#[tokio::test]
async fn test_subscriber_failure_does_not_affect_others() {
    // GIVEN multiple subscribers, one configured to fail
    let event_bus = EventBus::new(100);
    let bronze_store = Arc::new(InMemoryRawStore::new());

    let mut failing_subscriber = MockSubscriber::new();
    failing_subscriber.expect_start()
        .returning(|_| Err(SubscriberError::StartupFailed("test failure".into())));

    let mut coordinator = SubscriberCoordinator::new(event_bus.clone());
    coordinator.add_subscriber(Box::new(BronzeSubscriber::new(bronze_store.clone())));
    coordinator.add_subscriber(Box::new(failing_subscriber));

    // WHEN coordinator starts (with failures)
    // Note: coordinator may log error but should continue
    let _ = coordinator.start_all().await;

    // WHEN a point is published
    let point = RawDataPoint::new("air-quality-Mqtt", json!({"value": 1}));
    event_bus.publish(point).unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // THEN Bronze still receives the data
    let bronze_data = bronze_store.query_raw(now - 1min, now, None).await.unwrap();
    assert_eq!(bronze_data.len(), 1);
}
```

---

## 6. Component Specifications

### 6.1 EventBus (core/src/event_bus/mod.rs)

| Aspect | Specification |
|--------|---------------|
| Channel Type | `tokio::broadcast` |
| Message Type | `Arc<RawDataPoint>` |
| Default Capacity | 10,000 |
| Lag Warning Threshold | 1,000 |
| Error Handling | Log warning on lag, continue |

### 6.2 SubscriberCoordinator (core/src/subscribers/coordinator.rs)

| Aspect | Specification |
|--------|---------------|
| Subscriber Storage | `HashMap<String, Box<dyn Subscriber>>` |
| Handle Storage | `HashMap<String, JoinHandle<()>>` |
| Lifecycle | Start all, stop all, add/remove individual |
| Error Handling | Continue on subscriber failure, log error |

### 6.3 BronzeSubscriber (core/src/subscribers/bronze.rs)

| Aspect | Specification |
|--------|---------------|
| Storage | Reuse existing `ParquetStore` |
| Batching | Configurable size (default 50) and timeout (default 2s) |
| WAL | Configurable (default enabled) |
| Partitioning | Daily |
| Stream Filter | Optional HashSet |

### 6.4 SilverSubscriber (core/src/subscribers/silver.rs)

| Aspect | Specification |
|--------|---------------|
| Database | TimescaleDB via tokio-postgres |
| Config | Reuse existing `SilverEtlConfig` from stream configs |
| Transform | Port logic from silver-etl to Rust functions |
| Batching | Configurable (default 100, 5s timeout) |
| Startup | Catch-up from Bronze since watermark |
| Write Mode | UPSERT (ON CONFLICT DO UPDATE) |

### 6.5 SilverTransform (core/src/silver/transform.rs)

| Aspect | Specification |
|--------|---------------|
| Input | `RawDataPoint` + `SilverEtlConfig` |
| Output | `SilverRow` (columnar representation) |
| Field Extraction | JSON path extraction from raw_payload |
| Type Casting | Map PG types to Rust types |
| DQ Evaluation | Evaluate rules, populate dq_flags |

### 6.6 ThresholdProcessor (core/src/processors/threshold.rs)

| Aspect | Specification |
|--------|---------------|
| Config | YAML-driven rules |
| Conditions | > < >= <= = != |
| Field Access | JSON path extraction |
| Cooldown | Per-rule, in-memory tracking |
| Outputs | Alert struct |

### 6.7 EventNotifier (core/src/subscribers/event_notifier.rs)

| Aspect | Specification |
|--------|---------------|
| Client | rumqttc AsyncClient |
| QoS | 0 (at-most-once) |
| Topic | `ndp/events/{stream_id}` |
| Payload | `{ stream_id, ndp_id, timestamp }` |
| Toggle | `EVENT_NOTIFIER_ENABLED` env var |

---

## 7. Data Flow Specification

### 7.1 Current Flow (Before DP-012)

```
┌─────────────┐       ┌────────────────────┐       ┌─────────────────┐
│   Sources   │──────▶│   mpsc channel     │──────▶│ RawStorageWriter│
│ (MQTT, HTTP)│       │   (1000 capacity)  │       │   (batches)     │
└─────────────┘       └────────────────────┘       └────────┬────────┘
                                                            │
                                                            ▼
                                                   ┌─────────────────┐
                                                   │  Bronze Layer   │
                                                   │  (Parquet)      │
                                                   └────────┬────────┘
                                                            │
                                            (5-minute batch ETL)
                                                            │
                                                            ▼
                                                   ┌─────────────────┐
                                                   │  Silver Layer   │
                                                   │ (TimescaleDB)   │
                                                   └─────────────────┘
```

### 7.2 Target Flow (After DP-012)

```
┌─────────────┐       ┌────────────────────────────────────────────────┐
│   Sources   │──────▶│              EVENT BUS                         │
│ (MQTT, HTTP)│       │     (tokio::broadcast, 10000 capacity)         │
└─────────────┘       └─────────┬──────────┬──────────┬───────────────┘
                                │          │          │          │
                                ▼          ▼          ▼          ▼
                         ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
                         │  Bronze  │ │  Silver  │ │Threshold │ │  Event   │
                         │Subscriber│ │Subscriber│ │Processor │ │ Notifier │
                         └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘
                              │            │            │            │
                              ▼            ▼            ▼            ▼
                        ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐
                        │ Parquet  │ │TimescaleDB│ │  MQTT    │ │  MQTT    │
                        │ (Bronze) │ │ (Silver)  │ │ (Alerts) │ │ (Events) │
                        └──────────┘ └──────────┘ └──────────┘ └──────────┘
```

### 7.3 Message Sequence

```
1. Source receives data (MQTT message, HTTP response)
2. Source creates RawDataPoint
3. Source publishes to EventBus
4. EventBus broadcasts Arc<RawDataPoint> to all subscribers
5. Each subscriber receives independently:
   a. Bronze: buffers, batches, writes Parquet
   b. Silver: transforms, buffers, UPSERTS to TimescaleDB
   c. Threshold: evaluates rules, publishes alerts
   d. EventNotifier: publishes notification to MQTT
6. Subscribers acknowledge internally (no bus acknowledgment)
```

---

## 8. Migration Specification

### 8.1 Phase 1: Event Bus Foundation

**Goal**: Replace mpsc with broadcast, Bronze as first subscriber

| Step | Action | Validation |
|------|--------|------------|
| 1.1 | Create EventBus struct | Unit tests pass |
| 1.2 | Create Subscriber trait | Unit tests pass |
| 1.3 | Create SubscriberCoordinator | Unit tests pass |
| 1.4 | Create BronzeSubscriber (refactor RawStorageWriter) | Unit tests pass |
| 1.5 | Wire into air-quality-app | Bronze works as before |
| 1.6 | Validate Bronze output unchanged | Compare Parquet files |

**Rollback**: Revert to mpsc channel if issues found.

### 8.2 Phase 2: Streaming Silver

**Goal**: Silver as streaming subscriber with < 5s latency

| Step | Action | Validation |
|------|--------|------------|
| 2.1 | Create core/src/silver/transform.rs | Unit tests pass |
| 2.2 | Create core/src/silver/dq_evaluator.rs | Unit tests pass |
| 2.3 | Create SilverSubscriber | Unit tests pass |
| 2.4 | Implement startup catch-up | Integration test: restart recovers |
| 2.5 | Wire into air-quality-app | Silver receives streaming data |
| 2.6 | Validate Silver output matches batch ETL | Compare query results |

**Rollback**: Keep batch ETL running in parallel, disable streaming.

### 8.3 Phase 3: Processor Framework

**Goal**: Config-driven threshold alerts + Event Notifier

| Step | Action | Validation |
|------|--------|------------|
| 3.1 | Create Processor trait | Unit tests pass |
| 3.2 | Create ThresholdProcessor | Unit tests pass |
| 3.3 | Create ProcessorSubscriber wrapper | Unit tests pass |
| 3.4 | Create MQTT output sink | Unit tests pass |
| 3.5 | Create EventNotifier subscriber | Unit tests pass |
| 3.6 | Wire into air-quality-app | Alerts fire correctly |

**Rollback**: Disable processor subscribers.

### 8.4 Phase 4: Polish

**Goal**: Production readiness

| Step | Action | Validation |
|------|--------|------------|
| 4.1 | silver-etl backfill mode | Manual backfill works |
| 4.2 | Grafana dashboard | Metrics visible |
| 4.3 | MCP tools | Tools work |
| 4.4 | Documentation | Docs complete |
| 4.5 | Performance testing | Meets NFR targets |

---

## 9. Appendix

### 9.1 Existing Assets to Reuse

| Asset | Location | Reuse Strategy |
|-------|----------|----------------|
| `RawDataPoint` | `core/src/types/raw_data_point.rs` | Use as-is |
| `SilverEtlConfig` | `core/src/config/silver_etl.rs` | Use as-is |
| `SilverFieldMapping` | `core/src/config/silver_etl.rs` | Use as-is |
| `DqRule` | `core/src/config/silver_etl.rs` | Use as-is |
| `ParquetStore` | `core/src/storage/parquet.rs` | Use as-is |
| Stream configs | `config/base/streams/*/config.yaml` | Use as-is |
| SqlGenerator concepts | `apps/silver-etl/src/sql_gen.rs` | Port to Rust |
| DQ evaluation concepts | `apps/silver-etl/src/dq.rs` | Port to Rust |

### 9.2 New Modules to Create

| Module | Location | Purpose |
|--------|----------|---------|
| event_bus | `core/src/event_bus/` | Broadcast channel wrapper |
| subscribers | `core/src/subscribers/` | Subscriber trait + implementations |
| silver | `core/src/silver/` | Streaming transform logic |
| processors | `core/src/processors/` | Processor trait + implementations |
| outputs | `core/src/outputs/` | Output sink implementations |

### 9.3 Configuration References

- Platform config: `config/base/platform.yaml` (NEW)
- Stream configs: `config/base/streams/*/config.yaml` (EXISTING)
- Processor configs: `config/base/processors/*.yaml` (NEW)

---

*Specification created: 2026-01-18*
*Next phase: SPARC-P (Pseudocode)*
