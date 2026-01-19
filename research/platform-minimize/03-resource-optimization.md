# Resource Optimization Research

**Target**: Neural Data Platform on Raspberry Pi 5
**Constraint**: <2GB total memory target (current ~1.7GB)
**Date**: 2026-01-17

---

## Executive Summary

This document analyzes CPU and memory optimization opportunities for NDP on Raspberry Pi 5. The research covers five main areas:
1. Memory-intensive components (Parquet storage, channels, WAL)
2. CPU usage patterns (async runtime, serialization)
3. Docker service resource allocation
4. Unnecessary allocations/clones
5. Compression trade-offs

---

## 1. Current Memory Allocation (docker-compose.yml)

### Service Memory Limits

| Service | Memory Limit | Memory Reserved | Notes |
|---------|--------------|-----------------|-------|
| mosquitto | 128M | - | MQTT broker |
| etcd | 256M | - | Config store |
| air-quality-app | 512M | - | Main ingestion app |
| timescaledb | 256M | 128M | Silver layer storage |
| ndp-mcp-server | 96M | - | MCP interface |
| grafana | 256M | - | Visualization |
| silver-etl | 256M | - | One-shot profile |
| silver-etl-daemon | 256M | - | Continuous profile |

**Total Allocated**: ~1.76GB (when all services running)

### Observations

1. **etcd quota is 512MB** (`ETCD_QUOTA_BACKEND_BYTES=536870912`) - may be excessive for config-only usage
2. **air-quality-app at 512M** is the largest consumer - likely oversized
3. **ndp-mcp-server at 96M** is well-optimized with target <50MB runtime

---

## 2. Channel and Buffer Configuration

### Current Buffer Sizes

| Component | Buffer Size | Location |
|-----------|-------------|----------|
| Ingestion channel | 1000 | `IngestionCoordinatorConfig::default()` |
| Storage channel | 500 | `IngestionCoordinatorConfig::default()` |
| MQTT buffer | 1000 | `MqttConfig::buffer_capacity` |
| HTTP buffer | 1000 | `HttpPollingSourceConfig::buffer_capacity` |
| Batch size | 100 | `StorageConfig::batch_size` |
| Batch timeout | 5s | `StorageConfig::batch_timeout_secs` |
| Storage batch (writer) | 50 | `RawStorageWriter::new()` in main.rs |

### Memory Impact Analysis

**Channel overhead per item** (approximate):
- `RawDataPoint`: ~200-500 bytes (JSON payload varies)
- `TimeSeriesPoint`: ~150-300 bytes
- `StreamRecord`: ~300-600 bytes

**Estimated channel memory**:
- 1000 items x 400 bytes avg = ~400KB per channel
- With 3-4 active channels = ~1.6MB in channel buffers

### Recommendations

| Buffer | Current | Recommended | Rationale |
|--------|---------|-------------|-----------|
| Ingestion channel | 1000 | 250 | Pi5 has limited cores; backpressure is acceptable |
| Storage channel | 500 | 200 | Writers can batch smaller amounts more frequently |
| MQTT buffer | 1000 | 250 | Sensors send ~1msg/min; 250 = 4+ hours buffer |
| HTTP buffer | 1000 | 100 | HTTP polling is scheduled; less burst traffic |
| Batch size | 100 | 50 | Smaller batches = faster writes, less memory |

**Estimated savings**: ~1MB from channel reduction

---

## 3. Parquet Storage Analysis

### Good Patterns Already Implemented

1. **Pre-allocation** (P2-02): `Vec::with_capacity(len)` before batch operations
2. **spawn_blocking** (P3-02): CPU-intensive Parquet work offloaded to blocking pool
3. **Daily partitioning**: Reduces small file proliferation
4. **Snappy compression**: Good balance of speed vs compression

### Memory Concerns

#### 3.1 Read-Modify-Write in `append_to_parquet`

**Issue**: When appending to existing Parquet files, the entire file is read into memory, merged with new data, then rewritten.

```rust
// Current pattern (parquet.rs:157-225)
async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path) -> CoreResult<()> {
    let mut all_points = points;

    if path.exists() {
        // Reads entire file into memory
        let file = std::fs::File::open(path)?;
        let df = ParquetReader::new(file).finish()?;
        // ... deserializes all rows into all_points ...
    }

    self.write_parquet(all_points, path).await  // Rewrites everything
}
```

**Impact**: For a day's data (~15KB/hour x 24 = ~360KB), this could cause memory spikes of 2-3x file size during writes.

**Recommendation**:
- Implement row group appending instead of full file rewrite
- Or accumulate in WAL longer before flushing to Parquet

