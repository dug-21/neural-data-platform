# Latency Optimization Research - Neural Data Platform

**Date**: 2026-01-17
**Author**: ndp-rust-dev (Research Mode)
**Scope**: End-to-end ingestion latency analysis and optimization opportunities

## Executive Summary

This document analyzes the Neural Data Platform's data flow from ingestion to queryable data, identifying bottlenecks and recommending optimizations to reduce end-to-end latency.

**Current Latency Profile (Estimated)**:
- MQTT ingestion to Bronze: 5-10 seconds (batch timeout)
- HTTP polling to Bronze: 10-600+ seconds (poll interval)
- Bronze to Silver ETL: 5 minutes (daemon interval)
- **Total**: 5-10 minutes typical, up to 15 minutes worst case

**Target**: Reduce to 30-60 seconds for MQTT streams, 2-3 minutes for HTTP polling streams.

---

## 1. Current Architecture Analysis

### 1.1 Ingestion Pipeline (Bronze Layer)

**MQTT Sources** (`core/src/sources/mqtt/`):
- Real-time event-driven via rumqttc async client
- Messages routed through `MqttRouter` to appropriate handlers
- Data flows through mpsc channels to storage writer

**HTTP Polling Sources** (`core/src/sources/http_poll.rs`):
- Configurable poll intervals (default: 60s, weather: 600s)
- Sequential sensor polling (AIR-010 note: parallel attempted, deferred due to lifetime issues)
- Retry with exponential backoff (default: 3 retries, 1-60s delay)

**Storage Writer** (`apps/air-quality-app/src/pipeline/storage_writer.rs`):
- **Batch size**: 100 points (configurable)
- **Batch timeout**: 5 seconds (configurable)
- Uses `tokio::select!` for either batch-full or timeout-triggered flush
- `std::mem::take` optimization to avoid buffer cloning

**Parquet Storage** (`core/src/storage/parquet.rs`):
- WAL (Write-Ahead Log) with immediate flush per entry
- Parquet writes use `spawn_blocking` for CPU-intensive serialization
- Snappy compression enabled
- Day-partitioned storage (`year=YYYY/month=MM/day=DD/`)

### 1.2 ETL Pipeline (Silver Layer)

**ETL Daemon** (`apps/silver-etl/src/daemon.rs`):
- **Default interval**: 300 seconds (5 minutes)
- Processes all enabled streams sequentially per cycle
- Watermark-based incremental loading

**ETL Execution** (`apps/silver-etl/src/etl.rs`):
- DuckDB in-memory with PostgreSQL extension for TimescaleDB writes
- Parquet files read via glob pattern: `{bronze_path}/{stream_id}/**/*.parquet`
- SQL generation with configurable transforms, DQ rules, deduplication

**Configuration** (stream configs):
- Incremental: `lag_interval: 5 minutes` (lookback window)
- Deduplication: `ON CONFLICT (observation_time, ndp_id) DO UPDATE SET`
- DQ output: Inline `dq_flags` column (TEXT[])

### 1.3 Database Layer (TimescaleDB)

**Hypertables**:
- 1-day chunk intervals (optimized for Pi memory constraints)
- Primary keys on (observation_time, ndp_id)
- GIN indexes on dq_flags for DQ queries

**Write Pattern**:
- UPSERT via `ON CONFLICT DO UPDATE SET`
- No continuous aggregates defined (opportunity)

---

## 2. Identified Bottlenecks

### 2.1 High Impact Bottlenecks

| Bottleneck | Current | Impact | Component |
|------------|---------|--------|-----------|
| ETL daemon interval | 5 minutes | Primary latency contributor | silver-etl |
| HTTP polling interval | 600s (weather) | Limits data freshness | http_poll.rs |
| Batch timeout | 5 seconds | Delays low-rate streams | storage_writer.rs |
| WAL per-entry flush | Every write | I/O overhead | wal.rs |
| Sequential stream processing | All streams in series | Cycle duration scales linearly | daemon.rs |

### 2.2 Medium Impact Bottlenecks

