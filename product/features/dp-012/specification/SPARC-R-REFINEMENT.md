# DP-012 SPARC Refinement (SPARC-R)

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Phase**: Refinement
**Created**: 2026-01-18
**Status**: Complete

---

## 1. Executive Summary

This document refines the DP-012 implementation by addressing edge cases, error handling, performance optimization, configuration validation, and operational concerns. It ensures the design is production-ready.

---

## 2. Edge Cases and Error Handling

### 2.1 Event Bus Edge Cases

#### EC-EB-001: No Subscribers Connected

**Scenario**: Event bus has no subscribers when publish is called.

**Current Behavior**: `broadcast::send()` returns error when no receivers.

**Refined Behavior**:
```rust
pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError> {
    let arc_point = Arc::new(point);
    match self.sender.send(arc_point) {
        Ok(_) => {
            self.metrics.published.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
        Err(_) => {
            // No receivers - this is OK during startup or if all subscribers stopped
            // Log at debug level, don't fail
            debug!("Event published with no subscribers");
            self.metrics.no_receiver_count.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }
    }
}
```

**Test Case**:
```rust
#[tokio::test]
async fn test_publish_with_no_subscribers_succeeds() {
    let bus = EventBus::new(EventBusConfig::default());
    let point = RawDataPoint::test_point();
    // No subscribers added
    assert!(bus.publish(point).is_ok());
}
```

---

#### EC-EB-002: Subscriber Drops Mid-Stream

**Scenario**: Subscriber task panics or is cancelled while event bus continues.

**Behavior**:
- Event bus continues operating
- Subscriber count decreases automatically (broadcast behavior)
- Other subscribers unaffected
- Coordinator detects via JoinHandle

**Implementation**:
```rust
// SubscriberCoordinator monitors handles
async fn monitor_health(&self) {
    for (id, handle) in &self.handles {
        if handle.is_finished() {
            warn!("Subscriber {} task finished unexpectedly", id);
            // Could implement auto-restart here
        }
    }
}
```

---

#### EC-EB-003: Event Bus Capacity Exceeded

**Scenario**: Publishers faster than slowest subscriber, buffer fills.

**Behavior**:
- `broadcast` automatically drops oldest messages
- Slow subscriber receives `RecvError::Lagged(n)`
- n indicates how many messages were missed

**Refined Handling**:
```rust
// In subscriber receive loop
Err(broadcast::error::RecvError::Lagged(n)) => {
    warn!(
        subscriber_id = %self.id,
        lagged_count = n,
        "Subscriber lagged, missed {} events",
        n
    );
    self.metrics.lagged_events.add(n as u64);

    // For Silver: this is acceptable, will catch up on restart
    // For Bronze: this is concerning, may need larger buffer
    if n > 1000 {
        error!(
            subscriber_id = %self.id,
            "Severe lag detected ({}), consider increasing buffer size",
            n
        );
    }

    continue; // Don't break, continue processing
}
```

---

### 2.2 Bronze Subscriber Edge Cases

#### EC-BRONZE-001: Parquet Write Failure

**Scenario**: Disk full, permission error, or I/O failure during write.

**Refined Handling**:
```rust
async fn flush(&mut self) -> Result<(), SubscriberError> {
    if self.buffer.is_empty() {
        return Ok(());
    }

    let batch: Vec<RawDataPoint> = self.buffer.drain(..).map(|arc| (*arc).clone()).collect();
    let batch_size = batch.len();

    // Retry with exponential backoff
    let mut attempts = 0;
    let max_attempts = 3;
    let mut delay = Duration::from_millis(100);

    loop {
        match self.store.write_raw_batch(batch.clone()).await {
            Ok(()) => {
                self.metrics.written.add(batch_size as u64);
                return Ok(());
            }
            Err(e) => {
                attempts += 1;
                if attempts >= max_attempts {
                    error!(
                        subscriber_id = %self.id,
                        error = %e,
                        batch_size = batch_size,
                        "Bronze write failed after {} attempts, data may be lost",
                        max_attempts
                    );
                    // For Bronze, this is CRITICAL - data loss
                    // In production, could write to fallback location
                    return Err(SubscriberError::StorageError(e.to_string()));
                }

                warn!(
                    subscriber_id = %self.id,
                    error = %e,
                    attempt = attempts,
                    "Bronze write failed, retrying in {:?}",
                    delay
                );
                tokio::time::sleep(delay).await;
                delay *= 2;
            }
        }
    }
}
```

