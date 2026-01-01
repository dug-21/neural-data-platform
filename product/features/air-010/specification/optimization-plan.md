# AIR-010: Rust Ingestion Application Optimization Plan

**Date**: 2026-01-01
**Status**: PLANNING COMPLETE (Documentation Only)
**Methodology**: Parallel Mesh Swarm Analysis (6 agents)

---

## Executive Summary

This comprehensive optimization plan consolidates findings from 6 parallel analysis domains to maximize operational efficiency of the Neural Data Platform's Rust ingestion application.

### Aggregate Projections

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| **Throughput** | Baseline | +40-60% | Via async/concurrency optimizations |
| **Memory Usage** | Baseline | -15-25% | Via allocation reduction |
| **Compile Time** | ~63.5s | ~30s | **-53%** via config-store removal + dependency optimization |
| **Binary Size** | Baseline | -20-30% | Via config-store removal + dependency trimming |
| **Code Lines** | ~25,500 | ~20,000 | -22% via dead code + config-store removal |
| **Complexity** | 31 public exports | 12 public exports | -61% API surface |
| **Transitive Deps** | ~300+ | ~250 | -50+ deps via config-store removal |

---

## Analysis Summary by Domain

### 1. Dead Code Analysis
- **Findings**: 14 items, ~985 LOC removable (2.3%)
- **High Priority**: Commented-out forecast module (~880 LOC)
- **Report**: `reports/dead-code-analysis.md`

### 2. Memory Optimization Analysis
- **Findings**: 12 high-impact, 18 medium-impact issues
- **High Priority**: RwLock→Atomic, eliminate Vec clones in hot paths
- **Report**: `reports/memory-optimization-analysis.md`

### 3. Async/Concurrency Analysis
- **Findings**: 10 optimization opportunities
- **High Priority**: Parallel sensor polling (+30-50%), WAL async I/O (+15-20%)
- **Report**: `reports/async-concurrency-analysis.md`

### 4. Dependency Analysis
- **Findings**: 24 optimization opportunities
- **CRITICAL**: Remove entire `config-store` crate (-8-12s compile, -3MB binary, -50+ deps)
- **High Priority**: Reduce polars features (-15s), remove unused sqlx (-2s)
- **Report**: `reports/dependency-analysis.md`

### 5. Error Handling Analysis
- **Findings**: 2 high-risk, 5 medium-risk patterns
- **High Priority**: Remove .unwrap() in production signal handling
- **Report**: `reports/error-handling-analysis.md`

### 6. Architecture Analysis
- **Findings**: 7 major patterns to simplify, 5 code duplications
- **High Priority**: Consolidate dual coordinator, unify Source/RawSource traits
- **Report**: `reports/architecture-analysis.md`

---

## Implementation Phases

### Phase 0: Critical Dead Crate Removal (HIGHEST PRIORITY)
**Estimated Time**: 1 day
**Expected Improvement**: -8-12s compile time, -3MB binary, -50+ transitive deps
**Risk Level**: Low (crate is completely unused)

The `config-store` crate is **entirely dead code** - declared as a dependency but never imported or used.

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P0-01 | Remove `config-store` dependency | `apps/air-quality-app/Cargo.toml:21` | Removes unused dep |
| P0-02 | Remove `config-store` from workspace | `Cargo.toml` workspace members | -8-12s compile |
| P0-03 | Delete or archive `config-store/` directory | `config-store/` | -2000+ LOC, -3MB |
| P0-04 | Remove orphaned proto directories | `proto/`, `schemas/` | Cleanup |

**Evidence of Dead Code:**
```bash
# No usage in air-quality-app source code
grep -r "use config_store" apps/air-quality-app/src/  → No matches

# Proto files don't exist (gRPC never configured)
ls proto/*.proto  → No files found
ls schemas/*.proto  → No files found
```

**Heavy Dependencies Eliminated:**
| Dependency | Purpose | Compile Cost |
|------------|---------|--------------|
| `tonic` 0.10 | gRPC framework | ~4s |
| `prost` 0.12 | Protobuf | ~2s |
| `redis` 0.25 | Redis client | ~2s |
| `jsonschema` 0.17 | Schema validation | ~3s |
| `clap` 4.0 | CLI parsing | ~1s |

