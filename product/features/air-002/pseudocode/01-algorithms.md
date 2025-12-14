# AIR-002: Ingestion Pipeline - Pseudocode & Algorithms

## Overview

This document defines the orchestration algorithms for the air quality data ingestion pipeline, connecting MQTT source, parser, validator, adapter, and Parquet storage components.

---

## 1. Main Application Startup

### Algorithm: Application Initialization

```rust
ALGORITHM: main()
INPUT: None
OUTPUT: Running application or error

BEGIN
    // Phase 1: Initialize logging and tracing
    init_tracing_subscriber()
    LOG("Starting air quality ingestion platform")

    // Phase 2: Load configuration
    config ← load_config("config.yaml")
    IF config is Error THEN
        WARN("Failed to load config, using defaults")
        config ← AppConfig::default_config()
    END IF

    // Phase 3: Initialize storage with WAL replay
    storage_path ← config.storage.base_path
    storage ← ParquetStore::new(storage_path)
    TRY
        storage.replay_wal().await  // Recover any uncommitted writes
        INFO("WAL replay completed")
    CATCH error
        ERROR("WAL replay failed: {}", error)
        RETURN error
    END TRY

    // Phase 4: Initialize MQTT source
    mqtt_config ← MqttConfig {
        broker_url: config.mqtt.broker_url,
        port: config.mqtt.port,
        client_id: config.mqtt.client_id,
        topic_pattern: "airgradient/readings/{SERIAL_NUMBER}",
        qos: QoS::AtLeastOnce,
        buffer_capacity: 1000
    }
    mqtt_source ← MqttSource::new(mqtt_config)

    // Phase 5: Create shared state for API and ingestion
    shared_state ← Arc::new(RwLock::new(AppState {
        storage: storage,
        recent_readings: LRUCache::new(100),
        health_status: HealthStatus::healthy(),
        metrics: IngestionMetrics::new()
    }))

    // Phase 6: Spawn background ingestion task
    ingestion_handle ← tokio::spawn(ingestion_loop(
        mqtt_source.clone(),
        shared_state.clone()
    ))

    // Phase 7: Start API server
    api_addr ← format!("{}:{}", config.server.host, config.server.port)
    api_handle ← tokio::spawn(start_api_server(
        api_addr,
        shared_state.clone()
    ))

    // Phase 8: Setup graceful shutdown
    shutdown_signal ← setup_shutdown_handler()

    // Phase 9: Wait for shutdown or error
    SELECT
        CASE shutdown_signal.await:
            INFO("Shutdown signal received")
            graceful_shutdown(mqtt_source, storage, ingestion_handle, api_handle).await
        CASE ingestion_handle.await:
            ERROR("Ingestion task terminated unexpectedly")
        CASE api_handle.await:
            ERROR("API server terminated unexpectedly")
    END SELECT

    INFO("Application shutdown complete")
    RETURN Ok(())
END
```

**Time Complexity**: O(n) where n = number of WAL entries to replay
**Space Complexity**: O(1) + O(cache_size) for LRU cache

---

## 2. Ingestion Loop

### Algorithm: Real-time Data Ingestion