| Bottleneck | Current | Impact | Component |
|------------|---------|--------|-----------|
| Sequential sensor polling | One at a time | Limits HTTP throughput | http_poll.rs |
| Parquet append read-modify-write | Full file rewrite | I/O amplification | parquet.rs |
| DuckDB per-stream connection | New connection per stream | Connection overhead | etl.rs |

### 2.3 Lower Impact (But Worth Noting)

| Item | Observation | Component |
|------|-------------|-----------|
| Snappy compression | CPU trade-off vs storage, generally good | parquet.rs |
| Day partitioning | Good for queries, manageable file count | parquet.rs |
| GIN indexes | Efficient for dq_flags queries | 002_silver_indexes.sql |

---

## 3. Recommendations

### 3.1 Quick Wins (Low Effort, High Impact)

#### R1: Reduce ETL Daemon Interval

**Current**: 300 seconds
**Recommended**: 60 seconds (1 minute)

**Impact**: Reduces worst-case latency from 5+ minutes to ~1-2 minutes.

**Trade-offs**:
- More frequent database connections
- Higher ETL overhead (mitigated by watermark - empty runs are fast)
- Monitor CPU/memory impact on Pi

**Configuration change**:
```bash
silver-etl daemon --interval 60
```

#### R2: Reduce Batch Timeout for Real-Time Streams

**Current**: 5 seconds (all streams)
**Recommended**: 1-2 seconds for MQTT streams

For real-time MQTT data where messages arrive frequently, reducing batch timeout improves freshness without significant overhead.

**Configuration change** (per stream):
```yaml
storage:
  batch_size: 100
  batch_timeout_secs: 2  # Reduced from 5
```

**Alternative**: Adaptive batching based on message rate.

#### R3: Reduce HTTP Polling Interval for Weather

**Current**: 600 seconds (10 minutes)
**Recommended**: 300 seconds (5 minutes) or 180 seconds (3 minutes)

OpenWeatherMap current weather API refreshes data every ~10 minutes, but NWS observations update more frequently. Consider API rate limits and cost.

### 3.2 Medium-Term Improvements (Moderate Effort)

#### R4: Parallel Stream ETL Processing

**Current**: Sequential processing of all enabled streams per cycle
**Recommended**: Parallel processing with bounded concurrency

**Implementation approach**:
```rust
// In daemon.rs run_cycle()
use futures::stream::{self, StreamExt};

let results: Vec<_> = stream::iter(streams)
    .map(|stream_id| {
        let executor = self.executor.clone(); // Requires Arc<Mutex<E>>
        async move { executor.lock().await.run_stream(&stream_id) }
    })
    .buffer_unordered(3) // Process up to 3 streams concurrently
    .collect()
    .await;
```

**Trade-offs**:
- Requires refactoring executor to support concurrent access
- DuckDB Connection is not Sync, may need connection pool
- Memory usage increases with concurrency

#### R5: Batched WAL Commits

**Current**: Immediate flush per WAL entry
**Recommended**: Batch WAL writes, commit on timer or batch boundary

**Implementation approach**:
```rust
// Instead of flush per append
pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
    writeln!(self.file, "{}", json_str)?;
    // Remove: self.file.flush()?;
    Ok(())
}

// Add periodic flush (e.g., every second or every N entries)
pub fn flush(&mut self) -> CoreResult<()> {
    self.file.flush()?;
    Ok(())
}
```

**Impact**: Reduces fsync overhead, improves write throughput.

**Trade-offs**: Small window for data loss on crash (acceptable for sensor data).

#### R6: Streaming/Micro-Batch ETL Mode

**Current**: Full batch ETL every 5 minutes
**Recommended**: Hybrid approach with configurable micro-batches

**Option A**: Reduce interval to 30 seconds with smarter watermark handling
**Option B**: Event-driven ETL triggered by new Parquet files (file watcher)