**Root Cause:** Application was rewritten to use `config-client` (etcd) and YAML config. The legacy gRPC-based `config-store` was abandoned but never removed.

---

### Phase 1: Quick Wins (Low Risk, High Impact)
**Estimated Time**: 2-3 days
**Expected Improvement**: +35-50% throughput, -5s compile time

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P1-01 | Parallel sensor polling with `buffer_unordered` | `http_poll.rs:462` | +30-50% |
| P1-02 | Parallel partition writes with `try_join_all` | `parquet.rs:260` | +20-30% |
| P1-03 | Cache regex with `Lazy<Regex>` | `source_manager.rs:632` | +5% |
| P1-04 | Replace `RwLock<HashMap>` with `DashMap` | `router.rs:189` | +10-15% |
| P1-05 | Replace `RwLock<CoordinatorStats>` with atomics | `ingestion_coordinator.rs:214` | 50x faster |
| P1-06 | Remove unused imports | Multiple | Clean |
| P1-07 | Add `#[inline]` to builder methods | Multiple | +5% |

### Phase 2: Memory Optimization (Medium Risk)
**Estimated Time**: 3-5 days
**Expected Improvement**: -15-25% memory, -40% allocations

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P2-01 | Use `std::mem::take` instead of `.clone()` in flush | `storage_writer.rs:135` | -100KB/flush |
| P2-02 | Pre-allocate Vecs in `write_parquet` | `parquet.rs:93` | -6 allocs/batch |
| P2-03 | Reuse buffer for WAL serialization | `parquet.rs:246` | -100 allocs/batch |
| P2-04 | Use `Cow<'static, str>` for tag keys | `router.rs:181` | -2 allocs/point |
| P2-05 | Static ParserConfig with `Lazy` | `source_manager.rs:402` | -7 allocs/spawn |
| P2-06 | Use `PathBuf::push` instead of `join` chains | `parquet.rs:64` | -5 allocs/path |

### Phase 3: I/O and Async (Medium-High Risk)
**Estimated Time**: 5-7 days
**Expected Improvement**: +25-35% I/O throughput

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P3-01 | Convert WAL to async I/O with `tokio::fs` | `wal.rs:24` | +15-20% |
| P3-02 | Wrap Parquet ops in `spawn_blocking` | `parquet.rs:83` | +10-15% |
| P3-03 | Batch MQTT cache updates | `mqtt/mod.rs:341` | +15-25% |
| P3-04 | Use atomic flags for MQTT state | `mqtt/mod.rs:314` | Reduce contention |
| P3-05 | Add backpressure handling to channels | Various | Reliability |

### Phase 4: Dependency Optimization
**Estimated Time**: 1-2 days
**Expected Improvement**: -35% additional compile time (on top of Phase 0), -10% binary

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P4-01 | Remove unused `sqlx` dependency | `Cargo.toml` | -2s, -800KB |
| P4-02 | Reduce polars features (remove `dtype-duration`) | `Cargo.toml` | -5s, -2MB |
| P4-03 | Use `rustls-tls` for reqwest | `Cargo.toml` | -3s, -1MB |
| P4-04 | Minimize tokio features | `Cargo.toml` | -1s, -200KB |
| P4-05 | Align mockall version in workspace | `Cargo.toml` | -1s |
| P4-06 | Consolidate workspace dependencies | `Cargo.toml` | -2s |

*Note: Items P4-05/06/07 from original plan (config-store specific) moved to Phase 0 since entire crate is being removed.*

### Phase 5: Architecture Refactoring (Higher Risk)
**Estimated Time**: 1-2 weeks (future feature)
**Expected Improvement**: -25% codebase, -50% complexity

| ID | Optimization | Location | Impact |
|----|-------------|----------|--------|
| P5-01 | Consolidate dual coordinator pattern | `core/` + `app/coordinator/` | -600 LOC |
| P5-02 | Unify Source/RawSource traits | `traits.rs` | -15% traits |
| P5-03 | Unify Store/RawStore traits | `traits.rs` | -15% traits |
| P5-04 | Remove legacy types | `types/` | -15-20% types |
| P5-05 | Simplify config loading | `main.rs:27-102` | -40% config code |
| P5-06 | Delete forecast module or complete it | `core/src/forecast/` | -880 LOC |
| P5-07 | Create prelude module for API | `lib.rs` | -61% exports |

