# Swarm Kickoff Report: DP-004 Bronze Raw JSON Schema

**Date**: 2026-01-01
**Author**: ndp-scrum-master
**Feature**: dp-004

---

## Executive Summary

DP-004 is ready for implementation. The ADR has been approved, defining a new Bronze layer schema that stores raw JSON payloads instead of parsed metrics. This feature enables data recovery, replay capability, and support for non-numeric data types.

---

## Project State Assessment

### Active Features Overview

| Feature | Phase | Status | Priority |
|---------|-------|--------|----------|
| **dp-004** | Refinement | Ready for implementation | HIGH |
| **air-009** | Completion | 529 tests passing, ready for deployment | MEDIUM |
| **dp-003** | Completion | Implementation complete, integration testing | MEDIUM |
| **dp-002** | Completion | Ready for Pi deployment | LOW |
| **dp-001** | Completion | Ready for deployment verification | LOW |

### Feature Dependencies

```
dp-004 (Bronze Raw JSON)
   |
   +-- depends on: AIR-009 (ndp_id/context) [COMPLETE]
   |
   +-- enables: dp-005 (Silver ETL) [FUTURE]
```

---

## DP-004 Implementation Plan

### Phase 1: Core Types (Priority: P0)

**Goal**: Add `RawDataPoint` struct for raw JSON storage

**Files to Modify**:
- `/workspaces/neural-data-platform/core/src/traits.rs`

**New Type** (from ADR-001):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDataPoint {
    pub timestamp: DateTime<Utc>,
    pub source_id: String,
    pub ndp_id: Option<String>,
    pub context: Option<Value>,
    pub raw_payload: Value,
}
```

**Assigned Agent**: `ndp-rust-dev`
**Estimated Effort**: 2-3 hours

---

### Phase 2: Storage Layer (Priority: P0)

**Goal**: Update Parquet storage for new 5-column schema

**Files to Modify**:
- `/workspaces/neural-data-platform/core/src/storage/parquet.rs`

**Changes**:
1. Add `RawStore` trait (parallel to `Store` trait)
2. Add `write_raw()` and `write_raw_batch()` methods
3. Create new Parquet schema: `timestamp | source_id | ndp_id | context | raw_payload`
4. Implement schema version detection for reading old vs new files

**Assigned Agent**: `ndp-parquet-dev`
**Estimated Effort**: 4-6 hours

---

### Phase 3: Source Updates (Priority: P1)

**Goal**: Update sources to optionally emit `RawDataPoint`

**Files to Modify**:
- `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs`
- `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`

**Changes**:
1. Add `RawSource` trait (parallel to `Source` trait)
2. MQTT source: emit raw MQTT payload as JSON
3. HTTP source: emit raw HTTP response body as JSON

**Assigned Agent**: `ndp-rust-dev`
**Estimated Effort**: 3-4 hours

---

### Phase 4: Testing (Priority: P0-P1)

**Goal**: Comprehensive test coverage

**Test Cases**:
1. `RawDataPoint` serde roundtrip (P0)
2. Parquet write/read with new schema (P0)
3. Schema version detection (P1)
4. Backward compatibility with existing files (P1)
5. Integration test: MQTT -> RawDataPoint -> Parquet (P1)

**Assigned Agent**: `ndp-tester`
**Estimated Effort**: 3-4 hours

---

## Recommended Implementation Order

```
Day 1:
  [1] ndp-rust-dev: Add RawDataPoint struct + tests
  [2] ndp-tester: Unit tests for RawDataPoint serde

Day 2:
  [3] ndp-parquet-dev: Update ParquetStore for new schema
  [4] ndp-tester: Parquet roundtrip tests

Day 3:
  [5] ndp-rust-dev: Update sources (MQTT, HTTP)
  [6] ndp-parquet-dev: Schema version detection
  [7] ndp-tester: Integration tests
```

---

## Agent Assignments

| Agent | Tasks | Start Condition |
|-------|-------|-----------------|
| `ndp-rust-dev` | RawDataPoint struct, Source updates | Immediate |
| `ndp-parquet-dev` | ParquetStore update, schema detection | After RawDataPoint |
| `ndp-tester` | All test phases | Parallel with implementation |
| `ndp-architect` | Review, ADR clarifications if needed | On-demand |

---

## Risks and Concerns

### Risk 1: Schema Migration Complexity
**Concern**: Existing Parquet files use 6-column tall schema; new files will use 5-column wide schema.
**Mitigation**:
- Schema version detection in reader
- Dual-write period during transition
- Old files remain readable

### Risk 2: Increased Storage Size
**Concern**: JSON blobs may increase storage requirements.
**Mitigation**:
- ADR analysis shows raw JSON is actually MORE compact (400 bytes vs 750 bytes per reading)
- Parquet compression reduces JSON column overhead

### Risk 3: Silver Layer Dependency
**Concern**: Bronze now stores raw data; Silver ETL needed for analytics.
**Mitigation**:
- Silver layer already exists (TimescaleDB from dp-002)
- ETL pipeline scoped for dp-005
- DuckDB can query raw JSON directly for interim period

---

## Success Criteria

1. `RawDataPoint` struct implemented with full serde support
2. New Parquet files use 5-column schema
3. Sources can emit `RawDataPoint` with raw payloads
4. All existing tests continue to pass (529+ tests)
5. New tests for raw data path passing
6. Schema version detection works for old files

---

## Next Actions

1. **Immediate**: Create `feature/dp-004` branch
2. **Phase 1**: Begin `RawDataPoint` implementation
3. **Parallel**: Start test planning with `ndp-tester`
4. **Review**: Daily STATUS.md updates

---

## Related Documents

- `/workspaces/neural-data-platform/product/features/dp-004/SCOPE.md`
- `/workspaces/neural-data-platform/product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md`
- `/workspaces/neural-data-platform/core/src/traits.rs` (current TimeSeriesPoint)
- `/workspaces/neural-data-platform/core/src/storage/parquet.rs` (current storage)
