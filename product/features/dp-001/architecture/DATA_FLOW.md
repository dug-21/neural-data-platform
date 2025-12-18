# Data Flow Architecture - DP-001

## Document Information

- **Feature**: DP-001 - DuckDB Analytics Layer
- **Created**: 2025-12-18
- **Author**: NDP Architect
- **Status**: Architecture Phase

## 1. Data Flow Overview

```
┌────────────────────────────────────────────────────────────────┐
│                     DATA INGESTION (existing)                   │
│                                                                 │
│  MQTT Sensor ─────┐                                            │
│  (AirGradient)    │                                            │
│                   ├──▶ air-quality-app ──▶ Parquet Files       │
│  HTTP APIs ───────┘    (Rust ingestion)    /data/{stream}/     │
│  (OpenWeatherMap)                                               │
└────────────────────────────────────────────────────────────────┘
                              │
                              │ Parquet files (Bronze Layer)
                              ▼
┌────────────────────────────────────────────────────────────────┐
│                     ANALYTICS LAYER (new - DP-001)             │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                      DuckDB                              │  │
│  │                                                          │  │
│  │  Bronze Access (raw Parquet)                            │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ read_parquet('/data/air-quality/**/*.parquet')  │   │  │
│  │  │ read_parquet('/data/outdoor-weather/**/*.parquet')│  │  │
│  │  │ read_parquet('/data/outdoor-air-quality/**/*.parquet')│ │
│  │  └─────────────────────────────────────────────────┘   │  │
│  │                         │                                │  │
│  │                         ▼                                │  │
│  │  Virtual Silver Views (DQ applied)                      │  │
│  │  ┌─────────────────────────────────────────────────┐   │  │
│  │  │ silver_indoor_air      - NULL/range filtered    │   │  │
│  │  │ silver_outdoor_weather - NULL/range filtered    │   │  │
│  │  │ silver_outdoor_air     - NULL/range filtered    │   │  │
│  │  │ cross_stream_aligned   - Time-bucketed JOIN     │   │  │
│  │  └─────────────────────────────────────────────────┘   │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              │ SQL queries                      │
│                              ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │                      Grafana                             │  │
│  │                                                          │  │
│  │  ┌─────────────────┐  ┌─────────────────┐              │  │
│  │  │ Indoor Air      │  │ Outdoor Weather │              │  │
│  │  │ Dashboard       │  │ Dashboard       │              │  │
│  │  └─────────────────┘  └─────────────────┘              │  │
│  │  ┌─────────────────┐  ┌─────────────────┐              │  │
│  │  │ Outdoor AQI     │  │ Indoor vs       │              │  │
│  │  │ Dashboard       │  │ Outdoor Compare │              │  │
│  │  └─────────────────┘  └─────────────────┘              │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              │ HTTP :3000                       │
│                              ▼                                  │
│                         Web Browser                             │
└────────────────────────────────────────────────────────────────┘
```

## 2. Query Execution Flow

### Step-by-Step Execution

1. **User Interaction**
   - User selects time range in Grafana dashboard
   - User chooses visualization panel (e.g., Indoor Temperature)

2. **Grafana Query Generation**
   - Grafana constructs SQL query with:
     - Time range filter (WHERE timestamp BETWEEN $__timeFrom() AND $__timeTo())
     - Selected metrics (e.g., temperature, humidity)
     - Aggregation if needed (e.g., AVG per 5-minute bucket)

3. **DuckDB Query Processing**
   - Receives SQL via datasource plugin
   - Parses query and identifies referenced view
   - Expands view definition (e.g., silver_indoor_air)
   - Optimizes query plan

4. **Parquet File Scanning**
   - Executes read_parquet() with glob pattern
   - Applies partition pruning based on time range
   - Reads only relevant files (e.g., only 2025-12-18_readings.parquet)
   - Uses Parquet metadata to skip row groups

5. **Data Quality Filtering**
   - Applies NULL filters (WHERE field IS NOT NULL)
   - Validates ranges (CASE WHEN ... THEN ... ELSE NULL END)
   - Type coercion and precision rounding

6. **Result Return**
   - DuckDB returns result set to Grafana
   - Grafana receives time-series data

