# Traditional Gold Layer Patterns for Neural Data Platform

**Research Date**: 2026-02-02
**Author**: Research Agent
**Context**: Bronze (Parquet/WAL) -> Silver (TimescaleDB) -> Gold architecture on Raspberry Pi 5
**Status**: Complete

---

## Executive Summary

This research examines traditional Gold layer patterns in data lake architectures, with specific focus on time-series/IoT data and resource-efficient approaches suitable for Raspberry Pi edge deployment. The Gold layer represents the final, consumption-ready stage of data transformation, optimized for specific analytical workloads such as dashboards, reports, and machine learning.

### Key Findings

| Aspect | Traditional Pattern | NDP Recommendation |
|--------|--------------------|--------------------|
| **Primary Purpose** | Business-ready, denormalized data | ML-ready features + dashboard aggregates |
| **Schema Design** | Star schema or wide tables | Hybrid: continuous aggregates + wide feature tables |
| **Storage** | Physical tables (materialized) | TimescaleDB continuous aggregates (automatic) |
| **Refresh Pattern** | Batch ETL (scheduled) | Incremental (continuous aggregates auto-refresh) |
| **Aggregation Levels** | Hourly/Daily/Weekly/Monthly | 10min/1hour/1day (time-series optimized) |
| **Resource Efficiency** | Cloud-scale focus | Edge-first: <100MB memory footprint |

### Recommendation Summary

For NDP's resource-constrained Raspberry Pi deployment:

1. **Use TimescaleDB continuous aggregates** as the primary Gold layer mechanism
2. **Implement hierarchical aggregates** (raw -> 10min -> hourly -> daily)
3. **Create wide feature tables** for ML inference (denormalized, pre-computed)
4. **Apply compression** to historical Gold data (TimescaleDB native compression)
5. **Avoid** traditional star schema complexity (overkill for edge deployment)

---

## 1. Gold Layer Fundamentals

### 1.1 Definition and Purpose

