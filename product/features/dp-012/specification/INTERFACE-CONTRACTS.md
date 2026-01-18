# DP-012 Interface Contracts

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Document**: Interface Contracts and API Specifications
**Created**: 2026-01-18

---

## 1. Event Bus Interface

### 1.1 EventBus Struct

```rust
/// Event bus for broadcasting RawDataPoint to multiple subscribers
///
/// # Thread Safety
/// - `Sender` is `Clone + Send + Sync`
/// - Safe to share across tokio tasks
///
/// # Memory Model
/// - Events are wrapped in `Arc` for zero-copy broadcasting
/// - Each subscriber gets the same `Arc<RawDataPoint>`
pub struct EventBus {
    sender: broadcast::Sender<Arc<RawDataPoint>>,
    config: EventBusConfig,
    metrics: EventBusMetrics,
}

impl EventBus {
    /// Create a new event bus
    ///
    /// # Arguments
    /// * `config` - Event bus configuration
    ///
    /// # Returns
    /// New EventBus instance
    pub fn new(config: EventBusConfig) -> Self;

    /// Publish a data point to all subscribers
    ///
    /// # Arguments
    /// * `point` - RawDataPoint to broadcast
    ///
    /// # Returns
    /// * `Ok(())` - Point was broadcast successfully
    /// * `Err(EventBusError::NoReceivers)` - No subscribers connected
    ///
    /// # Performance
    /// - O(1) for publish operation
    /// - Creates Arc wrapper (~50 bytes overhead)
    pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError>;

    /// Subscribe to the event bus
    ///
    /// # Returns
    /// Broadcast receiver that will receive all future events
    ///
    /// # Notes
    /// - New subscribers don't receive historical events
    /// - Subscribers that fall behind will get `Lagged` error
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<RawDataPoint>>;

    /// Get current number of active subscribers
    pub fn subscriber_count(&self) -> usize;

    /// Get metrics for monitoring
    pub fn metrics(&self) -> &EventBusMetrics;
}
```

### 1.2 EventBusConfig

```rust
/// Event bus configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventBusConfig {
    /// Broadcast channel capacity (default: 10000)
    #[serde(default = "default_capacity")]
    pub capacity: usize,

    /// Lag threshold for warnings (default: 1000)
    #[serde(default = "default_lag_threshold")]
    pub lag_warning_threshold: usize,
}

fn default_capacity() -> usize { 10000 }
fn default_lag_threshold() -> usize { 1000 }
```

### 1.3 EventBusError

```rust
/// Event bus errors
#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("No receivers connected to event bus")]
    NoReceivers,

    #[error("Event bus channel closed")]
    ChannelClosed,

    #[error("Internal error: {0}")]
    Internal(String),
}
```

---

## 2. Subscriber Interface

### 2.1 Subscriber Trait

```rust
/// Core trait for event bus subscribers
///
/// # Lifecycle
/// 1. Create subscriber with configuration
/// 2. Add to SubscriberCoordinator
/// 3. Coordinator calls `start()` with broadcast receiver
/// 4. Subscriber processes events until `stop()` called
/// 5. On `stop()`, flush buffers and cleanup
///
/// # Error Handling
/// - Subscribers should handle errors internally
/// - Log errors but continue processing
/// - Propagate fatal errors via Result
#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Unique identifier for this subscriber
    ///
    /// Used for:
    /// - Configuration lookup
    /// - Metrics labeling
    /// - Logging context
    fn id(&self) -> &str;

    /// Start consuming from the event bus
    ///
    /// # Arguments
    /// * `receiver` - Broadcast receiver for events
    ///
    /// # Implementation Notes
    /// 1. If subscriber needs catch-up, do it before entering loop
    /// 2. Use `tokio::select!` for timeout-based flushing
    /// 3. Handle `RecvError::Lagged` by logging and continuing
    /// 4. Exit loop on `RecvError::Closed`
    ///
    /// # Example Implementation
    /// ```rust
    /// async fn start(&mut self, mut receiver: Receiver<Arc<RawDataPoint>>) -> Result<...> {
    ///     self.catch_up().await?;
    ///     loop {
    ///         tokio::select! {
    ///             result = receiver.recv() => {
    ///                 match result {
    ///                     Ok(point) => self.handle(point).await?,
    ///                     Err(RecvError::Lagged(n)) => warn!("Lagged {n}"),
    ///                     Err(RecvError::Closed) => break,
    ///                 }
    ///             }
    ///             _ = flush_interval.tick() => self.flush().await?,
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn start(
        &mut self,
        receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError>;

    /// Stop consuming gracefully
    ///
    /// # Implementation Notes
    /// 1. Signal internal tasks to stop
    /// 2. Flush any buffered data
    /// 3. Close connections/resources
    async fn stop(&mut self) -> Result<(), SubscriberError>;

    /// Check if this subscriber processes a given stream
    ///
    /// # Arguments
    /// * `stream_id` - Stream identifier from RawDataPoint.source_id
    ///
    /// # Returns
    /// * `true` - Process this stream
    /// * `false` - Skip this stream
    ///
    /// # Default Behavior
    /// If no stream filter configured, returns true for all streams
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Health check for monitoring
    ///
    /// # Returns
    /// HealthStatus indicating subscriber state
    async fn health_check(&self) -> HealthStatus;

    /// Reconfigure subscriber (hot reload)
    ///
    /// # Default Implementation
    /// Returns error indicating hot reload not supported
    async fn reconfigure(&mut self, config: serde_json::Value) -> Result<(), SubscriberError> {
        Err(SubscriberError::HotReloadNotSupported)
    }
}
```

### 2.2 SubscriberError

```rust
/// Subscriber errors
#[derive(Debug, thiserror::Error)]
pub enum SubscriberError {
    #[error("Failed to start subscriber: {0}")]
    StartupFailed(String),

