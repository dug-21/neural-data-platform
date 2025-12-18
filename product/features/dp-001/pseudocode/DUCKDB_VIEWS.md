# DuckDB Silver Layer Views - Design Document

**Feature**: DP-001 - Silver Layer Query Infrastructure
**Author**: ndp-parquet-dev
**Date**: 2025-12-18
**Status**: Pseudocode Phase

---

## Overview

This document describes the DuckDB SQL view layer for the Neural Data Platform's Silver layer. DuckDB provides fast, analytical queries over Parquet files stored in the Bronze layer, with automatic schema inference and parallel query execution.

---

## Architecture

### Data Flow

```
Bronze Layer (Parquet)
    /data/air-quality/*.parquet
    /data/outdoor-weather/*.parquet
    /data/outdoor-air-quality/*.parquet
         │
         │ DuckDB read_parquet()
         ▼
Silver Layer (Views)
    silver_indoor_air
    silver_outdoor_weather
    silver_outdoor_air
         │
         │ JOIN + time_bucket()
         ▼
Cross-Stream View
    cross_stream_aligned
```

### View Responsibilities

| View | Purpose | Data Quality | Performance Target |
|------|---------|--------------|-------------------|
| `silver_indoor_air` | Indoor air quality with validation | Range checks, NULL handling | <5s for 7 days |
| `silver_outdoor_weather` | Outdoor weather with validation | Range checks, rounding | <5s for 7 days |
| `silver_outdoor_air` | Outdoor air quality with validation | Range checks, NULL handling | <5s for 7 days |
| `cross_stream_aligned` | Time-aligned multi-stream data | 10-minute buckets | <15s for 30 days |

---

## Schema Mappings

### Indoor Air Quality (air-quality stream)

**Bronze Schema** (from Parquet):
```
timestamp: TIMESTAMP (UTC)
stream_id: VARCHAR
location_id: VARCHAR (nullable)
fields: VARCHAR (JSON-encoded)
tags: VARCHAR (JSON-encoded, nullable)
```

**Silver Schema** (extracted from fields):
```sql
timestamp: TIMESTAMP,
pm25: FLOAT (0-500 µg/m³),
pm10: FLOAT (0-1000 µg/m³),
co2: FLOAT (400-5000 ppm),
temperature: FLOAT (-10-50°C, rounded to 1 decimal),
humidity: FLOAT (0-100%, rounded to 1 decimal),
tvoc: FLOAT (ppb),
nox: FLOAT (ppb)
```

### Outdoor Weather (outdoor-weather stream)

**Silver Schema**:
```sql
timestamp: TIMESTAMP,
temperature: FLOAT (-50-60°C, rounded to 1 decimal),
feels_like: FLOAT (-50-60°C, rounded to 1 decimal),
pressure: FLOAT (800-1200 hPa, rounded to 1 decimal),
humidity: FLOAT (0-100%, rounded to 1 decimal),
wind_speed: FLOAT (0-100 m/s, rounded to 2 decimals),
wind_deg: FLOAT (0-360 degrees, rounded to 0 decimals),
wind_gust: FLOAT (0-150 m/s, rounded to 2 decimals),
clouds: FLOAT (0-100%, rounded to 0 decimals),
visibility: FLOAT (0-50000 meters, rounded to 0 decimals),
rain_1h: FLOAT (0-500 mm, rounded to 2 decimals),
snow_1h: FLOAT (0-500 mm, rounded to 2 decimals)
```

### Outdoor Air Quality (outdoor-air-quality stream)

**Silver Schema**:
```sql
timestamp: TIMESTAMP,
aqi: FLOAT (1-5 scale, rounded to 0 decimals),
co: FLOAT (0-50000 µg/m³, rounded to 1 decimal),
no: FLOAT (0-1000 µg/m³, rounded to 2 decimals),
no2: FLOAT (0-1000 µg/m³, rounded to 2 decimals),
o3: FLOAT (0-1000 µg/m³, rounded to 2 decimals),
so2: FLOAT (0-1000 µg/m³, rounded to 2 decimals),
pm2_5: FLOAT (0-1000 µg/m³, rounded to 1 decimal),
pm10: FLOAT (0-1000 µg/m³, rounded to 1 decimal),
nh3: FLOAT (0-200 µg/m³, rounded to 2 decimals)
```

---

## Data Quality Rules

### Range Validation

Each field is validated against expected physical ranges:

```sql
CASE WHEN {field} BETWEEN {min} AND {max}
     THEN {field}
     ELSE NULL
END as {field}
```

**Why**: Out-of-range values indicate sensor errors or data corruption. Setting them to NULL allows downstream systems to handle missing data gracefully.

### Rounding Strategy

Different field types require different precision levels:

| Field Type | Precision | Rationale |
|-----------|-----------|-----------|
| Temperature | 1 decimal | Sensor accuracy ±0.5°C |
| Humidity | 1 decimal | Sensor accuracy ±2% |
| PM2.5/PM10 | 1 decimal | Meaningful precision for health assessments |
| Wind speed | 2 decimals | Standard meteorological precision |
| Gases (ppb/ppm) | 0-2 decimals | Varies by concentration range |

