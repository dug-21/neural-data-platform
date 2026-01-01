# DP-004: Implementation Checklist

## Overview

This document tracks the implementation progress for the Bronze Raw JSON Schema feature. The implementation follows a phased approach as defined in ADR-001.

## Phase Summary

| Phase | Description | Status | Owner |
|-------|-------------|--------|-------|
| Phase 1 | Core Types | Not Started | ndp-rust-dev |
| Phase 2 | Storage Layer | Not Started | ndp-parquet-dev |
| Phase 3 | Sources | Not Started | ndp-rust-dev |
| Phase 4 | Integration | Not Started | ndp-tester |
| Phase 5 | Silver ETL | Future (dp-005) | - |

---

## Phase 1: Core Types

**Goal**: Define `RawDataPoint` struct alongside existing `TimeSeriesPoint`

### Tasks

- [ ] **1.1 Create RawDataPoint struct**
  - File: `core/src/traits.rs`
  - Add `RawDataPoint` with fields:
    - `timestamp: DateTime<Utc>`
    - `source_id: String`
    - `ndp_id: Option<String>`
    - `context: Option<serde_json::Value>`
    - `raw_payload: serde_json::Value`
  - Derive: `Debug, Clone, Serialize, Deserialize, PartialEq`

- [ ] **1.2 Add RawDataPoint to module exports**
  - File: `core/src/lib.rs`
  - Export `RawDataPoint` from public API

- [ ] **1.3 Unit tests for RawDataPoint**
  - File: `core/src/traits.rs` (test module)
  - Test: Serialization round-trip
  - Test: Deserialization from JSON
  - Test: Optional fields (ndp_id, context)
  - Test: Nested raw_payload structures

### Acceptance Criteria
- [ ] `RawDataPoint` compiles without errors
- [ ] Unit tests pass (`cargo test --lib raw_data_point`)
- [ ] No breaking changes to `TimeSeriesPoint`

---

## Phase 2: Storage Layer

**Goal**: Update Parquet storage to write new 5-column schema

### Tasks

- [ ] **2.1 Define new Parquet schema**
  - File: `core/src/storage/parquet.rs`
  - Schema: `timestamp`, `source_id`, `ndp_id`, `context`, `raw_payload`
  - Use `LargeUtf8` for JSON columns

- [ ] **2.2 Implement RawDataPoint writer**
  - File: `core/src/storage/parquet.rs`
  - New function: `write_raw_data_points(path, points) -> Result<()>`
  - Use Arrow RecordBatch with new schema

- [ ] **2.3 Implement dual-write capability**
  - File: `core/src/storage/parquet.rs`
  - Config flag: `bronze_schema_version: "v2"` (or "v1" for legacy)
  - During transition: write both formats if configured

- [ ] **2.4 Add schema version detection for reading**
  - File: `core/src/storage/parquet.rs`
  - Detect schema version from Parquet metadata
  - Handle both v1 (tall) and v2 (wide) schemas

- [ ] **2.5 Unit tests for new schema**
  - Test: Write RawDataPoint to Parquet
  - Test: Read back and verify fields
  - Test: JSON column querying with DuckDB
  - Test: Schema version detection

- [ ] **2.6 Integration with WAL**
  - File: `core/src/storage/wal.rs` (if exists)
  - Ensure WAL can handle RawDataPoint serialization

### Acceptance Criteria
- [ ] New Parquet files use 5-column schema
- [ ] DuckDB can query JSON columns
- [ ] Backward compatibility with existing files
- [ ] Tests pass: `cargo test --lib storage`

---

## Phase 3: Sources

**Goal**: Update data sources to emit `RawDataPoint` instead of `Vec<TimeSeriesPoint>`

### Tasks

- [ ] **3.1 Define RawDataSource trait**
  - File: `core/src/traits.rs`
  - New trait: `RawDataSource` with `emit_raw() -> Stream<RawDataPoint>`
  - Parallel to existing `DataSource` trait

- [ ] **3.2 Update HTTP Poll source**
  - File: `core/src/sources/http_poll.rs`
  - Implement `RawDataSource` trait
  - Capture full response body as `raw_payload`
  - Extract `source_id` from config
  - Populate `ndp_id` and `context` from stream config

