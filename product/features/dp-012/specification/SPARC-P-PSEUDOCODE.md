# DP-012 SPARC Pseudocode (SPARC-P)

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Phase**: Pseudocode
**Created**: 2026-01-18
**Status**: Complete

---

## 1. Executive Summary

This document defines the algorithmic design and function signatures for all DP-012 components. Following London TDD methodology, these pseudocode definitions serve as the contract for implementation - tests will be written against these signatures before implementation.

---

## 2. Event Bus Module (`core/src/event_bus/`)

### 2.1 EventBus Struct

```rust
// File: core/src/event_bus/mod.rs

/// Event bus for broadcasting RawDataPoint to multiple subscribers
///
/// ALGORITHM:
/// 1. Create tokio::broadcast channel with configured capacity
/// 2. Wrap published points in Arc for zero-copy sharing
/// 3. Track metrics (published count, lag events)
/// 4. Log warning when lag exceeds threshold

pub struct EventBus {
    sender: broadcast::Sender<Arc<RawDataPoint>>,
    config: EventBusConfig,
    metrics: Arc<EventBusMetrics>,
}

impl EventBus {
    /// Constructor
    ///
    /// ALGORITHM:
    /// 1. Validate capacity > 0
    /// 2. Create broadcast::channel(capacity)
    /// 3. Initialize metrics to zero
    /// 4. Return EventBus instance
    pub fn new(config: EventBusConfig) -> Self {
        assert!(config.capacity > 0, "Event bus capacity must be > 0");
        let (sender, _) = broadcast::channel(config.capacity);
        Self {
            sender,
            config,
            metrics: Arc::new(EventBusMetrics::default()),
        }
    }

    /// Publish a data point to all subscribers
    ///
    /// ALGORITHM:
    /// 1. Wrap point in Arc
    /// 2. Call sender.send(arc_point)
    /// 3. If no receivers, return Ok (don't fail)
    /// 4. Increment published counter
    /// 5. Return Ok(())
    pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError> {
        let arc_point = Arc::new(point);
        match self.sender.send(arc_point) {
            Ok(_) => {
                self.metrics.published.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(_) => {
                // No receivers - this is OK, just means no subscribers yet
                Ok(())
            }
        }
    }

    /// Subscribe to receive events
    ///
    /// ALGORITHM:
    /// 1. Call sender.subscribe()
    /// 2. Increment subscriber count metric
    /// 3. Return receiver
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<RawDataPoint>> {
        self.metrics.subscriber_count.fetch_add(1, Ordering::Relaxed);
        self.sender.subscribe()
    }

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Get metrics for monitoring
    pub fn metrics(&self) -> &EventBusMetrics {
        &self.metrics
    }
}
```

### 2.2 EventBusMetrics

```rust
/// Metrics for event bus monitoring
#[derive(Debug, Default)]
pub struct EventBusMetrics {
    /// Total events published
    pub published: AtomicU64,
    /// Total lag events across all subscribers
    pub lagged_total: AtomicU64,
    /// Current subscriber count
    pub subscriber_count: AtomicUsize,
}
```

---

## 3. Subscriber Trait and Coordinator

### 3.1 Subscriber Trait

```rust
// File: core/src/subscribers/mod.rs

/// Core trait for event bus subscribers
///
/// LIFECYCLE:
/// 1. Create subscriber with configuration
/// 2. Add to SubscriberCoordinator
/// 3. Coordinator spawns task, calls start() with receiver
/// 4. Subscriber processes events in loop
/// 5. On shutdown, coordinator calls stop()
/// 6. Subscriber flushes buffers and returns

#[async_trait]
pub trait Subscriber: Send + Sync {
    /// Unique identifier for logging and metrics
    fn id(&self) -> &str;

    /// Start processing events
    ///
    /// ALGORITHM (typical implementation):
    /// 1. Perform startup tasks (e.g., catch-up for Silver)
    /// 2. Enter receive loop:
    ///    a. Use tokio::select! for timeout-based flushing
    ///    b. On recv Ok(point): process point
    ///    c. On recv Lagged(n): log warning, continue
    ///    d. On recv Closed: exit loop
    /// 3. Flush any remaining buffered data
    /// 4. Return Ok(())
    async fn start(
        &mut self,
        receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError>;

    /// Stop processing gracefully
    ///
    /// ALGORITHM:
    /// 1. Signal internal shutdown (set flag or cancel token)
    /// 2. Wait for processing loop to exit
    /// 3. Flush any buffered data
    /// 4. Close connections/resources
    /// 5. Return Ok(())
    async fn stop(&mut self) -> Result<(), SubscriberError>;

    /// Check if this subscriber processes a given stream
    ///
    /// ALGORITHM:
    /// 1. If no stream_filter configured: return true
    /// 2. Else: return stream_filter.contains(stream_id)
    fn accepts_stream(&self, stream_id: &str) -> bool;

    /// Health check for monitoring
    ///
    /// ALGORITHM:
    /// 1. Check internal state (connected, buffer size, etc.)
    /// 2. Return HealthStatus::Healthy/Degraded/Unhealthy
    async fn health_check(&self) -> HealthStatus;
}
```

### 3.2 SubscriberCoordinator

