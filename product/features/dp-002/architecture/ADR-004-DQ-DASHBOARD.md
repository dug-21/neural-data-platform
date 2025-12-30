# ADR-004: Data Quality Dashboard Architecture

**Status**: Proposed
**Date**: 2025-12-30
**Decision Makers**: NDP Architecture Team
**Context**: DP-002 Online Data Dictionary and HomeAssistant Stream Preparation
**Supersedes**: None

---

## Context

DP-002 requires a **Data Quality Dashboard** in Grafana that displays:

1. **Stream Overview**: What streams exist, their status, field counts
2. **Field Details**: Schema documentation for each stream's fields
3. **Entity Schemas**: Pattern-based entity configurations for HomeAssistant
4. **Data Freshness**: Last data received per stream
5. **Volume Metrics**: Record counts, storage usage

### Critical Requirement

> **No JSON edits for schema changes**: When a new stream is added or a field is modified, the dashboard should automatically update without manual JSON editing of dashboard definitions.

This means the dashboard must be **schema-driven** - it queries the Data Dictionary for metadata and dynamically adjusts panel content.

### Data Sources

1. **TimescaleDB**: Data Dictionary tables (streams, fields, entity_schemas)
2. **DuckDB**: Analytics queries on Bronze layer Parquet files
3. **Parquet/File System**: For storage metrics (optional)

---

## Decision

**Create a schema-driven dashboard using Grafana's PostgreSQL data source with dynamic queries against the Data Dictionary.**

### Dashboard Architecture

```
+-------------------+     +-------------------+     +-------------------+
|   Grafana         |     |   TimescaleDB     |     |   DuckDB          |
|   Dashboard       |     |   (Data Dict)     |     |   (Analytics)     |
|                   |     |                   |     |                   |
|  +-------------+  |     |  streams          |     |  Parquet files    |
|  | Stream List |<-|-----|  fields           |     |  (via DuckDB)     |
|  +-------------+  |     |  entity_schemas   |     |                   |
|                   |     |                   |     |                   |
|  +-------------+  |     |                   |     |                   |
|  | Field Table |<-|-----|                   |     |                   |
|  +-------------+  |     |                   |     |                   |
|                   |     |                   |     |                   |
|  +-------------+  |     |                   |     |                   |
|  | Freshness   |<-|-----|------------------|-----|  Last timestamp   |
|  +-------------+  |     |                   |     |  per stream       |
+-------------------+     +-------------------+     +-------------------+
```

### Data Source Configuration

#### TimescaleDB Data Source

```yaml
# Grafana data source configuration
name: NDP-TimescaleDB
type: postgres
url: pi5-timescaledb:5432
database: ndp
user: grafana_reader
secureJsonData:
  password: ${GRAFANA_TIMESCALE_PASSWORD}
jsonData:
  sslmode: disable
  maxOpenConns: 10
  connMaxLifetime: 14400
  postgresVersion: 1500  # PostgreSQL 15
  timescaledb: true
```

#### DuckDB Data Source (for Analytics)

```yaml
# Using Grafana DuckDB plugin
name: NDP-DuckDB
type: duckdb
jsonData:
  path: /var/lib/grafana/duckdb/analytics.duckdb
```

### Dashboard Panels

#### 1. Stream Overview (Table Panel)

**Query** (TimescaleDB):
```sql
SELECT
    stream_id,
    description,
    version,
    CASE WHEN enabled THEN 'Active' ELSE 'Disabled' END AS status,
    retention_days || ' days' AS retention,
    field_count,
    source_count,
    entity_schema_count,
    TO_CHAR(updated_at, 'YYYY-MM-DD HH24:MI') AS last_updated
FROM data_dictionary.stream_overview
ORDER BY stream_id;
```

**Panel Configuration**:
- Type: Table
- Auto-refresh: 1 minute
- Column styles: Color-code status (green=Active, gray=Disabled)

#### 2. Field Details (Table Panel with Variable)

**Variable Definition**:
```yaml
name: stream_id
type: query
query: "SELECT stream_id FROM data_dictionary.streams WHERE enabled = true ORDER BY stream_id"
datasource: NDP-TimescaleDB
```

**Query**:
```sql
SELECT
    field_name AS "Field",
    field_type AS "Type",
    CASE WHEN nullable THEN 'Yes' ELSE 'No' END AS "Nullable",
    COALESCE(unit, '-') AS "Unit",
    description AS "Description",
    CASE
        WHEN validation_min IS NOT NULL AND validation_max IS NOT NULL
        THEN validation_min::text || ' - ' || validation_max::text
        ELSE '-'
    END AS "Valid Range"
FROM data_dictionary.fields
WHERE stream_id = '$stream_id'
ORDER BY sort_order, field_name;
```