---

#### EC-BRONZE-002: WAL Recovery on Startup

**Scenario**: Application crashed during write, WAL has uncommitted data.

**Behavior**:
- Existing WAL recovery in ParquetStore handles this
- Bronze subscriber delegates to ParquetStore
- No additional handling needed in subscriber

**Verification**:
```rust
#[tokio::test]
async fn test_bronze_wal_recovery() {
    // Create store with WAL
    let store = ParquetStore::new(config_with_wal);

    // Simulate crash by writing to WAL without commit
    store.wal_write(test_points).await;
    // Drop store (simulates crash)

    // Recreate store - should recover WAL
    let recovered_store = ParquetStore::new(config_with_wal);

    // Verify data recovered
    let data = recovered_store.query_raw(time_range).await.unwrap();
    assert!(!data.is_empty());
}
```

---

### 2.3 Silver Subscriber Edge Cases

#### EC-SILVER-001: Database Connection Lost

**Scenario**: TimescaleDB connection drops during operation.

**Refined Handling**:
```rust
async fn flush_upsert(&mut self) -> Result<(), SubscriberError> {
    if self.buffer.is_empty() {
        return Ok(());
    }

    // Retry with backoff for transient failures
    let mut attempts = 0;
    let max_attempts = 5;
    let mut delay = Duration::from_millis(500);

    loop {
        match self.do_upsert().await {
            Ok(()) => return Ok(()),
            Err(e) => {
                attempts += 1;

                // Check if error is transient
                let is_transient = e.to_string().contains("connection")
                    || e.to_string().contains("timeout")
                    || e.to_string().contains("temporarily unavailable");

                if !is_transient || attempts >= max_attempts {
                    warn!(
                        subscriber_id = %self.id,
                        error = %e,
                        buffered_rows = self.buffer.len(),
                        "Silver upsert failed, will catch up on restart"
                    );
                    // Don't clear buffer - keep for retry
                    // But don't block indefinitely either
                    return Err(SubscriberError::StorageError(e.to_string()));
                }

                warn!(
                    subscriber_id = %self.id,
                    error = %e,
                    attempt = attempts,
                    "Transient DB error, retrying in {:?}",
                    delay
                );
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(30));
            }
        }
    }
}
```

---

#### EC-SILVER-002: Catch-up with Large Gap

**Scenario**: Silver was down for days, needs to catch up from many Bronze files.

**Refined Handling**:
```rust
async fn catch_up(&mut self) -> Result<(), SubscriberError> {
    let watermark = self.get_watermark(&self.config.target_table).await?;

    let files = self.bronze_reader.list_files_since(&self.stream_id, watermark).await?;

    let file_count = files.len();
    info!(
        subscriber_id = %self.id,
        watermark = ?watermark,
        files_to_process = file_count,
        "Starting Silver catch-up"
    );

    // If many files, log progress periodically
    let log_interval = std::cmp::max(1, file_count / 10);

    for (idx, file) in files.iter().enumerate() {
        let points = self.bronze_reader.read_parquet(file).await
            .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

        for point in points {
            if let Ok(row) = transform_to_silver(&point, &self.etl_config) {
                self.buffer.push(row);
                if self.buffer.len() >= self.config.batch_size {
                    self.flush_upsert().await?;
                }
            }
        }

        // Progress logging
        if (idx + 1) % log_interval == 0 {
            info!(
                subscriber_id = %self.id,
                progress = format!("{}/{}", idx + 1, file_count),
                "Catch-up progress"
            );
        }

        // Yield to prevent blocking other tasks
        tokio::task::yield_now().await;
    }

    self.flush_upsert().await?;

    info!(
        subscriber_id = %self.id,
        files_processed = file_count,
        "Silver catch-up complete"
    );

    Ok(())
}
```

---

#### EC-SILVER-003: Transform Error on Required Field

**Scenario**: Required field missing or unparseable in raw_payload.