The Gold layer in medallion architecture represents **consumption-ready, business-optimized data**. According to [Databricks](https://www.databricks.com/glossary/medallion-architecture), it is the final stage where:

> "Data is refined and business-ready, ready for deeper insights, AI models, and decision-making."

**Core Characteristics**:
- **Highly denormalized**: Optimized for read performance, minimal joins
- **Aggregated**: Pre-computed KPIs, metrics, and business logic
- **Project-specific**: Tailored to specific consumption patterns
- **Read-optimized**: Designed for dashboard and reporting queries

### 1.2 Gold vs Silver: Key Differences

| Aspect | Silver Layer | Gold Layer |
|--------|--------------|------------|
| **Audience** | Data engineers, analysts | Business users, ML systems |
| **Granularity** | Full detail (individual readings) | Aggregated (hourly, daily) |
| **Schema** | Normalized or 3NF | Denormalized, star schema, or wide tables |
| **Joins** | Required for analysis | Pre-joined, minimal at query time |
| **Transformations** | Data quality, cleaning | Business logic, KPIs |
| **Retention** | Full history | Often recent/relevant data only |
| **Update Frequency** | Near real-time | Scheduled/incremental refresh |

### 1.3 Traditional Gold Layer Contents

Based on industry patterns ([Microsoft Learn](https://learn.microsoft.com/en-us/azure/databricks/lakehouse/medallion), [Weld Blog](https://weld.app/blog/medallion-layers)):

1. **Dimensional Models (Kimball)**
   - Fact tables (measurements, transactions)
   - Dimension tables (time, location, sensor)
   - Star or snowflake schema

2. **Data Marts**
   - Subject-area specific datasets
   - Customer analytics, product analytics
   - Pre-aggregated for BI tools

3. **Feature Stores**
   - ML-ready feature vectors
   - Historical feature values (for training)
   - Latest feature values (for inference)

4. **Reporting Tables**
   - Executive dashboards
   - Operational reports
   - KPI tracking

---

## 2. Traditional Modeling Approaches

### 2.1 Star Schema

The traditional dimensional modeling approach, popularized by Ralph Kimball.

```
                    ┌─────────────────┐
                    │   dim_time      │
                    │ - hour_of_day   │
                    │ - day_of_week   │
                    │ - month         │
                    │ - is_weekend    │
                    └────────┬────────┘
                             │
┌─────────────────┐         │         ┌─────────────────┐
│  dim_location   │         │         │   dim_sensor    │
│ - location_id   │◄────────┼────────►│ - sensor_id     │
│ - room_name     │         │         │ - sensor_type   │
│ - floor         │         │         │ - manufacturer  │
│ - building      │   ┌─────┴─────┐   │ - calibration   │
└─────────────────┘   │           │   └─────────────────┘
                      │ fact_air  │
                      │ _quality  │
                      │           │
                      │- timestamp│
                      │- pm25_avg │
                      │- temp_avg │
                      │- humidity │
                      │- aqi      │
                      └───────────┘
```

**Pros**:
- Clear separation of facts and dimensions
- Business users understand the model
- Predictable BI tool behavior
- Handles slowly changing dimensions (SCD)

**Cons**:
- Join overhead at query time
- Complex ETL for dimension management
- Overkill for simple IoT scenarios
- Higher storage (dimension redundancy)

### 2.2 Wide Tables (One Big Table / OBT)

Fully denormalized single tables with all attributes pre-joined.

```sql
-- Wide table example for air quality
CREATE TABLE gold.air_quality_wide AS
SELECT
    -- Time attributes (denormalized)
    timestamp,
    EXTRACT(HOUR FROM timestamp) AS hour_of_day,
    EXTRACT(DOW FROM timestamp) AS day_of_week,
    DATE_TRUNC('day', timestamp) AS day,

    -- Location attributes (denormalized)
    location_id,
    'Living Room' AS room_name,  -- Pre-joined
    'Ground Floor' AS floor,

    -- Sensor attributes (denormalized)
    sensor_id,
    'AirGradient' AS manufacturer,

    -- Metrics
    pm25,
    temperature,
    humidity,
    co2,

    -- Pre-computed features
    pm25_rolling_4h_avg,
    pm25_rolling_24h_avg,
    temp_diff_from_outdoor,
    aqi_category

FROM silver.air_quality_readings
JOIN dim_location USING (location_id)
JOIN dim_sensor USING (sensor_id);
```

**Pros**:
- **25-50% faster queries** than star schema with joins ([Fivetran study](https://www.fivetran.com/blog/star-schema-vs-obt))
- Zero join overhead at query time
- Simpler SQL for end users
- Works well with columnar storage (only scan needed columns)

**Cons**:
- Storage redundancy (repeated dimension values)
- Expensive updates (customer email change -> many row updates)
- Less flexible for changing requirements
- Doesn't handle SCD well

### 2.3 Performance Comparison

From [Datameer analysis](https://www.datameer.com/blog/snowflake-vs-star-vs-wide-table-schema-a-performance-comparison/):

| Schema Type | Query Speed | Storage | Flexibility | Maintenance |
|-------------|-------------|---------|-------------|-------------|
| Star Schema | Medium | Medium | High | Medium |
| Snowflake Schema | Slower | Low | Highest | High |
| Wide Table | **Fastest** | High | Low | Low |

**Recommendation for IoT/Time-Series**: Wide tables are preferred because:
1. IoT data rarely changes (append-only)
2. Dimensions are stable (sensor, location)
3. Query performance is critical for real-time dashboards
4. ML feature serving needs fast lookups

---

## 3. Time-Series Specific Patterns

### 3.1 Continuous Aggregates (TimescaleDB)

TimescaleDB continuous aggregates are purpose-built for time-series Gold layers. According to [TimescaleDB documentation](https://github.com/timescale/docs.timescale.com-content/blob/master/using-timescaledb/continuous-aggregates.md):

> "A continuous aggregate is an incrementally and automatically updated materialized view for an aggregate query over a hypertable."

**Key Advantages**:

1. **Automatic Incremental Refresh**
   - Only new/changed data is processed
   - 1,000x faster than full refresh ([Timescale Blog](https://www.timescale.com/blog/how-postgresql-views-and-materialized-views-work-and-how-they-influenced-timescaledb-continuous-aggregates/))

2. **Real-Time Aggregation**
   - Combines pre-aggregated data with latest raw data
   - Always up-to-date results

3. **Hierarchical Aggregates**
   - Stack aggregates on aggregates (e.g., hourly -> daily)
   - Progressively faster queries at coarser granularity

4. **Independent of Source Data**
   - Can drop underlying hypertable
   - Historical aggregates preserved

**Implementation Pattern**:

```sql
-- Level 1: 10-minute aggregates (closest to raw)
CREATE MATERIALIZED VIEW gold.air_quality_10min
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('10 minutes', timestamp) AS bucket,
    sensor_id,
    location_id,
    AVG(pm25) AS pm25_avg,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,
    STDDEV(pm25) AS pm25_stddev,
    AVG(temperature) AS temp_avg,
    AVG(humidity) AS humidity_avg,
    COUNT(*) AS sample_count
FROM silver.air_quality_readings
GROUP BY bucket, sensor_id, location_id;

-- Auto-refresh every 5 minutes
SELECT add_continuous_aggregate_policy('gold.air_quality_10min',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes');

-- Level 2: Hourly aggregates (for dashboards)
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', bucket) AS bucket,
    sensor_id,
    location_id,
    AVG(pm25_avg) AS pm25_avg,
    MAX(pm25_max) AS pm25_max,
    MIN(pm25_min) AS pm25_min,
    AVG(temp_avg) AS temp_avg,
    SUM(sample_count) AS sample_count
FROM gold.air_quality_10min
GROUP BY time_bucket('1 hour', bucket), sensor_id, location_id;

-- Level 3: Daily aggregates (for trends, ML training)
CREATE MATERIALIZED VIEW gold.air_quality_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', bucket) AS bucket,
    sensor_id,
    location_id,
    AVG(pm25_avg) AS pm25_avg,
    MAX(pm25_max) AS pm25_max,
    MIN(pm25_min) AS pm25_min,
    AVG(temp_avg) AS temp_avg,
    SUM(sample_count) AS sample_count
FROM gold.air_quality_hourly
GROUP BY time_bucket('1 day', bucket), sensor_id, location_id;
```

### 3.2 Materialized Views vs Physical Tables

| Aspect | Continuous Aggregates | Physical Gold Tables |
|--------|----------------------|---------------------|
| **Refresh** | Automatic, incremental | Manual ETL job |
| **Latency** | Near real-time (minutes) | Batch (hourly/daily) |
| **Storage** | TimescaleDB managed | Explicit allocation |
| **Compression** | Supported (since 2.6) | Manual |
| **Flexibility** | SQL-defined aggregates | Full ETL control |
| **Complexity** | Low (SQL only) | High (ETL pipeline) |
| **Resource Usage** | Minimal (incremental) | Higher (full scans) |

**Recommendation**: Use continuous aggregates as the primary Gold mechanism for NDP. Physical tables only for complex ML features that require procedural logic.

### 3.3 Aggregation Granularity Selection

For time-series IoT data, aggregation levels should match use cases:

| Granularity | Use Case | Retention | Storage |
|-------------|----------|-----------|---------|
| **10 minutes** | Real-time dashboards, alerting | 7 days | ~10MB/week |
| **1 hour** | Daily monitoring, trend analysis | 90 days | ~5MB/quarter |
| **1 day** | Long-term trends, ML training | 2 years | ~1MB/year |
| **1 week/month** | Historical reports, capacity planning | Forever | Minimal |

**NDP Recommendation**:
```
Raw (Silver) -> 10-min -> Hourly -> Daily
     |              |         |        |
  7 days        7 days   90 days   2 years
  (raw)       (detailed) (trends)  (history)
```

---

## 4. Feature Store Patterns

### 4.1 What is a Feature Store?

A feature store is a centralized repository for ML features, bridging data engineering and ML pipelines. According to [Databricks](https://www.databricks.com/blog/what-feature-store-complete-guide-ml-feature-engineering):

> "A feature store serves as a dedicated place where features—the input variables to ML models—are stored, curated, and made available for both model training and model serving."

### 4.2 Dual-Store Architecture

Feature stores use a dual-layer pattern ([Aerospike](https://aerospike.com/blog/feature-store/), [Dragonfly](https://www.dragonflydb.io/blog/feature-store-architecture-and-storage)):

```
┌─────────────────────────────────────────────────────────────────────┐
│                       FEATURE STORE                                  │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    OFFLINE STORE                              │  │
│  │  Purpose: ML training, batch inference, historical lookups    │  │
│  │  Storage: Data lake (Parquet/Delta), warehouse (BigQuery)     │  │
│  │  Latency: Seconds to minutes                                  │  │
│  │  Size: Terabytes of feature history                           │  │
│  │                                                               │  │
│  │  NDP: TimescaleDB Gold tables (continuous aggregates)         │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│                              │ Materialization                      │
│                              ▼                                      │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                    ONLINE STORE                               │  │
│  │  Purpose: Real-time inference, low-latency lookups            │  │
│  │  Storage: Key-value store (Redis, DynamoDB)                   │  │
│  │  Latency: Milliseconds (<10ms)                                │  │
│  │  Size: Latest feature values per entity                       │  │
│  │                                                               │  │
│  │  NDP: Redis cache (optional) or direct TimescaleDB query      │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.3 Feature Types for Time-Series

| Feature Type | Computation | Storage | Example |
|--------------|-------------|---------|---------|
| **Point-in-Time** | Direct lookup | Offline | PM2.5 reading at 3pm |
| **Rolling Window** | Aggregation | Both | Avg PM2.5 last 4 hours |
| **Lag Features** | Shift operation | Offline | PM2.5 24 hours ago |
| **Cross-Stream** | Join + aggregate | Offline | Indoor vs outdoor diff |
| **Derived** | Business logic | Both | AQI category, health risk |
| **Time-Based** | Extract from timestamp | Both | Hour of day, is_weekend |

### 4.4 NDP Feature Table Design

```sql
-- Gold feature table for ML inference
CREATE TABLE gold.ml_features (
    -- Keys
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,

    -- Current values
    pm25_current DOUBLE PRECISION,
    temp_current DOUBLE PRECISION,
    humidity_current DOUBLE PRECISION,
    co2_current DOUBLE PRECISION,

    -- Rolling window features (4-hour)
    pm25_mean_4h DOUBLE PRECISION,
    pm25_std_4h DOUBLE PRECISION,
    pm25_max_4h DOUBLE PRECISION,
    pm25_trend_4h DOUBLE PRECISION,  -- linear regression slope

    -- Rolling window features (24-hour)
    pm25_mean_24h DOUBLE PRECISION,
    pm25_std_24h DOUBLE PRECISION,
    pm25_p95_24h DOUBLE PRECISION,   -- 95th percentile

    -- Lag features
    pm25_lag_1h DOUBLE PRECISION,
    pm25_lag_6h DOUBLE PRECISION,
    pm25_lag_24h DOUBLE PRECISION,

    -- Cross-stream features
    pm25_indoor_outdoor_diff DOUBLE PRECISION,
    temp_indoor_outdoor_diff DOUBLE PRECISION,
    outdoor_wind_speed DOUBLE PRECISION,
    outdoor_aqi DOUBLE PRECISION,

    -- Time features
    hour_of_day INTEGER,
    day_of_week INTEGER,
    is_weekend BOOLEAN,

    -- Derived features
    aqi_category TEXT,
    health_risk_level TEXT,
    dewpoint_c DOUBLE PRECISION,

    PRIMARY KEY (timestamp, location_id)
);

SELECT create_hypertable('gold.ml_features', 'timestamp');
```

### 4.5 Point-in-Time Feature Joins

For ML training, features must be joined correctly to avoid data leakage. From [Databricks documentation](https://docs.databricks.com/aws/en/machine-learning/feature-store/time-series):

> "To perform a point-in-time lookup for feature values from a time series feature table, you must specify a timestamp_lookup_key."

**Pattern for NDP**:

```sql
-- Training dataset with point-in-time correct features
WITH training_labels AS (
    -- Target variable (e.g., PM2.5 1 hour in future)
    SELECT
        timestamp AS prediction_time,
        location_id,
        pm25 AS target_pm25_1h_ahead
    FROM silver.air_quality_readings
    WHERE timestamp BETWEEN '2025-01-01' AND '2025-12-31'
)
SELECT
    t.prediction_time,
    t.location_id,
    t.target_pm25_1h_ahead,

    -- Features as of prediction_time (no leakage)
    f.pm25_mean_4h,
    f.pm25_std_4h,
    f.pm25_lag_1h,
    f.hour_of_day,
    f.outdoor_aqi

FROM training_labels t
LEFT JOIN LATERAL (
    SELECT *
    FROM gold.ml_features f
    WHERE f.location_id = t.location_id
      AND f.timestamp <= t.prediction_time
    ORDER BY f.timestamp DESC
    LIMIT 1
) f ON true;
```

---

## 5. Resource-Efficient Approaches for Edge Deployment

### 5.1 Raspberry Pi Constraints

| Resource | Pi 5 Capacity | NDP Budget | Constraint |
|----------|---------------|------------|------------|
| **RAM** | 16 GB | <2 GB for analytics | Memory-bound |
| **CPU** | 4 cores @ 2.4 GHz | Shared with ingestion | CPU-limited |
| **Storage** | NVMe SSD | Depends on capacity | Generally OK |
| **Power** | 5V, 5A | Always-on operation | Thermal limits |

### 5.2 Edge-Optimized Gold Patterns

Based on research ([Pidora](https://pidora.ca/edge-computing-makes-your-raspberry-pi-10x-faster-heres-where-data-processing-happens/), [MDPI](https://www.mdpi.com/2079-9292/5/2/29)):

**Pattern 1: Hierarchical Aggregates with Auto-Compression**

```sql
-- Enable compression on Gold tables
ALTER TABLE gold.air_quality_hourly SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id',
    timescaledb.compress_orderby = 'bucket DESC'
);

-- Auto-compress data older than 7 days
SELECT add_compression_policy('gold.air_quality_hourly',
    INTERVAL '7 days');

-- Compression achieves 10-90% storage reduction
-- Also improves query performance on historical data
```

**Pattern 2: Retention Policies**

```sql
-- Keep 10-minute aggregates for 7 days only
SELECT add_retention_policy('gold.air_quality_10min',
    INTERVAL '7 days');

-- Keep hourly aggregates for 90 days
SELECT add_retention_policy('gold.air_quality_hourly',
    INTERVAL '90 days');

-- Daily aggregates: no retention (keep forever, compressed)
```

**Pattern 3: Memory-Efficient Refresh**

```sql
-- Refresh only recent data (bounded memory usage)
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',   -- Look back max 4 hours
    end_offset => INTERVAL '10 minutes',  -- Leave 10-min buffer
    schedule_interval => INTERVAL '15 minutes'
);
```

### 5.3 Resource Usage Estimates

| Gold Component | Memory Usage | CPU Usage | Storage/Month |
|----------------|-------------|-----------|---------------|
| Continuous aggregate refresh | ~50-100 MB (peak) | <5% avg | N/A |
| 10-minute aggregates | ~5 MB in memory | Minimal | ~40 MB |
| Hourly aggregates | ~2 MB in memory | Minimal | ~10 MB |
| Daily aggregates | <1 MB in memory | Minimal | ~1 MB |
| ML feature table | ~10 MB in memory | Minimal | ~20 MB |
| **Total Gold Layer** | **<100 MB** | **<5%** | **~70 MB/month** |

### 5.4 Avoiding Over-Engineering

For edge deployment, **avoid** traditional enterprise patterns:

| Pattern to Avoid | Why | Alternative |
|-----------------|-----|-------------|
| Full star schema | Too many joins, complex ETL | Wide tables or continuous aggregates |
| Separate dimension tables | Unnecessary for stable IoT dimensions | Inline dimension attributes |
| Complex SCD handling | IoT dimensions rarely change | Simple versioning if needed |
| Real-time streaming to Gold | Resource-intensive | Micro-batch via continuous aggregates |
| Dedicated feature store infrastructure | Overkill for single-node | TimescaleDB + optional Redis cache |

---

## 6. Comparison of Approaches

### 6.1 Decision Matrix for NDP

| Approach | Query Performance | Storage Efficiency | Implementation Complexity | Edge Suitability |
|----------|-------------------|-------------------|---------------------------|------------------|
| **Continuous Aggregates** | Excellent | Excellent | Low | Excellent |
| Physical Wide Tables | Excellent | Poor | Medium | Good |
| Star Schema | Good | Good | High | Poor |
| Streaming Aggregates | Excellent | Poor | High | Poor |
| External Feature Store | Excellent | Variable | High | Poor |

### 6.2 Pros and Cons Summary

#### Continuous Aggregates (Recommended)

**Pros**:
- Automatic incremental refresh
- Built into TimescaleDB (no extra infrastructure)
- Hierarchical aggregates supported
- Compression support
- Real-time aggregation option
- Minimal resource usage

**Cons**:
- Limited to SQL-expressible aggregates
- No complex procedural logic
- Requires TimescaleDB (already in NDP stack)

#### Physical Wide Tables

**Pros**:
- Maximum query flexibility
- Can include complex derived features
- Full control over ETL logic

**Cons**:
- Manual refresh required
- Higher storage usage
- More complex ETL pipeline
- No automatic compression

#### Star Schema

**Pros**:
- Industry standard, well-understood
- Handles slowly changing dimensions
- Flexible for ad-hoc analysis

**Cons**:
- Join overhead at query time
- Complex dimension management
- Overkill for simple IoT scenarios
- Higher development effort

---

## 7. Recommendations for NDP

### 7.1 Recommended Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    NDP GOLD LAYER ARCHITECTURE                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                  SILVER LAYER (TimescaleDB)                    │ │
│  │  - air_quality_readings (hypertable)                          │ │
│  │  - outdoor_weather_readings (hypertable)                      │ │
│  │  - outdoor_air_quality_readings (hypertable)                  │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                              │                                      │
│                              ▼                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │              GOLD LAYER - TIER 1: Aggregates                   │ │
│  │              (Continuous Aggregates, Auto-Refresh)             │ │
│  │                                                               │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐│ │
│  │  │ air_quality_10m │  │ weather_hourly  │  │ aqi_hourly     ││ │
│  │  │ (dashboards)    │  │ (dashboards)    │  │ (dashboards)   ││ │
│  │  └────────┬────────┘  └────────┬────────┘  └───────┬────────┘│ │
│  │           │                    │                    │         │ │
│  │           ▼                    ▼                    ▼         │ │
│  │  ┌─────────────────┐  ┌─────────────────┐  ┌────────────────┐│ │
│  │  │ air_quality_1h  │  │ weather_daily   │  │ aqi_daily      ││ │
│  │  │ (trends)        │  │ (history)       │  │ (history)      ││ │
│  │  └────────┬────────┘  └─────────────────┘  └────────────────┘│ │
│  │           │                                                   │ │
│  │           ▼                                                   │ │
│  │  ┌─────────────────┐                                         │ │
│  │  │ air_quality_1d  │                                         │ │
│  │  │ (long-term)     │                                         │ │
│  │  └─────────────────┘                                         │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                              │                                      │
│                              ▼                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │              GOLD LAYER - TIER 2: Cross-Stream                 │ │
│  │              (Continuous Aggregates, Cross-Join)               │ │
│  │                                                               │ │
│  │  ┌───────────────────────────────────────────────────────┐   │ │
│  │  │ cross_stream_hourly                                    │   │ │
│  │  │ - Indoor vs Outdoor comparisons                        │   │ │
│  │  │ - Weather context for air quality                      │   │ │
│  │  │ - Combined AQI metrics                                 │   │ │
│  │  └───────────────────────────────────────────────────────┘   │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                              │                                      │
│                              ▼                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │              GOLD LAYER - TIER 3: ML Features                  │ │
│  │              (Physical Table, Scheduled ETL)                   │ │
│  │                                                               │ │
│  │  ┌───────────────────────────────────────────────────────┐   │ │
│  │  │ ml_features                                            │   │ │
│  │  │ - Rolling window features (4h, 24h)                    │   │ │
│  │  │ - Lag features (1h, 6h, 24h)                           │   │ │
│  │  │ - Cross-stream derived features                        │   │ │
│  │  │ - Time-based features (hour, day, weekend)             │   │ │
│  │  │ - Health risk categorizations                          │   │ │
│  │  └───────────────────────────────────────────────────────┘   │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                              │                                      │
│                              ▼                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                      CONSUMERS                                 │ │
│  │                                                               │ │
│  │  ┌───────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐  │ │
│  │  │  Grafana  │  │ ruv-FANN  │  │  Alerts   │  │  Exports  │  │ │
│  │  │ Dashboards│  │ Inference │  │  Engine   │  │ (CSV/API) │  │ │
│  │  └───────────┘  └───────────┘  └───────────┘  └───────────┘  │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 7.2 Implementation Phases

#### Phase 1: Dashboard Aggregates (fe-001)

**Goal**: Fast Grafana queries via pre-computed aggregates.

```sql
-- Per-stream 10-minute and hourly aggregates
-- Automatic refresh, compression, retention policies
```

**Deliverables**:
- `gold.air_quality_10min` continuous aggregate
- `gold.air_quality_hourly` continuous aggregate
- `gold.air_quality_daily` continuous aggregate
- Grafana dashboards using Gold layer

#### Phase 2: Cross-Stream Aggregates (fe-002)

**Goal**: Combined indoor/outdoor analysis.

```sql
-- Cross-stream aligned aggregates
-- Weather + AQI context for indoor readings
```

**Deliverables**:
- `gold.cross_stream_hourly` continuous aggregate
- Indoor vs Outdoor comparison dashboard
- Health context alerts

#### Phase 3: ML Feature Layer (fe-003)

**Goal**: ML-ready features for ruv-FANN.

```sql
-- Physical feature table with complex derived features
-- ETL job for rolling windows and lag features
```

**Deliverables**:
- `gold.ml_features` hypertable
- Feature ETL pipeline (Rust + DuckDB)
- Training data export (Parquet)
- Inference feature serving

### 7.3 Schema Recommendations

#### Naming Conventions

```
gold.{stream}_{granularity}      -- Aggregates
gold.cross_stream_{granularity} -- Cross-stream
gold.ml_features                -- ML feature table
gold.{domain}_features          -- Domain-specific features
```

#### Column Naming

```
{metric}_{aggregation}_{window}  -- pm25_avg_4h, temp_max_24h
{metric}_lag_{hours}h            -- pm25_lag_1h, pm25_lag_24h
{derived}_category               -- aqi_category, health_risk
{time}_of_{period}               -- hour_of_day, day_of_week
is_{boolean}                     -- is_weekend, is_heating_season
```

### 7.4 Resource Budget

| Component | Memory | CPU | Storage/Month |
|-----------|--------|-----|---------------|
| Tier 1: Aggregates | 30 MB | 2% | 50 MB |
| Tier 2: Cross-Stream | 10 MB | 1% | 10 MB |
| Tier 3: ML Features | 20 MB | 2% | 20 MB |
| ETL Process (peak) | 100 MB | 5% | N/A |
| **Total Gold Layer** | **<100 MB** | **<5%** | **~80 MB** |

---

## 8. Sources

### Medallion Architecture
- [Databricks: What is a Medallion Architecture?](https://www.databricks.com/glossary/medallion-architecture)
- [Microsoft Learn: What is the medallion lakehouse architecture?](https://learn.microsoft.com/en-us/azure/databricks/lakehouse/medallion)
- [Weld Blog: Medallion Layers](https://weld.app/blog/medallion-layers)
- [Microsoft Fabric: Implement medallion lakehouse architecture](https://learn.microsoft.com/en-us/fabric/onelake/onelake-medallion-lakehouse-architecture)
- [Medium: What goes into bronze, silver, and gold layers](https://lakshmanok.medium.com/what-goes-into-bronze-silver-and-gold-layers-of-a-medallion-data-architecture-4b6fdfb405fc)
- [Chaos Genius: Medallion Architecture 101](https://www.chaosgenius.io/blog/medallion-architecture/)

### Data Modeling
- [Fivetran: Star Schema vs OBT](https://www.fivetran.com/blog/star-schema-vs-obt)
- [Datameer: Schema Performance Comparison](https://www.datameer.com/blog/snowflake-vs-star-vs-wide-table-schema-a-performance-comparison/)
- [Medium: Wide table vs dimensional modelling](https://medium.com/@iyi_bobby/data-warehouse-analytics-requirements-wide-table-vs-dimensional-modelling-a46ae6f61807)
- [Hightouch: Data Warehouse Modelling Guide](https://hightouch.com/blog/data-warehouse-modelling-part-2)

### TimescaleDB & Continuous Aggregates
- [TimescaleDB Continuous Aggregates Documentation](https://github.com/timescale/docs.timescale.com-content/blob/master/using-timescaledb/continuous-aggregates.md)
- [Timescale Blog: How Continuous Aggregates Work](https://www.timescale.com/blog/how-postgresql-views-and-materialized-views-work-and-how-they-influenced-timescaledb-continuous-aggregates/)
- [TigerData: Real-Time Analytics with Continuous Aggregates](https://www.tigerdata.com/blog/real-time-analytics-for-time-series-continuous-aggregates)
- [HackerNoon: From Materialized Views to Continuous Aggregates](https://hackernoon.com/from-materialized-views-to-continuous-aggregates-enhancing-postgresql-with-real-time-analytics)

### Feature Stores
- [Databricks: What is a Feature Store](https://www.databricks.com/blog/what-feature-store-complete-guide-ml-feature-engineering)
- [JFrog ML: Feature Store Architecture](https://www.qwak.com/post/feature-store-architecture)
- [Aerospike: Feature Store 101](https://aerospike.com/blog/feature-store/)
- [Dragonfly: Feature Store Architecture and Storage](https://www.dragonflydb.io/blog/feature-store-architecture-and-storage)
- [Featureform: Three Common Architectures](https://www.featureform.com/post/feature-stores-explained-the-three-common-architectures)
- [Databricks: Point-in-time feature joins](https://docs.databricks.com/aws/en/machine-learning/feature-store/time-series)

### Edge Computing & Resource Efficiency
- [Pidora: Edge Computing Makes Your Raspberry Pi 10x Faster](https://pidora.ca/edge-computing-makes-your-raspberry-pi-10x-faster-heres-where-data-processing-happens/)
- [MDPI: Understanding Performance of Low Power Raspberry Pi Cloud](https://www.mdpi.com/2079-9292/5/2/29)
- [Greptime: Building Time-Series Databases at Edge](https://greptime.com/blogs/2025-02-12-build-edge-database)
- [Medium: What You Need to Deploy Edge ML in IoT](https://medium.com/@ymala/what-you-need-to-deploy-edge-ml-in-iot-c803e62695c4)

---

## Appendix A: SQL Templates

### A.1 Continuous Aggregate Template

```sql
-- Template for creating a continuous aggregate
CREATE MATERIALIZED VIEW gold.{stream}_{granularity}
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('{bucket_size}', timestamp) AS bucket,
    {group_by_columns},

    -- Aggregations
    AVG({metric}) AS {metric}_avg,
    MAX({metric}) AS {metric}_max,
    MIN({metric}) AS {metric}_min,
    STDDEV({metric}) AS {metric}_stddev,
    COUNT(*) AS sample_count

FROM silver.{source_table}
GROUP BY bucket, {group_by_columns};

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.{stream}_{granularity}',
    start_offset => INTERVAL '{lookback}',
    end_offset => INTERVAL '{end_offset}',
    schedule_interval => INTERVAL '{refresh_interval}');

-- Compression policy (optional)
ALTER TABLE gold.{stream}_{granularity} SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = '{segment_column}'
);
SELECT add_compression_policy('gold.{stream}_{granularity}', INTERVAL '{compress_after}');

-- Retention policy (optional)
SELECT add_retention_policy('gold.{stream}_{granularity}', INTERVAL '{retention}');
```

### A.2 ML Feature Table Template

```sql
-- Template for ML feature table
CREATE TABLE gold.ml_features_{domain} (
    timestamp TIMESTAMPTZ NOT NULL,
    {entity_key} TEXT NOT NULL,

    -- Current values
    {metric}_current DOUBLE PRECISION,

    -- Rolling window features
    {metric}_mean_{window} DOUBLE PRECISION,
    {metric}_std_{window} DOUBLE PRECISION,
    {metric}_max_{window} DOUBLE PRECISION,
    {metric}_min_{window} DOUBLE PRECISION,
    {metric}_trend_{window} DOUBLE PRECISION,

    -- Lag features
    {metric}_lag_{lag1}h DOUBLE PRECISION,
    {metric}_lag_{lag2}h DOUBLE PRECISION,

    -- Time features
    hour_of_day INTEGER,
    day_of_week INTEGER,
    is_weekend BOOLEAN,

    -- Derived features
    {derived_feature} {type},

    PRIMARY KEY (timestamp, {entity_key})
);

SELECT create_hypertable('gold.ml_features_{domain}', 'timestamp');
CREATE INDEX idx_ml_features_{domain}_entity
    ON gold.ml_features_{domain} ({entity_key}, timestamp DESC);
```

---

## Appendix B: Glossary

| Term | Definition |
|------|------------|
| **Continuous Aggregate** | TimescaleDB's automatically-refreshed materialized view for time-series data |
| **Feature Store** | Centralized repository for ML features with offline and online serving |
| **Gold Layer** | Final, consumption-ready data optimized for specific use cases |
| **Hypertable** | TimescaleDB's partitioned table optimized for time-series |
| **Medallion Architecture** | Data design pattern with Bronze (raw), Silver (refined), Gold (curated) layers |
| **Point-in-Time Join** | Joining features using timestamps to avoid data leakage in ML |
| **Star Schema** | Dimensional modeling with fact tables surrounded by dimension tables |
| **Wide Table (OBT)** | Fully denormalized table with all attributes pre-joined |

---

**Document Version**: 1.0
**Last Updated**: 2026-02-02
**Status**: Complete - Ready for Implementation Planning