**Event-driven approach**:
```rust
use notify::{Watcher, RecursiveMode, watcher};

// Watch Bronze directory for new .parquet files
let (tx, rx) = std::sync::mpsc::channel();
let mut watcher = watcher(tx, Duration::from_secs(1))?;
watcher.watch(bronze_dir, RecursiveMode::Recursive)?;

// Trigger ETL on file create/modify
for event in rx {
    match event {
        DebouncedEvent::Create(path) | DebouncedEvent::Write(path) => {
            if path.extension() == Some("parquet") {
                trigger_etl_for_stream(extract_stream_id(&path));
            }
        }
        _ => {}
    }
}
```

**Impact**: Near-real-time ETL (seconds instead of minutes).

**Trade-offs**: More complex coordination, potential for frequent small ETL runs.

#### R7: Parallel HTTP Sensor Polling

**Current**: Sequential polling in `poll_all_sensors()`
**Recommended**: Concurrent polling with `futures::join_all`

The code comment notes: "AIR-010 attempted parallel polling but encountered lifetime issues."

**Suggested fix**: Wrap `HttpPollingSource` in `Arc` and use `Arc::clone` for concurrent tasks.

```rust
async fn poll_all_sensors(self: &Arc<Self>) -> CoreResult<()> {
    let futures: Vec<_> = self.config.sensors.iter()
        .map(|sensor| {
            let this = Arc::clone(self);
            let sensor = sensor.clone();
            async move { this.poll_sensor(&sensor).await }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    // Process results...
}
```

### 3.3 Advanced Optimizations (Higher Effort)

#### R8: TimescaleDB Continuous Aggregates

**Current**: Raw data only, aggregations computed on query
**Recommended**: Pre-compute common aggregates

```sql
-- Create continuous aggregate for hourly air quality summaries
CREATE MATERIALIZED VIEW silver.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_avg,
    MAX(pm25) AS pm25_max,
    AVG(temperature_c) AS temp_avg,
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id
WITH NO DATA;

-- Set refresh policy
SELECT add_continuous_aggregate_policy('silver.air_quality_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 minute',
    schedule_interval => INTERVAL '1 minute'
);
```

**Impact**: Faster dashboard queries, predictable query latency.

**Trade-offs**: Additional storage, refresh lag (configurable).

#### R9: Direct TimescaleDB Writes (Bypass Bronze)

For ultra-low-latency scenarios, consider dual-write pattern:
1. Write to Bronze (Parquet) for durability and reprocessing
2. Write directly to Silver (TimescaleDB) for immediate queryability

**Implementation approach**: Add `TimescaleStore` trait implementation alongside `ParquetStore`.

**Trade-offs**:
- Dual-write complexity
- Potential data inconsistency during ETL reprocessing
- Better for real-time alerting use cases

#### R10: COPY-Based Bulk Loading

**Current**: Single INSERT with ON CONFLICT
**Recommended**: COPY for bulk loads, then dedup

For large batches, PostgreSQL COPY is significantly faster than INSERT:

```sql
-- Staging table approach
CREATE TEMP TABLE staging LIKE silver.air_quality_observations;
COPY staging FROM '/tmp/batch.csv' WITH CSV;
INSERT INTO silver.air_quality_observations
SELECT DISTINCT ON (observation_time, ndp_id) * FROM staging
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET ...;
DROP TABLE staging;
```

**Impact**: 10x+ improvement for large batch loads.

**Trade-offs**: More complex ETL, temp table overhead, mainly benefits backfills.

---

## 4. Monitoring Recommendations

### 4.1 Latency Metrics to Track

| Metric | Description | Target |
|--------|-------------|--------|
| `ndp_ingestion_lag_seconds` | Time from source timestamp to Bronze write | < 5s (MQTT), < 60s (HTTP) |
| `ndp_etl_lag_seconds` | Time from Bronze write to Silver availability | < 60s |
| `ndp_batch_flush_latency_ms` | Storage writer flush duration | < 100ms |
| `ndp_etl_cycle_duration_ms` | Single ETL cycle duration | < 30s |
| `ndp_parquet_write_latency_ms` | Parquet file write duration | < 500ms |
| `ndp_timescale_insert_latency_ms` | TimescaleDB insert duration | < 200ms |