**Refined Handling**:
```rust
fn transform_and_buffer(&mut self, point: &RawDataPoint) {
    let stream_id = point.stream_id();

    let config = match self.configs.get(&stream_id) {
        Some(c) => c,
        None => return, // Not configured for Silver
    };

    match transform_to_silver(point, config) {
        Ok(row) => {
            self.buffer.push(row);
        }
        Err(TransformError::RequiredFieldMissing { field }) => {
            warn!(
                subscriber_id = %self.id,
                stream_id = %stream_id,
                field = %field,
                ndp_id = ?point.ndp_id,
                "Required field missing, skipping row"
            );
            self.metrics.transform_errors.add(1);
        }
        Err(TransformError::TypeConversion { field, reason }) => {
            warn!(
                subscriber_id = %self.id,
                stream_id = %stream_id,
                field = %field,
                reason = %reason,
                "Type conversion failed, skipping row"
            );
            self.metrics.transform_errors.add(1);
        }
        Err(e) => {
            debug!(
                subscriber_id = %self.id,
                error = %e,
                "Transform error, skipping row"
            );
            self.metrics.transform_errors.add(1);
        }
    }
}
```

---

#### EC-SILVER-004: Duplicate Key During UPSERT

**Scenario**: Same (observation_time, ndp_id) arrives twice.

**Behavior**: UPSERT handles this gracefully - second insert updates existing row.

**Verification**:
```rust
#[tokio::test]
async fn test_upsert_handles_duplicates() {
    let db = setup_test_db().await;
    let subscriber = SilverSubscriber::new(db.clone(), ...);

    let row = SilverRow::test_row();

    // Insert twice
    subscriber.upsert_batch(&[&row]).await.unwrap();
    subscriber.upsert_batch(&[&row]).await.unwrap();

    // Should have exactly one row
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM silver.test_table")
        .fetch_one(&db)
        .await
        .unwrap();

    assert_eq!(count, 1);
}
```

---

### 2.4 Threshold Processor Edge Cases

#### EC-THRESH-001: Field Not Present in Payload

**Scenario**: Rule references field that doesn't exist in some payloads.

**Refined Handling**:
```rust
fn evaluate_rules(&mut self, point: &RawDataPoint) -> Vec<ProcessorOutput> {
    let mut outputs = Vec::new();

    for rule in &self.rules {
        // Stream filter
        if !rule.matches_stream(&point.stream_id()) {
            continue;
        }

        // Extract field - silently skip if not present
        let value = match extract_json_path(&point.raw_payload, &rule.field) {
            Ok(v) => v,
            Err(_) => {
                // Field not present - this is normal for heterogeneous streams
                // e.g., some sensors report CO2, others don't
                continue;
            }
        };

        // Rest of evaluation...
    }

    outputs
}
```

---

#### EC-THRESH-002: Cooldown State Across Restarts

**Scenario**: Cooldown tracking is in-memory, lost on restart.

**Current Design**: Accept that restart resets cooldowns.

**Rationale**:
- Cooldowns are typically short (5 minutes)
- Restart is infrequent
- Re-alerting after restart is acceptable
- Complexity of persisting cooldowns not worth it

**Future Option**: If persistent cooldowns needed, store last_fired in TimescaleDB:
```sql
CREATE TABLE silver.alert_cooldowns (
    rule_name TEXT PRIMARY KEY,
    last_fired TIMESTAMPTZ NOT NULL
);
```

---

### 2.5 Event Notifier Edge Cases

#### EC-NOTIFY-001: MQTT Broker Unavailable

**Scenario**: Mosquitto is down when notification attempted.

**Behavior**: Fire-and-forget, no retry.

```rust
fn notify(&self, point: &RawDataPoint) -> Result<(), SubscriberError> {
    if !self.enabled {
        return Ok(());
    }

    let payload = build_notification_payload(point);

    // try_publish is non-blocking and ignores errors
    match self.mqtt_client.try_publish(&topic, QoS::AtMostOnce, false, payload) {
        Ok(_) => {
            self.metrics.notifications_sent.add(1);
        }
        Err(e) => {
            // Log at debug level - this is expected when MQTT is down
            debug!(
                subscriber_id = %self.id,
                error = %e,
                "MQTT publish failed (non-critical)"
            );
            self.metrics.notifications_failed.add(1);
        }
    }

    // Always return Ok - fire-and-forget
    Ok(())
}
```

---

#### EC-NOTIFY-002: High Event Rate

**Scenario**: Many events per second causing MQTT backpressure.

**Refined Handling**:
```rust
// Option 1: Rate limiting (recommended)
pub struct EventNotifier {
    // ... other fields
    rate_limiter: RateLimiter,
}

fn notify(&self, point: &RawDataPoint) -> Result<(), SubscriberError> {
    if !self.enabled {
        return Ok(());
    }

    // Check rate limit - skip if exceeded
    if !self.rate_limiter.check() {
        self.metrics.rate_limited.add(1);
        return Ok(()); // Silently drop
    }

    // ... rest of notify
}

// Option 2: Batching (if rate limiting not enough)
// Collect notifications, publish batch every N ms
```

