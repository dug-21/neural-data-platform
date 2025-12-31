# AIR-009: Migration Plan

## Overview

This document describes the strategy for migrating existing stream configurations and data to the new `ndp_id` and `context` architecture. The migration is designed to be **non-breaking** and **incremental**.

**Amendment**: Updated to reflect **ADR-002-AMENDMENT-002** simple blob context storage approach.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Change**: Maximum simplicity - NO flattening, NO promoted fields.

| Field | Bronze (Parquet) | Silver (TimescaleDB) |
|-------|------------------|----------------------|
| `ndp_id` | STRING | TEXT + B-tree index |
| `context` | STRING (JSON) | JSONB + GIN index |

**Queries use JSONB operators in Silver layer:**
```sql
WHERE ndp_id = 'airgradient-office-001'
WHERE context->>'device_type' = 'airgradient'
WHERE context->'location'->>'type' = 'indoor'
```

---

## Migration Philosophy

### Core Principles

1. **Forward-Only:** New records get new fields; existing records remain unchanged
2. **Additive Changes:** All schema changes add columns; no removals or renames
3. **Optional Fields:** `ndp_id` and `context` are optional during transition
4. **Graceful Degradation:** Systems handle missing fields without errors

---

## Backward Compatibility Matrix

| Component | Without ndp_id/context | With ndp_id/context |
|-----------|------------------------|---------------------|
| YAML Config Parsing | Works (fields optional) | Works (full features) |
| etcd Sync | Works (no new keys) | Works (new keys added) |
| SourceManager | Works (ignores new fields) | Works (passes to parser) |
| Parsers | Works (no injection) | Works (context injected) |
| ParquetStore | Works (columns nullable) | Works (columns populated) |
| TimescaleDB | Works (columns nullable) | Works (columns populated) |
| Queries | Works (filter optional) | Works (filter by ndp_id/JSONB) |

---

## Phase 1: Code Preparation (Pre-Migration)

### 1.1 All New Fields Must Be Optional

```rust
// SourceConfig - fields are Option<T>
pub struct SourceConfig {
    pub ndp_id: Option<String>,  // NOT required
    pub context: Option<serde_json::Value>,  // NOT required
    // ... existing fields
}
```

### 1.2 Parsers Handle Missing Context

```rust
impl FlatJsonParser {
    pub fn parse_with_context(
        &self,
        payload: &[u8],
        parse_context: &ParseContext,
    ) -> Result<Vec<TimeSeriesPoint>, ParserError> {
        let mut points = self.parse(payload)?;

        for point in &mut points {
            // Only inject if ndp_id is present
            if let Some(ref ndp_id) = parse_context.ndp_id {
                point.ndp_id = Some(ndp_id.clone());
            }

            // Only inject context if present
            if let Some(ref context) = parse_context.context {
                point.context = Some(context.clone());  // JSON string blob
            }
        }

        Ok(points)
    }
}
```

### 1.3 Parquet Handles Missing Columns

```rust
// When reading existing Parquet files
fn read_with_optional_columns(path: &Path) -> CoreResult<DataFrame> {
    let file = std::fs::File::open(path)?;
    let df = ParquetReader::new(file).finish()?;

    // Add missing columns as nulls if not present
    let df = if !df.get_column_names().contains(&"ndp_id") {
        df.with_column(Series::new("ndp_id", vec![None::<String>; df.height()]))?
    } else {
        df
    };

    let df = if !df.get_column_names().contains(&"context") {
        df.with_column(Series::new("context", vec![None::<String>; df.height()]))?
    } else {
        df
    };

    Ok(df)
}
```

---

## Phase 2: Configuration Migration

### 2.1 Migration Script

Create a script to add `ndp_id` and `context` to existing configs:

```bash
#!/bin/bash
# scripts/migrate_stream_configs.sh

STREAMS_DIR="config/base/streams"

# Define ndp_id mappings
declare -A NDP_IDS=(
    ["air-quality"]="airgradient-office-001"
    ["outdoor-weather"]="owm-home-001"
    ["outdoor-air-quality"]="owm-air-home-001"
    ["nws-observations"]="nws-ksgj-001"
    ["nws-forecast-hourly"]="nws-ksgj-forecast-001"
    ["nws-gridpoints-forecast"]="nws-ksgj-grid-001"
    ["homeassistant"]="ha-mqtt-001"
)

for stream in "${!NDP_IDS[@]}"; do
    config_file="${STREAMS_DIR}/${stream}/config.yaml"
    if [[ -f "$config_file" ]]; then
        echo "Migrating: $stream -> ${NDP_IDS[$stream]}"
        # Use yq or similar tool to add ndp_id
        # yq -i '.sources[0].ndp_id = "'${NDP_IDS[$stream]}'"' "$config_file"
    fi
done
```