```rust
ALGORITHM: ingestion_loop(mqtt_source, shared_state)
INPUT: mqtt_source (MqttSource), shared_state (Arc<AppState>)
OUTPUT: Continuous processing or error

CONSTANTS:
    BATCH_SIZE = 100
    BATCH_TIMEOUT = Duration::seconds(5)
    MAX_RETRIES = 3
    RETRY_DELAY_BASE = Duration::seconds(1)

BEGIN
    // Phase 1: Start MQTT connection
    TRY
        mqtt_source.start().await
        INFO("MQTT source started successfully")
    CATCH error
        ERROR("Failed to start MQTT source: {}", error)
        RETURN error
    END TRY

    // Initialize batch accumulator
    batch_buffer ← Vec::with_capacity(BATCH_SIZE)
    last_flush ← Utc::now()

    // Phase 2: Main ingestion loop
    WHILE NOT shutdown_requested DO
        // Sub-algorithm: Fetch MQTT messages
        TRY
            raw_points ← mqtt_source.fetch().await

            IF raw_points.is_empty() THEN
                // No data, check if we should flush partial batch
                IF batch_buffer.len() > 0 AND
                   (Utc::now() - last_flush) > BATCH_TIMEOUT THEN
                    flush_batch(batch_buffer, shared_state).await
                    batch_buffer.clear()
                    last_flush ← Utc::now()
                END IF

                sleep(Duration::milliseconds(100))  // Backpressure
                CONTINUE
            END IF

            // Phase 3: Process each point through pipeline
            FOR EACH raw_point IN raw_points DO
                result ← process_single_point(raw_point, shared_state).await

                IF result is Ok(processed_points) THEN
                    batch_buffer.extend(processed_points)

                    // Update metrics
                    metrics ← shared_state.write().await.metrics
                    metrics.points_ingested += processed_points.len()
                    metrics.last_success_time ← Utc::now()
                ELSE IF result is Err(error) THEN
                    // Send to Dead Letter Queue
                    dlq_entry ← DLQEntry {
                        raw_data: raw_point,
                        error: error.to_string(),
                        timestamp: Utc::now(),
                        retry_count: 0
                    }
                    send_to_dlq(dlq_entry).await

                    // Update error metrics
                    metrics ← shared_state.write().await.metrics
                    metrics.points_failed += 1
                END IF
            END FOR

            // Phase 4: Batch flushing logic
            IF batch_buffer.len() >= BATCH_SIZE OR
               (Utc::now() - last_flush) > BATCH_TIMEOUT THEN
                flush_batch(batch_buffer, shared_state).await
                batch_buffer.clear()
                last_flush ← Utc::now()
            END IF

        CATCH MqttError as mqtt_error
            ERROR("MQTT error: {}", mqtt_error)
            // Connection will auto-reconnect via MqttSource
            sleep(Duration::seconds(1))

        CATCH StorageError as storage_error
            ERROR("Storage error: {}", storage_error)
            // Data is safe in WAL, will retry on next batch

        END TRY
    END WHILE

    // Phase 5: Cleanup on shutdown
    INFO("Flushing remaining {} points before shutdown", batch_buffer.len())
    IF batch_buffer.len() > 0 THEN
        flush_batch(batch_buffer, shared_state).await
    END IF

    mqtt_source.stop().await
    INFO("Ingestion loop terminated gracefully")
END

// Sub-algorithm: Process single MQTT point through pipeline
SUBROUTINE: process_single_point(raw_point, shared_state)
INPUT: raw_point (TimeSeriesPoint), shared_state (Arc<AppState>)
OUTPUT: Result<Vec<TimeSeriesPoint>, ProcessingError>

BEGIN
    // Step 1: Extract JSON payload (already in TimeSeriesPoint tags)
    // MqttSource.parse_payload() already did JSON → TimeSeriesPoint
    // We need to reverse-engineer to AirQualityReading for validation

    // Step 2: Parse to domain model
    // Note: In actual implementation, MQTT should send raw JSON,
    // not pre-parsed TimeSeriesPoints
    json_payload ← raw_point.metadata.get("raw_json")

    TRY
        reading ← parser::parse_mqtt_payload(json_payload)
    CATCH ParserError as error
        RETURN Err(ProcessingError::Parse(error))
    END TRY

    // Step 3: Validate against sensor specifications
    TRY
        validation::validate_reading(&reading)
    CATCH ValidationError as error
        RETURN Err(ProcessingError::Validation(error))
    END TRY

    // Step 4: Adapt to time series points
    time_series_points ← adapter::to_time_series_points(&reading)

    // Step 5: Update recent readings cache
    state ← shared_state.write().await
    state.recent_readings.put(reading.device.serialno, reading)

    RETURN Ok(time_series_points)
END

// Sub-algorithm: Flush batch to storage
SUBROUTINE: flush_batch(points, shared_state)
INPUT: points (Vec<TimeSeriesPoint>), shared_state (Arc<AppState>)
OUTPUT: Result<(), StorageError>

BEGIN
    IF points.is_empty() THEN
        RETURN Ok(())
    END IF

    DEBUG("Flushing batch of {} points", points.len())
    start_time ← Utc::now()

    state ← shared_state.read().await
    storage ← state.storage

    TRY
        // WAL write happens inside storage.write_batch()
        storage.write_batch(points.clone()).await

        // Update metrics
        duration ← Utc::now() - start_time
        metrics ← shared_state.write().await.metrics
        metrics.batches_written += 1
        metrics.avg_batch_duration ← update_moving_average(
            metrics.avg_batch_duration,
            duration
        )

        INFO("Batch written successfully in {}ms", duration.num_milliseconds())
        RETURN Ok(())

    CATCH error
        ERROR("Batch write failed: {}", error)
        // Points are safe in WAL, will be replayed on restart
        RETURN Err(error)
    END TRY
END
```

