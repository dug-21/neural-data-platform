# AIR-009: Integration Checklist

## Overview

This checklist verifies that Source Identity and Context Configuration is fully integrated and working across all layers of the NDP stack.

**Amendment**: Updated to reflect **ADR-002-AMENDMENT-002** simple blob context storage approach.

---

## Simple Blob Approach (ADR-002-AMENDMENT-002)

**Key Points:**
- `ndp_id`: Dedicated column for fast indexed queries
- `context`: Single JSON blob (no flattening, no promoted fields)

| Layer | Schema |
|-------|--------|
| Bronze (Parquet) | `ndp_id` STRING, `context` STRING (JSON) |
| Silver (TimescaleDB) | `ndp_id` TEXT + B-tree index, `context` JSONB + GIN index |

**All context queries use JSONB operators:**
```sql
WHERE context->>'device_type' = 'airgradient'
WHERE context->'location'->>'type' = 'indoor'
```

---

## Pre-Deployment Verification

### Code Quality

- [ ] **Rust Compilation**
  ```bash
  cargo build --release --workspace
  # Expected: Build succeeds with no errors
  ```

- [ ] **Formatting**
  ```bash
  cargo fmt --check --all
  # Expected: No formatting issues
  ```

- [ ] **Linting**
  ```bash
  cargo clippy --workspace -- -D warnings
  # Expected: No warnings or errors
  ```

- [ ] **Unit Tests**
  ```bash
  cargo test --workspace
  # Expected: All tests pass
  ```

### Specific Test Suites

- [ ] **SourceConfig Tests**
  ```bash
  cargo test --package neural-core stream_config
  # Expected: Tests for ndp_id and context fields pass
  ```

- [ ] **Context Serialization Tests**
  ```bash
  cargo test --package neural-core context
  # Expected: JSON blob serialization tests pass
  ```

- [ ] **Parser Integration Tests**
  ```bash
  cargo test --package neural-core parsers
  # Expected: Context injection tests pass
  ```

- [ ] **Parquet Storage Tests**
  ```bash
  cargo test --package neural-core storage
  # Expected: Tests for ndp_id and context columns pass
  ```

- [ ] **Config Sync Tests**
  ```bash
  cargo test --package air-quality-app config_sync
  # Expected: YAML parsing with ndp_id/context passes
  ```

---

## Configuration Verification

### YAML Config Validation

- [ ] **air-quality stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/air-quality/config.yaml
  # Expected: airgradient-office-001
  ```

- [ ] **outdoor-weather stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/outdoor-weather/config.yaml
  # Expected: owm-home-001
  ```

- [ ] **outdoor-air-quality stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/outdoor-air-quality/config.yaml
  # Expected: owm-air-home-001
  ```

- [ ] **nws-observations stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/nws-observations/config.yaml
  # Expected: nws-ksgj-001
  ```

