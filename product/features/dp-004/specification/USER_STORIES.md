# DP-004: Bronze Raw JSON Schema - User Stories

## Overview

This document defines developer-focused user stories for implementing dp-004. Stories are organized by implementation phase and prioritized using MoSCoW (Must/Should/Could/Won't).

---

## Epic: Bronze Layer Raw JSON Storage

**As a** data platform developer
**I want to** store raw JSON payloads in the Bronze layer
**So that** I can preserve original source data for replay, debugging, and future schema evolution

---

## Phase 1: Core Data Structures

### Story 1.1: Create RawDataPoint Struct (Must Have)

**As a** Rust developer
**I want to** have a `RawDataPoint` struct in `core/src/traits.rs`
**So that** I can represent raw ingested data with platform metadata

**Tasks**:
- [ ] Define `RawDataPoint` struct with 5 fields
- [ ] Derive `Debug`, `Clone`, `Serialize`, `Deserialize`
- [ ] Add constructor methods for common use cases
- [ ] Write unit tests for serialization roundtrip
- [ ] Document struct fields with rustdoc

**Definition of Done**:
- Struct compiles and all derive macros work
- Unit tests pass for serialization/deserialization
- Struct is exported from `neural_core` crate

**Estimate**: 2 story points

---

### Story 1.2: Extend ParseContext with source_id (Must Have)

**As a** source developer
**I want to** include `source_id` in `ParseContext`
**So that** I can track which stream configuration produced each data point

**Tasks**:
- [ ] Add `source_id: String` field to `ParseContext`
- [ ] Update `ParseContext::new()` signature
- [ ] Update all call sites in source implementations
- [ ] Update unit tests

**Definition of Done**:
- ParseContext contains source_id field
- All existing sources compile with updated signature
- Tests pass

**Estimate**: 1 story point

---

## Phase 2: Storage Layer

### Story 2.1: Create RawStore Trait (Must Have)

**As a** storage developer
**I want to** define a `RawStore` trait for raw data storage
**So that** I can implement storage backends that handle `RawDataPoint`

**Tasks**:
- [ ] Define `RawStore` trait in `core/src/traits.rs`
- [ ] Add `async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>`
- [ ] Add `async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>`
- [ ] Document trait methods with rustdoc

**Definition of Done**:
- Trait is defined and exported
- Trait documentation is complete
- Trait can be implemented by external types

**Estimate**: 1 story point

---

### Story 2.2: Implement RawStore for ParquetStore (Must Have)

**As a** storage developer
**I want to** implement `RawStore` for `ParquetStore`
**So that** I can write `RawDataPoint` to Parquet files with the new 5-column schema

**Tasks**:
- [ ] Create `write_raw_parquet()` method with new schema
- [ ] Implement 5-column DataFrame construction:
  - timestamp (i64 microseconds)
  - source_id (String)
  - ndp_id (String nullable)
  - context (String nullable, JSON-serialized)
  - raw_payload (String, JSON-serialized)
- [ ] Update partition path to use `source_id`
- [ ] Implement `append_raw_to_parquet()` for existing files
- [ ] Add Snappy compression
- [ ] Write integration tests

**Definition of Done**:
- ParquetStore implements RawStore trait
- Parquet files have correct 5-column schema
- Files are partitioned by source_id/year/month/day
- Tests verify schema compliance

**Estimate**: 5 story points

---

### Story 2.3: Update WAL for RawDataPoint (Must Have)

**As a** storage developer
**I want to** serialize `RawDataPoint` to the Write-Ahead Log
**So that** raw data is recoverable after crashes

**Tasks**:
- [ ] Update WAL entry format for RawDataPoint
- [ ] Implement `replay_raw_wal()` method
- [ ] Handle mixed WAL entries (old TimeSeriesPoint + new RawDataPoint)
- [ ] Write recovery tests

**Definition of Done**:
- RawDataPoint survives WAL roundtrip
- WAL replay restores data to Parquet
- Mixed-format WAL is handled gracefully

**Estimate**: 3 story points

---

## Phase 3: Source Layer

### Story 3.1: Create RawSource Trait (Must Have)

**As a** source developer
**I want to** define a `RawSource` trait
**So that** sources can emit `RawDataPoint` instead of `TimeSeriesPoint`

**Tasks**:
- [ ] Define `RawSource` trait in `core/src/traits.rs`
- [ ] Add `async fn fetch_raw(&self) -> CoreResult<Vec<RawDataPoint>>`
- [ ] Add `async fn health_check(&self) -> CoreResult<HealthStatus>`
- [ ] Document trait methods

**Definition of Done**:
- Trait is defined and exported
- Trait can be implemented by source types

**Estimate**: 1 story point

---

### Story 3.2: Implement RawSource for HttpPollingSource (Must Have)

**As a** source developer
**I want to** update `HttpPollingSource` to implement `RawSource`
**So that** HTTP polling emits raw JSON payloads

**Tasks**:
- [ ] Add `ndp_id` and `source_id` fields to source config
- [ ] Implement `fetch_raw()` method:
  - Fetch HTTP response
  - Parse JSON (validation only)
  - Create RawDataPoint with:
    - timestamp = ingestion time
    - source_id from config
    - ndp_id from config
    - context from config
    - raw_payload = response JSON
- [ ] Update internal channel to carry `RawDataPoint`
- [ ] Write integration tests with mock server

**Definition of Done**:
- HttpPollingSource implements RawSource
- Raw payloads are preserved exactly
- Metadata is correctly attached
- Tests pass

**Estimate**: 5 story points

---

### Story 3.3: Implement RawSource for GenericHttpPollingSource (Must Have)

**As a** source developer
**I want to** update `GenericHttpPollingSource` to implement `RawSource`
**So that** generic HTTP endpoints emit raw JSON payloads

**Tasks**:
- [ ] Mirror implementation from Story 3.2
- [ ] Handle multi-endpoint configuration
- [ ] Ensure each endpoint's source_id is correctly tracked
- [ ] Write integration tests

**Definition of Done**:
- GenericHttpPollingSource implements RawSource
- Per-endpoint metadata is correct
- Tests pass

**Estimate**: 3 story points

---

### Story 3.4: Update MergeSource for RawDataPoint (Should Have)

**As a** source developer
**I want to** update `MergeSource` to merge `RawDataPoint` streams
**So that** multi-source ingestion works with the new schema

**Tasks**:
- [ ] Update merge logic to handle RawDataPoint
- [ ] Preserve source_id from child sources
- [ ] Write unit tests

**Definition of Done**:
- MergeSource works with RawSource children
- Source identity is preserved through merge

**Estimate**: 2 story points

---

## Phase 4: Parser Simplification

### Story 4.1: Create PassthroughParser (Must Have)

**As a** parser developer
**I want to** create a `PassthroughParser` that preserves raw JSON
**So that** sources can use it for zero-transformation ingestion

**Tasks**:
- [ ] Create `PassthroughParser` struct
- [ ] Implement `Parser` trait:
  - `parse()` returns empty vec (not used)
  - `parse_to_raw()` returns single RawDataPoint
- [ ] Add configuration for optional JSON validation
- [ ] Write unit tests

**Definition of Done**:
- PassthroughParser preserves JSON exactly
- Optional validation can be enabled
- Tests verify byte-for-byte preservation

**Estimate**: 2 story points

---

### Story 4.2: Deprecate Metric Extraction in Existing Parsers (Should Have)

**As a** parser developer
**I want to** mark metric extraction methods as deprecated
**So that** developers migrate to raw storage pattern

**Tasks**:
- [ ] Add `#[deprecated]` attributes to extraction methods
- [ ] Update documentation to recommend PassthroughParser
- [ ] Keep existing parsers functional for Silver ETL

**Definition of Done**:
- Deprecated warnings appear on old usage
- Documentation guides to new pattern

**Estimate**: 1 story point

---

## Phase 5: Pipeline Integration

### Story 5.1: Update IngestionCoordinator for RawDataPoint (Must Have)

**As a** pipeline developer
**I want to** update `IngestionCoordinator` to route `RawDataPoint`
**So that** the complete pipeline handles raw data

**Tasks**:
- [ ] Update channel type to `RawDataPoint`
- [ ] Update router to work with `RawDataPoint`
- [ ] Update storage writer to call `RawStore`
- [ ] Write end-to-end integration tests

**Definition of Done**:
- Pipeline flows RawDataPoint from source to storage
- E2E test ingests real-format data
- No regression in existing functionality

**Estimate**: 5 story points

---

### Story 5.2: Update Router for RawDataPoint (Must Have)

**As a** pipeline developer
**I want to** update the Router to enrich `RawDataPoint`
**So that** routing metadata is attached before storage

**Tasks**:
- [ ] Update Router to accept `RawDataPoint`
- [ ] Ensure source_id is populated from stream config
- [ ] Remove tag-based routing (source_id is now a field)
- [ ] Write unit tests

**Definition of Done**:
- Router correctly enriches RawDataPoint
- Routing decisions use source_id field
- Tests pass

**Estimate**: 2 story points

---

## Phase 6: Backward Compatibility

### Story 6.1: Schema Version Detection (Should Have)

**As a** storage developer
**I want to** detect Parquet file schema version
**So that** queries handle both old and new formats

**Tasks**:
- [ ] Add schema inspection on file read
- [ ] Detect 7-column vs 5-column schema
- [ ] Return appropriate data structure based on schema
- [ ] Write tests with both schema types

**Definition of Done**:
- Both schema versions are readable
- Correct struct is returned for each
- No data loss on read

**Estimate**: 3 story points

---

### Story 6.2: Dual-Write Mode (Could Have)

**As a** platform operator
**I want to** optionally write both old and new schema formats
**So that** I can validate new format before full migration

**Tasks**:
- [ ] Add configuration flag for dual-write
- [ ] Write TimeSeriesPoint alongside RawDataPoint
- [ ] Log size comparison between formats
- [ ] Add flag to disable after validation

**Definition of Done**:
- Dual-write produces both file types
- Configuration controls behavior
- Easy to disable after migration

**Estimate**: 2 story points

---

## Story Map

```
Phase 1: Core         Phase 2: Storage      Phase 3: Sources      Phase 4: Parsers     Phase 5: Pipeline
-------------------   ------------------    ------------------    -----------------    ------------------
[1.1 RawDataPoint]    [2.1 RawStore]        [3.1 RawSource]       [4.1 Passthrough]    [5.1 Coordinator]
         |                   |                     |                     |                    |
[1.2 ParseContext]    [2.2 Parquet Impl]    [3.2 HttpPolling]           |              [5.2 Router]
                             |                     |              [4.2 Deprecate]
                      [2.3 WAL Update]      [3.3 GenericHttp]
                                                   |
                                            [3.4 MergeSource]

Phase 6: Compat
-------------------
[6.1 Schema Detect]
         |
[6.2 Dual-Write]
```

---

## Priority Summary

| Priority | Stories | Total Points |
|----------|---------|--------------|
| Must Have | 1.1, 1.2, 2.1, 2.2, 2.3, 3.1, 3.2, 3.3, 4.1, 5.1, 5.2 | 30 |
| Should Have | 3.4, 4.2, 6.1 | 6 |
| Could Have | 6.2 | 2 |

**Total Estimated Effort**: 38 story points

---

## References

- [DP-004 Requirements](./REQUIREMENTS.md)
- [DP-004 Acceptance Criteria](./ACCEPTANCE_CRITERIA.md)
- [ADR-001: Bronze Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)