**Time Complexity**:
- Per point: O(1) for parse + O(k) for validate where k = number of metrics
- Per batch: O(n log n) for storage write (Parquet sorting)

**Space Complexity**: O(BATCH_SIZE) for accumulator buffer

---

## 3. Error Recovery & Resilience

### Algorithm: Connection Recovery (Exponential Backoff)

```rust
ALGORITHM: mqtt_reconnect(mqtt_source, attempt)
INPUT: mqtt_source (MqttSource), attempt (u32)
OUTPUT: Result<(), MqttError>

CONSTANTS:
    BASE_DELAY = Duration::seconds(1)
    MAX_DELAY = Duration::seconds(30)
    MAX_ATTEMPTS = 10

BEGIN
    IF attempt >= MAX_ATTEMPTS THEN
        ERROR("Max reconnection attempts reached")
        RETURN Err(MqttError::MaxRetriesExceeded)
    END IF

    // Calculate exponential backoff: min(base * 2^attempt, max)
    delay_secs ← min(
        BASE_DELAY.as_secs() * 2^attempt,
        MAX_DELAY.as_secs()
    )

    WARN("Reconnecting in {} seconds (attempt {})", delay_secs, attempt)
    sleep(Duration::seconds(delay_secs))

    // Create new MQTT connection
    mqtt_options ← MqttOptions::new(
        mqtt_source.config.client_id,
        mqtt_source.config.broker_url,
        mqtt_source.config.port
    )
    mqtt_options.set_keep_alive(Duration::seconds(30))

    TRY
        (client, event_loop) ← AsyncClient::new(
            mqtt_options,
            mqtt_source.config.buffer_capacity
        )

        // Subscribe to topic with wildcard
        topic ← mqtt_source.config.topic_pattern.replace(
            "{SERIAL_NUMBER}",
            "+"
        )
        client.subscribe(topic, mqtt_source.config.qos).await

        // Update source state
        mqtt_source.client ← Some(client)
        mqtt_source.event_loop ← Some(event_loop)
        mqtt_source.connection_healthy ← true

        INFO("MQTT reconnection successful")
        RETURN Ok(())

    CATCH error
        ERROR("Reconnection failed: {}", error)
        RETURN Err(error)
    END TRY
END
```

**Time Complexity**: O(1) per attempt
**Delay Progression**: 1s → 2s → 4s → 8s → 16s → 30s (capped)

### Algorithm: WAL Replay on Startup

```rust
ALGORITHM: replay_wal(storage)
INPUT: storage (ParquetStore)
OUTPUT: Result<(), StorageError>

BEGIN
    INFO("Starting WAL replay")

    // Phase 1: Read all WAL entries
    wal ← storage.wal.lock().await
    entries ← wal.replay()
    entry_count ← entries.len()

    IF entry_count == 0 THEN
        INFO("WAL is empty, nothing to replay")
        RETURN Ok(())
    END IF

    INFO("Replaying {} WAL entries", entry_count)

    // Phase 2: Deserialize entries to points
    points ← Vec::with_capacity(entry_count)
    failed_count ← 0

    FOR EACH entry IN entries DO
        TRY
            point ← serde_json::from_slice(entry)
            points.push(point)
        CATCH error
            WARN("Failed to deserialize WAL entry: {}", error)
            failed_count += 1
        END TRY
    END FOR

    // Phase 3: Write batch to storage (without re-WAL-ing)
    IF points.len() > 0 THEN
        TRY
            storage.write_batch(points).await
            INFO("Successfully replayed {} points", points.len())
        CATCH error
            ERROR("Failed to write replayed points: {}", error)
            RETURN Err(error)
        END TRY
    END IF

    // Phase 4: Commit WAL (truncate log)
    wal ← storage.wal.lock().await
    wal.commit()

    INFO("WAL replay complete. {} entries processed, {} failed",
         entry_count, failed_count)

    RETURN Ok(())
END
```

