# AIR-009: Implementation Phases

## Overview

This document defines the phased implementation plan for Source Identity and Context Configuration. Each phase builds on the previous, enabling incremental testing and validation.

**Amendment**: Updated to reflect **ADR-002-AMENDMENT-002** simple blob context storage approach.

**Total Estimated Effort:** 1-2 days (8-12 hours of focused development)

**Significant Reduction**: Simple JSON serialization vs complex flattening/promotion logic.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Change**: Maximum simplicity - NO flattening, NO promoted fields.

| Field | Storage |
|-------|---------|
| `ndp_id` | Dedicated column (fast indexed queries) |
| `context` | Single JSON blob (all queries via JSONB operators) |

**Implementation is trivial:**
```rust
let context_json = serde_json::to_string(&config.context)?;
record.insert("context", context_json);
```

---

## Phase 1: SourceConfig Struct Changes

**Goal:** Add `ndp_id` and `context` fields to the Rust type system.

**Duration:** 1-2 hours

### Tasks

1. **Modify `core/src/types/stream_config.rs`**
   - Add `ndp_id: Option<String>` field to `SourceConfig`
   - Add `context: Option<serde_json::Value>` field to `SourceConfig`
   - Update serde annotations for proper serialization

2. **Update Validation Logic**
   - Add `validate_ndp_id()` helper function (lowercase alphanumeric + hyphens, 3-64 chars)
   - Context validation is optional (any valid JSON)

3. **Update Tests**
   - Add unit tests for new fields in `stream_config.rs`
   - Test serialization/deserialization round-trip

### Success Criteria

- [ ] `SourceConfig` includes `ndp_id` and `context` fields
- [ ] Existing tests pass (backward compatibility)
- [ ] New tests for field validation pass
- [ ] `cargo clippy` and `cargo fmt --check` pass

### Dependencies

- None (foundational phase)

---

## Phase 2: Parser Integration (Simple Blob)

**Goal:** Modify parsers to attach `ndp_id` and context blob to parsed records.

**Duration:** 1-2 hours

### Tasks

1. **Modify `core/src/parsers/traits.rs`**
   - Add `ParseContext` struct to carry `ndp_id` and context string

   ```rust
   pub struct ParseContext {
       pub ndp_id: Option<String>,
       pub context: Option<String>,  // JSON blob
   }
   ```

2. **Update `core/src/parsers/flat_json.rs`**
   - Accept `ndp_id` and `context` in parser configuration
   - Simple string assignment:
     ```rust
     point.ndp_id = config.ndp_id.clone();
     point.context = config.context.as_ref()
         .map(|c| serde_json::to_string(c).unwrap());
     ```

3. **Update `core/src/parsers/json_path.rs`**
   - Same changes as FlatJsonParser

4. **Update `core/src/parsers/column_oriented.rs`**
   - Same changes for column-oriented data

5. **Update `core/src/parsers/array_iterator.rs`**
   - Same changes for array iteration

6. **Update Parser Factory**
   - Ensure `create_parser_from_config()` passes context through

7. **Update ConfigSyncService**
   - Ensure YAML context is converted to JSON Value

### Success Criteria

- [ ] All parsers inject `ndp_id` into output
- [ ] All parsers inject `context` as JSON string blob
- [ ] Existing parser tests still pass
- [ ] New tests verify context blob attachment

### Dependencies

- Phase 1 (SourceConfig changes)

---

## Phase 3: Bronze Layer Writer Updates (Simple Schema)

**Goal:** Ensure Parquet writer includes `ndp_id` and `context` columns.

**Duration:** 1-2 hours

### Tasks

1. **Update `core/src/storage/parquet.rs`**
   - Add `ndp_id` column to Parquet schema (STRING, nullable)
   - Add `context` column to Parquet schema (STRING for JSON blob, nullable)
   - Update `write_parquet()` to extract and write new columns
   - Update `append_to_parquet()` for schema consistency

2. **Schema Evolution Strategy**
   - Existing files: Leave as-is (no migration)
   - New files: Include new columns (all nullable)
   - Query compatibility: Handle missing columns gracefully

3. **Update Query Methods**
   - Add `ndp_id` filter support to `query()` method
   - Return `context` blob in query results

4. **Write Tests**
   - Write point with ndp_id and context
   - Query by `ndp_id`
   - Verify both columns in Parquet file

### Success Criteria

- [ ] Parquet files include `ndp_id` column
- [ ] Parquet files include `context` column (JSON blob)
- [ ] Query by `ndp_id` works
- [ ] Backward compatibility with existing files

### Dependencies

- Phase 2 (Parser integration - provides context in TimeSeriesPoint)

---

## Phase 4: Stream Config Migrations

**Goal:** Add `ndp_id` and `context` to all active stream configurations.

**Duration:** 1-2 hours

### Tasks

1. **Create NDP ID Registry**
   - Document assigned `ndp_id` values for each source
   - Format: `{device-type}-{location}-{sequence}`

2. **Update Stream Configs**

   | Stream | ndp_id | Context |
   |--------|--------|---------|
   | air-quality | `airgradient-office-001` | indoor, home/upstairs/office |
   | outdoor-weather | `owm-home-001` | outdoor, home |
   | outdoor-air-quality | `owm-air-home-001` | outdoor, home |
   | nws-observations | `nws-ksgj-001` | outdoor, ksgj |
   | nws-forecast-hourly | `nws-ksgj-forecast-001` | outdoor, ksgj |
   | nws-gridpoints-forecast | `nws-ksgj-grid-001` | outdoor, ksgj |