    #[error("Failed to stop subscriber: {0}")]
    ShutdownFailed(String),

    #[error("Processing error: {0}")]
    ProcessingError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Hot reload not supported for this subscriber")]
    HotReloadNotSupported,

    #[error("Internal error: {0}")]
    Internal(String),
}
```

### 2.3 SubscriberCoordinator

```rust
/// Manages subscriber lifecycles
///
/// # Responsibilities
/// - Start/stop all subscribers
/// - Add/remove subscribers dynamically
/// - Monitor subscriber health
/// - Handle subscriber failures
pub struct SubscriberCoordinator {
    event_bus: Arc<EventBus>,
    subscribers: HashMap<String, Box<dyn Subscriber>>,
    handles: HashMap<String, JoinHandle<()>>,
    shutdown_token: CancellationToken,
}

impl SubscriberCoordinator {
    /// Create a new coordinator
    pub fn new(event_bus: Arc<EventBus>) -> Self;

    /// Add a subscriber
    ///
    /// Subscriber will be started on next `start_all()` call
    /// or immediately if coordinator is already running
    pub async fn add_subscriber(&mut self, subscriber: Box<dyn Subscriber>);

    /// Remove a subscriber by ID
    ///
    /// Stops the subscriber if running
    pub async fn remove_subscriber(&mut self, id: &str) -> Result<(), CoordinatorError>;

    /// Start all subscribers
    ///
    /// Each subscriber runs in its own tokio task
    pub async fn start_all(&mut self) -> Result<(), CoordinatorError>;

    /// Stop all subscribers gracefully
    pub async fn stop_all(&mut self) -> Result<(), CoordinatorError>;

    /// Get health status of all subscribers
    pub async fn health_status(&self) -> HashMap<String, HealthStatus>;
}
```

---

## 3. Processor Interface

### 3.1 Processor Trait

```rust
/// Trait for real-time data processors
///
/// Processors analyze data and produce outputs (alerts, metrics, etc.)
/// They do NOT persist data - that's handled by subscriber + output sink
///
/// # Design Pattern
/// Processors are wrapped by ProcessorSubscriber to integrate with event bus
#[async_trait]
pub trait Processor: Send + Sync {
    /// Unique identifier
    fn id(&self) -> &str;

    /// Process a single data point
    ///
    /// # Arguments
    /// * `point` - RawDataPoint to process
    ///
    /// # Returns
    /// Vector of outputs (may be empty if no action needed)
    ///
    /// # Performance
    /// Should complete in < 10ms for real-time processing
    async fn process(
        &self,
        point: &RawDataPoint
    ) -> Result<Vec<ProcessorOutput>, ProcessorError>;

    /// Check if this processor handles a given stream
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Get processor configuration
    fn config(&self) -> &ProcessorConfig;
}
```

### 3.2 ProcessorOutput

```rust
/// Output types from processors
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ProcessorOutput {
    /// Alert output (threshold violation, anomaly, etc.)
    Alert(Alert),

    /// Metric output (aggregation, calculation)
    Metric(Metric),

    /// Generic event output
    Event(Event),
}