**Time Complexity**: O(n) where n = WAL entry count
**Space Complexity**: O(n) for point buffer

### Algorithm: Graceful Shutdown

```rust
ALGORITHM: graceful_shutdown(mqtt_source, storage, ingestion_task, api_task)
INPUT: mqtt_source, storage, ingestion_task, api_task
OUTPUT: Result<(), Error>

CONSTANTS:
    SHUTDOWN_TIMEOUT = Duration::seconds(30)

BEGIN
    INFO("Initiating graceful shutdown")

    // Phase 1: Stop accepting new MQTT messages
    INFO("Stopping MQTT source...")
    TRY_WITH_TIMEOUT(SHUTDOWN_TIMEOUT / 3)
        mqtt_source.stop().await
        INFO("MQTT source stopped")
    CATCH timeout
        WARN("MQTT stop timed out, forcing disconnect")
        mqtt_source.force_disconnect()
    END TRY_WITH_TIMEOUT

    // Phase 2: Wait for ingestion task to flush remaining data
    INFO("Waiting for ingestion task to complete...")
    TRY_WITH_TIMEOUT(SHUTDOWN_TIMEOUT / 3)
        ingestion_task.abort_with_grace()
        ingestion_task.await
        INFO("Ingestion task completed")
    CATCH timeout
        WARN("Ingestion task timed out, aborting")
        ingestion_task.abort()
    END TRY_WITH_TIMEOUT

    // Phase 3: Ensure WAL is committed
    INFO("Committing WAL...")
    wal ← storage.wal.lock().await
    wal.commit()
    INFO("WAL committed")

    // Phase 4: Stop API server
    INFO("Stopping API server...")
    TRY_WITH_TIMEOUT(SHUTDOWN_TIMEOUT / 3)
        api_task.abort_with_grace()
        api_task.await
        INFO("API server stopped")
    CATCH timeout
        WARN("API server timed out, aborting")
        api_task.abort()
    END TRY_WITH_TIMEOUT

    INFO("Graceful shutdown complete")
    RETURN Ok(())
END
```

**Time Complexity**: O(1) with bounded timeout
**Total Shutdown Time**: ≤ 30 seconds

---

## 4. State Management

### Data Structure: Shared Application State

```rust
STRUCTURE: AppState
FIELDS:
    storage: Arc<ParquetStore>
    recent_readings: LRUCache<String, AirQualityReading>
    health_status: HealthStatus
    metrics: IngestionMetrics
    shutdown_signal: Arc<Notify>

INVARIANTS:
    - storage is always initialized
    - recent_readings capacity = 100 (LRU eviction)
    - metrics are monotonically increasing
    - health_status reflects actual system state

SYNCHRONIZATION:
    Type: Arc<RwLock<AppState>>
    Read operations: concurrent (RwLock::read())
    Write operations: exclusive (RwLock::write())
```

### Data Structure: LRU Cache for Recent Readings

```rust
STRUCTURE: LRUCache<K, V>
CAPACITY: 100
EVICTION: Least Recently Used

OPERATIONS:
    get(key: K): Option<V>
        Time: O(1)
        Updates access timestamp

    put(key: K, value: V): Option<V>
        Time: O(1)
        Returns evicted value if at capacity

    remove(key: K): Option<V>
        Time: O(1)

IMPLEMENTATION:
    Internal: HashMap + DoublyLinkedList
    Space: O(capacity)
```

### Data Structure: Ingestion Metrics