3. **Add Context to Each Stream**
   ```yaml
   sources:
     - type: mqtt
       ndp_id: airgradient-office-001
       context:
         location:
           coordinates: [29.95838, -81.30878]
           type: indoor
           path: home/upstairs/office
         device_type: airgradient
         model: ONE-V9
   ```

4. **Validate Configs**
   - Run `cargo test` with updated configs
   - Verify YAML parsing works

### Success Criteria

- [ ] All 6 streams have `ndp_id` assigned
- [ ] All 6 streams have appropriate `context`
- [ ] Config validation passes
- [ ] etcd sync works with new configs

### Dependencies

- Phase 2 (Parser handles new fields)

---

## Phase 5: Silver Layer Schema (Simple Blob)

**Goal:** Update TimescaleDB schema for simple blob context storage.

**Duration:** 1-2 hours

### Tasks

1. **Create Migration Script**
   ```sql
   -- AIR-009: Source Identity and Context Configuration

   -- Add ndp_id column
   ALTER TABLE sensor_readings
   ADD COLUMN IF NOT EXISTS ndp_id TEXT;

   -- Add context JSONB column
   ALTER TABLE sensor_readings
   ADD COLUMN IF NOT EXISTS context JSONB;

   -- Create indexes
   CREATE INDEX IF NOT EXISTS idx_sensor_readings_ndp_id
   ON sensor_readings(ndp_id);

   -- GIN index for JSONB context queries
   CREATE INDEX IF NOT EXISTS idx_sensor_readings_context
   ON sensor_readings USING GIN (context);
   ```

2. **Update ETL Process**
   - Bronze -> Silver transformation includes:
     - `ndp_id` (direct copy)
     - `context` JSONB (parse JSON string to JSONB)
   - Handle records without context (legacy data)

3. **Document Data Dictionary**
   - Add `ndp_id` definition
   - Add `context` JSONB schema (flexible keys)
   - Document query patterns using JSONB operators

4. **Write Verification Queries**
   ```sql
   -- Query by ndp_id (fast, indexed)
   SELECT * FROM sensor_readings
   WHERE ndp_id = 'airgradient-office-001'
   LIMIT 10;

   -- Query by context field via JSONB
   SELECT * FROM sensor_readings
   WHERE context->>'device_type' = 'airgradient'
   LIMIT 10;

   -- Query nested context field
   SELECT * FROM sensor_readings
   WHERE context->'location'->>'type' = 'indoor'
   LIMIT 10;
   ```

### Success Criteria

- [ ] TimescaleDB has `ndp_id` column with B-tree index
- [ ] TimescaleDB has `context` JSONB column with GIN index
- [ ] ETL includes both fields
- [ ] Query by `ndp_id` uses index
- [ ] Query by JSONB fields works

### Dependencies

- Phase 3 (Bronze layer has context data)
- Phase 4 (Stream configs provide context)

---

## Execution Order

```
Phase 1 (SourceConfig) ──── Phase 2 (Parsers) ──── Phase 3 (Bronze Writer)
                               │                         │
                               │                         v
                          Phase 4 (Configs) ──────── Phase 5 (Silver)
```

**Critical Path:** 1 -> 2 -> 3 -> 5

**Parallel Work:** Phase 4 can run in parallel with Phase 3

**Effort Comparison with Hybrid Approach:**
| Phase | Hybrid (Old) | Simple Blob (New) | Savings |
|-------|--------------|-------------------|---------|
| Phase 2 | 2-3 hours (ProcessedContext, promoted fields) | 1-2 hours (just string copy) | ~1 hour |
| Phase 3 | 2-3 hours (5+ columns) | 1-2 hours (2 columns) | ~1 hour |
| Phase 5 | 2-3 hours (promoted columns + JSONB) | 1-2 hours (just ndp_id + JSONB) | ~1 hour |
| Total | 12-18 hours | 8-12 hours | 4-6 hours |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Schema evolution breaks existing data | Additive-only changes; all new columns nullable |
| etcd sync fails with nested objects | Test YAML parsing before implementation |
| Parquet column mismatch | Handle missing columns as NULL in queries |
| JSONB query performance | GIN index; expression indexes for hot paths if needed |
| Need to add indexed columns later | Easy: add columns + indexes as needed |

---

## Rollback Strategy

1. **Config Rollback:** Remove `ndp_id` and `context` from YAML; redeploy
2. **Code Rollback:** Git revert to pre-AIR-009 commit
3. **Data:** New fields are additive; no data loss on rollback
4. **etcd:** Keys can be deleted; no impact on core functionality

---

## Implementation Simplicity

The entire context handling is now just:

```rust
// In SourceConfig
pub context: Option<serde_json::Value>,

// In Parser
if let Some(ref context) = config.context {
    point.context = Some(serde_json::to_string(context)?);
}

// In Parquet Writer
let contexts: Vec<Option<&str>> = points.iter()
    .map(|p| p.context.as_deref())
    .collect();

// In Silver ETL
INSERT INTO sensor_readings (ndp_id, context, ...)
VALUES ($1, $2::jsonb, ...)
```

No complex processing, no promoted fields, no multiple query patterns.

---

## References

- SCOPE.md: Feature requirements
- ADR-002-AMENDMENT-002: Simple blob decision
- Sample configs: `config/samples/mqtt_stream.yaml`
- ParquetStore: `core/src/storage/parquet.rs`
- ConfigSyncService: `apps/air-quality-app/src/config_sync/service.rs`