---

## Consolidated Metrics

### Throughput Improvements

| Component | Current | After Phase 1 | After Phase 3 |
|-----------|---------|---------------|---------------|
| HTTP Polling | 1x | 1.3-1.5x | 1.3-1.5x |
| MQTT Ingestion | 1x | 1.1-1.2x | 1.3-1.5x |
| Parquet Writes | 1x | 1.2-1.3x | 1.4-1.6x |
| Stats Updates | 1x | 50x (atomic) | 50x |
| **Overall** | **1x** | **1.35-1.5x** | **1.5-1.7x** |

### Memory Improvements

| Metric | Current | After Phase 2 |
|--------|---------|---------------|
| Allocs per point | 8-12 | 3-5 |
| Allocs per batch (100 pts) | 800+ | 200-300 |
| Peak memory (ingestion) | 1x | 0.85-0.90x |

### Build Improvements

| Metric | Current | After Phase 0 | After Phase 4 |
|--------|---------|---------------|---------------|
| Clean build | 63.5s | ~52s (-18%) | ~30s (-53%) |
| Binary size | ~25MB | ~22MB (-12%) | ~17MB (-32%) |
| Transitive deps | ~300+ | ~250 (-17%) | ~220 (-27%) |

### Code Complexity

| Metric | Current | After Phase 0 | After Phase 5 |
|--------|---------|---------------|---------------|
| Total LOC | ~25,500 | ~23,500 (-8%) | ~20,000 (-22%) |
| Workspace crates | 5 | 4 (-20%) | 4 |
| Public exports | 31 | 31 | 12 (-61%) |
| Trait definitions | 8 | 8 | 5 (-38%) |
| Data structs | 12 | 12 | 6 (-50%) |

---

## Risk Assessment

| Phase | Risk Level | Mitigation |
|-------|------------|------------|
| Phase 0 | **Very Low** | Crate is unused - verify with grep, remove and build |
| Phase 1 | Low | Parallel ops are additive, existing tests cover |
| Phase 2 | Medium | Memory changes need benchmarks, could affect behavior |
| Phase 3 | Medium-High | I/O changes require integration testing |
| Phase 4 | Low | Dependency changes have clear fallback |
| Phase 5 | High | Architecture changes need careful migration |

---

## Testing Requirements

### Before Implementation
1. Create baseline benchmarks with `criterion`
2. Document current memory usage with `heaptrack`
3. Record compile times and binary sizes

### Per-Phase Testing
- **Phase 0**: Build verification + grep verification (no `use config_store` anywhere)
- **Phase 1**: Unit tests + load tests (1000 pts/sec)
- **Phase 2**: Memory profiling + regression tests
- **Phase 3**: Integration tests + stress tests
- **Phase 4**: Build verification only
- **Phase 5**: Full system tests + migration validation

### Metrics to Track
- Points ingested per second (target: +40%)
- P99 write latency (target: -30%)
- Memory per 1000 points (target: -25%)
- Allocation rate (target: -50%)

---

## Related Documentation

### ADRs to Create (Phase 5)
- ADR-005: Unified Data Point Type Hierarchy
- ADR-006: Generic Trait Pattern for Source/Store
- ADR-007: Configuration Loading Strategy
- ADR-008: Coordinator Module Consolidation

### Reports Generated
1. `reports/dead-code-analysis.md`
2. `reports/memory-optimization-analysis.md`
3. `reports/async-concurrency-analysis.md`
4. `reports/dependency-analysis.md`
5. `reports/error-handling-analysis.md`
6. `reports/architecture-analysis.md`

---

## Implementation Notes

### DO NOT IMPLEMENT
This is a **planning document only**. Implementation requires:
1. Stakeholder review and approval
2. Feature branch creation
3. Incremental PR workflow
4. CI/CD validation

### Next Steps
1. Review this plan with team
2. Prioritize phases based on roadmap
3. Create implementation tickets
4. Establish baseline metrics
5. **Begin Phase 0 (Remove config-store) - Immediate, low-risk, high-impact**
6. Continue with Phase 1 (Quick Wins)

---

*Generated by AIR-010 Optimization Mesh Swarm*
*6 agents | 10M+ tokens analyzed | 100+ files reviewed*
