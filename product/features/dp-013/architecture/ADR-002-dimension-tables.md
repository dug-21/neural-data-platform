# ADR-002: Dimension Tables Bypass Bronze

## Status

Proposed

## Context

NDP needs to support **dimension tables** - reference data that enriches timeseries observations. The immediate use case is `entity_context` for air-012 (Home Assistant integration):

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
```

This data is fundamentally different from timeseries:

| Aspect | Timeseries (Streams) | Dimensions |
|--------|---------------------|------------|
| Nature | Observations over time | Reference/lookup data |
| Volatility | High (new data constantly) | Low (changes rarely) |
| Volume | Millions of rows | Hundreds of rows |
| Primary use | Queried by time range | Joined to facts |
| Audit trail | Critical (what was measured when) | Less critical |

### Design Question

Should dimensions flow through the same Bronze -> Silver pipeline as timeseries?

## Decision

**Dimensions bypass Bronze and load directly to Silver.**

### Rationale

1. **Different data semantics**: Dimensions are not "observations" - they're metadata that describes entities. Storing them in Bronze (raw JSON blobs with timestamps) adds complexity without value.

2. **No need for raw preservation**: Timeseries Bronze stores exact source payloads for reprocessing. Dimensions are authored data (we control the CSV) - no need to preserve "raw" form.

3. **Simpler load patterns**: Dimensions use `TRUNCATE + INSERT` or `UPSERT` patterns. These are native to relational databases, not suited for append-only Bronze Parquet.

4. **Managed alongside config**: Dimension CSVs live in `config/dimensions/`, version-controlled with stream configs. They're deployed via `deploy.sh sync`, not streamed.

### Data Flow

```
TIMESERIES (streams):
  MQTT/HTTP/CSV -> IngestionCoordinator -> Bronze (Parquet) -> ETL -> Silver

DIMENSIONS:
  CSV -> DimensionLoader -> Silver (direct)
```

### Configuration

New config type: `DimensionConfig`

```yaml
# config/base/dimensions/entity_context.yaml
dimension_id: entity-context
target:
  table: silver.entity_context
  primary_key: [ndp_id]
source:
  type: csv
  path: config/dimensions/entity_context.csv
schema:
  fields:
    - name: ndp_id
      data_type: text
      required: true
    - name: category
      data_type: text
      required: true
    - name: friendly_name
      data_type: text
    - name: location_path
      data_type: text
    - name: correlates_with
      data_type: text
    - name: orientation
      data_type: text
load:
  strategy: truncate_and_load
```

### Load Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `truncate_and_load` | DELETE all, INSERT new (in transaction) | Full refresh, small tables |
| `upsert` | INSERT or UPDATE based on primary_key | Incremental updates, large tables |

### CLI Integration

```bash
ndp dimension list              # List configured dimensions
ndp dimension sync <id>         # Sync specific dimension
ndp dimension sync --all        # Sync all dimensions
ndp dimension sync <id> --dry-run  # Validate without side effects
```

### Deploy Integration

```bash
./deploy.sh sync    # Syncs streams AND dimensions
```

## Consequences

### Positive

- **Simplicity**: Dimensions are just SQL tables loaded from CSV
- **Native patterns**: Uses standard database UPSERT/TRUNCATE
- **Fast joins**: Silver queries join dimensions directly
- **Config-driven**: YAML defines schema, load strategy, source
- **Version-controlled**: CSV data in git alongside configs

### Negative

- **No audit trail**: Unlike Bronze, we don't preserve historical dimension states
  - Mitigation: Git history provides audit trail
  - Future: Could add `valid_from/valid_to` for slowly changing dimensions
- **Different code path**: DimensionLoader is separate from stream pipeline
  - Mitigation: Clear separation - dimensions are metadata, not observations

### Neutral

- **Silver-only storage**: Dimensions exist only in Silver (TimescaleDB)
- **No streaming**: Dimensions loaded via CLI/deploy, not continuous

## Schema Management

### Table Auto-Creation

DimensionLoader creates tables from `schema.fields`:

```sql
-- Generated from dimension config
CREATE TABLE IF NOT EXISTS silver.entity_context (
    ndp_id TEXT NOT NULL,
    category TEXT NOT NULL,
    friendly_name TEXT,
    location_path TEXT,
    correlates_with TEXT,
    orientation TEXT,
    PRIMARY KEY (ndp_id)
);
```

### Schema Evolution

1. **Add nullable column**: Add to YAML + CSV, re-sync. ALTER TABLE auto-runs.
2. **Add required column**: Add with default value, or recreate table.
3. **Rename column**: Requires DROP + CREATE (truncate_and_load handles this).
4. **Change type**: Requires DROP + CREATE.

## Alternatives Considered

### 1. Dimensions Through Bronze

Store dimensions in Bronze Parquet, ETL to Silver like streams.

**Rejected because:**
- Overhead: Parquet optimized for large timeseries, not small reference tables
- Complexity: Would need "dimension ETL" separate from "observation ETL"
- No value: We don't need raw payload preservation for authored data

### 2. Dimensions in etcd

Store dimension data in etcd (config store).

**Rejected because:**
- etcd is for configuration, not data
- Size limits (etcd values should be small)
- Not queryable for analytics

### 3. Dimensions as YAML Instead of CSV

Embed dimension data directly in YAML config.

**Rejected because:**
- CSV is more natural for tabular data
- Easier to edit in spreadsheet tools
- Cleaner separation: YAML = schema, CSV = data

### 4. PostgreSQL COPY from CSV

Use native COPY command instead of custom loader.

**Considered but deferred:**
- COPY is fast but requires exact schema match
- DimensionLoader provides validation, schema evolution
- Could use COPY internally as optimization

## Future Enhancements

1. **Slowly Changing Dimensions (SCD)**: Add `valid_from/valid_to` for Type 2 dimensions
2. **API source**: Load dimensions from external APIs (not just CSV)
3. **Hot reload**: Watch dimension files for changes, auto-sync
4. **Diff reporting**: Show what changed during sync

## References

- [DP-002: TimescaleDB Schema](../../dp-002/architecture/ADR-001-TIMESCALEDB-SCHEMA.md) - Silver layer design
- [AIR-012: Home Assistant Integration](../../air-012/SCOPE.md) - entity_context use case
- [Kimball Dimension Modeling](https://www.kimballgroup.com/) - Dimension table patterns