```rust
// File: core/src/subscribers/coordinator.rs

/// Manages subscriber lifecycles
///
/// RESPONSIBILITIES:
/// - Start/stop all subscribers as independent tasks
/// - Provide receivers from event bus to each subscriber
/// - Handle subscriber failures (log, continue others)
/// - Aggregate health status

pub struct SubscriberCoordinator {
    event_bus: Arc<EventBus>,
    subscribers: HashMap<String, Box<dyn Subscriber>>,
    handles: HashMap<String, JoinHandle<Result<(), SubscriberError>>>,
    shutdown_token: CancellationToken,
}

impl SubscriberCoordinator {
    /// Constructor
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            subscribers: HashMap::new(),
            handles: HashMap::new(),
            shutdown_token: CancellationToken::new(),
        }
    }

    /// Add a subscriber to be managed
    ///
    /// ALGORITHM:
    /// 1. Get subscriber ID
    /// 2. Check for duplicate ID (return error if exists)
    /// 3. Insert into subscribers map
    /// 4. Log subscriber added
    pub fn add_subscriber(&mut self, subscriber: Box<dyn Subscriber>) -> Result<(), CoordinatorError> {
        let id = subscriber.id().to_string();
        if self.subscribers.contains_key(&id) {
            return Err(CoordinatorError::DuplicateSubscriberId(id));
        }
        info!("Adding subscriber: {}", id);
        self.subscribers.insert(id, subscriber);
        Ok(())
    }

    /// Start all subscribers
    ///
    /// ALGORITHM:
    /// 1. For each subscriber:
    ///    a. Create broadcast receiver from event bus
    ///    b. Clone shutdown token
    ///    c. Spawn tokio task that:
    ///       i.  Calls subscriber.start(receiver)
    ///       ii. Handles errors (log, return)
    ///    d. Store JoinHandle in handles map
    /// 2. Log all subscribers started
    /// 3. Return Ok(())
    pub async fn start_all(&mut self) -> Result<(), CoordinatorError> {
        for (id, subscriber) in self.subscribers.iter_mut() {
            let receiver = self.event_bus.subscribe();
            let id_clone = id.clone();

            info!("Starting subscriber: {}", id);

            // Note: In real impl, we'd move subscriber into task
            // Here showing pseudocode structure
            let handle = tokio::spawn(async move {
                subscriber.start(receiver).await
            });

            self.handles.insert(id_clone, handle);
        }

        info!("All {} subscribers started", self.handles.len());
        Ok(())
    }

    /// Stop all subscribers gracefully
    ///
    /// ALGORITHM:
    /// 1. Cancel shutdown token (signals all tasks)
    /// 2. For each subscriber:
    ///    a. Call subscriber.stop()
    ///    b. Await JoinHandle with timeout
    ///    c. Log result
    /// 3. Clear handles map
    /// 4. Return Ok(())
    pub async fn stop_all(&mut self) -> Result<(), CoordinatorError> {
        info!("Stopping all subscribers...");
        self.shutdown_token.cancel();

        for (id, subscriber) in self.subscribers.iter_mut() {
            if let Err(e) = subscriber.stop().await {
                warn!("Error stopping subscriber {}: {}", id, e);
            }
        }

        for (id, handle) in self.handles.drain() {
            match tokio::time::timeout(Duration::from_secs(30), handle).await {
                Ok(Ok(Ok(()))) => info!("Subscriber {} stopped cleanly", id),
                Ok(Ok(Err(e))) => warn!("Subscriber {} error: {}", id, e),
                Ok(Err(e)) => warn!("Subscriber {} task panic: {}", id, e),
                Err(_) => warn!("Subscriber {} stop timeout", id),
            }
        }

        Ok(())
    }

    /// Get health status of all subscribers
    ///
    /// ALGORITHM:
    /// 1. Create empty HashMap for results
    /// 2. For each subscriber: call health_check(), store result
    /// 3. Return map
    pub async fn health_status(&self) -> HashMap<String, HealthStatus> {
        let mut status = HashMap::new();
        for (id, subscriber) in &self.subscribers {
            status.insert(id.clone(), subscriber.health_check().await);
        }
        status
    }
}
```

---

## 4. Bronze Subscriber

### 4.1 BronzeSubscriber