**Panel Configuration**:
- Type: Table
- Repeats for each stream (using variable)
- No manual update needed when fields change

#### 3. Entity Schemas (Table Panel)

**Query**:
```sql
SELECT
    entity_pattern AS "Pattern",
    entity_domain AS "Domain",
    COALESCE(device_class, '-') AS "Device Class",
    COALESCE(protocol, '-') AS "Protocol",
    CASE WHEN enabled THEN 'Active' ELSE 'Disabled' END AS "Status",
    priority AS "Priority",
    description AS "Description"
FROM data_dictionary.entity_schema_details
WHERE stream_id = '$stream_id'
ORDER BY priority DESC, entity_pattern;
```

#### 4. Data Freshness (Stat Panel)

**Query** (DuckDB - queries Bronze Parquet):
```sql
-- Get most recent timestamp per stream
SELECT
    stream_id,
    MAX(timestamp) / 1000 AS last_data_epoch,
    NOW() - TO_TIMESTAMP(MAX(timestamp) / 1000) AS age
FROM read_parquet('data/bronze/*/2025/*/*/*.parquet', filename=true)
GROUP BY 1
ORDER BY stream_id;
```

**Alternative** (TimescaleDB - if Silver layer populated):
```sql
SELECT
    'air-quality' AS stream_id,
    MAX(time) AS last_data,
    NOW() - MAX(time) AS age
FROM air_quality
UNION ALL
SELECT
    'home-events' AS stream_id,
    MAX(time) AS last_data,
    NOW() - MAX(time) AS age
FROM home_events;
```

**Panel Configuration**:
- Type: Stat
- Thresholds: Green (<5 min), Yellow (<15 min), Red (>15 min)
- Auto-refresh: 30 seconds

#### 5. Record Volume (Time Series)

**Query** (DuckDB):
```sql
SELECT
    time_bucket('1 hour', to_timestamp(timestamp/1000)) AS time,
    stream_id,
    COUNT(*) AS records
FROM read_parquet('data/bronze/*/*/*/*.parquet', filename=true)
WHERE timestamp >= (EXTRACT(EPOCH FROM NOW() - INTERVAL '24 hours') * 1000)
GROUP BY 1, 2
ORDER BY 1, 2;
```

### Schema-Driven Dynamic Updates

The key to "no JSON edits" is using **Grafana variables** and **query-based panels**:

1. **Stream List Variable**: Populated from `data_dictionary.streams`
2. **Field Table**: Queries `data_dictionary.fields` filtered by variable
3. **Entity Schema Table**: Queries `data_dictionary.entity_schemas` filtered by variable

When a new stream is added:
1. Run `deploy.sh sync` to update Data Dictionary
2. Grafana variable query returns new stream
3. Tables automatically show new stream's data

**No dashboard JSON changes required.**

### Dashboard JSON Structure

```json
{
  "dashboard": {
    "title": "NDP Data Quality Dashboard",
    "uid": "ndp-data-quality",
    "tags": ["ndp", "data-quality", "dp-002"],
    "timezone": "browser",
    "refresh": "1m",

    "templating": {
      "list": [
        {
          "name": "stream_id",
          "type": "query",
          "query": "SELECT stream_id FROM data_dictionary.streams WHERE enabled = true ORDER BY stream_id",
          "datasource": "NDP-TimescaleDB",
          "refresh": 2,
          "multi": false,
          "includeAll": true,
          "allValue": "*"
        }
      ]
    },

    "panels": [
      {
        "title": "Stream Overview",
        "type": "table",
        "gridPos": { "x": 0, "y": 0, "w": 24, "h": 8 },
        "datasource": "NDP-TimescaleDB",
        "targets": [
          {
            "rawSql": "SELECT ... FROM data_dictionary.stream_overview ...",
            "format": "table"
          }
        ]
      },
      {
        "title": "Fields for $stream_id",
        "type": "table",
        "gridPos": { "x": 0, "y": 8, "w": 12, "h": 10 },
        "datasource": "NDP-TimescaleDB",
        "targets": [
          {
            "rawSql": "SELECT ... FROM data_dictionary.fields WHERE stream_id = '$stream_id' ...",
            "format": "table"
          }
        ],
        "repeat": null,
        "repeatDirection": "h"
      },
      {
        "title": "Entity Schemas for $stream_id",
        "type": "table",
        "gridPos": { "x": 12, "y": 8, "w": 12, "h": 10 },
        "datasource": "NDP-TimescaleDB",
        "targets": [
          {
            "rawSql": "SELECT ... FROM data_dictionary.entity_schema_details WHERE stream_id = '$stream_id' ...",
            "format": "table"
          }
        ]
      }
    ]
  }
}
```