### 2.2 Manual Migration Template

For each stream, add the following after `enabled: true` in sources:

```yaml
sources:
  - type: mqtt  # or http_poll
    enabled: true
    # ADD THESE LINES:
    ndp_id: {assigned-id}
    context:
      location:
        coordinates: [LAT, LON]
        type: indoor | outdoor
        path: hierarchy/path
      device_type: {device-type}
      # Add domain-specific fields as needed
```

### 2.3 NDP ID Registry

| Stream | ndp_id | Location Type | Coordinates |
|--------|--------|---------------|-------------|
| air-quality | `airgradient-office-001` | indoor | [29.95838, -81.30878] |
| outdoor-weather | `owm-home-001` | outdoor | [29.95838, -81.30878] |
| outdoor-air-quality | `owm-air-home-001` | outdoor | [29.95838, -81.30878] |
| nws-observations | `nws-ksgj-001` | outdoor | Station KSGJ |
| nws-forecast-hourly | `nws-ksgj-forecast-001` | outdoor | Station KSGJ |
| nws-gridpoints-forecast | `nws-ksgj-grid-001` | outdoor | Station KSGJ |
| homeassistant | `ha-mqtt-001` | mixed | Home |

### 2.4 Validation During Transition

Before deploying updated configs:

```bash
# Validate YAML syntax
for f in config/base/streams/*/config.yaml; do
    echo "Validating: $f"
    yq eval '.' "$f" > /dev/null && echo "  OK" || echo "  FAILED"
done

# Run config sync in dry-run mode (if available)
cargo run --bin air-quality-app -- config-sync --dry-run
```

---

## Phase 3: Database Schema Migration

### 3.1 TimescaleDB Migration Script (Simple Blob)

```sql
-- Migration: 20241231_add_ndp_id_context.sql
-- AIR-009: Add ndp_id and context columns (Simple Blob Approach)

BEGIN;

-- Step 1: Add ndp_id column (nullable for existing data)
ALTER TABLE sensor_readings
ADD COLUMN IF NOT EXISTS ndp_id TEXT;

-- Step 2: Add context as JSONB (nullable for existing data)
ALTER TABLE sensor_readings
ADD COLUMN IF NOT EXISTS context JSONB;

-- Step 3: Create B-tree index for ndp_id (fast equality queries)
CREATE INDEX IF NOT EXISTS idx_readings_ndp_id
ON sensor_readings(ndp_id)
WHERE ndp_id IS NOT NULL;

-- Step 4: Create GIN index for JSONB context queries
CREATE INDEX IF NOT EXISTS idx_readings_context
ON sensor_readings USING GIN (context jsonb_path_ops)
WHERE context IS NOT NULL;

-- Step 5: Add comments for documentation
COMMENT ON COLUMN sensor_readings.ndp_id IS 'Stable NDP-assigned source identifier';
COMMENT ON COLUMN sensor_readings.context IS 'Full context as JSONB blob - query with JSONB operators';

COMMIT;
```

### 3.2 Verification Queries

```sql
-- Verify columns exist
SELECT column_name, data_type, is_nullable
FROM information_schema.columns
WHERE table_name = 'sensor_readings'
  AND column_name IN ('ndp_id', 'context');

-- Verify indexes exist
SELECT indexname, indexdef
FROM pg_indexes
WHERE tablename = 'sensor_readings'
  AND (indexname LIKE '%ndp%' OR indexname LIKE '%context%');

-- Count records with/without ndp_id (after some data flows)
SELECT
    CASE WHEN ndp_id IS NULL THEN 'legacy' ELSE 'new' END AS record_type,
    COUNT(*) AS count
FROM sensor_readings
GROUP BY 1;
```

### 3.3 Parquet Schema Evolution

Existing Parquet files are NOT migrated. New files include new columns:

| Column | Old Files | New Files |
|--------|-----------|-----------|
| timestamp | Present | Present |
| location_id | Present | Present |
| metric | Present | Present |
| value | Present | Present |
| ndp_id | Missing | Present (nullable) |
| context | Missing | Present (nullable) |

**Query Handling:**

```rust
// When querying across old and new files
fn merge_results(old_df: DataFrame, new_df: DataFrame) -> DataFrame {
    // Old files: add NULL columns for compatibility
    let old_df = old_df
        .with_column(Series::new("ndp_id", vec![None::<String>; old_df.height()]))
        .with_column(Series::new("context", vec![None::<String>; old_df.height()]));

    // Concatenate
    concat([old_df.lazy(), new_df.lazy()], true)?.collect()
}
```