```rust
// File: core/src/subscribers/bronze.rs

/// Bronze layer subscriber - writes raw data to Parquet
///
/// BEHAVIOR:
/// - Batches incoming points (configurable size/timeout)
/// - Writes to Parquet via existing ParquetStore
/// - Respects WAL configuration
/// - Filters by stream if configured

pub struct BronzeSubscriber {
    id: String,
    store: Arc<dyn RawStore>,
    config: BronzeSubscriberConfig,
    buffer: Vec<Arc<RawDataPoint>>,
    last_flush: Instant,
    stream_filter: Option<HashSet<String>>,
    shutdown: Arc<AtomicBool>,
}

impl BronzeSubscriber {
    /// Constructor
    ///
    /// ALGORITHM:
    /// 1. Create empty buffer with capacity = batch_size
    /// 2. Set last_flush to now
    /// 3. Parse stream_filter if provided
    /// 4. Return BronzeSubscriber
    pub fn new(
        store: Arc<dyn RawStore>,
        config: BronzeSubscriberConfig,
    ) -> Self {
        Self {
            id: "bronze".to_string(),
            store,
            buffer: Vec::with_capacity(config.batch_size),
            last_flush: Instant::now(),
            stream_filter: config.stream_filter.clone(),
            config,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Buffer a point, flush if needed
    ///
    /// ALGORITHM:
    /// 1. If !accepts_stream(point.stream_id): return
    /// 2. Push point to buffer
    /// 3. If buffer.len() >= batch_size: flush()
    fn buffer_point(&mut self, point: Arc<RawDataPoint>) {
        if !self.accepts_stream(&point.stream_id()) {
            return;
        }
        self.buffer.push(point);
        if self.buffer.len() >= self.config.batch_size {
            // Will flush in next iteration
        }
    }

    /// Flush buffer to storage
    ///
    /// ALGORITHM:
    /// 1. If buffer is empty: return
    /// 2. Drain buffer into Vec
    /// 3. Call store.write_raw_batch(batch)
    /// 4. Update last_flush timestamp
    /// 5. Update metrics
    async fn flush(&mut self) -> Result<(), SubscriberError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let batch: Vec<RawDataPoint> = self.buffer
            .drain(..)
            .map(|arc| (*arc).clone())
            .collect();

        let count = batch.len();
        self.store.write_raw_batch(batch).await
            .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

        self.last_flush = Instant::now();
        debug!("Bronze flushed {} points", count);

        Ok(())
    }
}

#[async_trait]
impl Subscriber for BronzeSubscriber {
    fn id(&self) -> &str {
        &self.id
    }

    /// Start processing events
    ///
    /// ALGORITHM:
    /// 1. Create flush interval timer
    /// 2. Loop:
    ///    a. select! on:
    ///       - receiver.recv(): buffer_point(), check flush
    ///       - flush_interval.tick(): flush() if time exceeded
    ///       - shutdown signal: break
    ///    b. Handle Lagged: log, continue
    ///    c. Handle Closed: break
    /// 3. Final flush()
    /// 4. Return Ok(())
    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError> {
        let flush_interval = Duration::from_secs(self.config.batch_timeout_secs);
        let mut interval = tokio::time::interval(flush_interval);

        info!("Bronze subscriber started");

        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            self.buffer_point(point);
                            if self.buffer.len() >= self.config.batch_size {
                                self.flush().await?;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Bronze subscriber lagged, missed {} events", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event bus closed, stopping Bronze subscriber");
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    if self.last_flush.elapsed() >= flush_interval {
                        self.flush().await?;
                    }
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
        }

        // Final flush
        self.flush().await?;
        info!("Bronze subscriber stopped");

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        match &self.stream_filter {
            Some(filter) => filter.contains(stream_id),
            None => true,
        }
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}
```

---

## 5. Silver Subscriber

### 5.1 SilverSubscriber