```rust
STRUCTURE: IngestionMetrics
FIELDS:
    points_ingested: AtomicU64
    points_failed: AtomicU64
    batches_written: AtomicU64
    avg_batch_duration: AtomicU64  // microseconds
    last_success_time: AtomicI64   // timestamp
    last_failure_time: AtomicI64   // timestamp

OPERATIONS:
    increment_ingested(count: u64):
        points_ingested.fetch_add(count, Ordering::Relaxed)

    update_average_duration(new_duration: Duration):
        // Exponential moving average
        alpha ← 0.1
        old_avg ← avg_batch_duration.load()
        new_avg ← alpha * new_duration + (1 - alpha) * old_avg
        avg_batch_duration.store(new_avg)

    get_ingestion_rate(): f64
        // Points per second over last minute
        recent_points ← points_ingested - points_at_minute_ago
        RETURN recent_points / 60.0
```

---

## 5. Performance Characteristics

### Throughput Analysis

```
SCENARIO: High-throughput ingestion

Given:
    - MQTT messages: 100 sensors × 1 reading/min = 100 msg/min
    - Each reading: 5 metrics = 500 points/min ≈ 8.3 points/sec
    - Batch size: 100 points
    - Batch timeout: 5 seconds

Expected:
    - Batch frequency: ~12 batches/min (500 points / 100 per batch)
    - Storage write latency: 50-200ms per batch (Parquet write)
    - Memory usage: ~2MB for batch buffer (100 points × 20KB each)

Bottleneck:
    - Parquet write if batch_frequency > 1/write_latency
    - Solution: Increase batch size or use async parallel writes
```

### Memory Usage

```
COMPONENT MEMORY BREAKDOWN:

1. MQTT Buffer:
   - Capacity: 1000 messages
   - Size per message: ~1KB (JSON payload)
   - Total: ~1MB

2. Batch Accumulator:
   - Capacity: 100 points
   - Size per point: ~200 bytes
   - Total: ~20KB

3. Recent Readings Cache:
   - Capacity: 100 readings
   - Size per reading: ~500 bytes
   - Total: ~50KB

4. WAL Buffer:
   - Unbounded until commit
   - Size: depends on batch write frequency
   - Typical: 100-500KB between commits

TOTAL STEADY STATE: ~2-5MB
PEAK (with backlog): ~10-20MB
```

### Latency Budget

```
END-TO-END LATENCY (MQTT → Storage):

Component                 | Latency    | Notes
--------------------------|------------|---------------------------
MQTT receive              | 1-5ms      | Network + parse
JSON parse                | 0.1-0.5ms  | serde_json
Validation                | 0.05-0.1ms | Simple range checks
Adapter transform         | 0.1-0.5ms  | 5-15 metrics
Batch accumulation        | 0-5000ms   | Depends on batch timeout
WAL write                 | 1-5ms      | Sequential file append
Parquet write             | 50-200ms   | Compression + columnar
--------------------------|------------|---------------------------
TOTAL (p50)               | ~100ms     | Without batch wait
TOTAL (p99)               | ~5s        | With full batch timeout
```

---

## 6. Complexity Summary

| Algorithm               | Time Complexity | Space Complexity | Notes                    |
|-------------------------|----------------|------------------|--------------------------|
| main()                  | O(w)           | O(c)             | w=WAL entries, c=cache   |
| ingestion_loop()        | O(∞)           | O(b)             | b=batch size             |
| process_single_point()  | O(m)           | O(m)             | m=metrics per reading    |
| flush_batch()           | O(n log n)     | O(n)             | Parquet sorting          |
| mqtt_reconnect()        | O(1)           | O(1)             | Per attempt              |
| replay_wal()            | O(w)           | O(w)             | w=WAL entries            |
| graceful_shutdown()     | O(1)           | O(1)             | Bounded timeout          |

---

## 7. Error Handling Strategy

### Error Classification

```
ERROR TYPE HIERARCHY:

1. TRANSIENT (Retry):
   - NetworkError: MQTT connection lost
   - StorageError: Disk temporarily full
   - Action: Exponential backoff retry
   - Limit: MAX_RETRIES = 3

2. PERMANENT (DLQ):
   - ParserError: Malformed JSON
   - ValidationError: Out-of-range values
   - Action: Send to Dead Letter Queue
   - No retry

3. FATAL (Shutdown):
   - ConfigError: Invalid configuration
   - InitializationError: Cannot create storage
   - Action: Log and terminate gracefully
```