- [ ] **3.3 Update MQTT source (if exists)**
  - File: `core/src/sources/mqtt.rs` (or similar)
  - Implement `RawDataSource` trait
  - Store MQTT message payload as `raw_payload`
  - Use topic as part of source identification

- [ ] **3.4 Update Demo/Test source**
  - File: `core/src/sources/demo.rs` (if exists)
  - Generate synthetic `RawDataPoint` for testing

- [ ] **3.5 Update source merge logic**
  - File: `core/src/sources/merge.rs`
  - Handle merging of `RawDataPoint` streams

- [ ] **3.6 Unit tests for each source**
  - Test: HTTP poll returns valid RawDataPoint
  - Test: MQTT message converts to RawDataPoint
  - Test: source_id populated correctly
  - Test: context metadata preserved

### Acceptance Criteria
- [ ] All sources implement `RawDataSource`
- [ ] raw_payload contains exact source data
- [ ] Tests pass: `cargo test --lib sources`

---

## Phase 4: Integration

**Goal**: End-to-end pipeline testing and verification

### Tasks

- [ ] **4.1 Update IngestionCoordinator**
  - File: `apps/air-quality-app/src/pipeline/coordinator.rs` (or similar)
  - Route `RawDataPoint` to Parquet storage
  - Maintain backward compatibility mode

- [ ] **4.2 Integration tests**
  - Test: Full pipeline from source to Parquet file
  - Test: Verify Parquet file structure
  - Test: Query raw data with DuckDB
  - Test: Multiple sources writing concurrently

- [ ] **4.3 Performance benchmarks**
  - Benchmark: Write throughput comparison (v1 vs v2 schema)
  - Benchmark: Storage size comparison
  - Benchmark: Query performance on JSON columns

- [ ] **4.4 Backward compatibility verification**
  - Test: Old Parquet files still readable
  - Test: Mixed schema directory works
  - Test: Grafana queries still functional (if applicable)

- [ ] **4.5 Documentation updates**
  - Update: `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
  - Update: `docs/procedures/HOW_TO_ADD_NEW_STREAM.md`
  - Add: Bronze schema migration guide

### Acceptance Criteria
- [ ] Full pipeline integration test passes
- [ ] Performance meets baseline requirements
- [ ] No regression in existing functionality
- [ ] Documentation reflects new architecture

---

## Phase 5: Silver ETL (Future - dp-005)

**Note**: This phase is out of scope for dp-004. See FUTURE_WORK.md.

- [ ] Design Silver ETL pipeline
- [ ] Implement TimescaleDB transformations
- [ ] Build streaming or batch ETL
- [ ] Create monitoring dashboards

---

## Pre-Implementation Checklist

Before starting implementation:

- [x] ADR-001 approved
- [ ] Create feature branch: `feature/dp-004`
- [ ] Verify existing tests pass: `cargo test`
- [ ] Review current Parquet schema
- [ ] Identify all affected files

## Code Review Checklist

For each PR:

- [ ] Unit tests added/updated
- [ ] Integration tests added (if applicable)
- [ ] Documentation updated
- [ ] No breaking changes to public API
- [ ] Clippy warnings addressed
- [ ] rustfmt applied

---

## Risk Register

| Risk | Mitigation | Status |
|------|------------|--------|
| Storage size increase | Parquet compression + wide format offsets | Monitored |
| Query performance on JSON | Index strategy + DuckDB optimization | Planned |
| Backward compatibility | Dual-write + schema detection | Planned |
| Parser simplification scope creep | Defer parser changes to Phase 3 | Controlled |

---

## Implementation Notes

### File Dependencies

```
Phase 1 (Types)
    |
    v
Phase 2 (Storage) <-- Phase 3 (Sources)
    |                    |
    v                    v
    Phase 4 (Integration)
```

### Key Files to Modify

| Phase | File | Change Type |
|-------|------|-------------|
| 1 | `core/src/traits.rs` | Add struct |
| 1 | `core/src/lib.rs` | Export |
| 2 | `core/src/storage/parquet.rs` | Major refactor |
| 3 | `core/src/sources/http_poll.rs` | Implement trait |
| 3 | `core/src/sources/merge.rs` | Update logic |
| 4 | `apps/air-quality-app/src/pipeline/*.rs` | Integration |

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | - | - | - |
| Reviewer | - | - | - |
| Tester | - | - | - |
| Architect | - | - | - |