- [ ] **nws-forecast-hourly stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/nws-forecast-hourly/config.yaml
  # Expected: nws-ksgj-forecast-001
  ```

- [ ] **nws-gridpoints-forecast stream**
  ```bash
  yq eval '.sources[0].ndp_id' config/base/streams/nws-gridpoints-forecast/config.yaml
  # Expected: nws-ksgj-grid-001
  ```

### Context Structure Validation

- [ ] **All configs have valid context structure**
  ```bash
  for f in config/base/streams/*/config.yaml; do
      echo "Checking: $f"
      yq eval '.sources[0].context' "$f" 2>/dev/null && echo "  OK" || echo "  MISSING"
  done
  ```

---

## etcd Sync Verification

### Sync Operation

- [ ] **Sync configs to etcd**
  ```bash
  ./deploy/pi/deploy.sh sync
  # Expected: Sync completes without errors
  ```

### Key Verification

- [ ] **ndp_id keys visible in etcd**
  ```bash
  etcdctl get /streams/ --prefix --keys-only | grep ndp_id
  # Expected: Keys for each stream's ndp_id
  # /streams/air-quality/sources/0/ndp_id
  # /streams/outdoor-weather/sources/0/ndp_id
  # etc.
  ```

- [ ] **context keys visible in etcd**
  ```bash
  etcdctl get /streams/ --prefix --keys-only | grep context
  # Expected: Single context key per source (blob, not flattened)
  # /streams/air-quality/sources/0/context
  # /streams/outdoor-weather/sources/0/context
  # etc.
  ```

- [ ] **Verify specific ndp_id value**
  ```bash
  etcdctl get /streams/air-quality/sources/0/ndp_id
  # Expected: airgradient-office-001
  ```

- [ ] **Verify context is stored as JSON blob**
  ```bash
  etcdctl get /streams/air-quality/sources/0/context
  # Expected: JSON blob like {"location":{"type":"indoor",...},"device_type":"airgradient",...}
  ```

---

## Ingestion Pipeline Verification

### Application Startup

- [ ] **Application starts successfully**
  ```bash
  ./deploy/pi/deploy.sh start
  # Expected: Services start without errors
  ```

- [ ] **No config parsing errors in logs**
  ```bash
  ./deploy/pi/deploy.sh logs | grep -i "error.*config\|parse.*fail"
  # Expected: No matching lines
  ```

### Data Flow

- [ ] **MQTT source receives data**
  ```bash
  ./deploy/pi/deploy.sh logs | grep -i "mqtt.*received\|mqtt.*message"
  # Expected: Messages being received
  ```

- [ ] **HTTP poll sources active**
  ```bash
  ./deploy/pi/deploy.sh logs | grep -i "http.*poll\|fetching"
  # Expected: Periodic poll activity
  ```

---

## Bronze Layer (Parquet) Verification

### File Structure

- [ ] **Parquet files created with correct structure**
  ```bash
  ls -la data/bronze/air-quality/year=*/month=*/day=*/
  # Expected: readings.parquet files exist
  ```

### Column Verification

- [ ] **ndp_id column present in Parquet files**
  ```bash
  # Using Python/Polars or similar:
  python3 -c "
  import polars as pl
  import glob
  files = glob.glob('data/bronze/air-quality/**/*.parquet', recursive=True)
  if files:
      df = pl.read_parquet(files[-1])  # Most recent file
      print('Columns:', df.columns)
      print('ndp_id present:', 'ndp_id' in df.columns)
      if 'ndp_id' in df.columns:
          print('Sample ndp_id:', df['ndp_id'].head(5).to_list())
  "
  # Expected: ndp_id column present and populated
  ```

- [ ] **context column present**
  ```bash
  python3 -c "
  import polars as pl
  import glob
  files = glob.glob('data/bronze/air-quality/**/*.parquet', recursive=True)
  if files:
      df = pl.read_parquet(files[-1])
      print('context present:', 'context' in df.columns)
      if 'context' in df.columns:
          sample = df['context'].head(1).to_list()[0]
          print('Sample context (truncated):', sample[:100] if sample else 'NULL')
  "
  # Expected: context column present, contains JSON blob
  ```

### Query by ndp_id

- [ ] **Can filter Parquet by ndp_id**
  ```bash
  python3 -c "
  import polars as pl
  df = pl.scan_parquet('data/bronze/air-quality/**/*.parquet')
  filtered = df.filter(pl.col('ndp_id') == 'airgradient-office-001').collect()
  print(f'Records with ndp_id: {len(filtered)}')
  "
  # Expected: Records returned for the ndp_id
  ```

---

## Silver Layer (TimescaleDB) Verification

### Schema Verification

- [ ] **ndp_id column exists**
  ```sql
  SELECT column_name, data_type, is_nullable
  FROM information_schema.columns
  WHERE table_name = 'sensor_readings'
    AND column_name = 'ndp_id';
  -- Expected: ndp_id | text | YES
  ```

- [ ] **context column exists as JSONB**
  ```sql
  SELECT column_name, data_type, is_nullable
  FROM information_schema.columns
  WHERE table_name = 'sensor_readings'
    AND column_name = 'context';
  -- Expected: context | jsonb | YES
  ```

- [ ] **ndp_id index created**
  ```sql
  SELECT indexname, indexdef
  FROM pg_indexes
  WHERE tablename = 'sensor_readings'
    AND indexname LIKE '%ndp%';
  -- Expected: idx_readings_ndp_id index present
  ```

- [ ] **context GIN index created**
  ```sql
  SELECT indexname, indexdef
  FROM pg_indexes
  WHERE tablename = 'sensor_readings'
    AND indexname LIKE '%context%';
  -- Expected: GIN index on context column
  ```

### Data Verification

- [ ] **Query by ndp_id returns results**
  ```sql
  SELECT ndp_id, COUNT(*) as record_count
  FROM sensor_readings
  WHERE ndp_id IS NOT NULL
  GROUP BY ndp_id;
  -- Expected: Rows for each active ndp_id
  ```

- [ ] **Context JSONB queryable with top-level field**
  ```sql
  SELECT ndp_id, context->>'device_type' as device_type
  FROM sensor_readings
  WHERE context IS NOT NULL
  LIMIT 10;
  -- Expected: device_type accessible via JSONB operator
  ```

- [ ] **Context JSONB queryable with nested field**
  ```sql
  SELECT ndp_id, context->'location'->>'type' as location_type
  FROM sensor_readings
  WHERE context IS NOT NULL
  LIMIT 10;
  -- Expected: Nested fields accessible via JSONB operators
  ```

- [ ] **Sample complete record**
  ```sql
  SELECT
      time,
      ndp_id,
      location_id,
      context,
      metric,
      value
  FROM sensor_readings
  WHERE ndp_id = 'airgradient-office-001'
  ORDER BY time DESC
  LIMIT 5;
  -- Expected: Complete records with all new fields, context as JSONB
  ```

---

## End-to-End Verification

### Full Pipeline Test

- [ ] **Ingest -> Store -> Query cycle works**
  1. Trigger data ingestion (wait for MQTT/HTTP cycle)
  2. Verify Parquet file updated
  3. Verify TimescaleDB has new records
  4. Query by ndp_id returns recent data

### Cross-Layer Consistency

- [ ] **ndp_id matches across layers**
  ```bash
  # Compare Parquet and TimescaleDB
  # Parquet:
  python3 -c "
  import polars as pl
  df = pl.scan_parquet('data/bronze/air-quality/**/*.parquet')
  ndp_ids = df.select('ndp_id').unique().collect()
  print('Parquet ndp_ids:', ndp_ids['ndp_id'].to_list())
  "

  # TimescaleDB:
  psql -c "SELECT DISTINCT ndp_id FROM sensor_readings WHERE ndp_id IS NOT NULL;"
  # Expected: Same ndp_id values in both layers
  ```

---

## Query Pattern Verification

### ndp_id Queries (Fast Path)

- [ ] **Equality query by ndp_id**
  ```sql
  EXPLAIN ANALYZE
  SELECT * FROM sensor_readings
  WHERE ndp_id = 'airgradient-office-001'
  AND time > NOW() - INTERVAL '1 hour';
  -- Expected: Index scan used, fast execution
  ```

### Context JSONB Queries

- [ ] **Query by top-level context field**
  ```sql
  SELECT * FROM sensor_readings
  WHERE context->>'device_type' = 'airgradient'
  ORDER BY time DESC
  LIMIT 10;
  -- Expected: Results filtered by device_type
  ```

- [ ] **Query by nested context field**
  ```sql
  SELECT * FROM sensor_readings
  WHERE context->'location'->>'type' = 'indoor'
  ORDER BY time DESC
  LIMIT 10;
  -- Expected: Results filtered by location.type
  ```

- [ ] **Query using JSONB containment (uses GIN index)**
  ```sql
  EXPLAIN ANALYZE
  SELECT * FROM sensor_readings
  WHERE context @> '{"device_type": "airgradient"}'
  AND time > NOW() - INTERVAL '1 hour';
  -- Expected: GIN index scan
  ```

---

## Performance Verification

### No Latency Regression

- [ ] **Ingestion latency within bounds**
  ```bash
  # Check logs for processing time
  ./deploy/pi/deploy.sh logs | grep -i "processed.*ms\|latency"
  # Expected: Similar latency to pre-migration baseline
  ```

- [ ] **Query performance acceptable**
  ```sql
  EXPLAIN ANALYZE
  SELECT * FROM sensor_readings
  WHERE ndp_id = 'airgradient-office-001'
    AND time > NOW() - INTERVAL '1 hour';
  -- Expected: Index scan used, execution time < 100ms
  ```

### Storage Overhead

- [ ] **Parquet file size reasonable**
  ```bash
  du -sh data/bronze/air-quality/year=*/month=*/day=*/
  # Expected: Modest increase (~10-20%) from new columns
  ```

---

## Rollback Readiness

### Rollback Artifacts Available

- [ ] **Config backup exists**
  ```bash
  ls -la config/base/streams.bak/
  # Expected: Backup directory with original configs
  ```

- [ ] **Previous Docker image tagged**
  ```bash
  docker images | grep air-quality-app
  # Expected: Previous version image available
  ```

- [ ] **Rollback SQL script ready**
  ```bash
  cat docs/migrations/rollback_20241231_ndp_id.sql
  # Expected: Script to drop new columns if needed
  ```

---

## Documentation Verification

- [ ] **SCOPE.md updated with completion status**
- [ ] **STATUS.md reflects current state**
- [ ] **ADR-002-AMENDMENT-002 documented (simple blob decision)**
- [ ] **Sample configs in config/samples/ are valid**
- [ ] **Data dictionary updated with new fields**

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Developer | | | |
| Reviewer | | | |
| QA | | | |
| DevOps | | | |

---

## Notes

### Known Limitations

1. Existing records will NOT have ndp_id populated (NULL)
2. Existing records will NOT have context populated (NULL)
3. Backfill is out of scope for this feature
4. Home Assistant stream may have multiple sources; each needs unique ndp_id

### Follow-Up Items

1. [ ] Create dashboard showing ndp_id distribution
2. [ ] Add monitoring alert for NULL ndp_id in new records
3. [ ] Document JSONB query patterns for context fields
4. [ ] Consider expression indexes for hot context paths if needed
5. [ ] Consider continuous aggregate for ndp_id summaries

---

## Verification Commands Summary

```bash
# Quick verification script
echo "=== Code Quality ==="
cargo build --release --workspace && echo "Build: OK"
cargo test --workspace && echo "Tests: OK"