7. **Visualization Rendering**
   - Grafana renders chart/graph
   - User sees updated visualization

### Query Example

```sql
-- Grafana sends:
SELECT
  time_bucket(INTERVAL '5 minutes', timestamp) AS time,
  AVG(temperature) AS avg_temp
FROM silver_indoor_air
WHERE timestamp BETWEEN '2025-12-18 00:00:00' AND '2025-12-18 23:59:59'
GROUP BY time
ORDER BY time;

-- DuckDB executes (expanded view):
SELECT
  time_bucket(INTERVAL '5 minutes', timestamp) AS time,
  AVG(
    CASE
      WHEN temperature BETWEEN -40 AND 85 THEN ROUND(temperature, 1)
      ELSE NULL
    END
  ) AS avg_temp
FROM read_parquet('/data/air-quality/2025-12-18_readings.parquet')
WHERE timestamp BETWEEN '2025-12-18 00:00:00' AND '2025-12-18 23:59:59'
  AND timestamp IS NOT NULL
  AND temperature IS NOT NULL
GROUP BY time
ORDER BY time;
```

## 3. Data Refresh Pattern

### No ETL - Query-Time Processing

- **Bronze Layer Updates**: Continuous
  - air-quality-app writes Parquet files as data arrives
  - New rows appended to current day's file
  - New file created at midnight UTC

- **DuckDB Access**: On-Demand
  - No materialized views or caching
  - Reads Parquet files directly on each query
  - View definitions evaluated at query time
  - No staleness - always queries latest data

- **Grafana Refresh**: Configurable
  - Default: 5 minutes (adjustable per dashboard)
  - Auto-refresh when time range changes
  - Manual refresh button available

### Refresh Flow

```
Sensor → MQTT → air-quality-app → Parquet (immediate)
                                       ↓
User → Grafana → Query → DuckDB → read_parquet() (on-demand)
                                       ↓
                                  Latest data
```

### Performance Considerations

- Query latency: <1 second for 24-hour range
- Partition pruning minimizes file scanning
- DuckDB's columnar engine optimized for analytics
- No ETL lag or batch windows

## 4. Partition Strategy

### Daily Partitioning Pattern

Parquet files follow consistent daily partitioning:

```
/data/air-quality/
├── 2025-12-15_readings.parquet  (previous days)
├── 2025-12-16_readings.parquet
├── 2025-12-17_readings.parquet
└── 2025-12-18_readings.parquet  (current day - active writes)

/data/outdoor-weather/
├── 2025-12-15_readings.parquet
├── 2025-12-16_readings.parquet
├── 2025-12-17_readings.parquet
└── 2025-12-18_readings.parquet

/data/outdoor-air-quality/
├── 2025-12-15_readings.parquet
├── 2025-12-16_readings.parquet
├── 2025-12-17_readings.parquet
└── 2025-12-18_readings.parquet
```

### Partition Pruning Benefits

DuckDB automatically prunes partitions when WHERE clause includes timestamp:

```sql
-- Query for last 24 hours
WHERE timestamp >= NOW() - INTERVAL '24 hours'
-- DuckDB scans only: 2025-12-17_readings.parquet, 2025-12-18_readings.parquet

-- Query for specific day
WHERE timestamp BETWEEN '2025-12-15 00:00:00' AND '2025-12-15 23:59:59'
-- DuckDB scans only: 2025-12-15_readings.parquet

-- Query for last 7 days
WHERE timestamp >= NOW() - INTERVAL '7 days'
-- DuckDB scans 7 files
```

### File Size Management

- Typical file size: 1-5 MB per day (low-volume sensors)
- Retention: 90 days (configurable)
- Archival: Move to cold storage after retention period

## 5. Data Quality Flow

### Bronze → Silver Transformation

Virtual views apply DQ rules at query time:

```sql
-- Bronze (raw Parquet)
read_parquet('/data/air-quality/**/*.parquet')
-- Contains: NULL values, out-of-range, inconsistent precision

          ↓

-- Silver (DQ-filtered view)
CREATE VIEW silver_indoor_air AS
SELECT
  timestamp,
  CASE WHEN temperature BETWEEN -40 AND 85
       THEN ROUND(temperature, 1)
       ELSE NULL END AS temperature,
  CASE WHEN humidity BETWEEN 0 AND 100
       THEN ROUND(humidity, 1)
       ELSE NULL END AS humidity,
  CASE WHEN co2 BETWEEN 400 AND 5000
       THEN ROUND(co2, 0)
       ELSE NULL END AS co2,
  CASE WHEN pm25 BETWEEN 0 AND 500
       THEN ROUND(pm25, 1)
       ELSE NULL END AS pm25,
  CASE WHEN pm10 BETWEEN 0 AND 500
       THEN ROUND(pm10, 1)
       ELSE NULL END AS pm10,
  CASE WHEN voc BETWEEN 0 AND 60000
       THEN ROUND(voc, 0)
       ELSE NULL END AS voc,
  CASE WHEN nox BETWEEN 0 AND 500
       THEN ROUND(nox, 1)
       ELSE NULL END AS nox
FROM read_parquet('/data/air-quality/**/*.parquet')
WHERE timestamp IS NOT NULL;
```

### DQ Rule Categories

1. **NULL Filtering**
   - Required fields: timestamp (always checked)
   - Optional fields: Allow NULL after range validation

2. **Range Validation**
   - Physical limits (e.g., humidity 0-100%)
   - Sensor specs (e.g., temperature -40 to 85°C)
   - Out-of-range → NULL (not rejected)

3. **Type Coercion**
   - Ensure DOUBLE precision for floats
   - BIGINT for timestamps
   - VARCHAR for metadata

4. **Precision Rounding**
   - Temperature/humidity: 1 decimal place
   - PM2.5/PM10: 1 decimal place
   - CO2/VOC: Integer
   - NOx: 1 decimal place

### DQ Impact on Results

- Invalid data excluded from aggregations (AVG, MAX, MIN)
- COUNT(*) vs COUNT(field) differs when NULLs present
- Grafana displays gaps for missing data periods

## 6. Cross-Stream Alignment

### Time Bucket Strategy

Different streams have different resolutions:

| Stream | Native Resolution | Aligned Resolution |
|--------|-------------------|-------------------|
| Indoor Air | ~1 minute | 10 minutes |
| Outdoor Weather | 10 minutes | 10 minutes |
| Outdoor AQI | 10 minutes | 10 minutes |

### 10-Minute Alignment View

```sql
CREATE VIEW cross_stream_aligned AS
WITH bucketed_indoor AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', timestamp) AS bucket,
    AVG(temperature) AS avg_indoor_temp,
    AVG(humidity) AS avg_indoor_humidity,
    AVG(co2) AS avg_indoor_co2,
    AVG(pm25) AS avg_indoor_pm25
  FROM silver_indoor_air
  GROUP BY bucket
),
bucketed_outdoor_weather AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', timestamp) AS bucket,
    AVG(temperature) AS avg_outdoor_temp,
    AVG(humidity) AS avg_outdoor_humidity,
    AVG(pressure) AS avg_pressure
  FROM silver_outdoor_weather
  GROUP BY bucket
),
bucketed_outdoor_air AS (
  SELECT
    time_bucket(INTERVAL '10 minutes', timestamp) AS bucket,
    AVG(aqi) AS avg_aqi,
    AVG(pm25) AS avg_outdoor_pm25,
    AVG(pm10) AS avg_outdoor_pm10
  FROM silver_outdoor_air
  GROUP BY bucket
)
SELECT
  COALESCE(i.bucket, ow.bucket, oa.bucket) AS timestamp,
  i.avg_indoor_temp,
  i.avg_indoor_humidity,
  i.avg_indoor_co2,
  i.avg_indoor_pm25,
  ow.avg_outdoor_temp,
  ow.avg_outdoor_humidity,
  ow.avg_pressure,
  oa.avg_aqi,
  oa.avg_outdoor_pm25,
  oa.avg_outdoor_pm10
FROM bucketed_indoor i
FULL OUTER JOIN bucketed_outdoor_weather ow ON i.bucket = ow.bucket
FULL OUTER JOIN bucketed_outdoor_air oa ON i.bucket = oa.bucket
ORDER BY timestamp;
```

