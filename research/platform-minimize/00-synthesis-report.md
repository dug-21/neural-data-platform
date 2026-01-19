# NDP Platform Optimization - Research Synthesis

**Date**: 2026-01-17
**Research Team**: Hive-Mind Swarm (4 parallel agents)
**Status**: Research Complete

---

## Executive Summary

This synthesis combines findings from four parallel research agents analyzing the Neural Data Platform for optimization opportunities aligned with:

1. **Config-driven enhancements** - Reducing code changes for new functionality
2. **Resource optimization** - Minimizing CPU/memory for Raspberry Pi 5 deployment
3. **Latency reduction** - Faster time from ingestion to queryable data

### Key Findings at a Glance

| Area | Current State | Optimization Potential |
|------|---------------|------------------------|
| Memory Usage | ~1.76GB allocated | Reduce to ~1.15GB (-600MB) |
| End-to-End Latency | 5-15 minutes | Target: 30-90 seconds |
| Config Coverage | ~70% config-driven | Target: ~95% config-driven |
| Code Changes for New Streams | 2-3 files | Target: 0 files (config only) |

---

## Unified Recommendations

### Phase 1: Zero-Code Quick Wins (Week 1)

These require no Rust code changes - only configuration/deployment updates:

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 1.1 | **Reduce Docker memory limits** | -600MB total footprint | Trivial |
| | - etcd: 256M → 128M | -128MB | |
| | - air-quality-app: 512M → 256M | -256MB | |
| | - mosquitto: 128M → 64M | -64MB | |
| 1.2 | **Reduce ETL daemon interval** (300s → 60s) | -4 min latency | Trivial |
| 1.3 | **Reduce batch timeout** (5s → 2s for MQTT) | -3s latency | Config |
| 1.4 | **Reduce channel buffers** (1000 → 250) | -1MB RAM | Config |
| 1.5 | **Set tokio worker threads** (4 → 2) | Better resource sharing | Env var |

**Combined Phase 1 Impact**:
- Memory: -600MB (~35% reduction)
- Latency: 5-15 min → 2-5 min (50-70% reduction)
- Code changes: None

### Phase 2: Configuration Enhancements (Week 2-3)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 2.1 | **Create `config/base/defaults.yaml`** | Eliminate 15+ hardcoded values | Medium |
| 2.2 | **Add JSON Schema for stream configs** | IDE autocompletion, validation | Medium |
| 2.3 | **Implement watch reconnection** | More reliable hot-reload | Medium |
| 2.4 | **Add per-stream storage settings** | Fine-grained tuning | Medium |

**Phase 2 Impact**: More operational flexibility, fewer "magic numbers" in code

### Phase 3: Code Optimizations (Week 3-4)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 3.1 | **Batched WAL commits** (every 10 entries) | Reduce I/O by 90% | Medium |
| 3.2 | **String interning** (`Arc<str>` for IDs) | -10-20% heap allocation | Medium |
| 3.3 | **Parallel stream ETL** (3 concurrent) | -50% ETL cycle time | Higher |
| 3.4 | **Implement `Custom` transform formula** | Full config-driven transforms | Higher |

**Phase 3 Impact**: Significant performance improvements, near-zero-code stream additions

### Phase 4: Advanced Architecture (Future)

| # | Action | Impact | Effort |
|---|--------|--------|--------|
| 4.1 | **Event-driven ETL** (file watcher) | Near-real-time Silver | High |
| 4.2 | **Continuous aggregates** | Faster dashboard queries | Medium |
| 4.3 | **Plugin system for sources/parsers** | True zero-code extensibility | High |
| 4.4 | **Tiered compression** (Zstd for archives) | Better long-term storage | Medium |
| 4.5 | **Direct TimescaleDB writes** | Sub-second alerting | High |

---

## Detailed Synthesis by Goal

### Goal 1: Config-Driven Enhancements

**Current State**: NDP is already ~70% config-driven with YAML stream configs, etcd hot-reload, and declarative DQ rules.

**Gaps Identified**:
1. **15+ hardcoded defaults** in `source_manager.rs` and `http_poll.rs`
2. **Custom transforms stubbed but not implemented** (`ConversionFormula::Custom`)
3. **No plugin system** - new source/parser types require code changes
4. **No validation on hot-reload** - invalid config can break running system

**Recommended Enhancements**:

```yaml
# config/base/defaults.yaml (NEW)
mqtt:
  port: 1883
  qos: 1
  reconnect_delay_secs: 5
  buffer_capacity: 250  # Reduced from 1000

http_poll:
  poll_interval_secs: 60
  timeout_secs: 10
  max_concurrent_fetches: 10

storage:
  batch_size: 50
  batch_timeout_secs: 2
  buffer_capacity: 250
```

**Priority Files to Modify**:
- `core/src/coordinator/source_manager.rs:94-127` - Defaults extraction
- `core/src/config/silver_etl.rs:294-306` - Implement Custom formula
- `config-client/src/watch.rs` - Add reconnection logic

### Goal 2: CPU/Memory Optimization

**Current State**: ~1.76GB Docker allocation, well within Pi5 8GB but oversized.

