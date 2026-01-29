# dp-013: CSV Source Type & Dimension Tables

## Overview

Extend the NDP configuration language to support:
1. **CSV as a source type** for stream configs (timeseries batch data)
2. **Dimension table configs** for reference/lookup data (new config type)

This follows NDP's config-driven architecture - no new "loader" system, just extensions to existing patterns.

---

## Architecture Decision

**All timeseries data enters via Bronze, regardless of transport.**

| Source Type | Example | Target |
|-------------|---------|--------|
| `http` | AirGradient API polling | Bronze → Silver |
| `mqtt` | Home Assistant events (air-012) | Bronze → Silver |
| `csv` | Historical backfill, batch imports | Bronze → Silver |

CSV is simply another source type. The stream config defines the schema, Bronze stores the Parquet, ETL promotes to Silver. Same pipeline, different adapter.

**Dimensions are config, not streams.**

Dimension tables hold reference data that enriches observations. They:
- Don't flow through Bronze (they're metadata, not measurements)
- Load directly to Silver
- Are managed alongside stream configs in `config/base/`

---

## Part 1: CSV Source Type for Streams

Add `source.type: csv` to the existing stream config schema.

**Example Stream Config (`config/base/streams/historical-aq.yaml`):**
```yaml
stream_id: historical-aq
enabled: true
source:
  type: csv
  path: data/imports/historical_readings.csv
  timestamp_field: timestamp
  timestamp_format: iso8601  # or: epoch_seconds, custom
entity_schemas:
  - entity_type: air_quality
    fields:
      - name: pm25
        source_field: pm25
        data_type: float
      - name: temperature
        source_field: temperature
        data_type: float
# ... rest follows existing stream config pattern
```

**How it works:**
- Stream config defines schema (same as HTTP/MQTT streams)
- CSV adapter reads file, maps columns via `entity_schemas`
- Data lands in Bronze as Parquet (same format as other sources)
- Normal ETL promotes to Silver

**CSV-specific source properties:**
| Property | Required | Description |
|----------|----------|-------------|
| `path` | Yes | Path to CSV file (relative to config root or absolute) |
| `timestamp_field` | Yes | Column name containing timestamps |
| `timestamp_format` | No | `iso8601` (default), `epoch_seconds`, or strftime format |
| `delimiter` | No | Field delimiter (default: `,`) |
| `encoding` | No | File encoding (default: `utf-8`) |

---

## Part 2: Dimension Table Configs

New config type for reference data that enriches streaming observations.

**Example Dimension Config (`config/base/dimensions/entity_context.yaml`):**
```yaml
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
  strategy: truncate_and_load  # or: upsert
```

**Example CSV (`config/dimensions/entity_context.csv`):**
```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```

**Load strategies:**
| Strategy | Behavior |
|----------|----------|
| `truncate_and_load` | DELETE all, INSERT new (default for dimensions) |
| `upsert` | INSERT or UPDATE based on `primary_key` |

---

## Part 3: CLI & Sync Integration

**Sync dimensions on deploy:**
```bash
./deploy.sh sync  # Syncs streams AND dimensions
```

**Manual operations:**
```bash
ndp dimension list              # List configured dimensions
ndp dimension sync <id>         # Sync specific dimension
ndp dimension sync --all        # Sync all dimensions
ndp dimension sync <id> --dry-run  # Validate without loading
```

**For CSV stream sources (one-time imports):**
```bash
ndp stream ingest <stream_id>   # Trigger CSV ingest for stream
```

---

## Acceptance Criteria

### Part 1: CSV Source Type
- [ ] `source.type: csv` recognized in stream config validation
- [ ] CSV adapter implemented following existing source adapter pattern
- [ ] `timestamp_field` and `timestamp_format` parsing supported
- [ ] Column mapping uses existing `entity_schemas` pattern
- [ ] Data lands in Bronze Parquet (same format as HTTP/MQTT sources)
- [ ] Normal ETL promotes CSV-sourced data to Silver
- [ ] Invalid rows logged and skipped (configurable: `on_error: skip|abort`)
- [ ] `ndp stream ingest <stream_id>` triggers CSV ingest

### Part 2: Dimension Table Configs
- [ ] Dimension config schema defined (`dimension_id`, `target`, `source`, `schema`, `load`)
- [ ] Config files in `config/base/dimensions/*.yaml`
- [ ] CSV source type for dimensions (path, delimiter, encoding)
- [ ] Schema validation: required fields, data types
- [ ] `truncate_and_load` strategy: DELETE + INSERT in transaction
- [ ] `upsert` strategy: INSERT or UPDATE based on primary_key
- [ ] Silver table auto-created from dimension schema if not exists
- [ ] `deploy.sh sync` processes dimension configs

### Part 3: CLI
- [ ] `ndp dimension list` shows configured dimensions
- [ ] `ndp dimension sync <id>` loads specific dimension
- [ ] `ndp dimension sync --all` loads all dimensions
- [ ] `ndp dimension sync --dry-run` validates without side effects
- [ ] Summary output: rows processed, loaded, errors
- [ ] Exit code 0 on success, non-zero on failure

### Error Handling
- [ ] Malformed CSV: parse error with line number, operation aborts
- [ ] Missing required columns: validation error before load
- [ ] Type conversion failures: logged, row skipped (or abort if configured)
- [ ] File not found: clear error with path
- [ ] Empty file: warning, no-op

### Integration Tests
- [ ] Test: CSV stream source ingests to Bronze
- [ ] Test: Bronze CSV data promoted to Silver via normal ETL
- [ ] Test: Dimension truncate_and_load creates/replaces data
- [ ] Test: Dimension upsert updates existing, inserts new
- [ ] Test: `deploy.sh sync` processes dimensions
- [ ] Test: Dry-run validates without side effects
- [ ] Test: Malformed CSV aborts with clear error

---

## Initial Deliverable: Entity Context for air-012

**Dimension config (`config/base/dimensions/entity_context.yaml`):**
```yaml
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

**CSV data (`config/dimensions/entity_context.csv`):**
```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```

**Gold view (`gold.events_with_context`):**
```sql
CREATE VIEW gold.events_with_context AS
SELECT
    e.*,
    c.category,
    c.friendly_name,
    c.location_path,
    c.correlates_with
FROM silver.state_events e
LEFT JOIN silver.entity_context c USING (ndp_id);
```

---

## Out of Scope (Future)

- Watch directory trigger (file drop auto-ingest)
- Scheduled/cron triggers for recurring CSV imports
- Excel file support (.xlsx)
- Remote file sources (S3, HTTP)
- Non-CSV dimension sources (API, database)