```rust
// File: core/src/subscribers/silver.rs

/// Silver layer subscriber - streaming ETL to TimescaleDB
///
/// BEHAVIOR:
/// - Catches up from Bronze on startup (no data loss)
/// - Transforms RawDataPoint using SilverEtlConfig
/// - Batches and UPSERTs to TimescaleDB
/// - Handles lag gracefully (catch-up on restart)

pub struct SilverSubscriber {
    id: String,
    db_pool: Pool<Postgres>,
    bronze_reader: Arc<dyn BronzeReader>,
    configs: HashMap<String, SilverEtlConfig>,  // per-stream configs
    buffer: Vec<SilverRow>,
    config: SilverSubscriberConfig,
    last_flush: Instant,
    shutdown: Arc<AtomicBool>,
}

impl SilverSubscriber {
    /// Constructor
    pub fn new(
        db_pool: Pool<Postgres>,
        bronze_reader: Arc<dyn BronzeReader>,
        stream_configs: HashMap<String, SilverEtlConfig>,
        config: SilverSubscriberConfig,
    ) -> Self {
        Self {
            id: "silver".to_string(),
            db_pool,
            bronze_reader,
            configs: stream_configs,
            buffer: Vec::with_capacity(config.batch_size),
            config,
            last_flush: Instant::now(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Catch up from Bronze since Silver watermark
    ///
    /// ALGORITHM:
    /// 1. For each configured stream:
    ///    a. Query Silver: SELECT MAX(observation_time) FROM table
    ///    b. If None: watermark = epoch (process all)
    ///    c. List Bronze Parquet files since watermark
    ///    d. For each file:
    ///       i.   Read points from Parquet
    ///       ii.  For each point: transform and buffer
    ///       iii. Flush when buffer full
    ///    e. Final flush for this stream
    /// 2. Log catch-up complete
    async fn catch_up(&mut self) -> Result<(), SubscriberError> {
        info!("Silver subscriber starting catch-up from Bronze");

        for (stream_id, etl_config) in &self.configs {
            let watermark = self.get_watermark(&etl_config.target_table).await?;
            info!(
                "Stream {} watermark: {:?}, catching up...",
                stream_id, watermark
            );

            let files = self.bronze_reader
                .list_files_since(stream_id, watermark)
                .await
                .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

            for file in files {
                let points = self.bronze_reader
                    .read_parquet(&file)
                    .await
                    .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

                for point in points {
                    if let Ok(row) = transform_to_silver(&point, etl_config) {
                        self.buffer.push(row);
                        if self.buffer.len() >= self.config.batch_size {
                            self.flush_upsert().await?;
                        }
                    }
                }
            }

            self.flush_upsert().await?;
        }

        info!("Silver subscriber catch-up complete");
        Ok(())
    }

    /// Get Silver watermark for a table
    ///
    /// ALGORITHM:
    /// 1. Execute: SELECT MAX(observation_time) FROM table
    /// 2. Return Option<DateTime>
    async fn get_watermark(&self, table: &str) -> Result<Option<DateTime<Utc>>, SubscriberError> {
        let query = format!(
            "SELECT MAX(observation_time) FROM {}",
            table
        );

        let row = sqlx::query_scalar::<_, Option<DateTime<Utc>>>(&query)
            .fetch_one(&self.db_pool)
            .await
            .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

        Ok(row)
    }

    /// Transform and buffer a point
    ///
    /// ALGORITHM:
    /// 1. Extract stream_id from point
    /// 2. Lookup SilverEtlConfig for stream
    /// 3. If no config: skip (not configured for Silver)
    /// 4. Call transform_to_silver(point, config)
    /// 5. If Ok: push to buffer
    /// 6. If Err: log warning, continue
    fn transform_and_buffer(&mut self, point: &RawDataPoint) -> Result<(), SubscriberError> {
        let stream_id = point.stream_id();

        let config = match self.configs.get(&stream_id) {
            Some(c) => c,
            None => return Ok(()), // Not configured for this stream
        };

        match transform_to_silver(point, config) {
            Ok(row) => {
                self.buffer.push(row);
                Ok(())
            }
            Err(e) => {
                warn!("Transform error for {}: {}", stream_id, e);
                Ok(()) // Continue processing other points
            }
        }
    }

    /// Flush buffer with UPSERT
    ///
    /// ALGORITHM:
    /// 1. If buffer empty: return
    /// 2. Group rows by target table
    /// 3. For each table group:
    ///    a. Build UPSERT SQL with ON CONFLICT DO UPDATE
    ///    b. Execute batch insert
    /// 4. Clear buffer
    /// 5. Update last_flush timestamp
    async fn flush_upsert(&mut self) -> Result<(), SubscriberError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Group by target table
        let mut by_table: HashMap<String, Vec<&SilverRow>> = HashMap::new();
        for row in &self.buffer {
            by_table.entry(row.table.clone())
                .or_default()
                .push(row);
        }

        // UPSERT each table group
        for (table, rows) in by_table {
            self.upsert_batch(&table, &rows).await?;
        }

        let count = self.buffer.len();
        self.buffer.clear();
        self.last_flush = Instant::now();
        debug!("Silver flushed {} rows", count);

        Ok(())
    }

    /// UPSERT a batch to a table
    ///
    /// ALGORITHM:
    /// 1. Get column list from first row
    /// 2. Build INSERT ... ON CONFLICT DO UPDATE SQL
    /// 3. Execute with batch parameters
    async fn upsert_batch(&self, table: &str, rows: &[&SilverRow]) -> Result<(), SubscriberError> {
        if rows.is_empty() {
            return Ok(());
        }

        // Build dynamic UPSERT SQL
        // This is pseudocode - real impl uses parameterized queries
        let columns: Vec<&str> = rows[0].values.keys().map(|s| s.as_str()).collect();
        let conflict_cols = vec!["observation_time", "ndp_id"]; // Primary key
        let update_cols: Vec<&str> = columns.iter()
            .filter(|c| !conflict_cols.contains(c))
            .copied()
            .collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT ({}) DO UPDATE SET {}",
            table,
            columns.join(", "),
            (1..=columns.len()).map(|i| format!("${}", i)).collect::<Vec<_>>().join(", "),
            conflict_cols.join(", "),
            update_cols.iter()
                .map(|c| format!("{} = EXCLUDED.{}", c, c))
                .collect::<Vec<_>>()
                .join(", ")
        );

        // Execute for each row
        // In production: use batch/copy for performance
        for row in rows {
            let params = row.to_params();
            sqlx::query(&sql)
                .execute(&self.db_pool)
                .await
                .map_err(|e| SubscriberError::StorageError(e.to_string()))?;
        }

        Ok(())
    }
}

#[async_trait]
impl Subscriber for SilverSubscriber {
    fn id(&self) -> &str {
        &self.id
    }

    /// Start processing events
    ///
    /// ALGORITHM:
    /// 1. Catch up from Bronze (ensure no data loss)
    /// 2. Enter streaming loop:
    ///    a. select! on receiver, flush timer, shutdown
    ///    b. On point: transform_and_buffer, check flush
    ///    c. On timer: flush if needed
    ///    d. On Lagged: log warning, continue (will catch up on restart)
    ///    e. On Closed/shutdown: break
    /// 3. Final flush
    /// 4. Return Ok(())
    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError> {
        // Phase 1: Catch-up
        self.catch_up().await?;

        // Phase 2: Streaming
        let flush_interval = Duration::from_secs(self.config.batch_timeout_secs);
        let mut interval = tokio::time::interval(flush_interval);

        info!("Silver subscriber entering streaming mode");

        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            self.transform_and_buffer(&point)?;
                            if self.buffer.len() >= self.config.batch_size {
                                self.flush_upsert().await?;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Silver subscriber lagged, missed {} events (will catch up on restart)", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event bus closed, stopping Silver subscriber");
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    if self.last_flush.elapsed() >= flush_interval {
                        self.flush_upsert().await?;
                    }
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
        }

        // Final flush
        self.flush_upsert().await?;
        info!("Silver subscriber stopped");

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        self.configs.contains_key(stream_id)
    }

    async fn health_check(&self) -> HealthStatus {
        // Check DB connectivity
        match sqlx::query("SELECT 1").execute(&self.db_pool).await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        }
    }
}
```

---

## 6. Silver Transform Module

### 6.1 Transform Function

