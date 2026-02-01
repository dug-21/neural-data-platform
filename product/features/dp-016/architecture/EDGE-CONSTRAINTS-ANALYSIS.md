# Edge Device Constraints Analysis for Configuration Architecture

**Feature**: dp-016 (Configuration Architecture Review)
**Target Platform**: Raspberry Pi 5
**Date**: 2026-02-01

---

## Executive Summary

This document analyzes the edge device constraints that affect configuration architecture decisions for the Neural Data Platform. The platform runs on Raspberry Pi 5 devices with limited resources compared to cloud/datacenter deployments. These constraints directly influence choices around config caching, validation overhead, storage strategies, and startup behavior.

**Key Findings**:
1. **Memory**: Tight budget (~1.7GB allocated), favors lazy loading with TTL cache
2. **Storage**: SD card wear concerns, favors batched writes and reduced sync frequency
3. **CPU**: 4 cores available, validation overhead acceptable if async
4. **Network**: etcd is local (same Docker network), latency is minimal
5. **Startup**: Cold start matters - 15-30 minute builds, config sync on every boot

---

## 1. Memory Constraints

### 1.1 Available Memory

| Source | Evidence | Value |
|--------|----------|-------|
| Pi README | `deploy/pi/README.md:8` | "Raspberry Pi 5 (16GB RAM recommended)" |
| docker-compose.prod.yml | `docker-compose.prod.yml:4` | "Optimized for: 8GB RAM" |
| Resource research | `research/platform-minimize/03-resource-optimization.md:4` | "<2GB total memory target (current ~1.7GB)" |

**Actual Pi Models in Use**:
- Recommended: 16GB RAM Pi 5 (headroom for development)
- Optimized for: 8GB RAM Pi 5 (production target)
- Memory budget: <2GB for all NDP services combined

### 1.2 Current Memory Allocation

From `deploy/pi/docker-compose.yml`:

| Service | Memory Limit | Reserved | Evidence |
|---------|-------------|----------|----------|
| mosquitto | 128M | - | Line 31 |
| etcd | 256M | - | Line 62 |
| air-quality-app | 512M | - | Line 117 |
| timescaledb | 256M | 128M | Lines 143-145 |
| ndp-mcp-server | 96M | - | Line 187 |
| grafana | 256M | - | Line 228 |
| silver-etl-daemon | 256M | - | Line 294 |
| **Total** | **~1.76GB** | **128M** | |

### 1.3 Config Caching Impact

**Current Implementation** (`config-client/src/stream/registry.rs:10-12`):
```rust
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<std::collections::HashMap<String, StreamConfig>>>,
}
```

**Cache Behavior**:
- Unbounded HashMap (no TTL, no size limit)
- All loaded configs cached indefinitely
- Cache cleared only on explicit `clear_cache()` call

**Memory Impact Analysis** (`research/platform-minimize/03-resource-optimization.md:66-68`):
- StreamConfig: ~300-600 bytes per stream
- With 10 streams: ~3-6KB in cache
- With 50 streams: ~15-30KB in cache

**Recommendation**: Current approach is acceptable. Even 100 streams would be <60KB, negligible compared to 512M app limit. However, recommend adding TTL (e.g., 5 minutes) for config freshness.

### 1.4 Memory Optimization Recommendations

From `research/platform-minimize/03-resource-optimization.md`:

| Current | Recommended | Savings | Evidence |
|---------|-------------|---------|----------|
| etcd 256M | 128M | 128M | Lines 286-299 |
| air-quality-app 512M | 256M | 256M | Lines 302-315 |
| Ingestion channel 1000 | 250 | ~1MB | Lines 74-78 |
| **Total potential** | | **~500MB** | |

---

## 2. Storage Constraints

### 2.1 Storage Configuration

**Primary Storage**: SD card or USB SSD

| Volume | Container | Usage | Evidence |
|--------|-----------|-------|----------|
| `etcd-data` | etcd | Config store | `deploy/pi/docker-compose.yml:303` |
| `air-quality-data` | air-quality-app | Parquet files | Line 305 |
| `timescaledb-data` | timescaledb | Silver layer | Line 308 |
| `grafana-data` | grafana | Dashboards | Line 309 |

**etcd Storage Quota** (`deploy/pi/docker-compose.yml:50`):
```yaml
- ETCD_QUOTA_BACKEND_BYTES=536870912  # 512MB quota
```

### 2.2 SD Card Wear Concerns

From `research/platform-minimize/03-resource-optimization.md:143-171`:

**Current WAL Behavior**:
```rust
// wal.rs - sync on every append
self.file.flush()?;  // Sync on every write
```

**Issues Identified**:
1. Flush on every write causes high I/O
2. Each point triggers a syscall
3. SD card wear from frequent small writes

**Recommended Pattern**:
```rust
// Buffered flushing
if self.pending_count >= FLUSH_THRESHOLD {
    self.file.flush()?;
    self.pending_count = 0;
}
```