---

## Rationale

### Why PostgreSQL/TimescaleDB Over DuckDB for Data Dictionary

| Criterion | TimescaleDB | DuckDB |
|-----------|-------------|--------|
| **Grafana Support** | Native data source | Requires plugin |
| **Real-time Updates** | Immediate | Requires file refresh |
| **Multi-user Concurrency** | Full support | Limited |
| **SQL Compatibility** | Standard PostgreSQL | Some differences |

**Decision**: TimescaleDB for Data Dictionary, DuckDB for heavy analytics.

### Why Variables Over Panel Repeats

**Variables**: User selects stream from dropdown, panels update
**Panel Repeats**: Dashboard creates N copies of panel

**Decision**: Variables are more flexible and reduce dashboard size.

### Why Not Auto-Generated Dashboard

Could generate dashboard JSON from Data Dictionary at deploy time.

**Pros**: Truly zero maintenance
**Cons**: Complex generation logic, harder to customize styling

**Decision**: Schema-driven queries are simpler and achieve the same goal.

---

## Consequences

### Positive

1. **Zero JSON Edits**: Schema changes reflected automatically
2. **Single Source of Truth**: Data Dictionary drives all displays
3. **Consistent Display**: All streams show same panel structure
4. **Easy Extension**: Add new panels without modifying existing

### Negative

1. **Query Complexity**: Some queries join multiple tables
2. **Plugin Dependency**: DuckDB plugin needed for analytics
3. **Variable Limits**: Very large stream counts may be unwieldy

### Risks

1. **Query Performance**: Complex queries on large data
   - **Mitigation**: Indexes on Data Dictionary tables (ADR-001)
2. **Plugin Availability**: DuckDB Grafana plugin may have issues
   - **Mitigation**: Fall back to TimescaleDB for all queries if needed

---

## Alternatives Considered

### Alternative 1: Static Dashboard with Manual Updates

Create fixed panels for each known stream.

**Rejected because**:
- Violates "no JSON edits" requirement
- Doesn't scale with stream growth
- Error-prone maintenance

### Alternative 2: Grafana Provisioning from YAML

Generate dashboard YAML from stream configs during deployment.

```bash
# deploy.sh
generate_dashboard_yaml() {
    for stream in streams/*; do
        create_panel_yaml "$stream"
    done
}
```

**Considered but rejected because**:
- Adds deploy complexity
- Still requires Grafana restart/reload
- Query-based approach is simpler

### Alternative 3: External Dashboard Tool

Use Superset, Metabase, or custom web app.

**Rejected because**:
- Additional tool to maintain
- Grafana already deployed
- Would need separate authentication

---

## Implementation Impact

### Files to Create

- `deploy/pi/grafana/dashboards/data-quality.json` - Dashboard definition
- `deploy/pi/grafana/provisioning/datasources/timescaledb.yaml` - Data source config

### Files to Modify

- `deploy/pi/docker-compose.yml` - Add TimescaleDB data source to Grafana

### Grafana Provisioning

```yaml
# deploy/pi/grafana/provisioning/dashboards/dashboards.yaml
apiVersion: 1
providers:
  - name: 'NDP Dashboards'
    orgId: 1
    folder: 'NDP'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 30
    options:
      path: /var/lib/grafana/dashboards
```

### Database User for Grafana

```sql
-- Create read-only user for Grafana
CREATE USER grafana_reader WITH PASSWORD '${GRAFANA_TIMESCALE_PASSWORD}';
GRANT USAGE ON SCHEMA data_dictionary TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA data_dictionary TO grafana_reader;
```

---

## Related Decisions

- **ADR-001 (DP-002)**: TimescaleDB Schema Design (data source)
- **ADR-002 (DP-002)**: Entity Schema Format (displayed in dashboard)
- **ADR-003 (DP-002)**: Sync Mechanism (populates data dictionary)

---

## References

- [Grafana PostgreSQL Data Source](https://grafana.com/docs/grafana/latest/datasources/postgres/)
- [Grafana Variables](https://grafana.com/docs/grafana/latest/dashboards/variables/)
- [Grafana DuckDB Plugin](https://grafana.com/grafana/plugins/motherduck-duckdb-datasource/) (if available)

---

**Last Updated**: 2025-12-30
**Next Review**: After dashboard deployment and user feedback