---

## 3. Performance Optimizations

### 3.1 Zero-Copy Broadcasting

**Optimization**: Use `Arc<RawDataPoint>` to avoid cloning for each subscriber.

```rust
// Already in design, but emphasize:
pub fn publish(&self, point: RawDataPoint) -> Result<(), EventBusError> {
    // Wrap once, share across all subscribers
    let arc_point = Arc::new(point);
    self.sender.send(arc_point)?;
    Ok(())
}

// Each subscriber receives same Arc, no data copying
async fn start(&mut self, mut receiver: Receiver<Arc<RawDataPoint>>) {
    loop {
        match receiver.recv().await {
            Ok(point) => {
                // point is Arc<RawDataPoint>, not cloned
                self.process(&point).await;
            }
            // ...
        }
    }
}
```

---

### 3.2 Batch Sizing Optimization

**Bronze Subscriber**:
```rust
// Optimal batch sizes for Parquet
pub struct BronzeSubscriberConfig {
    /// Batch size: 50-100 for real-time, 1000+ for throughput
    pub batch_size: usize,  // Default: 50

    /// Flush timeout: 2-5 seconds for real-time
    pub batch_timeout_secs: u64,  // Default: 2
}
```

**Silver Subscriber**:
```rust
pub struct SilverSubscriberConfig {
    /// Batch size: 100-500 for balance of latency and throughput
    pub batch_size: usize,  // Default: 100

    /// Flush timeout: 5 seconds max for < 5s latency target
    pub batch_timeout_secs: u64,  // Default: 5
}
```

---

### 3.3 Silver UPSERT Optimization

**Use COPY for Bulk Inserts**:
```rust
async fn upsert_batch(&self, table: &str, rows: &[&SilverRow]) -> Result<(), SubscriberError> {
    if rows.len() < 10 {
        // Small batch: use individual UPSERT
        self.upsert_individual(table, rows).await
    } else {
        // Large batch: use COPY to temp table, then UPSERT
        self.upsert_via_copy(table, rows).await
    }
}

async fn upsert_via_copy(&self, table: &str, rows: &[&SilverRow]) -> Result<(), SubscriberError> {
    // 1. Create temp table
    sqlx::query(&format!(
        "CREATE TEMP TABLE tmp_{} (LIKE {} INCLUDING ALL) ON COMMIT DROP",
        table.replace(".", "_"), table
    ))
    .execute(&self.pool)
    .await?;

    // 2. COPY to temp table (much faster than individual INSERTs)
    let copy_cmd = format!(
        "COPY tmp_{} FROM STDIN WITH (FORMAT BINARY)",
        table.replace(".", "_")
    );
    // Use tokio-postgres COPY support...

    // 3. UPSERT from temp to target
    let upsert_sql = format!(
        "INSERT INTO {} SELECT * FROM tmp_{} ON CONFLICT (observation_time, ndp_id) DO UPDATE SET ...",
        table, table.replace(".", "_")
    );
    sqlx::query(&upsert_sql).execute(&self.pool).await?;

    Ok(())
}
```

---

### 3.4 Connection Pool Sizing

```rust
// TimescaleDB pool configuration
pub fn create_pool(config: &DbConfig) -> Result<Pool<Postgres>, Error> {
    PgPoolOptions::new()
        // Connections per subscriber
        .max_connections(5)
        // Don't wait too long for connections
        .acquire_timeout(Duration::from_secs(10))
        // Keep connections warm
        .idle_timeout(Duration::from_secs(600))
        // Test connections before use
        .test_before_acquire(true)
        .connect(&config.connection_string)
        .await
}
```

---

## 4. Configuration Validation

### 4.1 Startup Validation