### 2.3 etcd Storage Impact

**Current Usage Pattern**:
- etcd stores ~10-50 stream configs
- Each config: 2-10KB JSON
- Total: 50-500KB for configs

**Quota Analysis** (`research/platform-minimize/03-resource-optimization.md:39`):
- Current quota: 512MB (excessive for config-only usage)
- Recommended: 128MB is sufficient

**Impact on Config Architecture**:
- etcd storage is not a constraint
- 128MB quota supports thousands of configs
- Syncing YAML to etcd on every startup is acceptable

---

## 3. CPU Constraints

### 3.1 Available CPU

| Source | Evidence | Value |
|--------|----------|-------|
| Pi 5 Hardware | Standard | 4 cores @ 2.4GHz |
| docker-compose.prod.yml | Line 66-67 | 2 cores allocated to air-quality-app |
| Setup script | `scripts/setup-pi5.sh:57-58` | `RAYON_NUM_THREADS=2`, `TOKIO_WORKER_THREADS=2` |

**CPU Allocation**:
```yaml
# docker-compose.prod.yml:66-67
limits:
  cpus: '2.0'      # 2 cores max
reservations:
  cpus: '1.0'      # 1 core guaranteed
```

### 3.2 Validation Overhead Impact

**Current Validation** (`config-client/src/stream/registry.rs:46-49`):
```rust
config
    .validate()
    .map_err(|e| ConfigError::EnvError(format!("Invalid stream config: {}", e)))?;
```

**Validation Scope** (from neural-core StreamConfig):
- Stream ID regex validation
- Fields array non-empty check
- Sources array non-empty check
- Field type validation
- Source parameter validation

**CPU Impact**:
- Validation per config: <1ms (regex + array checks)
- 50 configs on startup: <50ms total
- **Acceptable overhead** for edge devices

### 3.3 Serialization Overhead

From `research/platform-minimize/03-resource-optimization.md:243-247`:

**Current Pattern**:
```rust
// JSON serialization for each point
contexts.push(p.context.as_ref().map(|c| c.to_string()));
```

**Recommendation**: Consider MessagePack for context field (20-30% space savings)

**Impact on Config**:
- Config loading uses serde_json (fast)
- Config size is small (<10KB per stream)
- **Not a bottleneck** for config architecture

---

## 4. Network Constraints

### 4.1 etcd Network Configuration

**Local etcd** (`deploy/pi/docker-compose.yml:34-48`):
```yaml
etcd:
  container_name: etcd
  ports:
    - "2379:2379"
  environment:
    - ETCD_LISTEN_CLIENT_URLS=http://0.0.0.0:2379
    - ETCD_ADVERTISE_CLIENT_URLS=http://etcd:2379
```

**Network Topology**:
- All services on same Docker bridge network (`neural-network`)
- etcd accessible via `http://etcd:2379` (container name)
- No external network hops

### 4.2 Latency Characteristics

| Path | Latency | Evidence |
|------|---------|----------|
| Container-to-container | <1ms | Same Docker network |
| etcd read | <5ms | Local gRPC |
| etcd write | <10ms | Local with fsync |

**Impact on Config Architecture**:
- etcd is effectively "local" storage
- No need to minimize etcd calls for latency reasons
- Cache primarily for connection pooling, not latency reduction

### 4.3 Remote etcd Considerations

**Current**: etcd is always local (same Pi)

**Future Considerations**:
- Cluster mode with multiple Pis would need remote etcd
- Remote etcd latency: 10-50ms over LAN
- If remote etcd, aggressive caching with TTL becomes important

**Recommendation**: Design for local etcd but support optional caching for future remote scenarios.

---

## 5. Startup Time Constraints

### 5.1 Cold Start Characteristics

**Build Time** (`deploy/pi/README.md:24`):
> First build takes **15-30 minutes** (Rust compilation)

**Service Startup Order** (`deploy/pi/docker-compose.yml` dependencies):
```
1. etcd starts (health: 30s interval)
2. mosquitto starts (health: 30s interval)
3. timescaledb starts (health: 30s interval, 30s start_period)
4. air-quality-app starts (depends on all above)
5. Config sync runs inside air-quality-app
```

### 5.2 Config Loading on Startup

**Current Behavior** (`product/features/dp-016/specification/DEPLOYMENT-RESEARCH.md:260-270`):
```rust
// Sync YAML configs to etcd on every startup
if std::path::Path::new(&config_dir).exists() {
    let sync_service = ConfigSyncService::new(&config_dir);
    match registry.sync_all(&registry).await {
        Ok(count) => info!("Synced {} stream configs to etcd", count),
        Err(e) => warn!("Config sync failed: {}. Using existing etcd configs.", e),
    }
}
```

**Startup Sequence** (`deploy/pi/deploy.sh:1080-1094`):
```bash
start() {
    dc up -d
    sleep 10
    sync_config
    init_streams
    status
}
```

### 5.3 Startup Time Impact

**Current Startup Timeline** (approximate):

