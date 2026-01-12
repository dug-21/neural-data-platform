# Silver Layer ETL Alternatives Research

**Date**: 2026-01-05
**Author**: ndp-timescale-dev
**Feature**: DP-006 Silver Layer ETL
**Target**: Raspberry Pi 5 (16GB RAM)

---

## Executive Summary

This document evaluates ETL approaches for moving data from the Bronze layer (Parquet files) to the Silver layer (TimescaleDB) on a resource-constrained Raspberry Pi 5 deployment. After analyzing four primary approaches, **DuckDB as ETL Engine** emerges as the recommended solution due to its optimal balance of performance, memory efficiency, and implementation simplicity.

### Recommendation: DuckDB-Based ETL with Hourly Cron Scheduling

| Factor | Rating | Rationale |
|--------|--------|-----------|
| Pi Suitability | Excellent | 512MB memory cap already configured, proven in production |
| Performance | Excellent | 15x faster than raw PostgreSQL inserts, native Parquet support |
| Complexity | Low | SQL-only implementation, existing view definitions reusable |
| Maintainability | High | Aligns with existing DuckDB virtual Silver layer |
| Risk | Low | DuckDB already deployed and tested on Pi |

---

## Current Architecture Context

### Bronze Layer (Production)

```
/data/raw/{stream-id}/year=/month=/day=/data.parquet

7 Active Streams:
- air-quality         (~95KB/day)  - Indoor AirGradient sensor
- outdoor-weather     (~7KB/day)   - OpenWeatherMap
- outdoor-air-quality (~5KB/day)   - OpenWeatherMap AQI
- nws-observations    (~38KB/day)  - NWS station KSGJ
- nws-station-observations (~14KB/day)
- nws-forecast-hourly (~127KB/day)
- nws-gridpoints-forecast (~261KB/day)

Total: ~550KB/day (~200MB/year)
```

### Current Virtual Silver Layer (DuckDB Views)

The existing DP-001 implementation uses DuckDB views over Parquet files:
- No data duplication
- Query-time transformation
- 512MB memory limit configured
- Performance: <5s for 7-day queries

### Proposed Silver Layer (TimescaleDB)

TimescaleDB already deployed with 256MB limit:
- Hypertables for time-series optimization
- Continuous aggregates for dashboards
- SQL-standard querying from Grafana
- 90-day retention for raw data

---

## ETL Approach Comparison Matrix

| Approach | Memory | Complexity | Performance | Pi Suitability | Recovery |
|----------|--------|------------|-------------|----------------|----------|
| **DuckDB ETL** | 100-200MB | Low | Excellent | Excellent | Good |
| **Rust Native** | 50-100MB | High | Good | Excellent | Excellent |
| **Python Polars** | 200-400MB | Medium | Good | Good | Good |
| **pg_parquet FDW** | 50MB | Low | Fair | Good | Fair |

---

## Approach 1: DuckDB as ETL Engine (RECOMMENDED)

### Architecture

```
Bronze (Parquet)                    Silver (TimescaleDB)
     |                                     ^
     |  read_parquet()                     |
     v                                     |
 DuckDB In-Memory  -----> COPY TO postgres.readings
     |
     +-- Existing SQL view definitions reusable
```

### Implementation

```sql
-- duckdb_etl.sql
-- Bronze to Silver ETL via DuckDB postgres extension

LOAD postgres;
ATTACH 'host=timescaledb dbname=ndp user=postgres password=...' AS pg (TYPE postgres);

-- Incremental load: last hour only
INSERT INTO pg.readings (time, stream_id, ndp_id, temperature, humidity, pm25, co2, extra_fields)
SELECT
    to_timestamp(timestamp / 1000000) as time,
    'air-quality' as stream_id,
    ndp_id,
    CASE WHEN json_extract(raw_payload, '$.atmp')::FLOAT BETWEEN -10 AND 50
         THEN ROUND(json_extract(raw_payload, '$.atmp')::FLOAT, 1) END as temperature,
    CASE WHEN json_extract(raw_payload, '$.rhum')::FLOAT BETWEEN 0 AND 100
         THEN ROUND(json_extract(raw_payload, '$.rhum')::FLOAT, 1) END as humidity,
    CASE WHEN json_extract(raw_payload, '$.pm02')::FLOAT BETWEEN 0 AND 500
         THEN ROUND(json_extract(raw_payload, '$.pm02')::FLOAT, 1) END as pm25,
    CASE WHEN json_extract(raw_payload, '$.rco2')::INT BETWEEN 400 AND 5000
         THEN json_extract(raw_payload, '$.rco2')::INT END as co2,
    raw_payload as extra_fields
FROM read_parquet('/data/raw/air-quality/**/*.parquet')
WHERE to_timestamp(timestamp / 1000000) > (
    SELECT COALESCE(MAX(time), '1970-01-01'::TIMESTAMP) FROM pg.readings WHERE stream_id = 'air-quality'
)
AND to_timestamp(timestamp / 1000000) <= current_timestamp - INTERVAL '5 minutes';
```