```rust
/// Validate all configuration at startup
pub fn validate_config(config: &PlatformConfig) -> Result<(), ConfigError> {
    // Event bus
    if config.event_bus.capacity == 0 {
        return Err(ConfigError::Invalid("event_bus.capacity must be > 0".into()));
    }
    if config.event_bus.capacity < 1000 {
        warn!("event_bus.capacity < 1000 may cause excessive lag");
    }

    // Subscribers
    let mut seen_ids = HashSet::new();
    for sub in &config.subscribers {
        // Unique IDs
        if !seen_ids.insert(&sub.id) {
            return Err(ConfigError::Invalid(
                format!("Duplicate subscriber ID: {}", sub.id)
            ));
        }

        // Type-specific validation
        match &sub.subscriber_type {
            SubscriberType::Storage => validate_storage_config(&sub.config)?,
            SubscriberType::Timescale => validate_timescale_config(&sub.config)?,
            SubscriberType::Processor => validate_processor_config(&sub.config)?,
            SubscriberType::Notifier => validate_notifier_config(&sub.config)?,
        }
    }

    // Silver ETL configs
    for (stream_id, etl_config) in &config.silver_etl_configs {
        validate_silver_etl_config(stream_id, etl_config)?;
    }

    Ok(())
}

fn validate_silver_etl_config(stream_id: &str, config: &SilverEtlConfig) -> Result<(), ConfigError> {
    // Target table required
    if config.target_table.is_empty() {
        return Err(ConfigError::Invalid(
            format!("{}: target_table required", stream_id)
        ));
    }

    // At least one field mapping
    if config.field_mappings.is_empty() {
        return Err(ConfigError::Invalid(
            format!("{}: at least one field_mapping required", stream_id)
        ));
    }

    // Validate each field mapping
    for mapping in &config.field_mappings {
        if mapping.source_path.is_empty() {
            return Err(ConfigError::Invalid(
                format!("{}: source_path required", stream_id)
            ));
        }
        if mapping.target_column.is_empty() {
            return Err(ConfigError::Invalid(
                format!("{}: target_column required", stream_id)
            ));
        }
        // Validate column_type is known
        validate_column_type(&mapping.column_type)?;
    }

    Ok(())
}
```

---

### 4.2 Runtime Validation

```rust
/// Validate Silver table schema matches config
pub async fn validate_silver_schema(
    pool: &Pool<Postgres>,
    config: &SilverEtlConfig,
) -> Result<(), SchemaError> {
    // Get actual table columns
    let actual_columns: Vec<(String, String)> = sqlx::query_as(
        "SELECT column_name, data_type FROM information_schema.columns
         WHERE table_schema || '.' || table_name = $1"
    )
    .bind(&config.target_table)
    .fetch_all(pool)
    .await?;

    // Check each mapping has corresponding column
    for mapping in &config.field_mappings {
        let found = actual_columns.iter().any(|(name, data_type)| {
            name == &mapping.target_column &&
            types_compatible(data_type, &mapping.column_type)
        });

        if !found {
            return Err(SchemaError::ColumnMissing {
                table: config.target_table.clone(),
                column: mapping.target_column.clone(),
                expected_type: mapping.column_type.clone(),
            });
        }
    }

    Ok(())
}
```

---

## 5. Operational Concerns

### 5.1 Logging Strategy

```rust
// Structured logging with tracing
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self, point), fields(subscriber_id = %self.id))]
async fn process_point(&mut self, point: &RawDataPoint) -> Result<(), SubscriberError> {
    debug!(
        stream_id = %point.stream_id(),
        ndp_id = ?point.ndp_id,
        "Processing point"
    );

    // ... processing

    Ok(())
}

// Log levels:
// ERROR: Data loss risk, intervention needed
// WARN:  Degraded operation, automatic recovery expected
// INFO:  Normal lifecycle events (start, stop, catch-up)
// DEBUG: Per-message processing details
// TRACE: Very detailed internal state
```

---

### 5.2 Metrics Strategy

```rust
// Key metrics to expose via Prometheus

// Event Bus
counter!("ndp_event_bus_published_total").increment(1);
counter!("ndp_event_bus_lagged_total").increment(n);
gauge!("ndp_event_bus_subscriber_count").set(count);

// Per-Subscriber
counter!("ndp_subscriber_processed_total", "subscriber" => id).increment(1);
counter!("ndp_subscriber_errors_total", "subscriber" => id, "error_type" => type).increment(1);
gauge!("ndp_subscriber_buffer_size", "subscriber" => id).set(size);
histogram!("ndp_subscriber_processing_time_seconds", "subscriber" => id).record(duration);

// Silver-Specific
counter!("ndp_silver_transform_errors_total", "stream" => stream_id).increment(1);
counter!("ndp_silver_upsert_rows_total").increment(count);
gauge!("ndp_silver_catchup_progress").set(pct);

// Threshold-Specific
counter!("ndp_threshold_alerts_total", "rule" => rule_name, "severity" => sev).increment(1);
counter!("ndp_threshold_cooldown_suppressed_total", "rule" => rule_name).increment(1);
```