### 4.2 Prometheus Metrics (Suggested Additions)

```rust
// In storage_writer.rs
static BATCH_LATENCY: Histogram = register_histogram!(
    "ndp_batch_flush_latency_seconds",
    "Time to flush a batch to storage"
)?;

// In etl.rs
static ETL_LATENCY: Histogram = register_histogram!(
    "ndp_etl_stream_latency_seconds",
    "Time to ETL a single stream",
    &["stream_id"]
)?;
```

### 4.3 Grafana Dashboard Panels

1. **End-to-End Latency**: `observation_time` vs `ingestion_time` vs query time
2. **ETL Cycle Health**: Cycle duration, rows processed, failure rate
3. **Data Freshness**: Time since last data point per stream
4. **Backpressure Indicators**: Channel queue depth, batch buffer utilization

---

## 5. Implementation Priority

| Priority | Recommendation | Effort | Impact | Risk |
|----------|---------------|--------|--------|------|
| 1 | R1: Reduce ETL interval to 60s | Low | High | Low |
| 2 | R2: Reduce batch timeout to 2s | Low | Medium | Low |
| 3 | R3: Reduce HTTP poll interval | Low | Medium | Low |
| 4 | R8: Continuous aggregates | Medium | High | Low |
| 5 | R5: Batched WAL commits | Medium | Medium | Medium |
| 6 | R4: Parallel stream ETL | Medium | High | Medium |
| 7 | R6: Event-driven ETL | High | Very High | Medium |
| 8 | R7: Parallel HTTP polling | Medium | Medium | Low |
| 9 | R9: Direct TimescaleDB writes | High | Very High | High |
| 10 | R10: COPY-based loading | Medium | Medium | Low |

---

## 6. Estimated Latency After Optimization

**With Quick Wins (R1, R2, R3)**:
- MQTT streams: 2-5 seconds to Bronze, 60-90 seconds to Silver
- HTTP streams: 60-180 seconds to Bronze, 120-240 seconds to Silver

**With Medium-Term Improvements (R4, R5, R6)**:
- MQTT streams: 1-2 seconds to Bronze, 10-30 seconds to Silver
- HTTP streams: 30-60 seconds to Bronze, 60-90 seconds to Silver

**With Advanced Optimizations (R9)**:
- MQTT streams: 1-2 seconds to Silver (direct write)
- Sub-second alerting capability

---

## 7. Trade-offs Summary

| Optimization | Benefit | Cost |
|--------------|---------|------|
| Shorter ETL interval | Lower latency | Higher CPU/IO overhead |
| Smaller batches | Faster flush | More write operations |
| Parallel processing | Higher throughput | More memory, complexity |
| Direct writes | Near-real-time | Dual-write complexity |
| Continuous aggregates | Faster queries | Storage, refresh lag |

---

## Appendix A: Current Configuration Reference

### Batch Settings

| Stream | Batch Size | Batch Timeout | Poll Interval |
|--------|------------|---------------|---------------|
| air-quality | 100 | 5s | N/A (MQTT) |
| outdoor-weather | 50 | 30s | 600s |
| outdoor-air-quality | 50 | 30s | 600s |
| nws-* | 50 | 30s | varies |

### ETL Settings

| Setting | Value |
|---------|-------|
| Daemon interval | 300s |
| Watermark lag | 5 minutes |
| Deduplication | upsert |
| DQ output | enabled |

### TimescaleDB Settings

| Setting | Value |
|---------|-------|
| Chunk interval | 1 day |
| Compression | Not enabled |
| Continuous aggregates | None |

---

## Appendix B: Related Files

- `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs`
- `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
- `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
- `/workspaces/neural-data-platform/core/src/storage/wal.rs`
- `/workspaces/neural-data-platform/apps/silver-etl/src/daemon.rs`
- `/workspaces/neural-data-platform/apps/silver-etl/src/etl.rs`
- `/workspaces/neural-data-platform/config/base/streams/*/config.yaml`
- `/workspaces/neural-data-platform/deploy/timescaledb/migrations/*.sql`