#### 3.2 String Clones During Column Extraction

```rust
// parquet.rs:116-124
for p in &points {
    location_ids.push(p.location_id.clone());  // Clone
    metrics.push(p.tags.get("metric").cloned().unwrap_or_else(...));  // Clone
    ndp_ids.push(p.ndp_id.clone());  // Clone
}
```

**Recommendation**: Use `Cow<str>` or string interning for repeated values like `location_id`.

---

## 4. WAL Implementation Analysis

### Current Behavior

```rust
// wal.rs
pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
    let json_str = std::str::from_utf8(entry)?;
    writeln!(self.file, "{}", json_str)?;
    self.file.flush()?;  // Sync on every append
}
```

**Issues**:
1. **Flush on every write** - causes high I/O on Pi's SD card
2. **No buffering** - each point triggers a syscall
3. **JSON serialization** - already done before WAL, then done again for Parquet

**Recommendations**:

1. **Buffered WAL flushing**:
   ```rust
   // Recommendation: Flush every N entries or every T seconds
   pub fn append(&mut self, entry: &[u8]) -> CoreResult<()> {
       writeln!(self.file, "{}", json_str)?;
       self.pending_count += 1;
       if self.pending_count >= FLUSH_THRESHOLD {
           self.file.flush()?;
           self.pending_count = 0;
       }
   }
   ```

2. **Binary WAL format**: Store raw bytes instead of JSON strings to reduce serialization overhead

---

## 5. Async Runtime Configuration

### Current Configuration

All services use `#[tokio::main]` with default configuration:
- Default worker threads: number of CPU cores (4 on Pi5)
- Default blocking threads: 512 (excessive for Pi5)

### Recommendations

```rust
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    // For Pi5, 2 worker threads is sufficient
    // Leaves cores for blocking tasks and other services
}
```

**Environment variable alternative**:
```bash
TOKIO_WORKER_THREADS=2
```

### Blocking Thread Pool

Current Parquet writes use `spawn_blocking`. The default pool size (512) is excessive.

**Recommendation**:
```rust
tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .max_blocking_threads(4)  // Reduced from 512
    .build()
```

---

## 6. Data Structure Optimizations

### 6.1 HashMap Pre-allocation

**Good**: Already implemented in several places:
```rust
// parquet.rs:260
let mut grouped: HashMap<PathBuf, Vec<TimeSeriesPoint>> = HashMap::with_capacity(3);
```

**Missing**: Some HashMaps created without capacity hints.

### 6.2 String Allocation Patterns

**High-frequency clones identified**:

| Location | Pattern | Frequency |
|----------|---------|-----------|
| `traits.rs` | `location_id.to_string()` | Every point creation |
| `traits.rs` | `tags.clone()` | Every point copy |
| `parquet.rs` | `p.location_id.clone()` | Every batch write |

**Recommendation**: Use `Arc<str>` for repeated strings:
```rust
pub struct TimeSeriesPoint {
    pub location_id: Arc<str>,  // Instead of String
    // ...
}
```

### 6.3 JSON Context Serialization

```rust
// Current: Serialize to string for storage
contexts.push(p.context.as_ref().map(|c| c.to_string()));
```

**Recommendation**: Store as binary (MessagePack) instead of JSON string for context field - saves 20-30% space.

---

## 7. Compression Trade-offs

### Current: Snappy Compression

**Pros**:
- Fast compression/decompression (~250MB/s)
- Low CPU overhead
- Good for real-time writes

**Cons**:
- Moderate compression ratio (~1.5-2x)
- Not optimal for archival

### Alternative: Zstd Compression

For archival data (>7 days old per `compression_after_days` config):

| Codec | Speed | Ratio | CPU | Recommendation |
|-------|-------|-------|-----|----------------|
| Snappy | Fast | 1.5-2x | Low | Keep for real-time |
| Zstd Level 1 | Medium | 2.5-3x | Medium | Good for archives |
| Zstd Level 3 | Slower | 3-4x | Higher | Consider for cold data |

**Recommendation**: Implement tiered compression:
- Hot data (<24h): Snappy (current)
- Warm data (1-7 days): Zstd level 1
- Cold data (>7 days): Zstd level 3

---

## 8. Docker Service Optimizations

### 8.1 Reduce etcd Memory

Current: 256MB limit + 512MB quota

**Recommendation**:
```yaml
etcd:
  deploy:
    resources:
      limits:
        memory: 128M  # Reduced from 256M
  environment:
    - ETCD_QUOTA_BACKEND_BYTES=134217728  # 128MB (from 512MB)
```

**Rationale**: NDP uses etcd for config only (~10-50 stream configs). 128MB is more than sufficient.