---

### 5.3 Health Checks

```rust
/// Comprehensive health check for /health endpoint
pub async fn check_health(coordinator: &SubscriberCoordinator) -> HealthReport {
    let mut report = HealthReport::default();

    // Event bus health
    let sub_count = coordinator.event_bus.subscriber_count();
    if sub_count == 0 {
        report.add_warning("event_bus", "No subscribers connected");
    }

    // Per-subscriber health
    for (id, status) in coordinator.health_status().await {
        match status {
            HealthStatus::Healthy => {
                report.add_healthy(&id);
            }
            HealthStatus::Degraded => {
                report.add_warning(&id, "Subscriber degraded");
            }
            HealthStatus::Unhealthy => {
                report.add_error(&id, "Subscriber unhealthy");
            }
        }
    }

    // Database connectivity (for Silver)
    if let Err(e) = check_db_connectivity().await {
        report.add_error("timescaledb", &e.to_string());
    }

    // MQTT connectivity (for alerts and notifier)
    if let Err(e) = check_mqtt_connectivity().await {
        report.add_warning("mqtt", &e.to_string()); // Warning, not error
    }

    report
}
```

---

### 5.4 Graceful Shutdown

```rust
/// Graceful shutdown sequence
pub async fn shutdown(coordinator: &mut SubscriberCoordinator) -> Result<(), ShutdownError> {
    info!("Initiating graceful shutdown...");

    // 1. Stop accepting new events
    // (Sources should be stopped first, before this)

    // 2. Allow in-flight events to drain (short timeout)
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 3. Stop all subscribers
    if let Err(e) = coordinator.stop_all().await {
        warn!("Error during subscriber shutdown: {}", e);
    }

    // 4. Wait for all tasks to complete (with timeout)
    let timeout = Duration::from_secs(30);
    match tokio::time::timeout(timeout, coordinator.wait_all()).await {
        Ok(Ok(())) => info!("All subscribers stopped cleanly"),
        Ok(Err(e)) => warn!("Subscriber error during shutdown: {}", e),
        Err(_) => error!("Shutdown timeout after {:?}", timeout),
    }

    info!("Shutdown complete");
    Ok(())
}
```

---

## 6. Security Refinements

### 6.1 Input Validation

```rust
/// Validate RawDataPoint before publishing
pub fn validate_point(point: &RawDataPoint) -> Result<(), ValidationError> {
    // Source ID format
    if !point.source_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(ValidationError::InvalidSourceId);
    }

    // Timestamp sanity
    let now = Utc::now();
    let one_day = Duration::from_secs(86400);
    if point.timestamp > now + one_day {
        return Err(ValidationError::FutureTimestamp);
    }
    if point.timestamp < now - Duration::from_secs(86400 * 365) {
        return Err(ValidationError::AncientTimestamp);
    }

    // Payload size
    let payload_size = point.raw_payload.to_string().len();
    if payload_size > 1_000_000 {
        return Err(ValidationError::PayloadTooLarge);
    }

    Ok(())
}
```

---

### 6.2 SQL Injection Prevention

```rust
// All SQL uses parameterized queries
// NEVER interpolate user data into SQL strings

// WRONG:
let sql = format!("SELECT * FROM {} WHERE id = '{}'", table, id);

// RIGHT:
let sql = "SELECT * FROM silver.air_quality_observations WHERE ndp_id = $1";
sqlx::query(sql).bind(&ndp_id).fetch_all(pool).await?;

// Table names must be validated against whitelist
fn validate_table_name(name: &str) -> Result<(), ValidationError> {
    let allowed = ["silver.air_quality_observations", "silver.outdoor_weather_observations"];
    if !allowed.contains(&name) {
        return Err(ValidationError::InvalidTableName);
    }
    Ok(())
}
```

---

## 7. Testing Refinements

### 7.1 Test Categories