| Step | Duration | Notes |
|------|----------|-------|
| etcd healthy | 30-60s | Includes start_period |
| timescaledb healthy | 30-60s | Includes init scripts |
| mosquitto healthy | 10-30s | Lightweight |
| air-quality-app launch | 5-10s | Binary startup |
| Config sync to etcd | 2-5s | YAML parse + etcd writes |
| Stream initialization | 1-2s | Per-stream setup |
| **Total** | **~90-180s** | From `docker compose up` |

### 5.4 Startup Optimization Opportunities

From `research/platform-minimize/03-resource-optimization.md:349-369`:

**Lazy Loading Pattern**:
```rust
struct LazyStreamConfig {
    cache: LruCache<String, StreamConfig>,
    ttl: Duration,
}
```

**Recommendation**: Load configs on-demand rather than all at startup
- Only load configs for active sources
- Defer validation until first use
- Use LRU cache with TTL

---

## 6. Config Architecture Recommendations

Based on edge device constraints, the following recommendations apply:

### 6.1 Memory Recommendations

| Recommendation | Rationale | Priority |
|----------------|-----------|----------|
| Add TTL to StreamRegistry cache | Ensure config freshness without memory bloat | Medium |
| Consider LRU cache (max 50 entries) | Bound memory usage for large deployments | Low |
| Reduce Docker memory limits | Free ~500MB for other uses | High |

### 6.2 Storage Recommendations

| Recommendation | Rationale | Priority |
|----------------|-----------|----------|
| Reduce etcd quota to 128MB | Current 512MB is excessive | Medium |
| Batch config sync writes | Reduce SD card wear | Medium |
| Single YAML-to-etcd sync on deploy only | Not on every app restart | High |

### 6.3 CPU Recommendations

| Recommendation | Rationale | Priority |
|----------------|-----------|----------|
| Keep validation async/parallel | 4 cores available, validation is fast | Low |
| Pre-validate YAML at sync time | Fail fast, not at runtime | High |
| Consider schema caching | Avoid re-parsing on every load | Low |

### 6.4 Network Recommendations

| Recommendation | Rationale | Priority |
|----------------|-----------|----------|
| Maintain local etcd assumption | Simplifies architecture | - |
| Add connection pooling | Reduce connection overhead | Medium |
| Support optional remote etcd mode | Future clustering | Low |

### 6.5 Startup Recommendations

| Recommendation | Rationale | Priority |
|----------------|-----------|----------|
| Lazy load stream configs | Faster startup, load on demand | Medium |
| Sync YAML to etcd only on `deploy.sh sync` | Not on every restart | High |
| Parallel config validation | Use available cores | Low |

---

## 7. Summary Table

| Constraint | Current State | Impact on Config Architecture | Recommendation |
|------------|---------------|------------------------------|----------------|
| **Memory** | 1.7GB total, 512M for app | Unbounded cache acceptable for <100 streams | Add TTL cache, reduce Docker limits |
| **Storage** | SD card, 512MB etcd quota | Frequent writes cause wear | Batch syncs, reduce quota |
| **CPU** | 4 cores, 2 for app | Validation overhead minimal | Keep validation, make async |
| **Network** | Local etcd (<1ms latency) | No caching needed for latency | Design for local, support remote |
| **Startup** | 90-180s cold start | Config sync adds 2-5s | Lazy load, sync on deploy only |

---

## 8. Code Evidence References

| Finding | File | Lines |
|---------|------|-------|
| Memory limits | `deploy/pi/docker-compose.yml` | 31, 62, 117, 143-145, 187, 228, 294 |
| etcd quota | `deploy/pi/docker-compose.yml` | 50 |
| Cache implementation | `config-client/src/stream/registry.rs` | 10-12, 31-58 |
| Config sync on startup | `product/features/dp-016/specification/DEPLOYMENT-RESEARCH.md` | 260-270 |
| CPU allocation | `docker-compose.prod.yml` | 66-67 |
| Tokio workers | `scripts/setup-pi5.sh` | 57-58 |
| WAL flush pattern | `research/platform-minimize/03-resource-optimization.md` | 143-171 |
| Memory optimization | `research/platform-minimize/03-resource-optimization.md` | 286-342 |
| PostgreSQL tuning | `deploy/timescaledb/conf/postgresql.conf` | 18-31 |
| Build time | `deploy/pi/README.md` | 24 |
| Startup sequence | `deploy/pi/deploy.sh` | 1080-1094 |

---

## 9. Related Documents

- `research/platform-minimize/03-resource-optimization.md` - Detailed memory/CPU analysis
- `product/features/dp-016/specification/DEPLOYMENT-RESEARCH.md` - Deployment process documentation
- `deploy/pi/README.md` - Pi deployment guide
- `deploy/timescaledb/conf/postgresql.conf` - TimescaleDB memory tuning

---

*Analysis completed: 2026-02-01*
*Platform: Raspberry Pi 5 (8-16GB RAM)*