### Pros

1. **Existing Investment**: DuckDB already deployed (512MB), proven on Pi
2. **SQL-Only**: No new code compilation, reuse existing view SQL
3. **Memory Efficient**: Streaming query execution, predictable memory
4. **Native Parquet**: Zero-copy reads from Arrow format
5. **Postgres Extension**: Direct INSERT into TimescaleDB via postgres scanner

### Cons

1. **No Incremental State**: Must track watermarks externally or query target
2. **DuckDB Dependency**: Additional service to maintain
3. **Limited Error Handling**: SQL-based recovery is basic

### Memory Budget

| Component | Memory |
|-----------|--------|
| DuckDB process | 512MB (configured) |
| Query buffer | ~100MB per stream |
| Arrow buffers | ~50MB |
| **Peak** | **~200MB during ETL** |

### Performance Estimate

Based on benchmarks ([DuckDB Performance Guide](https://duckdb.org/docs/stable/guides/performance/overview)):
- Parquet scan: ~500K rows/second
- Postgres INSERT via extension: ~50K rows/second
- Expected daily load time: <30 seconds for all streams

---

## Approach 2: Rust Native ETL (arrow-rs + sqlx)

### Architecture

```
Bronze (Parquet)              Silver (TimescaleDB)
     |                               ^
     |  polars read                  |
     v                               |
 Rust Process  -----> sqlx COPY / UNNEST batch insert
     |
     +-- Compile into air-quality-app or standalone binary
```

### Implementation Pattern

```rust
use polars::prelude::*;
use sqlx::postgres::PgPool;
use chrono::{DateTime, Utc};

pub struct ParquetToTimescaleETL {
    pool: PgPool,
    parquet_store: ParquetStore,
    watermarks: HashMap<String, DateTime<Utc>>,
}

impl ParquetToTimescaleETL {
    pub async fn run_incremental(&mut self, stream_id: &str) -> Result<usize, CoreError> {
        let last_watermark = self.get_watermark(stream_id).await?;

        // Read new data from Parquet
        let df = LazyFrame::scan_parquet(
            format!("/data/raw/{}/**/*.parquet", stream_id),
            ScanArgsParquet::default()
        )?
        .filter(col("timestamp").gt(lit(last_watermark.timestamp_micros())))
        .collect()?;

        if df.height() == 0 {
            return Ok(0);
        }

        // Transform and insert using UNNEST for batch efficiency
        let timestamps: Vec<i64> = df.column("timestamp")?.i64()?.into_iter().collect();
        let payloads: Vec<String> = df.column("raw_payload")?.utf8()?.into_iter().collect();

        let inserted = sqlx::query(r#"
            INSERT INTO readings (time, stream_id, raw_payload)
            SELECT
                to_timestamp(t / 1000000),
                $1,
                p::jsonb
            FROM UNNEST($2::bigint[], $3::text[]) AS t(t, p)
            ON CONFLICT (time, stream_id) DO NOTHING
        "#)
        .bind(stream_id)
        .bind(&timestamps)
        .bind(&payloads)
        .execute(&self.pool)
        .await?
        .rows_affected();

        self.update_watermark(stream_id, max_timestamp).await?;
        Ok(inserted as usize)
    }
}
```

### Pros

1. **Memory Efficient**: Lowest footprint (~50-100MB)
2. **Type Safety**: Compile-time SQL checking with sqlx
3. **Integrated**: Can run as part of existing air-quality-app
4. **Watermark Management**: Native state handling in Rust
5. **Error Recovery**: Full programmatic control

### Cons

1. **Development Effort**: Significant code (~500-1000 LOC)
2. **Compile Time**: Rust compilation on Pi is slow
3. **Complexity**: More failure modes to handle
4. **sqlx Performance**: [Known to be slower than tokio-postgres](https://github.com/launchbadge/sqlx/issues/2436) (up to 2x)

### Memory Budget

| Component | Memory |
|-----------|--------|
| Rust process | ~50MB base |
| Polars DataFrame | ~50MB per batch |
| sqlx connection pool | ~10MB |
| **Peak** | **~110MB during ETL** |

### Performance Estimate

Based on [Rust PostgreSQL batching research](https://kerkour.com/postgresql-batching):
- UNNEST batch insert: ~100K rows/second
- Expected daily load time: <15 seconds for all streams

---

## Approach 3: Python Polars/DuckDB ETL

### Architecture

```
Bronze (Parquet)              Silver (TimescaleDB)
     |                               ^
     |  pl.scan_parquet()            |
     v                               |
 Python Process  -----> write_database(engine='adbc')
     |
     +-- Standalone Python script, cron scheduled
```

### Implementation

```python
#!/usr/bin/env python3
# etl_bronze_to_silver.py

import polars as pl
from datetime import datetime, timedelta
import os

STREAMS = [
    ('air-quality', 'indoor_air'),
    ('outdoor-weather', 'outdoor_weather'),
    ('outdoor-air-quality', 'outdoor_air'),
]

PG_URI = f"postgresql://postgres:{os.environ['POSTGRES_PASSWORD']}@timescaledb:5432/ndp"

def get_last_watermark(stream_id: str) -> datetime:
    """Query TimescaleDB for latest timestamp"""
    result = pl.read_database(
        f"SELECT MAX(time) as last_time FROM readings WHERE stream_id = '{stream_id}'",
        PG_URI
    )
    return result['last_time'][0] or datetime(1970, 1, 1)

def transform_air_quality(df: pl.LazyFrame) -> pl.LazyFrame:
    """Apply DQ rules to air-quality stream"""
    return df.with_columns([
        pl.col('raw_payload').str.json_extract('$.atmp').alias('temperature'),
        pl.col('raw_payload').str.json_extract('$.rhum').alias('humidity'),
        pl.col('raw_payload').str.json_extract('$.pm02').alias('pm25'),
        pl.col('raw_payload').str.json_extract('$.rco2').alias('co2'),
    ]).filter(
        (pl.col('temperature').is_between(-10, 50)) &
        (pl.col('humidity').is_between(0, 100))
    )

def run_etl(stream_id: str, table_name: str):
    watermark = get_last_watermark(stream_id)
    cutoff = datetime.utcnow() - timedelta(minutes=5)  # Lag for late arrivals

    df = (
        pl.scan_parquet(f'/data/raw/{stream_id}/**/*.parquet')
        .filter(pl.col('timestamp') / 1_000_000 > watermark.timestamp())
        .filter(pl.col('timestamp') / 1_000_000 <= cutoff.timestamp())
    )

    if stream_id == 'air-quality':
        df = transform_air_quality(df)

    result = df.collect()
    if len(result) > 0:
        result.write_database(
            table_name,
            PG_URI,
            engine='adbc',  # COPY-based, fast
            if_table_exists='append'
        )
        print(f"Inserted {len(result)} rows for {stream_id}")

if __name__ == '__main__':
    for stream_id, table in STREAMS:
        run_etl(stream_id, table)
```

### Pros

1. **Rapid Development**: Fastest to implement (~100 LOC)
2. **Polars Performance**: Lazy evaluation, streaming
3. **ADBC Driver**: Uses COPY for fast inserts ([~5.4s for large batches](https://aklaver.org/wordpress/2024/03/08/using-polars-duckdb-with-postgres/))
4. **Familiar**: Python ecosystem widely understood

### Cons

1. **Memory**: Python + Polars ~200-400MB
2. **Runtime Dependency**: Need Python environment on Pi
3. **No Compile-Time Checks**: Runtime errors only
4. **GIL**: Single-threaded for CPU-bound work

### Memory Budget

| Component | Memory |
|-----------|--------|
| Python interpreter | ~50MB |
| Polars library | ~100MB |
| DataFrame buffers | ~100MB per stream |
| ADBC driver | ~50MB |
| **Peak** | **~300MB during ETL** |

### Performance Estimate

Based on [Polars benchmarks](https://pola.rs/posts/benchmarks/):
- Parquet scan: ~400K rows/second (lazy)
- ADBC write: ~80K rows/second
- Expected daily load time: <45 seconds for all streams

---

## Approach 4: TimescaleDB pg_parquet / FDW

### Architecture

```
Bronze (Parquet)              Silver (TimescaleDB)
     |                               ^
     |  parquet_fdw foreign table    |
     v                               |
 PostgreSQL -----> INSERT INTO readings SELECT FROM foreign_parquet
     |
     +-- Pure SQL, runs inside TimescaleDB
```

### Implementation

```sql
-- Install parquet_fdw extension
CREATE EXTENSION parquet_fdw;

-- Create foreign data wrapper
CREATE SERVER parquet_srv FOREIGN DATA WRAPPER parquet_fdw;

-- Define foreign table for air-quality stream
CREATE FOREIGN TABLE bronze_air_quality (
    timestamp BIGINT,
    source_id TEXT,
    ndp_id TEXT,
    context TEXT,
    raw_payload TEXT
) SERVER parquet_srv
OPTIONS (
    filename '/data/raw/air-quality/**/*.parquet',
    sorted 'timestamp'
);

-- ETL procedure
CREATE OR REPLACE FUNCTION etl_air_quality()
RETURNS INTEGER AS $$
DECLARE
    last_ts TIMESTAMPTZ;
    inserted INTEGER;
BEGIN
    SELECT COALESCE(MAX(time), '1970-01-01'::TIMESTAMPTZ) INTO last_ts
    FROM readings WHERE stream_id = 'air-quality';

    INSERT INTO readings (time, stream_id, ndp_id, temperature, humidity, pm25, co2, extra_fields)
    SELECT
        to_timestamp(timestamp / 1000000),
        'air-quality',
        ndp_id,
        (raw_payload::jsonb->>'atmp')::FLOAT,
        (raw_payload::jsonb->>'rhum')::FLOAT,
        (raw_payload::jsonb->>'pm02')::FLOAT,
        (raw_payload::jsonb->>'rco2')::INT,
        raw_payload::jsonb
    FROM bronze_air_quality
    WHERE to_timestamp(timestamp / 1000000) > last_ts
      AND to_timestamp(timestamp / 1000000) <= current_timestamp - INTERVAL '5 minutes'
    ON CONFLICT DO NOTHING;

    GET DIAGNOSTICS inserted = ROW_COUNT;
    RETURN inserted;
END;
$$ LANGUAGE plpgsql;
```

### Pros

1. **Simplest Deployment**: No additional services
2. **Pure SQL**: Familiar PostgreSQL patterns
3. **Low Memory**: Extension runs within existing TimescaleDB container
4. **Atomic**: Transaction semantics built-in

### Cons

1. **Extension Availability**: [parquet_fdw](https://github.com/adjust/parquet_fdw) may not be in TimescaleDB image
2. **Performance**: FDW overhead, no columnar pushdown
3. **Limited Features**: Basic Parquet support only
4. **ARM Compatibility**: May require building from source for aarch64

### Memory Budget

| Component | Memory |
|-----------|--------|
| TimescaleDB (existing) | 256MB limit |
| parquet_fdw buffers | ~50MB |
| **Peak** | **~50MB additional** |

### Performance Estimate

Based on [pg_parquet research](https://www.crunchydata.com/blog/pg_parquet-an-extension-to-connect-postgres-and-parquet):
- Parquet scan via FDW: ~100K rows/second
- No columnar pushdown: Full scan required
- Expected daily load time: <2 minutes for all streams

---

## Scheduling Options

### Option A: Cron (Simple)

```bash
# /etc/cron.d/ndp-etl
# Run ETL every hour at :05 past
5 * * * * root /usr/local/bin/duckdb -c ".read /opt/ndp/etl/bronze_to_silver.sql" >> /var/log/ndp-etl.log 2>&1
```

**Pros**: Simple, universally available, no systemd dependency
**Cons**: No missed job handling, basic logging

### Option B: Systemd Timer (Recommended)

```ini
# /etc/systemd/system/ndp-etl.service
[Unit]
Description=NDP Bronze to Silver ETL
After=timescaledb.service

[Service]
Type=oneshot
ExecStart=/usr/local/bin/duckdb -c ".read /opt/ndp/etl/bronze_to_silver.sql"
StandardOutput=journal
StandardError=journal

# /etc/systemd/system/ndp-etl.timer
[Unit]
Description=Run NDP ETL hourly

[Timer]
OnCalendar=*:05:00
Persistent=true
RandomizedDelaySec=60

[Install]
WantedBy=timers.target
```

**Pros**:
- [Persistent=true catches missed jobs](https://akashrajpurohit.com/blog/systemd-timers-vs-cron-jobs/)
- Integrated journald logging
- RandomizedDelaySec prevents thundering herd
- `systemctl list-timers` for visibility

**Cons**: More complex setup

### Option C: Event-Driven (On New Partition)

```bash
# Using inotifywait to trigger on new Parquet files
inotifywait -m -r -e close_write /data/raw/ |
while read directory event filename; do
    if [[ "$filename" == *.parquet ]]; then
        /usr/local/bin/ndp-etl-trigger.sh "$directory" "$filename"
    fi
done
```

**Pros**: Near real-time, no polling
**Cons**: Complex, may trigger multiple times, inotify limits

### Scheduling Recommendation

**Hourly Systemd Timer** is the best balance:
- Matches typical dashboard refresh patterns
- Catches up after downtime (Persistent=true)
- Low overhead on Pi
- Easy monitoring via journald

---

## Memory Budget Analysis

### Current Production Memory Usage

| Service | Limit | Actual |
|---------|-------|--------|
| mosquitto | 128MB | ~50MB |
| etcd | 256MB | ~100MB |
| air-quality-app | 512MB | ~200MB |
| DuckDB HTTP | - | Removed (DP-001 uses Grafana DuckDB plugin) |
| Grafana | 256MB | ~150MB |
| TimescaleDB | 256MB | ~128MB |
| **Total** | **1408MB** | **~628MB** |

### With ETL Process

| Scenario | Peak Memory | Headroom |
|----------|-------------|----------|
| DuckDB ETL (hourly) | +200MB | 15GB - 828MB = 14.2GB |
| Rust ETL | +110MB | 15GB - 738MB = 14.3GB |
| Python ETL | +300MB | 15GB - 928MB = 14.1GB |
| FDW ETL | +50MB | 15GB - 678MB = 14.3GB |

**All approaches fit comfortably within Pi 5 16GB constraints.**

---

## Recovery and Replay Scenarios

### Scenario 1: TimescaleDB Rebuild

If Silver layer needs complete rebuild from Bronze:

| Approach | Recovery Time (90 days) | Complexity |
|----------|------------------------|------------|
| DuckDB ETL | ~15 minutes | Low (adjust date filter) |
| Rust ETL | ~10 minutes | Medium (add bulk mode) |
| Python ETL | ~20 minutes | Low (adjust watermark) |
| FDW ETL | ~45 minutes | Low (remove watermark check) |

### Scenario 2: Late-Arriving Data

Bronze data arrives out of order (e.g., device was offline):

| Approach | Handling | Quality |
|----------|----------|---------|
| DuckDB ETL | Use wider time window | Good |
| Rust ETL | Configurable watermark slack | Excellent |
| Python ETL | Configurable lag parameter | Good |
| FDW ETL | No special handling | Poor |

### Scenario 3: Schema Evolution

New field added to Bronze raw_payload:

| Approach | Impact | Migration |
|----------|--------|-----------|
| DuckDB ETL | Update SQL | Zero downtime |
| Rust ETL | Code change + recompile | Requires deployment |
| Python ETL | Update transform function | Zero downtime |
| FDW ETL | Update foreign table | Zero downtime |

---

## Implementation Complexity Assessment

| Task | DuckDB | Rust | Python | FDW |
|------|--------|------|--------|-----|
| Initial setup | 2 hours | 16 hours | 4 hours | 4 hours |
| Per-stream transform | 30 min | 2 hours | 1 hour | 1 hour |
| Watermark tracking | 1 hour | 4 hours | 2 hours | 1 hour |
| Error handling | 1 hour | 4 hours | 2 hours | 1 hour |
| Monitoring/alerting | 1 hour | 2 hours | 1 hour | 1 hour |
| **Total** | **~6 hours** | **~28 hours** | **~10 hours** | **~8 hours** |

---

## Final Recommendation

### Primary: DuckDB ETL with Hourly Systemd Timer

**Rationale:**
1. **Proven Technology**: DuckDB already tested on Pi with 512MB limit
2. **Reuse Existing Work**: SQL views from DP-001 are directly adaptable
3. **Lowest Complexity**: SQL-only implementation, no compilation
4. **Good Performance**: 15x faster than vanilla PostgreSQL ([benchmark](https://motherduck.com/blog/postgres-duckdb-options/))
5. **Flexible Recovery**: Easy to replay any time range

**Implementation Path:**
1. Create `etl/bronze_to_silver.sql` with incremental load logic
2. Configure systemd timer for hourly execution
3. Add Grafana dashboard for ETL monitoring
4. Document recovery procedures

### Fallback: Rust Native ETL

If DuckDB postgres extension proves unreliable or performance insufficient:
1. Implement in existing `platform-core` crate
2. Use sqlx with UNNEST batching for inserts
3. Integrate with air-quality-app or standalone binary
4. Lower memory footprint for tighter resource scenarios

---

## References

### Research Sources
- [DuckDB Performance Guide](https://duckdb.org/docs/stable/guides/performance/overview)
- [DuckDB PostgreSQL Integration](https://motherduck.com/blog/postgres-duckdb-options/)
- [Polars + DuckDB with Postgres](https://aklaver.org/wordpress/2024/03/08/using-polars-duckdb-with-postgres/)
- [Rust PostgreSQL Batching](https://kerkour.com/postgresql-batching)
- [sqlx Bulk Insert](https://www.alxolr.com/articles/rust-bulk-insert-to-postgre-sql-using-sqlx)
- [TimescaleDB Raspberry Pi Benchmark](https://ideia.me/time-series-benchmark-timescaledb-raspberry-pi)
- [pg_parquet Extension](https://www.crunchydata.com/blog/pg_parquet-an-extension-to-connect-postgres-and-parquet)
- [parquet_fdw](https://github.com/adjust/parquet_fdw)
- [Systemd Timers vs Cron](https://akashrajpurohit.com/blog/systemd-timers-vs-cron-jobs/)

### NDP Internal Documents
- `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
- `config/duckdb/views/silver_indoor_air.sql`
- `core/src/storage/parquet.rs`
- `research/agenticdataplatform/07-synthesis-and-recommendations.md`

---

## Appendix A: DuckDB ETL Script Template

```sql
-- etl/bronze_to_silver.sql
-- NDP Bronze to Silver ETL via DuckDB
-- Scheduled: Hourly via systemd timer

-- Configuration
SET memory_limit = '256MB';
SET threads = 2;

-- Load postgres extension
LOAD postgres;

-- Connect to TimescaleDB
ATTACH 'host=localhost port=5432 dbname=ndp user=postgres password=...' AS pg (TYPE postgres);

-- Stream: air-quality
INSERT INTO pg.readings (time, stream_id, ndp_id, temperature, humidity, pm25, pm10, co2, tvoc_index, nox_index, extra_fields)
WITH bronze AS (
    SELECT
        to_timestamp(timestamp / 1000000) as time,
        ndp_id,
        raw_payload::JSON as payload
    FROM read_parquet('/data/raw/air-quality/**/*.parquet', union_by_name=true)
    WHERE to_timestamp(timestamp / 1000000) > (
        SELECT COALESCE(MAX(time), '1970-01-01'::TIMESTAMP)
        FROM pg.readings WHERE stream_id = 'air-quality'
    )
    AND to_timestamp(timestamp / 1000000) <= current_timestamp - INTERVAL '5 minutes'
)
SELECT
    time,
    'air-quality' as stream_id,
    ndp_id,
    CASE WHEN json_extract(payload, '$.atmp')::FLOAT BETWEEN -10 AND 50
         THEN ROUND(json_extract(payload, '$.atmp')::FLOAT, 1) END,
    CASE WHEN json_extract(payload, '$.rhum')::FLOAT BETWEEN 0 AND 100
         THEN ROUND(json_extract(payload, '$.rhum')::FLOAT, 1) END,
    CASE WHEN json_extract(payload, '$.pm02')::FLOAT BETWEEN 0 AND 500
         THEN ROUND(json_extract(payload, '$.pm02')::FLOAT, 1) END,
    CASE WHEN json_extract(payload, '$.pm10')::FLOAT BETWEEN 0 AND 1000
         THEN ROUND(json_extract(payload, '$.pm10')::FLOAT, 1) END,
    CASE WHEN json_extract(payload, '$.rco2')::INT BETWEEN 400 AND 5000
         THEN json_extract(payload, '$.rco2')::INT END,
    CASE WHEN json_extract(payload, '$.tvocIndex')::INT BETWEEN 0 AND 500
         THEN json_extract(payload, '$.tvocIndex')::INT END,
    CASE WHEN json_extract(payload, '$.noxIndex')::INT BETWEEN 0 AND 500
         THEN json_extract(payload, '$.noxIndex')::INT END,
    payload
FROM bronze;

-- Report results
SELECT 'air-quality ETL complete' as status,
       (SELECT COUNT(*) FROM pg.readings WHERE stream_id = 'air-quality') as total_rows,
       current_timestamp as run_time;
```

---

## Appendix B: TimescaleDB Schema

```sql
-- Silver layer schema for TimescaleDB
-- Feature: DP-006

-- Enable TimescaleDB
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Main readings hypertable
CREATE TABLE IF NOT EXISTS readings (
    time TIMESTAMPTZ NOT NULL,
    stream_id TEXT NOT NULL,
    ndp_id TEXT,

    -- Common measurement columns
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    pressure DOUBLE PRECISION,

    -- Air quality measurements
    pm25 DOUBLE PRECISION,
    pm10 DOUBLE PRECISION,
    co2 INTEGER,
    tvoc_index INTEGER,
    nox_index INTEGER,
    aqi INTEGER,

    -- Weather measurements
    wind_speed DOUBLE PRECISION,
    wind_direction INTEGER,
    precipitation DOUBLE PRECISION,
    cloud_cover INTEGER,
    visibility INTEGER,

    -- Flexible storage for non-standard fields
    extra_fields JSONB,

    -- Deduplication constraint
    UNIQUE (time, stream_id, ndp_id)
);

-- Convert to hypertable
SELECT create_hypertable('readings', 'time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_readings_stream_time
    ON readings (stream_id, time DESC);
CREATE INDEX IF NOT EXISTS idx_readings_ndp_id
    ON readings (ndp_id, time DESC) WHERE ndp_id IS NOT NULL;

-- Retention policy: 90 days raw data
SELECT add_retention_policy('readings', INTERVAL '90 days', if_not_exists => TRUE);

-- Continuous aggregate: hourly rollups
CREATE MATERIALIZED VIEW IF NOT EXISTS readings_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    stream_id,
    AVG(temperature) AS avg_temperature,
    AVG(humidity) AS avg_humidity,
    AVG(pm25) AS avg_pm25,
    MAX(pm25) AS max_pm25,
    AVG(co2) AS avg_co2,
    COUNT(*) AS sample_count
FROM readings
GROUP BY bucket, stream_id
WITH NO DATA;

-- Refresh policy for continuous aggregate
SELECT add_continuous_aggregate_policy('readings_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- Retention for hourly aggregates: 1 year
SELECT add_retention_policy('readings_hourly', INTERVAL '365 days', if_not_exists => TRUE);
```