### NULL Handling

- **Nullable fields**: Preserved as NULL (e.g., optional sensor readings)
- **Required fields**: Must have non-NULL timestamp (enforced by WHERE clause)
- **Invalid values**: Set to NULL after range validation

---

## Performance Optimizations

### Parquet File Reading

```sql
FROM read_parquet('/data/{stream-id}/**/*.parquet',
                  union_by_name=true,
                  filename=true)
```

**Parameters**:
- `union_by_name=true`: Handles schema evolution (new fields added over time)
- `filename=true`: Includes file path for debugging and partition pruning
- `**/*.parquet`: Recursive glob pattern for hourly/daily partitioned files

### Partition Pruning

DuckDB automatically prunes Parquet files based on:
1. **Filename patterns**: `/data/{stream-id}/2025-12-18_*.parquet`
2. **WHERE clauses**: `WHERE timestamp >= '2025-12-11'`
3. **Min/max statistics**: Parquet row group metadata

Expected performance:
- 7-day query: Reads ~168 hourly files (or 7 daily files) per stream
- 30-day query: Reads ~720 hourly files (or 30 daily files)

### Timestamp Indexing

```sql
WHERE timestamp IS NOT NULL
AND timestamp >= current_timestamp - INTERVAL '7 days'
```

**Why**: Parquet files are sorted by timestamp. DuckDB uses min/max stats to skip entire row groups.

---

## Cross-Stream Alignment

### Time Bucket Strategy

The `cross_stream_aligned` view uses 10-minute time buckets to align streams with different sampling rates:

- **Indoor air quality**: ~1 reading/minute (via MQTT)
- **Outdoor weather**: 1 reading/10 minutes (via HTTP poll)
- **Outdoor air quality**: 1 reading/10 minutes (via HTTP poll)

```sql
time_bucket(INTERVAL '10 minutes', timestamp) as time_bucket
```

**Aggregation within bucket**:
- Use `AVG()` for most fields (indoor sensors)
- Use `FIRST()` for already-aggregated fields (outdoor APIs)

### JOIN Strategy

```sql
FULL OUTER JOIN ... USING (time_bucket)
```

**Why FULL OUTER JOIN**:
- Indoor sensors may have data when outdoor APIs are down
- Outdoor APIs may poll at bucket boundaries when indoor sensors are offline
- Downstream ML models can handle sparse data

### Performance Considerations

The cross-stream view is the most expensive query:

- **Worst case**: 3 streams × 720 files/stream = 2,160 files for 30-day query
- **Mitigation**:
  - Pre-filter each stream by date range before JOIN
  - Use `time_bucket` as JOIN key (indexed)
  - Limit columns to only those needed

**Expected performance**: <15s for 30-day query on Raspberry Pi 5

---

## View Initialization

The `config/duckdb/init.sql` bootstrap script:

1. Creates database (if not exists)
2. Sources individual view SQL files
3. Validates views with sample queries
4. Logs creation timestamps

**Usage**:
```bash
duckdb /data/silver.duckdb < /config/duckdb/init.sql
```

**Idempotency**: Uses `CREATE OR REPLACE VIEW` to allow re-runs.

---

## File Organization

```
/config/duckdb/
├── init.sql                          # Bootstrap script
└── views/
    ├── silver_indoor_air.sql         # Indoor air quality view
    ├── silver_outdoor_weather.sql    # Outdoor weather view
    ├── silver_outdoor_air.sql        # Outdoor air quality view
    └── cross_stream_aligned.sql      # Cross-stream correlation view
```

---

## Future Enhancements

### Phase 1 (DP-002): Materialized Views
- Pre-compute hourly/daily aggregates
- Store in separate Parquet files
- Reduce query time for dashboards

### Phase 2 (DP-003): Incremental Refresh
- Detect new Parquet files
- Refresh only affected time ranges
- Use DuckDB's `CREATE TEMPORARY TABLE` for state

### Phase 3 (FE-001): Feature Engineering Views
- Moving averages (1h, 6h, 24h)
- Anomaly detection (Z-score, IQR)
- Correlation matrices

---

## Testing Strategy

### Unit Tests (per view)
1. **Schema validation**: Verify column names and types
2. **Range validation**: Test boundary conditions
3. **NULL handling**: Test missing data scenarios

### Integration Tests (cross-stream)
1. **Time alignment**: Verify 10-minute buckets
2. **JOIN correctness**: Test all combinations (all data, partial data, no data)
3. **Performance**: Benchmark 7-day and 30-day queries

### Load Tests
1. Query 1000 times with random date ranges
2. Verify memory usage stays <500MB
3. Measure P50, P95, P99 latencies

---

## References

- [DP-001 Specification](../specification/SPECIFICATION.md)
- [DuckDB Parquet Documentation](https://duckdb.org/docs/data/parquet)
- [Stream Configurations](../../../../config/base/streams/)
- [Platform Architecture Overview](../../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-18 | ndp-parquet-dev | Initial design with 3 silver views + cross-stream view |