```rust
// File: core/src/silver/transform.rs

/// Transform RawDataPoint to SilverRow using SilverEtlConfig
///
/// This is the streaming equivalent of silver-etl's SQL generation.
/// Uses the SAME config format (SilverEtlConfig) for consistency.

/// Main transform function
///
/// ALGORITHM:
/// 1. Create empty SilverRow with target table from config
/// 2. Extract and set timestamp
/// 3. Extract and set identity fields (ndp_id, location_id, etc.)
/// 4. For each field_mapping:
///    a. Extract value from raw_payload using JSON path
///    b. Apply type cast (string -> target SQL type)
///    c. Apply transform (unit conversion, expression, etc.)
///    d. Set value in row
/// 5. Evaluate DQ rules, set dq_flags
/// 6. Return SilverRow
pub fn transform_to_silver(
    point: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<SilverRow, TransformError> {
    let mut row = SilverRow::new(config.target_table.clone());

    // 1. Timestamp
    let timestamp = extract_timestamp(point, &config.timestamp)?;
    row.set("observation_time", SqlValue::Timestamptz(timestamp));

    // 2. Identity fields
    for identity in &config.identity_fields {
        let value = extract_identity(point, identity)?;
        row.set(&identity.target_column, value);
    }

    // 3. Field mappings
    for mapping in &config.field_mappings {
        match extract_and_transform(point, mapping) {
            Ok(value) => row.set(&mapping.target_column, value),
            Err(e) => {
                if mapping.required {
                    return Err(e);
                }
                // Optional field: set NULL and continue
                row.set(&mapping.target_column, SqlValue::Null);
            }
        }
    }

    // 4. DQ evaluation
    let dq_flags = evaluate_dq_rules(&row, config)?;
    if !dq_flags.is_empty() {
        row.set("dq_flags", SqlValue::Json(serde_json::to_value(&dq_flags)?));
    }

    Ok(row)
}

/// Extract timestamp from point
///
/// ALGORITHM:
/// 1. If timestamp config specifies JSON path: extract from raw_payload
/// 2. Else: use point.timestamp
/// 3. Apply timezone conversion if specified
/// 4. Return DateTime<Utc>
fn extract_timestamp(
    point: &RawDataPoint,
    config: &TimestampConfig,
) -> Result<DateTime<Utc>, TransformError> {
    match &config.source {
        TimestampSource::PointTimestamp => Ok(point.timestamp),
        TimestampSource::JsonPath(path) => {
            let value = extract_json_path(&point.raw_payload, path)?;
            parse_timestamp(&value, &config.format)
        }
    }
}

/// Extract value using JSON path
///
/// ALGORITHM:
/// 1. Split path by '.' (e.g., "raw_payload.pm02Compensated")
/// 2. Navigate through JSON object
/// 3. If any part missing: return None
/// 4. Return final value
fn extract_json_path(
    json: &serde_json::Value,
    path: &str,
) -> Result<serde_json::Value, TransformError> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = json;

    for part in parts {
        // Skip "raw_payload" prefix if present
        if part == "raw_payload" {
            continue;
        }

        current = current.get(part)
            .ok_or_else(|| TransformError::FieldExtraction {
                path: path.to_string(),
                reason: format!("Field '{}' not found", part),
            })?;
    }

    Ok(current.clone())
}

/// Extract and transform a field
///
/// ALGORITHM:
/// 1. Extract raw value from JSON path
/// 2. Cast to target SQL type
/// 3. Apply transform (if configured)
/// 4. Return SqlValue
fn extract_and_transform(
    point: &RawDataPoint,
    mapping: &SilverFieldMapping,
) -> Result<SqlValue, TransformError> {
    // 1. Extract
    let raw_value = extract_json_path(&point.raw_payload, &mapping.source_path)?;

    // 2. Type cast
    let typed_value = cast_to_sql_type(&raw_value, &mapping.column_type)?;

    // 3. Transform
    let final_value = apply_transform(typed_value, &mapping.transform)?;

    Ok(final_value)
}

/// Cast JSON value to SQL type
///
/// ALGORITHM:
/// Match on target type, convert:
/// - "double precision" / "real" -> SqlValue::DoublePrecision/Real
/// - "integer" / "smallint" / "bigint" -> SqlValue::Integer/etc.
/// - "text" / "varchar" -> SqlValue::Text
/// - "boolean" -> SqlValue::Boolean
/// - "timestamptz" -> SqlValue::Timestamptz
/// - "jsonb" -> SqlValue::Json
fn cast_to_sql_type(
    value: &serde_json::Value,
    target_type: &str,
) -> Result<SqlValue, TransformError> {
    match target_type.to_lowercase().as_str() {
        "double precision" => {
            let f = value.as_f64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to double".into(),
                })?;
            Ok(SqlValue::DoublePrecision(f))
        }
        "real" => {
            let f = value.as_f64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to real".into(),
                })? as f32;
            Ok(SqlValue::Real(f))
        }
        "integer" => {
            let i = value.as_i64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to integer".into(),
                })? as i32;
            Ok(SqlValue::Integer(i))
        }
        "smallint" => {
            let i = value.as_i64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to smallint".into(),
                })? as i16;
            Ok(SqlValue::SmallInt(i))
        }
        "bigint" => {
            let i = value.as_i64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to bigint".into(),
                })?;
            Ok(SqlValue::BigInt(i))
        }
        "text" | "varchar" => {
            let s = match value {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Ok(SqlValue::Text(s))
        }
        "boolean" => {
            let b = value.as_bool()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Cannot convert to boolean".into(),
                })?;
            Ok(SqlValue::Boolean(b))
        }
        "jsonb" => Ok(SqlValue::Json(value.clone())),
        _ => Err(TransformError::Config(format!("Unknown type: {}", target_type))),
    }
}

/// Apply transform to value
///
/// ALGORITHM:
/// Match on transform type:
/// - None -> return value as-is
/// - UnitConversion { factor, offset } -> value * factor + offset
/// - Expression -> evaluate expression (limited subset)
/// - Lookup -> lookup in table (future)
fn apply_transform(
    value: SqlValue,
    transform: &Option<TransformConfig>,
) -> Result<SqlValue, TransformError> {
    match transform {
        None => Ok(value),
        Some(TransformConfig::UnitConversion { factor, offset }) => {
            match value {
                SqlValue::DoublePrecision(v) => {
                    Ok(SqlValue::DoublePrecision(v * factor + offset.unwrap_or(0.0)))
                }
                SqlValue::Real(v) => {
                    Ok(SqlValue::Real((v as f64 * factor + offset.unwrap_or(0.0)) as f32))
                }
                _ => Err(TransformError::TypeConversion {
                    field: "unknown".into(),
                    reason: "Unit conversion requires numeric type".into(),
                }),
            }
        }
        Some(TransformConfig::Expression(expr)) => {
            // Limited expression support - implement as needed
            Err(TransformError::Config("Expression transforms not yet implemented".into()))
        }
        Some(TransformConfig::Lookup { .. }) => {
            // Lookup tables - implement as needed
            Err(TransformError::Config("Lookup transforms not yet implemented".into()))
        }
    }
}
```