### Alignment Benefits

- **Comparable Time Periods**: All streams aligned to same 10-minute buckets
- **Efficient JOINs**: Bucketed aggregation before JOIN reduces row count
- **Consistent Dashboards**: Indoor vs Outdoor comparisons use same time axis
- **Flexible Granularity**: Can change bucket size (5 min, 15 min, 1 hour)

### Use Cases

1. Indoor vs Outdoor Temperature Correlation
2. Indoor PM2.5 vs Outdoor AQI Relationship
3. Humidity Differential Analysis
4. Combined Air Quality Index

## 7. Performance Characteristics

### Query Latency Targets

| Query Type | Time Range | Target Latency | Typical Result Size |
|------------|------------|----------------|---------------------|
| Single Stream | 24 hours | <500ms | ~1,440 rows (1 min res) |
| Single Stream | 7 days | <1 second | ~10,080 rows |
| Cross-Stream | 24 hours | <1 second | ~144 rows (10 min buckets) |
| Cross-Stream | 7 days | <2 seconds | ~1,008 rows |
| Dashboard Load | Mixed | <3 seconds | Multiple queries |

### Optimization Techniques

1. **Partition Pruning**: Only scan relevant date files
2. **Columnar Scanning**: Read only requested columns from Parquet
3. **Predicate Pushdown**: Apply filters at Parquet row group level
4. **View Inlining**: DuckDB optimizes view expansion
5. **Parallel Execution**: Multi-threaded query execution

### Scalability Limits

- **Data Volume**: Handles 1+ year of data (100+ million rows)
- **Concurrent Users**: 10-20 simultaneous Grafana users
- **Query Complexity**: Supports complex aggregations and JOINs
- **Resource Constraints**: Runs efficiently on Raspberry Pi 5

## 8. Error Handling and Edge Cases

### Missing Data Periods

- Sensor offline: NULLs in time series, Grafana shows gaps
- Network outage: Data buffered, written when connection restored
- File corruption: DuckDB skips unreadable row groups

### Clock Drift and Time Zones

- All timestamps stored in UTC
- Grafana handles timezone display conversion
- Time bucket alignment uses UTC boundaries

### Schema Evolution

- New fields added to Parquet: Views ignore unknown columns
- Field type changes: DuckDB attempts coercion
- Missing fields: Views handle with COALESCE or default NULL

### Query Failures

- Invalid SQL: Grafana shows error message to user
- Timeout (>30s): Reduce time range or simplify query
- Out of memory: DuckDB spills to disk (temp directory)

## 9. Monitoring and Observability

### Key Metrics to Track

1. **Query Performance**
   - Average query latency
   - 95th percentile latency
   - Queries exceeding timeout

2. **Data Freshness**
   - Time since last Parquet write
   - Gap detection in time series

3. **Data Quality**
   - NULL percentage by field
   - Out-of-range value count
   - Missing data periods

4. **System Health**
   - DuckDB process CPU/memory
   - Grafana datasource errors
   - Parquet file count and size

### Logging

- DuckDB query logs (optional profiling)
- Grafana datasource plugin logs
- air-quality-app write logs

## 10. Future Enhancements

### Potential Optimizations

1. **Materialized Views**: Cache pre-aggregated data for faster queries
2. **Incremental Refresh**: Only recompute new data
3. **Result Caching**: Cache recent query results in Grafana
4. **Compression**: ZSTD compression for older Parquet files

### Advanced Analytics

1. **Anomaly Detection**: Flag unusual patterns in data
2. **Forecasting**: Predict future values using trends
3. **Correlation Analysis**: Automated cross-stream insights
4. **Alerting**: Threshold-based notifications

### Integration Points

- Export to ML pipeline (Feature Engineering phase)
- API for external applications
- Real-time streaming views (beyond batch)

## Related Documentation

- `SCHEMA_DESIGN.md` - DuckDB view definitions and DQ rules
- `GRAFANA_DASHBOARDS.md` - Dashboard specifications
- `../specification/SPECIFICATION.md` - Feature requirements
- `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` - Overall platform design