---

## Phase 4: Rollback Strategy

### 4.1 Code Rollback

If issues arise with the new code:

```bash
# Option 1: Feature flag (if implemented)
export NDP_CONTEXT_ENABLED=false

# Option 2: Git revert
git revert <air-009-commit>
cargo build --release

# Option 3: Redeploy previous version
docker pull ndp/air-quality-app:v1.x.x
docker-compose up -d
```

### 4.2 Configuration Rollback

Remove `ndp_id` and `context` from YAML files:

```bash
# Using yq to remove fields
for f in config/base/streams/*/config.yaml; do
    yq -i 'del(.sources[].ndp_id)' "$f"
    yq -i 'del(.sources[].context)' "$f"
done

# Sync to etcd
./deploy/pi/deploy.sh sync
```

### 4.3 Database Rollback

```sql
-- CAUTION: Only if absolutely necessary
-- This loses any ndp_id/context data

BEGIN;

-- Drop indexes first
DROP INDEX IF EXISTS idx_readings_ndp_id;
DROP INDEX IF EXISTS idx_readings_context;

-- Drop columns
ALTER TABLE sensor_readings DROP COLUMN IF EXISTS ndp_id;
ALTER TABLE sensor_readings DROP COLUMN IF EXISTS context;

COMMIT;
```

### 4.4 etcd Rollback

```bash
# Delete new keys from etcd
etcdctl del /streams/ --prefix | grep -E "(ndp_id|context)"

# Or more specifically:
for stream in air-quality outdoor-weather outdoor-air-quality nws-observations nws-forecast-hourly nws-gridpoints-forecast; do
    etcdctl del "/streams/${stream}/sources/0/ndp_id"
    etcdctl del "/streams/${stream}/sources/0/context"
done
```

---

## Phase 5: Verification Checklist

### Pre-Migration

- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] Backup current configs: `cp -r config/base/streams config/base/streams.bak`
- [ ] Document current etcd state: `etcdctl get /streams/ --prefix --keys-only > etcd_keys_before.txt`

### During Migration

- [ ] Each config file validates after changes
- [ ] etcd sync completes without errors
- [ ] Application starts successfully
- [ ] No errors in logs related to parsing

### Post-Migration

- [ ] New records have `ndp_id` populated
- [ ] New records have `context` JSONB populated
- [ ] TimescaleDB columns exist with correct types
- [ ] Query by `ndp_id` returns results
- [ ] Query by JSONB context field works
- [ ] Legacy data still queryable (NULLs for new columns)
- [ ] No performance degradation

---

## Migration Timeline

| Day | Activity | Owner |
|-----|----------|-------|
| 1 | Code changes (Phase 1) | Dev |
| 1 | Unit tests | Dev |
| 2 | Config sync changes | Dev |
| 2 | Integration tests | Dev |
| 3 | Stream config updates | Dev |
| 3 | Deploy to staging | DevOps |
| 3 | Staging verification | QA |
| 4 | Database migration | DBA |
| 4 | Production deploy | DevOps |
| 4-5 | Monitor and verify | All |

---

## Communication Plan

### Before Migration

- [ ] Notify stakeholders of planned changes
- [ ] Document expected behavior changes
- [ ] Prepare rollback procedures

### During Migration

- [ ] Monitor logs for errors
- [ ] Track data flow through pipeline
- [ ] Verify new fields in storage

### After Migration

- [ ] Confirm success metrics
- [ ] Document any issues encountered
- [ ] Update runbooks with new query patterns

---

## Success Criteria

1. **Zero Data Loss:** All existing data remains accessible
2. **No Downtime:** Migration performed without service interruption
3. **New Records Complete:** All new records include `ndp_id` and `context`
4. **Query Works:**
   - `SELECT * FROM readings WHERE ndp_id = 'x'` returns expected results
   - `SELECT * FROM readings WHERE context->>'device_type' = 'y'` works
5. **Performance Maintained:** No significant latency increase (<5%)

---

## Query Patterns Reference

After migration, use these patterns to query by context:

```sql
-- By ndp_id (fast, indexed)
SELECT * FROM sensor_readings
WHERE ndp_id = 'airgradient-office-001';

-- By top-level context field
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- By nested context field
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- By multiple context conditions
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient'
  AND context->'location'->>'type' = 'indoor';

-- Check if context contains specific key
SELECT * FROM sensor_readings
WHERE context ? 'calibration';

-- JSONB containment (uses GIN index)
SELECT * FROM sensor_readings
WHERE context @> '{"device_type": "airgradient"}';
```
