# Pseudocode: Migration Logic

## Overview

This document describes the migration strategy from the current parsed-metrics Bronze layer to the new raw-JSON Bronze layer.

> **Simplified Approach**: Platform is <1 week old. No backward compatibility required.
> Clean cutover to new schema. Existing data can be retired.

## Related ADR

- [ADR-001: Bronze Layer Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)

---

## Migration Strategy: Clean Cutover

Since the platform is less than one week old, we use a **clean cutover** approach:

```
1. Stop ingestion
2. Archive/delete old Bronze data
3. Deploy new schema implementation
4. Start ingestion with new schema
```

No dual-write. No schema detection. No backward compatibility layer.

---

## Simple Migration Flow

```pseudocode
FUNCTION migrate_to_raw_schema():
    // Step 1: Stop services
    ingestion_coordinator.stop()

    // Step 2: Archive old data (optional)
    IF config.archive_old_data:
        archive_path = format!("/data/archive/bronze-v1-{}", today())
        move("/data/bronze", archive_path)
    ELSE:
        delete("/data/bronze/*")
    END IF

    // Step 3: Deploy new code
    deploy_new_binary()
    sync_configuration()

    // Step 4: Start with new schema
    ingestion_coordinator.start()

    // Step 5: Verify
    verify_new_schema()
END FUNCTION
```

---

## Schema Comparison

### Old Schema (Retired)

```
timestamp | location_id | metric | value | tags | ndp_id | context
```

7 columns, tall format (one row per metric)

### New Schema (Active)

```
timestamp | source_id | ndp_id | context | raw_payload
```

5 columns, wide format (one row per message, raw JSON preserved)

---

## Storage Changes

### Directory Structure

**Old (retired)**:
```
/data/bronze/
  air-quality/
    2026-01-01_00.parquet
    2026-01-01_01.parquet
```

**New (active)**:
```
/data/bronze/
  year=2026/month=01/day=01/
    air-quality-Http.parquet
    outdoor-weather-Http.parquet
```

---

## Code Changes Required

### Remove from Codebase

Since no backward compatibility is needed, these components are NOT required:

- ~~DualWriteStore~~ - Not needed
- ~~SchemaVersion enum~~ - Not needed
- ~~detect_schema_version()~~ - Not needed
- ~~CompatibleReader~~ - Not needed
- ~~ValidationReport~~ - Not needed

### Add to Codebase

Simple, clean implementation:

```rust
// core/src/storage/parquet.rs

impl ParquetStore {
    /// Build schema for raw data storage (5 columns)
    fn build_raw_schema() -> Schema {
        Schema::new(vec![
            Field::new("timestamp", DataType::Timestamp(TimeUnit::Millisecond, None), false),
            Field::new("source_id", DataType::Utf8, false),
            Field::new("ndp_id", DataType::Utf8, true),
            Field::new("context", DataType::Utf8, true),
            Field::new("raw_payload", DataType::Utf8, false),
        ])
    }

    pub async fn write_raw(&self, point: RawDataPoint) -> Result<()> {
        self.write_raw_batch(vec![point]).await
    }

    pub async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> Result<()> {
        // Direct write to new schema, no dual-write complexity
        // ... implementation
    }
}
```

---

## Deployment Commands

```bash
# Pre-deployment
cargo build --release
cargo test --all

# Stop and clear
./deploy/pi/deploy.sh stop
rm -rf /data/bronze/*  # Or archive if desired

# Deploy
./deploy/pi/deploy.sh sync
./deploy/pi/deploy.sh start

# Verify
duckdb -c "SELECT * FROM parquet_schema('/data/bronze/*.parquet');"
```

---

## Verification

After deployment, verify the new schema is working:

```bash
# Check schema structure
duckdb -c "
SELECT * FROM parquet_schema('/data/bronze/*.parquet');
"
# Expected: timestamp, source_id, ndp_id, context, raw_payload

# Check data content
duckdb -c "
SELECT
    timestamp,
    source_id,
    raw_payload->>'$.pm02' as pm02,
    raw_payload->>'$.status' as status
FROM read_parquet('/data/bronze/*.parquet')
LIMIT 5;
"
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Deployment failure | Keep backup of old binary; rollback script ready |
| New schema bugs | Comprehensive test suite before deployment |
| Data loss | Platform is <1 week old; minimal impact |

---

## File Location

**Target**: No new migration module needed. Direct implementation in `core/src/storage/parquet.rs`.

## Related Files

| File | Change |
|------|--------|
| `core/src/storage/parquet.rs` | Add `write_raw()`, `write_raw_batch()`, `query_raw()` |
| `core/src/types/raw_data_point.rs` | New RawDataPoint struct |
| ~~`core/src/storage/migration.rs`~~ | Not needed (no backward compat) |