### 6.2 DQ Evaluator

```rust
// File: core/src/silver/dq_evaluator.rs

/// Evaluate data quality rules on a SilverRow
///
/// ALGORITHM:
/// 1. Create empty Vec<String> for flags
/// 2. For each field_mapping with dq_rules:
///    a. Get value from row
///    b. For each rule:
///       i.  Evaluate rule against value
///       ii. If fails: add flag string to Vec
/// 3. Return flags Vec

pub fn evaluate_dq_rules(
    row: &SilverRow,
    config: &SilverEtlConfig,
) -> Result<Vec<String>, TransformError> {
    let mut flags = Vec::new();

    for mapping in &config.field_mappings {
        if let Some(value) = row.get(&mapping.target_column) {
            for rule in &mapping.dq_rules {
                if let Some(flag) = evaluate_rule(value, rule, &mapping.target_column)? {
                    flags.push(flag);
                }
            }
        }
    }

    Ok(flags)
}

/// Evaluate a single DQ rule
///
/// ALGORITHM:
/// Match on rule type:
/// - RangeCheck { min, max } -> check value in range
/// - NotNull -> check value is not null
/// - Regex { pattern } -> check value matches pattern
/// - Enum { values } -> check value in set
/// - Custom { expression } -> evaluate expression
fn evaluate_rule(
    value: &SqlValue,
    rule: &DqRule,
    column: &str,
) -> Result<Option<String>, TransformError> {
    match rule {
        DqRule::RangeCheck { min, max, .. } => {
            if let Some(v) = value.as_f64() {
                if v < *min || v > *max {
                    return Ok(Some(format!("{}:range_check({}<{}<={})", column, min, v, max)));
                }
            }
            Ok(None)
        }
        DqRule::NotNull { .. } => {
            if matches!(value, SqlValue::Null) {
                return Ok(Some(format!("{}:not_null", column)));
            }
            Ok(None)
        }
        DqRule::Regex { pattern, .. } => {
            if let SqlValue::Text(s) = value {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| TransformError::DqEvaluation(e.to_string()))?;
                if !re.is_match(s) {
                    return Ok(Some(format!("{}:regex_match", column)));
                }
            }
            Ok(None)
        }
        DqRule::Enum { values, .. } => {
            if let SqlValue::Text(s) = value {
                if !values.contains(s) {
                    return Ok(Some(format!("{}:enum_check", column)));
                }
            }
            Ok(None)
        }
        // Add other rule types as needed
        _ => Ok(None),
    }
}
```

---

## 7. Threshold Processor

### 7.1 ThresholdProcessor

```rust
// File: core/src/processors/threshold.rs

/// Threshold processor for real-time alerting
///
/// BEHAVIOR:
/// - Evaluates field conditions (> < = etc.)
/// - Respects cooldown periods
/// - Outputs Alert structs

pub struct ThresholdProcessor {
    id: String,
    rules: Vec<ThresholdRule>,
    cooldowns: HashMap<String, Instant>,  // rule_name -> last_fired
}

impl ThresholdProcessor {
    pub fn new(config: ThresholdProcessorConfig) -> Self {
        Self {
            id: config.processor_id.clone(),
            rules: config.rules,
            cooldowns: HashMap::new(),
        }
    }

    /// Evaluate all rules against a point
    ///
    /// ALGORITHM:
    /// 1. Create empty Vec<ProcessorOutput>
    /// 2. For each rule:
    ///    a. If !rule.matches_stream(point.stream_id): skip
    ///    b. Extract field value from point
    ///    c. Evaluate condition
    ///    d. If condition true AND not in cooldown:
    ///       i.  Create Alert
    ///       ii. Add to outputs
    ///       iii. Update cooldown
    /// 3. Return outputs
    fn evaluate_rules(&mut self, point: &RawDataPoint) -> Vec<ProcessorOutput> {
        let mut outputs = Vec::new();

        for rule in &self.rules {
            // Stream filter
            if let Some(filter) = &rule.stream_filter {
                if !filter.contains(&point.stream_id()) {
                    continue;
                }
            }

            // Extract field value
            let value = match extract_json_path(&point.raw_payload, &rule.field) {
                Ok(v) => v,
                Err(_) => continue, // Field not present
            };

            // Evaluate condition
            let triggered = evaluate_condition(&value, &rule.condition);

            if triggered {
                // Check cooldown
                if let Some(last_fired) = self.cooldowns.get(&rule.name) {
                    if last_fired.elapsed().as_secs() < rule.cooldown_secs {
                        continue; // In cooldown
                    }
                }

                // Create alert
                let alert = Alert {
                    timestamp: Utc::now(),
                    rule_name: rule.name.clone(),
                    severity: rule.severity.clone(),
                    message: rule.message.clone(),
                    source: AlertSource {
                        stream_id: point.stream_id(),
                        ndp_id: point.ndp_id.clone(),
                        timestamp: point.timestamp,
                    },
                    context: HashMap::from([
                        ("field".to_string(), serde_json::to_value(&rule.field).unwrap()),
                        ("value".to_string(), value.clone()),
                        ("condition".to_string(), serde_json::to_value(&rule.condition).unwrap()),
                    ]),
                };

                outputs.push(ProcessorOutput::Alert(alert));
                self.cooldowns.insert(rule.name.clone(), Instant::now());
            }
        }

        outputs
    }
}

/// Evaluate a condition against a value
///
/// ALGORITHM:
/// 1. Parse condition string (e.g., "> 35.4")
/// 2. Extract operator and threshold
/// 3. Compare value against threshold
/// 4. Return bool
fn evaluate_condition(value: &serde_json::Value, condition: &str) -> bool {
    // Parse condition: "op threshold" (e.g., "> 35.4")
    let parts: Vec<&str> = condition.trim().splitn(2, ' ').collect();
    if parts.len() != 2 {
        return false;
    }

    let op = parts[0];
    let threshold: f64 = match parts[1].parse() {
        Ok(t) => t,
        Err(_) => return false,
    };

    let v: f64 = match value.as_f64() {
        Some(v) => v,
        None => return false,
    };

    match op {
        ">" => v > threshold,
        ">=" => v >= threshold,
        "<" => v < threshold,
        "<=" => v <= threshold,
        "=" | "==" => (v - threshold).abs() < f64::EPSILON,
        "!=" => (v - threshold).abs() >= f64::EPSILON,
        _ => false,
    }
}

#[async_trait]
impl Processor for ThresholdProcessor {
    fn id(&self) -> &str {
        &self.id
    }

    async fn process(
        &mut self,
        point: &RawDataPoint
    ) -> Result<Vec<ProcessorOutput>, ProcessorError> {
        Ok(self.evaluate_rules(point))
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        // Check if any rule accepts this stream
        self.rules.iter().any(|rule| {
            match &rule.stream_filter {
                Some(filter) => filter.contains(&stream_id.to_string()),
                None => true,
            }
        })
    }

    fn config(&self) -> &ProcessorConfig {
        // Return config reference
        unimplemented!()
    }
}
```