### 8.2 Right-size air-quality-app

Current: 512MB limit

**Recommendation**:
```yaml
air-quality-app:
  deploy:
    resources:
      limits:
        memory: 256M  # Reduced from 512M
      reservations:
        memory: 128M  # Ensure minimum
```

**With buffer optimizations**: Actual usage should be ~100-150MB

### 8.3 TimescaleDB Tuning

For Pi5 with limited memory:

```yaml
timescaledb:
  environment:
    # Add PostgreSQL memory tuning
    - POSTGRES_SHARED_BUFFERS=64MB
    - POSTGRES_WORK_MEM=4MB
    - POSTGRES_MAINTENANCE_WORK_MEM=32MB
    - POSTGRES_EFFECTIVE_CACHE_SIZE=128MB
```

### 8.4 Proposed Optimized Allocation

| Service | Current | Proposed | Savings |
|---------|---------|----------|---------|
| mosquitto | 128M | 64M | 64M |
| etcd | 256M | 128M | 128M |
| air-quality-app | 512M | 256M | 256M |
| timescaledb | 256M | 256M | - |
| ndp-mcp-server | 96M | 64M | 32M |
| grafana | 256M | 192M | 64M |
| silver-etl-daemon | 256M | 192M | 64M |
| **Total** | **1.76GB** | **1.15GB** | **~600MB** |

---

## 9. Lazy Loading Opportunities

### 9.1 etcd Configuration

**Current**: All stream configs loaded at startup
**Recommendation**: Load configs on-demand with TTL cache

```rust
struct LazyStreamConfig {
    cache: LruCache<String, StreamConfig>,
    ttl: Duration,
}
```

### 9.2 Parquet Schema

**Current**: Schema parsed on every read
**Recommendation**: Cache schema per stream

### 9.3 DuckDB in silver-etl

**Current**: Full DuckDB instance created per ETL run
**Recommendation**: Maintain persistent connection with lazy postgres attachment

---

## 10. Summary of Recommendations

### High Impact (Implement First)

1. **Reduce channel buffers** (250 from 1000): ~1MB savings, improved backpressure
2. **Docker memory limits** (optimized allocation): ~600MB total savings
3. **Buffered WAL flushing** (every 10-50 entries): Reduced I/O, less SD card wear
4. **Tokio worker threads** (2 instead of 4): Better resource sharing

### Medium Impact

5. **etcd quota reduction** (128MB from 512MB): 384MB savings
6. **String interning** (`Arc<str>` for location_id): Reduced heap allocation
7. **Tiered compression** (Zstd for archives): Better storage efficiency

### Low Impact / Future

8. **Binary WAL format**: Reduced serialization overhead
9. **Row group appending**: Avoid read-modify-write pattern
10. **Lazy config loading**: Faster startup, lower baseline memory

---

## 11. Implementation Priority

| Phase | Changes | Estimated Impact |
|-------|---------|------------------|
| **Phase 1** (No code changes) | Docker memory limits, env vars | 400-600MB |
| **Phase 2** (Config changes) | Buffer sizes, batch sizes | 1-2MB + better latency |
| **Phase 3** (Code changes) | WAL buffering, string interning | 10-20% heap reduction |
| **Phase 4** (Architecture) | Tiered compression, lazy loading | Long-term efficiency |

---

## 12. Monitoring Recommendations

To validate optimizations:

1. **Add memory metrics** to Prometheus:
   ```rust
   // In air-quality-app
   process_resident_memory_bytes
   process_virtual_memory_bytes
   ```

2. **Track channel utilization**:
   ```rust
   channel_capacity_remaining{stream="air-quality"}
   ```

3. **Monitor GC/allocation** via jemalloc profiling:
   ```toml
   [profile.release]
   opt-level = 3
   lto = true
   ```

---

## Appendix A: Current Code References

| File | Function | Memory Pattern |
|------|----------|----------------|
| `core/src/storage/parquet.rs` | `append_to_parquet` | Read-modify-write |
| `core/src/storage/wal.rs` | `append` | Sync on every write |
| `core/src/coordinator/ingestion_coordinator.rs` | `IngestionCoordinatorConfig` | Channel sizes |
| `apps/air-quality-app/src/config.rs` | `MqttConfig` | Buffer capacity |
| `deploy/pi/docker-compose.yml` | All services | Memory limits |

## Appendix B: Test Commands

```bash
# Monitor actual memory usage
docker stats --no-stream

# Check specific container
docker exec air-quality-app cat /proc/meminfo

# Profile with heaptrack (requires setup)
heaptrack /usr/local/bin/air-quality-app
```
