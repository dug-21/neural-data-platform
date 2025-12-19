# BUG-003: Grafana Direct-to-Bronze Parquet Queries

**Status:** Open
**Priority:** High
**Created:** 2025-12-19
**Assignee:** TBD

---

## Executive Summary

Reconfigure the Indoor vs Outdoor comparison dashboard to query parquet files directly from Grafana's DuckDB plugin, bypassing the intermediate DuckDB container's view layer. This simplifies the architecture and resolves ARM64 compatibility issues.

---

## Background & Decision Context

### Problem Discovery Timeline

On 2025-12-19, while debugging Grafana dashboard issues on Raspberry Pi 5, we uncovered a fundamental architectural flaw in the DP-001 Silver Layer implementation.

#### Initial Issue: SQLite Plugin ARM64 Incompatibility

The original approach used:
1. DuckDB container creates views over parquet files
2. DuckDB exports data to SQLite for Grafana compatibility
3. Grafana uses `frser-sqlite-datasource` plugin to query SQLite

**Problem:** The SQLite plugin returned empty results on ARM64 (Raspberry Pi 5) due to binary incompatibility.

#### Attempted Fix: Direct DuckDB Plugin

Switched to `motherduck-duckdb-datasource` plugin directly:
- v0.4.0 required glibc 2.38 (Ubuntu 22.04 has 2.35)
- v0.2.1 works with glibc 2.35
- Plugin loaded successfully on `grafana:latest-ubuntu`

#### Architecture Flaw Discovered

When Grafana queried the DuckDB database, queries failed with:
```
IO Error: No files found that match the pattern "/data/data/air-quality/**/*.parquet"
```

**Root Cause:** The Grafana DuckDB plugin is an **embedded DuckDB engine**, not a client connecting to a DuckDB server. The `duckdb` container only runs an init script to create the database file with view definitions - it's not a running database server.

When Grafana's plugin opens the database file and executes a view like:
```sql
SELECT * FROM readings_hourly
```

The view definition contains `read_parquet('/data/data/...')`. DuckDB runs **inside Grafana's process**, so Grafana needs filesystem access to the parquet files.

#### Architectural Question Raised

This revealed the "virtual Silver layer" was misleading:
- Views don't persist validated data
- DQ validation only happens at query time, not ingest time
- The duckdb container adds complexity without providing real value
- Grafana would need parquet volume access anyway

### Decision: Direct Bronze Queries

After discussion, decided to:
1. **Keep duckdb container** (for now) - may be useful for future materialized Silver layer
2. **Bypass duckdb views** - Grafana queries parquet directly
3. **Start with Indoor vs Outdoor dashboard** - most valuable cross-stream dashboard
4. **Defer other dashboards** - simplify scope

This approach:
- Eliminates the view dependency chain
- Grafana DuckDB plugin queries parquet directly
- All aggregation logic lives in dashboard SQL
- Simpler, more transparent architecture

---

## Scope

### In Scope
- Modify `indoor-vs-outdoor.json` dashboard to query parquet directly
- Update Grafana volume mounts in `docker-compose.yml`
- Update datasource configuration if needed
- Test on Raspberry Pi 5

### Out of Scope
- Removing duckdb container (future consideration)
- Modifying other dashboards (indoor-air-quality, outdoor-air-quality, outdoor-conditions)
- Implementing materialized Silver layer
- Data quality validation logic

---

## Technical Specification

### 1. Docker Compose Changes

**File:** `deploy/pi/docker-compose.yml`

Add parquet volume mount to Grafana service:

```yaml
grafana:
  image: grafana/grafana:latest-ubuntu
  volumes:
    - grafana-data:/var/lib/grafana
    - ../../config/grafana/grafana.ini:/etc/grafana/grafana.ini:ro
    - ../../config/grafana/provisioning:/etc/grafana/provisioning:ro
    - ../../config/grafana/dashboards:/var/lib/grafana/dashboards:ro
    - duckdb-data:/duckdb                   # Keep for potential future use
    - air-quality-data:/data:ro             # ADD: Parquet files for direct queries
```

### 2. Datasource Configuration

**File:** `config/grafana/provisioning/datasources/duckdb.yaml`

The datasource can remain configured to use the DuckDB database file, but queries will use `read_parquet()` directly instead of views.

Alternatively, configure without a database file (in-memory):
```yaml
apiVersion: 1
datasources:
  - name: NDP-DuckDB
    type: motherduck-duckdb-datasource
    uid: duckdb-ndp
    orgId: 1
    access: proxy
    isDefault: true
    editable: true
    jsonData:
      path: ":memory:"  # No persistent database needed
```

### 3. Dashboard Query Patterns

#### Parquet File Locations

| Stream | Path Pattern |
|--------|--------------|
| Indoor Air Quality | `/data/data/air-quality/**/*.parquet` |
| Outdoor Weather | `/data/data/outdoor-weather/**/*.parquet` |
| Outdoor Air Quality | `/data/data/outdoor-air-quality/**/*.parquet` |

#### Bronze Schema (Long Format)

All parquet files use the same schema:
```
timestamp: BIGINT (microseconds since epoch)
location_id: VARCHAR
metric: VARCHAR
value: DOUBLE
```

#### Key Metrics by Stream

**Indoor Air Quality (air-quality):**
- `pm02` - PM2.5 (main air quality indicator)
- `rco2` - CO2 in ppm
- `atmp` or `temperature` - Temperature in Celsius
- `rhum` or `humidity` - Relative humidity %

**Outdoor Weather (outdoor-weather):**
- `temperature` - Temperature in Celsius
- `feels_like` - Apparent temperature
- `humidity` - Relative humidity %
- `pressure` - Atmospheric pressure hPa
- `wind_speed` - Wind speed m/s