### Dead Letter Queue Schema

```rust
STRUCTURE: DLQEntry
FIELDS:
    id: Uuid
    raw_data: Vec<u8>
    error_type: String
    error_message: String
    timestamp: DateTime<Utc>
    retry_count: u32
    metadata: HashMap<String, String>

STORAGE: Separate Parquet table
PATH: {base_path}/dlq/year={year}/month={month}/errors.parquet

OPERATIONS:
    send_to_dlq(entry: DLQEntry):
        Time: O(1) - async write

    query_dlq(start, end): Vec<DLQEntry>
        Time: O(n) where n = entries in range

    retry_dlq_entry(id: Uuid):
        Reprocess entry through ingestion pipeline
```

---

## 8. Monitoring & Observability

### Metrics to Track

```rust
METRICS (Prometheus format):

# Counter: Total points successfully ingested
air_quality_points_ingested_total{location_id, metric_type}

# Counter: Total points failed
air_quality_points_failed_total{error_type}

# Histogram: Batch write latency
air_quality_batch_write_duration_seconds{quantile}

# Gauge: Current batch buffer size
air_quality_batch_buffer_size

# Gauge: MQTT connection status
air_quality_mqtt_connected{status}

# Counter: WAL replay events
air_quality_wal_replay_total{status}

# Gauge: Recent readings cache hit rate
air_quality_cache_hit_rate
```

### Health Check Algorithm

```rust
ALGORITHM: health_check()
OUTPUT: HealthStatus

BEGIN
    checks ← []

    // Check MQTT connection
    mqtt_health ← mqtt_source.health_check().await
    checks.push(mqtt_health)

    // Check storage availability
    storage_health ← storage.health_check().await
    checks.push(storage_health)

    // Check recent ingestion activity
    time_since_last ← Utc::now() - metrics.last_success_time
    IF time_since_last > Duration::minutes(5) THEN
        checks.push(HealthStatus {
            healthy: false,
            message: "No data ingested in 5 minutes"
        })
    END IF

    // Overall health = all checks pass
    overall_healthy ← checks.all(|c| c.healthy)

    RETURN HealthStatus {
        healthy: overall_healthy,
        message: IF overall_healthy THEN "Healthy" ELSE "Degraded",
        details: checks
    }
END
```

---

## 9. Testing Strategy

### Unit Tests

```
TEST: parse_mqtt_payload_success()
    Given: Valid JSON payload
    When: parser::parse_mqtt_payload(json)
    Then: Returns Ok(AirQualityReading)
    Complexity: O(1)

TEST: validate_reading_out_of_range()
    Given: Reading with CO2 > 10,000 ppm
    When: validation::validate_reading(&reading)
    Then: Returns Err(ValidationError::Co2OutOfRange)

TEST: adapter_creates_time_series_points()
    Given: Valid AirQualityReading with 5 metrics
    When: adapter::to_time_series_points(&reading)
    Then: Returns Vec<TimeSeriesPoint> with length 5
```

### Integration Tests

```
TEST: end_to_end_ingestion()
    Given: MQTT broker running
    When: Publish reading to topic
    Then: Reading appears in Parquet storage within 5s

TEST: wal_replay_after_crash()
    Given: WAL with uncommitted entries
    When: Application restarts
    Then: All WAL entries written to Parquet

TEST: graceful_shutdown_flushes_batch()
    Given: Batch with 50 points accumulated
    When: Shutdown signal received
    Then: All 50 points written before exit
```

---

## References

- MQTT Source: `/workspaces/neural-data-platform/core/src/sources/mqtt.rs`
- Parser: `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs`
- Validator: `/workspaces/neural-data-platform/domains/air-quality/src/validation.rs`
- Adapter: `/workspaces/neural-data-platform/domains/air-quality/src/adapter.rs`
- Storage: `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
- Main App: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

---

**Document Status**: Complete
**Author**: SPARC Pseudocode Agent
**Date**: 2025-12-14
**Version**: 1.0