---

## 8. Event Notifier

### 8.1 EventNotifier

```rust
// File: core/src/subscribers/event_notifier.rs

/// Event notifier - MQTT notifications for external consumers
///
/// BEHAVIOR:
/// - Fire-and-forget MQTT publish (QoS 0)
/// - Minimal payload (IDs + timestamp only)
/// - Never blocks, never fails the pipeline
/// - Toggle via environment variable

pub struct EventNotifier {
    id: String,
    mqtt_client: AsyncClient,
    enabled: bool,
    topic_pattern: String,
    shutdown: Arc<AtomicBool>,
}

impl EventNotifier {
    pub fn new(config: EventNotifierConfig) -> Result<Self, SubscriberError> {
        // Check environment variable
        let enabled = std::env::var("EVENT_NOTIFIER_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(config.enabled);

        if !enabled {
            info!("Event notifier disabled");
        }

        // Create MQTT client (only if enabled)
        let mqtt_client = if enabled {
            create_mqtt_client(&config.mqtt_broker)?
        } else {
            create_dummy_client()
        };

        Ok(Self {
            id: "event-notifier".to_string(),
            mqtt_client,
            enabled,
            topic_pattern: config.topic_pattern,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Publish notification for a data point
    ///
    /// ALGORITHM:
    /// 1. If !enabled: return Ok(()) immediately
    /// 2. Build topic from pattern
    /// 3. Build minimal payload (NO raw_payload)
    /// 4. Call try_publish (non-blocking, QoS 0)
    /// 5. Ignore any errors (fire-and-forget)
    /// 6. Return Ok(())
    fn notify(&self, point: &RawDataPoint) -> Result<(), SubscriberError> {
        if !self.enabled {
            return Ok(());
        }

        let topic = self.topic_pattern
            .replace("{stream_id}", &point.stream_id());

        let payload = serde_json::json!({
            "stream_id": point.stream_id(),
            "ndp_id": point.ndp_id,
            "timestamp": point.timestamp.to_rfc3339(),
            // CRITICAL: No raw_payload - consumers query Silver for data
        });

        let payload_bytes = serde_json::to_vec(&payload)
            .unwrap_or_default();

        // Fire-and-forget: ignore result
        let _ = self.mqtt_client.try_publish(
            &topic,
            QoS::AtMostOnce,  // QoS 0
            false,            // Not retained
            payload_bytes,
        );

        Ok(())
    }
}

#[async_trait]
impl Subscriber for EventNotifier {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>
    ) -> Result<(), SubscriberError> {
        if !self.enabled {
            info!("Event notifier disabled, not starting receive loop");
            // Just wait for shutdown
            while !self.shutdown.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            return Ok(());
        }

        info!("Event notifier started");

        loop {
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            let _ = self.notify(&point); // Ignore errors
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            debug!("Event notifier lagged {} events (acceptable)", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }
        }

        info!("Event notifier stopped");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        self.shutdown.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn accepts_stream(&self, _stream_id: &str) -> bool {
        true // Notifies on all streams
    }

    async fn health_check(&self) -> HealthStatus {
        if !self.enabled {
            return HealthStatus::Healthy; // Disabled is healthy
        }

        // Check MQTT connection
        if self.mqtt_client.is_connected() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Degraded // Not critical - fire-and-forget
        }
    }
}
```

---

## 9. Integration: Air Quality App Wiring

### 9.1 Main Function Updates

