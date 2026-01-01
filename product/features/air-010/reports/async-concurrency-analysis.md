# Async and Concurrency Optimization Analysis

**Feature:** AIR-010 Performance Optimization
**Date:** 2026-01-01
**Analyst:** ndp-rust-dev

## Executive Summary

This analysis examines the Neural Data Platform's Rust ingestion application for async and concurrency optimization opportunities. The codebase demonstrates generally solid async patterns but has several areas where throughput can be significantly improved through parallelization, lock contention reduction, and better async/sync boundary management.

**Estimated Total Throughput Improvement: 40-60%** (achievable with medium-priority items)

---

## Table of Contents

1. [Blocking Operations in Async Contexts](#1-blocking-operations-in-async-contexts)
2. [Sequential Operations That Could Be Parallel](#2-sequential-operations-that-could-be-parallel)
3. [Lock Contention Risks](#3-lock-contention-risks)
4. [Channel Pattern Optimizations](#4-channel-pattern-optimizations)
5. [Spawn Pattern Issues](#5-spawn-pattern-issues)
6. [Async Functions That Could Be Sync](#6-async-functions-that-could-be-sync)
7. [spawn_blocking Opportunities](#7-spawn_blocking-opportunities)
8. [Summary and Prioritization](#8-summary-and-prioritization)

---

## 1. Blocking Operations in Async Contexts

### CRITICAL: WAL File Operations in Async Context

**File:** `/workspaces/neural-data-platform/core/src/storage/wal.rs`
**Lines:** 24-32, 34-46, 49-59

**Current Pattern:**
```rust
// WAL::append performs synchronous file I/O in async context
pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
    let json_str = std::str::from_utf8(entry)
        .map_err(|e| CoreError::Storage(format!("Invalid UTF-8 in WAL entry: {}", e)))?;

    writeln!(self.file, "{}", json_str)?;
    self.file.flush()?;  // BLOCKING: sync flush in async context

    Ok(())
}
```

**Problem:** `writeln!` and `flush()` are synchronous operations that block the tokio runtime thread. When called from async code in `ParquetStore::write_batch`, this blocks other async tasks.

**Recommended Optimization:**
```rust
use tokio::fs::OpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter};

pub struct AsyncWriteAheadLog {
    path: PathBuf,
    writer: BufWriter<tokio::fs::File>,
}

impl AsyncWriteAheadLog {
    pub async fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
        let json_str = std::str::from_utf8(entry)?;
        self.writer.write_all(json_str.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;
        Ok(())
    }
}
```

**Throughput Impact:** +15-20% for write-heavy workloads

---

### HIGH: Parquet File I/O Blocking Operations

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
**Lines:** 83-153, 155-223

**Current Pattern:**
```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    // ...
    std::fs::create_dir_all(parent)?;  // BLOCKING
    let file = std::fs::File::create(path)?;  // BLOCKING
    ParquetWriter::new(file)
        .with_compression(ParquetCompression::Snappy)
        .finish(&mut df)?;  // BLOCKING - CPU-intensive compression
    Ok(())
}
```

**Problem:** Parquet file creation, directory creation, and Snappy compression are all synchronous, CPU-intensive operations that block the async runtime.

**Recommended Optimization:**
```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    let path = path.to_path_buf();
    let points = points;

    // Move CPU-intensive work to blocking thread pool
    tokio::task::spawn_blocking(move || {
        let parent = path.parent().ok_or_else(|| CoreError::Storage("No parent".into()))?;
        std::fs::create_dir_all(parent)?;
        // ... parquet writing logic
    }).await.map_err(|e| CoreError::Storage(e.to_string()))?
}
```

**Throughput Impact:** +10-15% for write operations

---

### MEDIUM: Regex Compilation in Hot Path

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
**Lines:** 630-642

**Current Pattern:**
```rust
fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();  // BLOCKING: regex compilation

    for cap in re.captures_iter(s) {
        // ...
    }
    result
}
```

**Problem:** Regex compilation is expensive and happens on every call to `expand_env_vars`.

**Recommended Optimization:**
```rust
use once_cell::sync::Lazy;
use regex::Regex;

static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex")
});

fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    for cap in ENV_VAR_REGEX.captures_iter(s) {
        // ...
    }
    result
}
```

**Throughput Impact:** +5% for configuration loading

---

## 2. Sequential Operations That Could Be Parallel

### HIGH: Sequential Sensor Polling

**File:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
**Lines:** 462-490

**Current Pattern:**
```rust
async fn poll_all_sensors(&self) -> CoreResult<()> {
    for sensor in &self.config.sensors {  // SEQUENTIAL
        match self.poll_sensor(sensor).await {
            Ok(points) => {
                // ...
            }
            Err(e) => {
                error!("Failed to poll sensor {}: {}", sensor.serial_number, e);
            }
        }
    }
    Ok(())
}
```

**Problem:** Sensors are polled sequentially. With 10 sensors at 5 second timeout each, worst case is 50 seconds per poll cycle.

**Recommended Optimization:**
```rust
use futures::stream::{self, StreamExt};

async fn poll_all_sensors(&self) -> CoreResult<()> {
    let results = stream::iter(&self.config.sensors)
        .map(|sensor| self.poll_sensor(sensor))
        .buffer_unordered(10)  // Poll up to 10 sensors concurrently
        .collect::<Vec<_>>()
        .await;

    for (sensor, result) in self.config.sensors.iter().zip(results) {
        match result {
            Ok(points) => { /* ... */ }
            Err(e) => { error!("Failed: {}", e); }
        }
    }
    Ok(())
}
```

**Alternative using tokio::join!:**
```rust
async fn poll_all_sensors(&self) -> CoreResult<()> {
    let futures: Vec<_> = self.config.sensors
        .iter()
        .map(|sensor| self.poll_sensor(sensor))
        .collect();

    let results = futures::future::join_all(futures).await;
    // Process results...
}
```

**Throughput Impact:** +30-50% for HTTP polling with multiple sensors

---

### HIGH: Sequential Partition Writes

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
**Lines:** 240-268

**Current Pattern:**
```rust
async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()> {
    // ...
    let mut grouped: HashMap<PathBuf, Vec<TimeSeriesPoint>> = HashMap::new();
    for point in points {
        let path = self.partition_path(&partition_key, point.timestamp);
        grouped.entry(path).or_insert_with(Vec::new).push(point);
    }

    for (path, partition_points) in grouped {  // SEQUENTIAL writes
        self.append_to_parquet(partition_points, &path).await?;
    }
    // ...
}
```

**Problem:** Partitions are written sequentially. With data spanning multiple days/streams, this serializes I/O.

**Recommended Optimization:**
```rust
async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()> {
    // ... grouping logic ...

    // Write partitions concurrently
    let write_futures: Vec<_> = grouped
        .into_iter()
        .map(|(path, partition_points)| {
            self.append_to_parquet(partition_points, &path)
        })
        .collect();

    // Use try_join_all for early failure propagation
    futures::future::try_join_all(write_futures).await?;

    // ... commit WAL ...
}
```

**Throughput Impact:** +20-30% for multi-partition writes

---

### MEDIUM: Sequential Source Stopping

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
**Lines:** 917-934

**Current Pattern:**
```rust
pub async fn stop_all_sources(&mut self) -> Result<(), SourceManagerError> {
    let source_ids: Vec<String> = { /* ... */ };

    for source_id in source_ids {  // SEQUENTIAL stops
        if let Err(e) = self.stop_source(&source_id).await {
            error!("Failed to stop source {}: {}", source_id, e);
        }
    }
    Ok(())
}
```

**Recommended Optimization:**
```rust
pub async fn stop_all_sources(&mut self) -> Result<(), SourceManagerError> {
    let source_ids: Vec<String> = { /* ... */ };

    let stop_futures: Vec<_> = source_ids
        .iter()
        .map(|id| self.stop_source(id))
        .collect();

    let results = futures::future::join_all(stop_futures).await;

    for (id, result) in source_ids.iter().zip(results) {
        if let Err(e) = result {
            error!("Failed to stop source {}: {}", id, e);
        }
    }
    Ok(())
}
```

**Throughput Impact:** +10% for graceful shutdown speed

---

### MEDIUM: Sequential Stream Registration

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs`
**Lines:** 108-134

**Current Pattern:**
```rust
pub async fn register_all_streams_from_registry(
    &self,
    storage_tx: mpsc::Sender<TimeSeriesPoint>,
) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
    let stream_ids = self.registry.list_streams().await?;

    for stream_id in &stream_ids {  // SEQUENTIAL registration
        self.register_storage_channel(stream_id.clone(), storage_tx.clone())
            .await;
    }
    Ok(stream_ids.len())
}
```

**Recommended Optimization:**
```rust
pub async fn register_all_streams_from_registry(...) -> Result<...> {
    let stream_ids = self.registry.list_streams().await?;

    // Register all channels concurrently
    let futures: Vec<_> = stream_ids.iter()
        .map(|id| self.register_storage_channel(id.clone(), storage_tx.clone()))
        .collect();

    futures::future::join_all(futures).await;
    Ok(stream_ids.len())
}
```

**Throughput Impact:** +5% for startup time

---

## 3. Lock Contention Risks

### HIGH: Frequent Lock Acquisition in MQTT Event Loop

**File:** `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs`
**Lines:** 314, 341-343, 366-367, 399-405

**Current Pattern:**
```rust
while *is_running.lock().await {  // Lock acquired every iteration
    match event_loop.poll().await {
        Ok(Event::Incoming(Packet::Publish(publish))) => {
            // ...
            {
                let mut raw_cache = cached_raw_points.lock().await;  // Lock for each message
                raw_cache.push(raw_point);
            }
            // ...
            {
                let mut cache = cached_points.lock().await;  // Another lock for each message
                cache.extend(points);
            }
        }
        Ok(Event::Incoming(Packet::ConnAck(_))) => {
            *connection_healthy.lock().await = true;  // Lock on connection
        }
        // ...
    }
}
```

**Problem:** Multiple Mutex locks are acquired per MQTT message. At high message rates (100+ msg/sec), this creates significant contention.

**Recommended Optimization:**
```rust
// Option 1: Use atomic flags for simple state
use std::sync::atomic::{AtomicBool, Ordering};

struct MqttState {
    is_running: AtomicBool,
    connection_healthy: AtomicBool,
    // Use crossbeam channels for lock-free cache
    raw_point_tx: crossbeam_channel::Sender<RawDataPoint>,
}

// Option 2: Batch updates to reduce lock frequency
let mut local_raw_cache = Vec::with_capacity(100);
let mut local_cache = Vec::with_capacity(100);

// Accumulate locally, then flush periodically
if local_raw_cache.len() >= 100 {
    let mut raw_cache = cached_raw_points.lock().await;
    raw_cache.extend(local_raw_cache.drain(..));
}
```

**Throughput Impact:** +15-25% for high-frequency MQTT streams

---

### MEDIUM: RwLock Contention in Router

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs`
**Lines:** 189-191

**Current Pattern:**
```rust
pub async fn route_point(...) -> Result<...> {
    // ...
    let channels = self.storage_channels.read().await;  // Read lock for every point
    if let Some(tx) = channels.get(stream_id) {
        if let Err(e) = tx.send(enriched).await {
            // ...
        }
    }
}
```

**Problem:** Every routed point acquires a read lock. While RwLock allows concurrent reads, there's still contention overhead.

**Recommended Optimization:**
```rust
use dashmap::DashMap;

pub struct IngestionRouter {
    // Replace RwLock<HashMap> with DashMap
    storage_channels: DashMap<String, mpsc::Sender<TimeSeriesPoint>>,
}

pub async fn route_point(...) -> Result<...> {
    // Lock-free concurrent access
    if let Some(tx) = self.storage_channels.get(stream_id) {
        tx.send(enriched).await?;
    }
}
```

**Throughput Impact:** +10-15% for high-throughput routing

---

### MEDIUM: WAL Mutex Held During Parquet Write

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
**Lines:** 245-267

**Current Pattern:**
```rust
async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()> {
    let mut wal = self.wal.lock().await;  // Lock acquired
    for point in &points {
        let entry = serde_json::to_vec(point)?;
        wal.append(&entry)?;
    }
    drop(wal);  // Lock released

    // ... parquet writing (can be slow) ...

    let mut wal = self.wal.lock().await;  // Re-acquired for commit
    wal.commit()?;
    Ok(())
}
```

**Analysis:** The current pattern correctly releases the WAL lock during parquet writing. However, the pattern of acquire-release-reacquire creates potential for other tasks to interleave, which is actually correct behavior. No change needed here.

---

## 4. Channel Pattern Optimizations

### MEDIUM: Unbounded Channel Growth Risk

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
**Line:** N/A (channel created externally)

**Current Pattern:**
The ingestion channel size is configurable via `buffer_capacity` in source configs, typically 1000. However, there's no explicit backpressure mechanism.

**Recommended Optimization:**
```rust
// Add backpressure handling
match tx.try_send(point) {
    Ok(_) => { /* success */ }
    Err(mpsc::error::TrySendError::Full(_)) => {
        // Backpressure: wait with timeout
        match tokio::time::timeout(
            Duration::from_secs(1),
            tx.send(point)
        ).await {
            Ok(Ok(_)) => { /* success after wait */ }
            Ok(Err(_)) => { error!("Channel closed"); }
            Err(_) => {
                warn!("Backpressure: dropping point");
                // Optionally: write to dead letter queue
            }
        }
    }
    Err(mpsc::error::TrySendError::Closed(_)) => {
        error!("Channel closed");
    }
}
```

**Throughput Impact:** Prevents memory exhaustion under load spikes

---

### LOW: Single Point Send vs Batch Send

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
**Lines:** 454-459, 785-789

**Current Pattern:**
```rust
for raw_point in raw_points {  // Send one at a time
    if let Err(e) = ingestion_sender.send(raw_point).await {
        error!("Failed to send point to ingestion channel: {}", e);
    }
}
```

**Recommended Optimization:**
```rust
// Consider sending batches through channel
type IngestionChannel = mpsc::Sender<Vec<RawDataPoint>>;

// Or use try_send for non-blocking batch sends
let batch_size = 100;
for chunk in raw_points.chunks(batch_size) {
    // Process chunk...
}
```

**Throughput Impact:** +5% for high-volume sources

---

## 5. Spawn Pattern Issues

### LOW: Unbounded Task Spawning

**File:** `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs`
**Lines:** 508-527

**Current Pattern:**
```rust
// Spawn background task for event processing
tokio::spawn(async move {
    if let Err(e) = Self::process_events(...).await {
        error!("MQTT event processing failed: {}", e);
    }
});
```

**Analysis:** The current spawn pattern is appropriate for long-running background tasks. Each MQTT source spawns exactly one task, which is correct.

**Potential Issue:** If sources are rapidly created/destroyed, there could be orphaned tasks. The cancellation token pattern already handles this correctly.

**No change needed.** Pattern is correct.

---

### LOW: Missing JoinHandle Tracking

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`
**Line:** 303

**Current Pattern:**
```rust
let source_info = SourceInfo {
    // ...
    task_handle: Some(tokio::spawn(async move { ... })),
};
```

**Analysis:** Task handles are properly stored and awaited during stop_source(). Pattern is correct.

---

## 6. Async Functions That Could Be Sync

### LOW: Simple Getters That Are Async

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs`
**Lines:** 93-102

**Current Pattern:**
```rust
pub async fn has_storage_channel(&self, stream_id: &str) -> bool {
    let channels = self.storage_channels.read().await;
    channels.contains_key(stream_id)
}

pub async fn registered_stream_count(&self) -> usize {
    let channels = self.storage_channels.read().await;
    channels.len()
}
```

**Analysis:** These must be async because they acquire a RwLock. If using DashMap (recommended in section 3), these could become sync:

```rust
// With DashMap
pub fn has_storage_channel(&self, stream_id: &str) -> bool {
    self.storage_channels.contains_key(stream_id)
}

pub fn registered_stream_count(&self) -> usize {
    self.storage_channels.len()
}
```

**Throughput Impact:** Negligible, but cleaner API

---

## 7. spawn_blocking Opportunities

### HIGH: Parquet File Operations

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
**Lines:** 83-153, 155-223

See Section 1 for details. Parquet operations should use `spawn_blocking`.

**Recommended Pattern:**
```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    let path = path.to_path_buf();

    tokio::task::spawn_blocking(move || {
        // All synchronous file I/O and CPU-intensive compression here
        let parent = path.parent().ok_or_else(|| ...)?;
        std::fs::create_dir_all(parent)?;

        let file = std::fs::File::create(&path)?;

        // Build DataFrame and write with Snappy compression
        // (CPU-intensive)
        ParquetWriter::new(file)
            .with_compression(ParquetCompression::Snappy)
            .finish(&mut df)?;

        Ok::<_, CoreError>(())
    })
    .await
    .map_err(|e| CoreError::Storage(format!("Task panicked: {}", e)))?
}
```

**Throughput Impact:** +10-15% by not blocking async runtime

---

### MEDIUM: JSON Serialization for Large Payloads

**File:** `/workspaces/neural-data-platform/core/src/storage/parquet.rs`
**Lines:** 246-249

**Current Pattern:**
```rust
for point in &points {
    let entry = serde_json::to_vec(point)?;  // CPU-bound for large payloads
    wal.append(&entry)?;
}
```

**For large batches, consider:**
```rust
let entries: Vec<Vec<u8>> = tokio::task::spawn_blocking(move || {
    points.iter()
        .map(|p| serde_json::to_vec(p))
        .collect::<Result<Vec<_>, _>>()
}).await??;

let mut wal = self.wal.lock().await;
for entry in entries {
    wal.append(&entry)?;
}
```

**Throughput Impact:** +5% for large batch writes

---

## 8. Summary and Prioritization

### Priority Matrix

| Priority | Issue | File | Estimated Impact | Effort |
|----------|-------|------|------------------|--------|
| **CRITICAL** | WAL sync I/O in async | `wal.rs` | +15-20% | Medium |
| **HIGH** | Sequential sensor polling | `http_poll.rs` | +30-50% | Low |
| **HIGH** | Sequential partition writes | `parquet.rs` | +20-30% | Low |
| **HIGH** | MQTT lock contention | `mqtt/mod.rs` | +15-25% | Medium |
| **HIGH** | Parquet blocking ops | `parquet.rs` | +10-15% | Medium |
| **MEDIUM** | Router RwLock contention | `router.rs` | +10-15% | Low |
| **MEDIUM** | Regex compilation | `source_manager.rs` | +5% | Trivial |
| **MEDIUM** | Sequential source stopping | `source_manager.rs` | +10% | Low |
| **LOW** | Channel backpressure | Various | N/A (reliability) | Low |
| **LOW** | Batch channel sends | `source_manager.rs` | +5% | Low |

### Implementation Roadmap

#### Phase 1: Quick Wins (1-2 days)
1. Parallelize sensor polling with `futures::stream::buffer_unordered`
2. Parallelize partition writes with `try_join_all`
3. Use `Lazy<Regex>` for environment variable expansion
4. Replace `RwLock<HashMap>` with `DashMap` in router

**Expected improvement: +35-50%**

#### Phase 2: I/O Optimization (3-5 days)
1. Migrate WAL to async I/O with `tokio::fs`
2. Wrap Parquet operations in `spawn_blocking`
3. Add proper backpressure handling to channels

**Expected improvement: +20-30%**

#### Phase 3: Lock Contention (3-5 days)
1. Use atomic flags for MQTT connection state
2. Batch MQTT cache updates
3. Consider lock-free data structures for hot paths

**Expected improvement: +15-25%**

### Metrics to Track

Before and after implementing optimizations:
- Points ingested per second
- P99 latency for write operations
- Channel buffer utilization
- Async runtime blocked time (`tokio_metrics`)
- Memory usage under load

### Testing Recommendations

1. **Load Testing:** Use `criterion` benchmarks with realistic data volumes
2. **Concurrency Testing:** Test with multiple concurrent sources
3. **Stress Testing:** Verify backpressure under extreme load
4. **Regression Testing:** Ensure correctness after parallelization

---

## Appendix: Key File Locations

| Component | File Path |
|-----------|-----------|
| WAL | `/workspaces/neural-data-platform/core/src/storage/wal.rs` |
| Parquet Store | `/workspaces/neural-data-platform/core/src/storage/parquet.rs` |
| HTTP Polling | `/workspaces/neural-data-platform/core/src/sources/http_poll.rs` |
| MQTT Source | `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs` |
| Source Manager | `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs` |
| Router | `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/router.rs` |
| Storage Writer | `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs` |