/// Alert structure
#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    /// Timestamp of the alert
    pub timestamp: DateTime<Utc>,

    /// Rule that triggered the alert
    pub rule_name: String,

    /// Severity level
    pub severity: Severity,

    /// Human-readable message
    pub message: String,

    /// Source data reference
    pub source: AlertSource,

    /// Additional context
    pub context: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertSource {
    pub stream_id: String,
    pub ndp_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

### 3.3 ProcessorSubscriber

```rust
/// Wraps a Processor to integrate with event bus
///
/// Handles:
/// - Event bus integration
/// - Output routing to sinks
/// - Error handling and retry
pub struct ProcessorSubscriber {
    processor: Box<dyn Processor>,
    output_sinks: Vec<Box<dyn OutputSink>>,
    id: String,
}

impl ProcessorSubscriber {
    pub fn new(
        processor: Box<dyn Processor>,
        output_sinks: Vec<Box<dyn OutputSink>>
    ) -> Self;
}

#[async_trait]
impl Subscriber for ProcessorSubscriber {
    fn id(&self) -> &str { &self.id }

    async fn start(&mut self, mut receiver: Receiver<Arc<RawDataPoint>>) -> Result<...> {
        loop {
            match receiver.recv().await {
                Ok(point) => {
                    if self.processor.accepts_stream(&point.stream_id()) {
                        let outputs = self.processor.process(&point).await?;
                        for output in outputs {
                            for sink in &self.output_sinks {
                                let _ = sink.write(output.clone()).await;
                            }
                        }
                    }
                }
                Err(RecvError::Lagged(n)) => warn!("Lagged {n}"),
                Err(RecvError::Closed) => break,
            }
        }
        Ok(())
    }
    // ... other trait methods
}
```

---

## 4. Output Sink Interface

### 4.1 OutputSink Trait

```rust
/// Output destination for processor results
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Sink identifier
    fn id(&self) -> &str;

    /// Write output to sink
    ///
    /// # Error Handling
    /// - MQTT sinks should be fire-and-forget
    /// - DB sinks may retry on transient errors
    async fn write(&self, output: ProcessorOutput) -> Result<(), OutputError>;

    /// Flush any buffered outputs
    async fn flush(&self) -> Result<(), OutputError>;

    /// Health check
    async fn health_check(&self) -> HealthStatus;
}
```

### 4.2 Built-in Sinks

```rust
/// MQTT output sink
pub struct MqttOutputSink {
    client: AsyncClient,
    topic_pattern: String,  // e.g., "ndp/alerts/{stream_id}/{severity}"
    qos: QoS,
}

#[async_trait]
impl OutputSink for MqttOutputSink {
    async fn write(&self, output: ProcessorOutput) -> Result<(), OutputError> {
        match output {
            ProcessorOutput::Alert(alert) => {
                let topic = self.topic_pattern
                    .replace("{stream_id}", &alert.source.stream_id)
                    .replace("{severity}", &alert.severity.to_string());
                let payload = serde_json::to_vec(&alert)?;
                // Fire-and-forget: ignore errors
                let _ = self.client.try_publish(topic, self.qos, false, payload);
                Ok(())
            }
            _ => Ok(()), // Ignore non-alert outputs
        }
    }
    // ...
}

/// TimescaleDB output sink
pub struct TimescaleOutputSink {
    pool: Pool<Postgres>,
    table: String,  // e.g., "silver.alerts"
}

#[async_trait]
impl OutputSink for TimescaleOutputSink {
    async fn write(&self, output: ProcessorOutput) -> Result<(), OutputError> {
        match output {
            ProcessorOutput::Alert(alert) => {
                sqlx::query(
                    "INSERT INTO silver.alerts (timestamp, rule_name, severity, message, source_stream, source_ndp_id, context)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)"
                )
                .bind(&alert.timestamp)
                .bind(&alert.rule_name)
                .bind(alert.severity.to_string())
                .bind(&alert.message)
                .bind(&alert.source.stream_id)
                .bind(&alert.source.ndp_id)
                .bind(serde_json::to_value(&alert.context)?)
                .execute(&self.pool)
                .await?;
                Ok(())
            }
            _ => Ok(()),
        }
    }
    // ...
}
```

---

## 5. Silver Transform Interface

### 5.1 Transform Function

```rust
/// Transform RawDataPoint to Silver row
///
/// # Arguments
/// * `point` - Source RawDataPoint
/// * `config` - SilverEtlConfig from stream configuration
///
/// # Returns
/// * `Ok(SilverRow)` - Transformed row ready for UPSERT
/// * `Err(TransformError)` - Transform failed
///
/// # Processing Steps
/// 1. Extract timestamp using config.timestamp mapping
/// 2. Extract identity fields using config.identity_fields
/// 3. Extract and transform fields using config.field_mappings
/// 4. Evaluate DQ rules and populate dq_flags
pub fn transform_to_silver(
    point: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<SilverRow, TransformError>;
```

### 5.2 SilverRow

```rust
/// Silver layer row representation
///
/// Columnar format suitable for TimescaleDB insertion
#[derive(Debug, Clone)]
pub struct SilverRow {
    /// Target table (from config)
    pub table: String,

    /// Column values as ordered Vec
    /// Order matches target table schema
    pub values: HashMap<String, SqlValue>,

    /// DQ flags for this row
    pub dq_flags: Option<Vec<String>>,
}

/// SQL value types for TimescaleDB
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Boolean(bool),
    SmallInt(i16),
    Integer(i32),
    BigInt(i64),
    Real(f32),
    DoublePrecision(f64),
    Text(String),
    Timestamptz(DateTime<Utc>),
    Json(Value),
}

impl SilverRow {
    /// Get value by column name
    pub fn get(&self, column: &str) -> Option<&SqlValue>;

    /// Set value by column name
    pub fn set(&mut self, column: &str, value: SqlValue);

    /// Generate UPSERT SQL for this row
    pub fn to_upsert_sql(&self) -> String;

    /// Get bind parameters for SQL
    pub fn to_params(&self) -> Vec<&dyn ToSql>;
}
```

### 5.3 TransformError

```rust
#[derive(Debug, thiserror::Error)]
pub enum TransformError {
    #[error("Field extraction failed for {path}: {reason}")]
    FieldExtraction { path: String, reason: String },

    #[error("Type conversion failed for {field}: {reason}")]
    TypeConversion { field: String, reason: String },

    #[error("Required field missing: {field}")]
    RequiredFieldMissing { field: String },

    #[error("DQ evaluation failed: {0}")]
    DqEvaluation(String),

    #[error("Configuration error: {0}")]
    Config(String),
}
```

---

## 6. Configuration Interface

### 6.1 Platform Configuration

```yaml
# config/base/platform.yaml
event_bus:
  capacity: 10000
  lag_warning_threshold: 1000

subscribers:
  - id: bronze
    type: storage
    enabled: true
    config:
      format: parquet
      path: /data/raw/{stream_id}
      partitioning: daily
      batch_size: 50
      batch_timeout_secs: 2
      wal_enabled: true

  - id: silver
    type: timescale
    enabled: true
    config:
      connection_string: ${TIMESCALE_URL}
      batch_size: 100
      batch_timeout_secs: 5
      use_stream_etl_config: true

  - id: threshold-alerts
    type: processor
    enabled: true
    processor_id: threshold-alerts

  - id: event-notifier
    type: notifier
    enabled: ${EVENT_NOTIFIER_ENABLED:-false}
    config:
      mqtt_broker: ${MQTT_BROKER:-mosquitto:1883}
      topic_pattern: "ndp/events/{stream_id}"
      qos: 0
      payload_fields:
        - stream_id
        - ndp_id
        - timestamp
```

### 6.2 Processor Configuration

```yaml
# config/base/processors/threshold-alerts.yaml
processor_id: threshold-alerts
type: threshold
version: "1.0.0"
description: "Air quality threshold alerts"

config:
  rules:
    - name: pm25_unhealthy
      stream_filter: ["air-quality"]
      field: raw_payload.pm02Compensated
      condition: "> 35.4"
      severity: warning
      message: "PM2.5 exceeds EPA 'Unhealthy for Sensitive Groups'"
      cooldown_secs: 300

    - name: co2_high
      stream_filter: ["air-quality"]
      field: raw_payload.rco2
      condition: "> 1000"
      severity: warning
      message: "CO2 elevated - consider ventilation"

outputs:
  - type: mqtt
    topic: "ndp/alerts/{stream_id}/{severity}"
  - type: timescale
    table: silver.alerts
```

---

## 7. Contract Verification

### 7.1 Compile-Time Checks

| Contract | Verification |
|----------|--------------|
| Subscriber trait bounds | `Send + Sync` enforced by `#[async_trait]` |
| EventBus thread safety | `broadcast::Sender` is `Clone + Send + Sync` |
| RawDataPoint compatibility | Existing type, no changes needed |
| SilverEtlConfig compatibility | Existing type, no changes needed |

### 7.2 Runtime Checks

| Contract | Verification |
|----------|--------------|
| Event bus capacity | Assert capacity > 0 in constructor |
| Subscriber unique IDs | Check in coordinator.add_subscriber() |
| Stream filter validity | Validate against known streams on startup |
| Config schema | JSON Schema validation on load |

### 7.3 Integration Checks

| Contract | Verification |
|----------|--------------|
| Bronze output unchanged | Compare Parquet file checksums |
| Silver output matches batch | Compare query results |
| UPSERT idempotency | Insert same row twice, verify single row |
| Catch-up completeness | Compare Bronze and Silver counts |

---

*Interface contracts defined: 2026-01-18*