```rust
// File: apps/air-quality-app/src/main.rs (pseudocode for changes)

/// Wire sources to event bus, start subscriber coordinator
///
/// ALGORITHM:
/// 1. Load configuration (existing)
/// 2. Create EventBus with config
/// 3. Create SubscriberCoordinator
/// 4. Create and add subscribers:
///    - BronzeSubscriber (always)
///    - SilverSubscriber (if enabled)
///    - ProcessorSubscriber<ThresholdProcessor> (if enabled)
///    - EventNotifier (if EVENT_NOTIFIER_ENABLED)
/// 5. Start all subscribers
/// 6. Create sources (existing)
/// 7. Wire sources to publish to EventBus (instead of mpsc)
/// 8. Wait for shutdown signal
/// 9. Stop all subscribers gracefully

async fn main() -> Result<()> {
    // 1. Load config
    let config = load_config().await?;

    // 2. Create event bus
    let event_bus_config = EventBusConfig {
        capacity: config.event_bus.capacity.unwrap_or(10000),
        lag_warning_threshold: config.event_bus.lag_warning_threshold.unwrap_or(1000),
    };
    let event_bus = Arc::new(EventBus::new(event_bus_config));

    // 3. Create coordinator
    let mut coordinator = SubscriberCoordinator::new(event_bus.clone());

    // 4. Create and add subscribers
    // Bronze (always enabled)
    let bronze_store = create_parquet_store(&config)?;
    let bronze_subscriber = BronzeSubscriber::new(
        bronze_store,
        BronzeSubscriberConfig::from(&config),
    );
    coordinator.add_subscriber(Box::new(bronze_subscriber))?;

    // Silver (if configured)
    if config.subscribers.silver.enabled {
        let db_pool = create_db_pool(&config).await?;
        let bronze_reader = create_bronze_reader(&config)?;
        let stream_configs = load_silver_etl_configs(&config).await?;

        let silver_subscriber = SilverSubscriber::new(
            db_pool,
            bronze_reader,
            stream_configs,
            SilverSubscriberConfig::from(&config),
        );
        coordinator.add_subscriber(Box::new(silver_subscriber))?;
    }

    // Threshold processor (if configured)
    if config.subscribers.threshold_alerts.enabled {
        let processor = ThresholdProcessor::new(
            load_threshold_config(&config).await?
        );
        let mqtt_sink = MqttOutputSink::new(&config)?;
        let processor_subscriber = ProcessorSubscriber::new(
            Box::new(processor),
            vec![Box::new(mqtt_sink)],
        );
        coordinator.add_subscriber(Box::new(processor_subscriber))?;
    }

    // Event notifier
    let event_notifier = EventNotifier::new(
        EventNotifierConfig::from(&config)
    )?;
    coordinator.add_subscriber(Box::new(event_notifier))?;

    // 5. Start all subscribers
    coordinator.start_all().await?;

    // 6. Create sources (modified to use event bus)
    let sources = create_sources(&config, event_bus.clone()).await?;

    // 7. Start sources (existing logic, but publishing to event_bus)
    for source in sources {
        tokio::spawn(async move {
            source.run(/* publishes to event_bus */).await
        });
    }

    // 8. Wait for shutdown
    wait_for_shutdown().await;

    // 9. Graceful shutdown
    coordinator.stop_all().await?;

    Ok(())
}
```

---

## 10. Function Signature Summary

### 10.1 Core Traits

| Trait | Method | Signature |
|-------|--------|-----------|
| `Subscriber` | `id` | `fn id(&self) -> &str` |
| `Subscriber` | `start` | `async fn start(&mut self, Receiver<Arc<RawDataPoint>>) -> Result<(), SubscriberError>` |
| `Subscriber` | `stop` | `async fn stop(&mut self) -> Result<(), SubscriberError>` |
| `Subscriber` | `accepts_stream` | `fn accepts_stream(&self, &str) -> bool` |
| `Subscriber` | `health_check` | `async fn health_check(&self) -> HealthStatus` |
| `Processor` | `id` | `fn id(&self) -> &str` |
| `Processor` | `process` | `async fn process(&mut self, &RawDataPoint) -> Result<Vec<ProcessorOutput>, ProcessorError>` |
| `Processor` | `accepts_stream` | `fn accepts_stream(&self, &str) -> bool` |
| `OutputSink` | `id` | `fn id(&self) -> &str` |
| `OutputSink` | `write` | `async fn write(&self, ProcessorOutput) -> Result<(), OutputError>` |
| `OutputSink` | `flush` | `async fn flush(&self) -> Result<(), OutputError>` |

### 10.2 Core Functions

| Module | Function | Signature |
|--------|----------|-----------|
| `event_bus` | `EventBus::new` | `fn new(EventBusConfig) -> Self` |
| `event_bus` | `EventBus::publish` | `fn publish(&self, RawDataPoint) -> Result<(), EventBusError>` |
| `event_bus` | `EventBus::subscribe` | `fn subscribe(&self) -> Receiver<Arc<RawDataPoint>>` |
| `silver::transform` | `transform_to_silver` | `fn transform_to_silver(&RawDataPoint, &SilverEtlConfig) -> Result<SilverRow, TransformError>` |
| `silver::transform` | `extract_json_path` | `fn extract_json_path(&Value, &str) -> Result<Value, TransformError>` |
| `silver::transform` | `cast_to_sql_type` | `fn cast_to_sql_type(&Value, &str) -> Result<SqlValue, TransformError>` |
| `silver::dq_evaluator` | `evaluate_dq_rules` | `fn evaluate_dq_rules(&SilverRow, &SilverEtlConfig) -> Result<Vec<String>, TransformError>` |
| `processors::threshold` | `evaluate_condition` | `fn evaluate_condition(&Value, &str) -> bool` |

---

*Pseudocode created: 2026-01-18*
*Next phase: SPARC-A (Architecture)*