```rust
// Unit tests: Fast, isolated, use mocks
#[cfg(test)]
mod unit_tests {
    // Test transform logic
    #[test]
    fn test_transform_extracts_field() { ... }

    // Test condition evaluation
    #[test]
    fn test_threshold_condition_greater_than() { ... }

    // Test DQ rules
    #[test]
    fn test_dq_range_check_flags_out_of_range() { ... }
}

// Component tests: In-memory stores, test subscriber behavior
#[cfg(test)]
mod component_tests {
    #[tokio::test]
    async fn test_bronze_subscriber_batches_writes() { ... }

    #[tokio::test]
    async fn test_silver_subscriber_catches_up() { ... }
}

// Integration tests: Real DB, real MQTT
#[cfg(test)]
#[ignore] // Run with --ignored
mod integration_tests {
    #[tokio::test]
    async fn test_end_to_end_event_flow() { ... }
}
```

---

### 7.2 Test Fixtures

```rust
// Reusable test fixtures
pub mod test_fixtures {
    pub fn test_raw_point() -> RawDataPoint {
        RawDataPoint {
            source_id: "test-Http".to_string(),
            timestamp: Utc::now(),
            raw_payload: json!({
                "pm02Compensated": 12.5,
                "rco2": 450,
                "temperature": 22.5
            }),
            ndp_id: Some("sensor-001".to_string()),
            location_id: Some("home".to_string()),
            received_at: Utc::now(),
        }
    }

    pub fn test_silver_etl_config() -> SilverEtlConfig {
        SilverEtlConfig {
            target_table: "silver.test_observations".to_string(),
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02Compensated".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double precision".to_string(),
                    ..Default::default()
                }
            ],
            ..Default::default()
        }
    }

    pub fn test_threshold_rule() -> ThresholdRule {
        ThresholdRule {
            name: "test_rule".to_string(),
            field: "raw_payload.pm02Compensated".to_string(),
            condition: "> 35.4".to_string(),
            severity: Severity::Warning,
            message: "Test alert".to_string(),
            cooldown_secs: 300,
            stream_filter: None,
        }
    }
}
```

---

## 8. Documentation Refinements

### 8.1 Inline Documentation Requirements

Every public type and function must have:
- Brief description
- Arguments (for functions)
- Returns (for functions)
- Errors (if can fail)
- Example (for complex APIs)

```rust
/// Transform a RawDataPoint to a Silver row using SilverEtlConfig.
///
/// This is the streaming equivalent of silver-etl's SQL-based transforms.
/// Uses the same configuration format for consistency.
///
/// # Arguments
/// * `point` - Source RawDataPoint from event bus
/// * `config` - SilverEtlConfig defining field mappings and transforms
///
/// # Returns
/// * `Ok(SilverRow)` - Successfully transformed row ready for UPSERT
/// * `Err(TransformError)` - Transform failed (see error variants)
///
/// # Errors
/// * `TransformError::FieldExtraction` - JSON path extraction failed
/// * `TransformError::TypeConversion` - Value couldn't be cast to target type
/// * `TransformError::RequiredFieldMissing` - Required field not present
///
/// # Example
/// ```rust
/// let point = RawDataPoint::new("air-quality-Mqtt", json!({"pm02Compensated": 12.5}));
/// let row = transform_to_silver(&point, &config)?;
/// assert_eq!(row.get("pm25"), Some(&SqlValue::DoublePrecision(12.5)));
/// ```
pub fn transform_to_silver(
    point: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<SilverRow, TransformError> {
    // ...
}
```

---

### 8.2 Configuration Reference

All configuration options documented in:
1. Inline code comments
2. JSON Schema definitions
3. Example YAML files with comments

```yaml
# config/base/platform.yaml - Reference Configuration

# Event bus configuration
event_bus:
  # Broadcast channel capacity
  # Higher = more memory, more buffer for slow subscribers
  # Lower = less memory, more lag events for slow subscribers
  # Recommendation: 10000 for most deployments
  capacity: 10000

  # Threshold for lag warnings (events behind)
  # When a subscriber is this many events behind, log a warning
  lag_warning_threshold: 1000

# Subscriber definitions
subscribers:
  # Bronze subscriber - writes raw data to Parquet
  # This should always be enabled for data durability
  - id: bronze
    type: storage
    enabled: true
    config:
      format: parquet
      path: /data/raw/{stream_id}
      partitioning: daily
      # Batch size: number of points before flush
      # Higher = better throughput, higher latency
      # Lower = lower latency, more I/O operations
      batch_size: 50
      # Timeout for partial batch flush
      batch_timeout_secs: 2
      # Write-ahead log for durability
      wal_enabled: true
```

---

*Refinement document created: 2026-01-18*
*SPARC Planning Complete - Ready for Implementation*