echo "=== Config Validation ==="
for f in config/base/streams/*/config.yaml; do
    ndp_id=$(yq eval '.sources[0].ndp_id' "$f" 2>/dev/null)
    if [ -n "$ndp_id" ]; then
        echo "  $f: ndp_id=$ndp_id"
    else
        echo "  $f: MISSING ndp_id"
    fi
done

echo "=== etcd Keys ==="
etcdctl get /streams/ --prefix --keys-only | grep -c ndp_id
echo "ndp_id keys found"

echo "=== Parquet Columns ==="
python3 -c "
import polars as pl
import glob
files = glob.glob('data/bronze/**/readings.parquet', recursive=True)
if files:
    df = pl.read_parquet(files[0])
    print('Columns:', df.columns)
    print('ndp_id present:', 'ndp_id' in df.columns)
    print('context present:', 'context' in df.columns)
else:
    print('No Parquet files found')
"

echo "=== TimescaleDB ==="
psql -c "SELECT column_name FROM information_schema.columns WHERE table_name='sensor_readings' AND column_name IN ('ndp_id', 'context');"

echo "=== Query Test ==="
psql -c "SELECT ndp_id, context->>'device_type' FROM sensor_readings WHERE ndp_id IS NOT NULL LIMIT 3;"

echo "=== Complete ==="
```