**Top Memory Hotspots**:
1. **Channel buffers** (4x 1000 items x 400 bytes = 1.6MB)
2. **etcd quota** (512MB configured, <1MB used)
3. **air-quality-app** (512MB limit, ~150MB actual)
4. **Parquet read-modify-write** (spikes to 2-3x file size during append)

**Recommended Docker Allocation**:

| Service | Current | Optimized | Savings |
|---------|---------|-----------|---------|
| mosquitto | 128M | 64M | 64M |
| etcd | 256M | 128M | 128M |
| air-quality-app | 512M | 256M | 256M |
| timescaledb | 256M | 256M | - |
| ndp-mcp-server | 96M | 64M | 32M |
| grafana | 256M | 192M | 64M |
| silver-etl-daemon | 256M | 192M | 64M |
| **Total** | **1.76GB** | **1.15GB** | **~600MB** |

**Rust Optimizations**:
```rust
// tokio runtime config (main.rs)
#[tokio::main(flavor = "multi_thread", worker_threads = 2)]

// String interning for repeated values (types.rs)
pub struct TimeSeriesPoint {
    pub location_id: Arc<str>,  // Instead of String
}

// WAL batched commits (wal.rs)
const WAL_FLUSH_THRESHOLD: usize = 10;
```

### Goal 3: Latency Reduction

**Current Latency Profile**:
- MQTT to Bronze: 5-10 seconds (batch timeout)
- HTTP to Bronze: 60-600 seconds (poll interval)
- Bronze to Silver: 300 seconds (ETL daemon interval)
- **Total**: 5-15 minutes typical

**Quick Win Latency Targets**:
| Change | Current | Target | Reduction |
|--------|---------|--------|-----------|
| ETL interval | 300s | 60s | -240s |
| Batch timeout | 5s | 2s | -3s |
| Weather polling | 600s | 300s | -300s |

**Expected Results After Phase 1**:
- MQTT streams: 2-5s to Bronze, 60-90s to Silver
- HTTP streams: 60-180s to Bronze, 120-240s to Silver
- **Total**: 1-4 minutes (60-80% reduction)

**Future Possibilities**:
- Event-driven ETL could achieve 10-30 second Silver latency
- Direct TimescaleDB writes could enable sub-second alerting

---

## Cross-Cutting Observations

### Pattern: Configuration Hierarchy

The codebase consistently uses a priority-based config hierarchy:
```
etcd stream registry > etcd legacy config > YAML files > code defaults
```

**Recommendation**: Document this hierarchy and ensure all defaults flow through it.

### Pattern: Trait-Based Abstraction

The `Source`, `RawSource`, `Store`, `RawStore` traits enable clean separation.

**Recommendation**: Extend this pattern for pluggable sources/parsers via a trait registry.

### Pattern: Batch + Timeout

Multiple components use "batch-or-timeout" patterns:
- Storage writer: 100 items or 5s
- WAL: immediate (no batching)
- ETL: full batch per stream

**Recommendation**: Standardize these patterns with configurable thresholds.

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Memory reduction causes OOM | Medium | High | Test on Pi before deploy |
| Shorter ETL interval overloads DB | Low | Medium | Monitor cycle duration |
| Smaller batches increase I/O | Medium | Low | Monitor disk utilization |
| Hot-reload breaks running system | Medium | Medium | Implement validation first |

---

## Implementation Checklist

### Week 1 (Zero-Code)
- [ ] Update `deploy/pi/docker-compose.yml` with optimized memory limits
- [ ] Set `TOKIO_WORKER_THREADS=2` in container environments
- [ ] Update silver-etl daemon command: `--interval 60`
- [ ] Update stream configs: `batch_timeout_secs: 2` (MQTT streams)
- [ ] Update stream configs: `buffer_capacity: 250`

### Week 2 (Config Files)
- [ ] Create `config/base/defaults.yaml`
- [ ] Create `config/schemas/stream_config.schema.json`
- [ ] Add watch reconnection to config-client

### Week 3 (Code Changes)
- [ ] Implement batched WAL commits
- [ ] Refactor to use `Arc<str>` for location_id
- [ ] Implement `ConversionFormula::Custom` with evalexpr

### Week 4 (Advanced)
- [ ] Design parallel ETL execution
- [ ] Add continuous aggregates for dashboards
- [ ] Prototype event-driven ETL

---

## Files Reference

| Research Area | Document |
|---------------|----------|
| Codebase Structure | `01-codebase-structure.md` |
| Config Enhancements | `02-config-enhancements.md` |
| Resource Optimization | `03-resource-optimization.md` |
| Latency Reduction | `04-latency-optimization.md` |

---

## Conclusion

The Neural Data Platform has a solid foundation with config-driven architecture and clean trait abstractions. The identified optimizations can achieve:

1. **35% memory reduction** (~600MB) through right-sized Docker limits and reduced buffers
2. **60-80% latency reduction** (from 5-15 min to 1-4 min) through shorter intervals
3. **Near-zero-code stream additions** through defaults extraction and transform implementation

Most Phase 1 improvements require no code changes and can be deployed immediately. The research provides a clear roadmap from quick wins to advanced architecture improvements.

---

*Research conducted by NDP Hive-Mind Swarm with shared memory coordination*