**Outdoor Air Quality (outdoor-air-quality):**
- `pm2_5` - PM2.5 concentration
- `aqi` - Air Quality Index (1-5 scale)
- `no2`, `o3`, `so2`, `co` - Pollutant concentrations

### 4. SQL Query Templates

#### Single Stream - Current Value (Stat Panel)

```sql
SELECT
  CASE WHEN metric = 'pm02' THEN value END as value
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE metric = 'pm02'
ORDER BY timestamp DESC
LIMIT 1
```

#### Single Stream - Time Series

```sql
SELECT
  epoch_ms(to_timestamp(timestamp/1000000)) as time,
  AVG(CASE WHEN metric = 'pm02' THEN value END) as "PM2.5"
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE to_timestamp(timestamp/1000000) BETWEEN $__timeFrom AND $__timeTo
GROUP BY time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000))
ORDER BY 1
```

#### Cross-Stream Comparison (Indoor vs Outdoor PM2.5)

```sql
WITH indoor AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as bucket,
    AVG(CASE WHEN metric = 'pm02' THEN value END) as pm25
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  WHERE to_timestamp(timestamp/1000000) BETWEEN $__timeFrom AND $__timeTo
  GROUP BY 1
),
outdoor AS (
  SELECT
    time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as bucket,
    AVG(CASE WHEN metric = 'pm2_5' THEN value END) as pm25
  FROM read_parquet('/data/data/outdoor-air-quality/**/*.parquet')
  WHERE to_timestamp(timestamp/1000000) BETWEEN $__timeFrom AND $__timeTo
  GROUP BY 1
)
SELECT
  epoch_ms(COALESCE(i.bucket, o.bucket)) as time,
  i.pm25 as "Indoor PM2.5",
  o.pm25 as "Outdoor PM2.5"
FROM indoor i
FULL OUTER JOIN outdoor o ON i.bucket = o.bucket
ORDER BY time
```

#### Cross-Stream Delta Calculation (Temperature Difference)

```sql
WITH indoor AS (
  SELECT
    AVG(CASE WHEN metric IN ('atmp', 'temperature') THEN value END) as temp
  FROM read_parquet('/data/data/air-quality/**/*.parquet')
  ORDER BY timestamp DESC
  LIMIT 100
),
outdoor AS (
  SELECT
    AVG(CASE WHEN metric = 'temperature' THEN value END) as temp
  FROM read_parquet('/data/data/outdoor-weather/**/*.parquet')
  ORDER BY timestamp DESC
  LIMIT 100
)
SELECT (indoor.temp - outdoor.temp) as value
FROM indoor, outdoor
```

### 5. Dashboard Panels to Implement

**File:** `config/grafana/dashboards/indoor-vs-outdoor.json`

| Panel ID | Title | Type | Streams | Metrics |
|----------|-------|------|---------|---------|
| 1 | Temperature Delta | stat | indoor + outdoor-weather | temperature |
| 2 | PM2.5 Delta | stat | indoor + outdoor-air | pm02, pm2_5 |
| 3 | Humidity Delta | stat | indoor + outdoor-weather | humidity |
| 4 | Temperature Comparison | timeseries | indoor + outdoor-weather | temperature |
| 5 | PM2.5 Comparison | timeseries | indoor + outdoor-air | pm02, pm2_5 |
| 6 | Humidity Comparison | timeseries | indoor + outdoor-weather | humidity |
| 7 | CO2 Levels (Indoor Only) | timeseries | indoor | rco2 |

---

## Implementation Steps

### Step 1: Update Docker Compose
1. Add `air-quality-data:/data:ro` volume mount to grafana service
2. Commit and push changes

### Step 2: Update Datasource (Optional)
1. Consider switching to `:memory:` path if no database file needed
2. Or keep existing duckdb-data mount for future use

### Step 3: Rewrite Dashboard JSON
1. Read existing `indoor-vs-outdoor.json`
2. Replace all `rawSql` queries with direct parquet queries
3. Use query templates from Section 4 above
4. Ensure proper time filtering with `$__timeFrom` and `$__timeTo`
5. Validate JSON syntax

### Step 4: Deploy and Test
1. Pull changes on Raspberry Pi
2. Restart grafana container
3. Verify dashboard loads without errors
4. Verify data displays correctly
5. Test time range selections

### Step 5: Validate Cross-Stream Alignment
1. Confirm indoor and outdoor data aligns on time axis
2. Verify delta calculations are correct
3. Check for any timezone issues

---

## Testing Checklist

- [ ] Docker compose starts without errors
- [ ] Grafana container has access to `/data` directory
- [ ] DuckDB plugin loads successfully
- [ ] Each panel executes without SQL errors
- [ ] Time series panels show data points
- [ ] Stat panels show current values
- [ ] Time range selector works correctly
- [ ] Cross-stream panels align data properly
- [ ] Delta calculations show reasonable values

---

## Rollback Plan

If implementation fails:
1. Revert dashboard JSON to previous version
2. Remove `/data:ro` volume mount from grafana
3. Dashboard will show errors but system remains stable

---

## Future Considerations

1. **Remove duckdb container** - Once direct queries are proven stable
2. **Add DQ validation** - WHERE clauses for range validation in queries
3. **Query optimization** - Add date partition pruning if performance is slow
4. **Materialized Silver layer** - If query performance becomes an issue, implement proper ETL

---

## References

- DuckDB Parquet documentation: https://duckdb.org/docs/data/parquet/overview
- Grafana DuckDB plugin: https://github.com/motherduckdb/grafana-duckdb-datasource
- DP-001 SCOPE.md: `product/features/dp-001/SCOPE.md`
- Docker Compose: `deploy/pi/docker-compose.yml`
- Dashboard JSON: `config/grafana/dashboards/indoor-vs-outdoor.json`
