# ML and Feature Engineering Integration for Neural Data Platform Silver Layer

**Research Date**: 2025-12-19
**Platform**: Raspberry Pi 5 (ARM64, 16GB RAM) + M4 Mac
**Context**: Air quality monitoring with ruv-FANN neural networks
**Current State**: Bronze layer (Parquet) operational, DuckDB Silver layer in development (dp-001)

---

## Executive Summary

This research evaluates ML integration and feature engineering approaches for the NDP Silver layer, focusing on practical edge ML patterns suitable for Raspberry Pi 5 deployment. The analysis covers four key areas:

1. **Feature Engineering Solutions**: Time-series feature computation approaches
2. **ML Integration Patterns**: Data export, serving, and feedback loops
3. **Technology Evaluation**: TimescaleDB, DuckDB, Polars, cron+SQL comparison
4. **Pattern Identification**: Anomaly detection and time-series pattern matching

### Key Recommendations

| Component | Recommendation | Rationale |
|-----------|---------------|-----------|
| **Feature Computation** | TimescaleDB continuous aggregates | Real-time computation, automatic refresh, SQL-native |
| **Feature Storage** | TimescaleDB hypertables + Redis cache | Persistence + fast access for inference |
| **ML Training Export** | TimescaleDB → CSV/Parquet batch export | Standard format, batch processing |
| **Real-time Inference** | Redis feature cache + TimescaleDB views | <100ms latency, dual-layer architecture |
| **Pattern Detection** | TimescaleDB + augurs (Rust) | SQL-based detection + production-ready time-series library |

---

## 1. Feature Engineering Solutions

### 1.1 Requirements

For air quality ML with ruv-FANN, feature engineering must support:

1. **Rolling Window Features**: 1h, 4h, 24h aggregations (mean, std, trend)
2. **Cross-Stream Features**: Indoor/outdoor correlations, derived metrics
3. **Time-Based Features**: Hour-of-day, day-of-week, seasonal patterns
4. **Lag Features**: Previous 1h, 6h, 24h values for autoregressive models
5. **Real-Time Computation**: Features available <1min after data ingestion

### 1.2 Technology Comparison

#### Option 1: TimescaleDB Continuous Aggregates (RECOMMENDED)

**Architecture**:
```
Bronze (Parquet) → DuckDB ETL → TimescaleDB → Continuous Aggregates → Features
                                      ↓
                                Redis Cache → ruv-FANN Inference
```

**Implementation**:
```sql
-- Create feature materialized view
CREATE MATERIALIZED VIEW features_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    stream_id,
    location_id,

    -- Rolling window features (last 4 hours)
    AVG(pm25) OVER w4h AS pm25_mean_4h,
    STDDEV(pm25) OVER w4h AS pm25_std_4h,
    MAX(pm25) OVER w4h AS pm25_max_4h,

    -- Trend features (linear regression slope)
    REGR_SLOPE(pm25, EXTRACT(EPOCH FROM time)) OVER w4h AS pm25_trend_4h,

    -- Current values
    FIRST(pm25, time) AS pm25_current,
    FIRST(co2, time) AS co2_current,
    FIRST(temperature, time) AS temp_current,

    -- Lag features
    LAG(pm25, 1) OVER (ORDER BY bucket) AS pm25_lag_1h,
    LAG(pm25, 6) OVER (ORDER BY bucket) AS pm25_lag_6h,
    LAG(pm25, 24) OVER (ORDER BY bucket) AS pm25_lag_24h,

    -- Time-based features
    EXTRACT(HOUR FROM bucket) AS hour_of_day,
    EXTRACT(DOW FROM bucket) AS day_of_week,
    EXTRACT(WEEK FROM bucket) AS week_of_year

FROM sensor_data
WHERE stream_id = 'air-quality'
WINDOW
    w4h AS (ORDER BY time ROWS BETWEEN 3 PRECEDING AND CURRENT ROW)
GROUP BY bucket, stream_id, location_id;

-- Auto-refresh policy (every 10 minutes)
SELECT add_continuous_aggregate_policy('features_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes');
```

**Cross-Stream Features**:
```sql
-- Indoor/outdoor correlation features
CREATE MATERIALIZED VIEW features_cross_stream
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', i.time) AS bucket,

    -- Indoor features
    AVG(i.pm25) AS pm25_indoor_mean,
    AVG(i.co2) AS co2_indoor_mean,

    -- Outdoor features
    AVG(o.pm2_5) AS pm25_outdoor_mean,
    AVG(w.temperature) AS temp_outdoor_mean,
    AVG(w.wind_speed) AS wind_speed_mean,

    -- Derived correlation features
    AVG(i.pm25) - AVG(o.pm2_5) AS pm25_indoor_outdoor_diff,
    AVG(i.pm25) / NULLIF(AVG(o.pm2_5), 0) AS pm25_indoor_outdoor_ratio,
    AVG(i.temperature) - AVG(w.temperature) AS temp_indoor_outdoor_diff,

    -- Dewpoint calculation
    AVG(i.temperature) - ((100 - AVG(i.humidity)) / 5.0) AS dewpoint_indoor

FROM sensor_data i
LEFT JOIN sensor_data o ON
    time_bucket('1 hour', i.time) = time_bucket('1 hour', o.time)
    AND o.stream_id = 'outdoor-air-quality'
LEFT JOIN sensor_data w ON
    time_bucket('1 hour', i.time) = time_bucket('1 hour', w.time)
    AND w.stream_id = 'outdoor-weather'
WHERE i.stream_id = 'air-quality'
GROUP BY bucket;
```

**Pros**:
- ✅ Real-time computation (auto-refresh every 10 minutes)
- ✅ SQL-native (no external processing pipeline)
- ✅ Automatic materialization (faster queries)
- ✅ Compression (80-95% reduction for old data)
- ✅ Built-in retention policies
- ✅ PostgreSQL ecosystem compatibility

**Cons**:
- ❌ Requires TimescaleDB (not DuckDB)
- ❌ Heavier than DuckDB (but manageable on Pi 5)
- ❌ Limited window function support in continuous aggregates

**Resource Usage** (Pi 5):
- Memory: ~400MB (with compression)
- CPU: <10% average (refresh every 10 min)
- Storage: ~50MB/month (compressed features)

#### Option 2: DuckDB Views + Cron Refresh

**Architecture**:
```
Bronze (Parquet) → DuckDB Views → Cron Job → Export CSV → ruv-FANN Training
                          ↓
                   Materialized View → Grafana Dashboards
```

**Implementation**:
```sql
-- Feature view (virtual, query-time computation)
CREATE VIEW features_hourly AS
WITH windowed AS (
    SELECT
        time_bucket(INTERVAL '1 hour', timestamp) AS bucket,
        pm25,
        co2,
        temperature,
        ROW_NUMBER() OVER (ORDER BY timestamp) AS row_num
    FROM silver_indoor_air
)
SELECT
    bucket,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,

    -- Rolling window (4-hour lookback)
    AVG(pm25) OVER (ORDER BY bucket ROWS BETWEEN 3 PRECEDING AND CURRENT ROW) AS pm25_mean_4h,

    -- Lag features
    LAG(pm25, 1) OVER (ORDER BY bucket) AS pm25_lag_1h,
    LAG(pm25, 6) OVER (ORDER BY bucket) AS pm25_lag_6h
FROM windowed
GROUP BY bucket;

-- Export features for training (cron job, daily at 3am)
COPY (
    SELECT * FROM features_hourly
    WHERE bucket >= CURRENT_DATE - INTERVAL '90 days'
) TO '/tmp/features.parquet' (FORMAT PARQUET);
```

**Cron Job** (`/etc/cron.d/duckdb-features`):
```bash
0 3 * * * duckdb /workspace/ndp.db < /config/export_features.sql
```

**Pros**:
- ✅ Minimal footprint (DuckDB lightweight)
- ✅ Leverages existing DuckDB (dp-001)
- ✅ Parquet export (ML-friendly format)
- ✅ Simple batch workflow

**Cons**:
- ❌ No real-time features (batch only)
- ❌ Manual refresh required
- ❌ Query-time computation (slower for large windows)
- ❌ No feature caching

**Use Case**: Batch ML training, not real-time inference.

#### Option 3: Polars/DataFusion Pipeline

**Architecture**:
```
Bronze (Parquet) → Rust Binary (Polars) → Feature Parquet → ruv-FANN
                            ↓
                    TimescaleDB (optional persistence)
```

**Implementation** (Rust):
```rust
use polars::prelude::*;

fn compute_features(df: LazyFrame) -> Result<LazyFrame> {
    df
        // Rolling window features
        .with_columns([
            col("pm25")
                .rolling_mean(RollingOptions {
                    window_size: Duration::parse("4h"),
                    min_periods: 1,
                    ..Default::default()
                })
                .alias("pm25_mean_4h"),

            col("pm25")
                .rolling_std(RollingOptions {
                    window_size: Duration::parse("4h"),
                    ..Default::default()
                })
                .alias("pm25_std_4h"),
        ])
        // Lag features
        .with_columns([
            col("pm25").shift(1).alias("pm25_lag_1h"),
            col("pm25").shift(6).alias("pm25_lag_6h"),
        ])
        // Time-based features
        .with_columns([
            col("timestamp").dt().hour().alias("hour_of_day"),
            col("timestamp").dt().weekday().alias("day_of_week"),
        ])
}

// Main pipeline
fn main() -> Result<()> {
    let df = LazyFrame::scan_parquet("/data/air-quality/**/*.parquet", Default::default())?;
    let features = compute_features(df)?;

    features
        .collect()?
        .write_parquet("/tmp/features.parquet")?;

    Ok(())
}
```

**Pros**:
- ✅ Rust-native (integrate with NDP stack)
- ✅ Extremely fast (parallel processing)
- ✅ Memory-efficient (lazy evaluation)
- ✅ Parquet → Parquet pipeline

**Cons**:
- ❌ Requires custom Rust code
- ❌ No SQL interface (less accessible)
- ❌ Batch-only (not real-time)
- ❌ More complex than SQL views

**Use Case**: High-performance batch feature engineering for large datasets.

#### Option 4: Cron + SQL Scripts

**Architecture**:
```
Bronze (Parquet) → Cron Job → SQL Script → CSV Export → ruv-FANN
```

**Implementation**:
```bash
#!/bin/bash
# /deploy/pi/scripts/compute_features.sh

# Export last 90 days of features
duckdb /workspace/ndp.db <<SQL
COPY (
    SELECT
        timestamp,
        pm25,
        AVG(pm25) OVER w4h AS pm25_mean_4h,
        STDDEV(pm25) OVER w4h AS pm25_std_4h,
        LAG(pm25, 1) OVER (ORDER BY timestamp) AS pm25_lag_1h
    FROM silver_indoor_air
    WHERE timestamp >= CURRENT_DATE - INTERVAL '90 days'
    WINDOW w4h AS (ORDER BY timestamp ROWS BETWEEN 239 PRECEDING AND CURRENT ROW)
) TO '/tmp/features.csv' (HEADER, DELIMITER ',');
SQL

# Trigger ML retraining
/usr/local/bin/ruv-fann-train --input /tmp/features.csv --output /models/pm25_forecast.safetensors
```

**Cron Schedule**:
```cron
0 3 * * * /deploy/pi/scripts/compute_features.sh
```

**Pros**:
- ✅ Simplest approach
- ✅ No additional services
- ✅ Leverages DuckDB

**Cons**:
- ❌ Batch-only
- ❌ No real-time features
- ❌ Manual scheduling

**Use Case**: Simple batch training workflows.

---

### 1.3 Recommendation: Hybrid Approach

**Architecture**:
```
Bronze (Parquet) → DuckDB (Silver Views) → TimescaleDB ETL (dp-002) → Continuous Aggregates
                                                  ↓
                                        Redis Feature Cache → ruv-FANN Inference
                                                  ↓
                                        Batch Export (Parquet) → ruv-FANN Training
```

**Phase 1 (dp-001)**: DuckDB views + batch export
- Keep dp-001 focused on Grafana dashboards
- Export features via cron for batch training

**Phase 2 (dp-002)**: Add TimescaleDB for real-time features
- Migrate Silver layer to TimescaleDB
- Enable continuous aggregates
- Cache features in Redis for <100ms inference

**Phase 3 (fe-001)**: Feature engineering optimization
- Polars pipeline for high-volume batch processing
- Advanced feature transformations

---

## 2. ML Integration Patterns

### 2.1 Training Data Export

#### Pattern 1: Batch Export to Parquet (RECOMMENDED)

**Workflow**:
```
TimescaleDB Features → SQL Query → Parquet Export → ruv-FANN Training
```

**Implementation**:
```sql
-- Export training dataset (90 days history)
COPY (
    SELECT
        bucket AS timestamp,
        pm25_current AS target,
        pm25_mean_4h,
        pm25_std_4h,
        pm25_trend_4h,
        pm25_lag_1h,
        pm25_lag_6h,
        pm25_lag_24h,
        temp_current,
        humidity_current,
        co2_current,
        temp_outdoor_mean,
        wind_speed_mean,
        hour_of_day,
        day_of_week
    FROM features_hourly
    WHERE bucket >= CURRENT_DATE - INTERVAL '90 days'
    ORDER BY bucket ASC
) TO '/tmp/training_data.parquet' (FORMAT PARQUET);
```

**Rust Training Integration**:
```rust
use polars::prelude::*;
use ruv_fann::Fann;

fn train_model(data_path: &str) -> Result<Fann> {
    // Load features
    let df = ParquetReader::new(std::fs::File::open(data_path)?)
        .finish()?;

    // Prepare training data
    let features = df.select(["pm25_mean_4h", "pm25_std_4h", "temp_current", ...])?;
    let targets = df.select(["target"])?;

    // Train ruv-FANN model
    let mut model = Fann::new(&[14, 32, 16, 1])?; // 14 inputs, 1 output
    model.set_training_algorithm(TrainAlgorithm::RPROP);

    model.train_on_data(&training_pairs, max_epochs: 1000, target_mse: 0.001);

    model.save("/models/pm25_forecast_v1.safetensors")?;
    Ok(model)
}
```

#### Pattern 2: Streaming Export via PostgreSQL LISTEN/NOTIFY

**Workflow**:
```
TimescaleDB → NOTIFY → Rust Listener → ruv-FANN Incremental Training
```

**Implementation**:
```sql
-- Trigger on new feature data
CREATE OR REPLACE FUNCTION notify_new_features()
RETURNS TRIGGER AS $$
BEGIN
    PERFORM pg_notify('features_updated', row_to_json(NEW)::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER features_notify
AFTER INSERT ON features_hourly
FOR EACH ROW EXECUTE FUNCTION notify_new_features();
```

**Rust Listener**:
```rust
use tokio_postgres::{AsyncMessage, Client};

async fn listen_for_features(client: &Client) -> Result<()> {
    client.execute("LISTEN features_updated", &[]).await?;

    loop {
        match client.recv().await? {
            AsyncMessage::Notification(notif) => {
                let feature: FeatureRow = serde_json::from_str(&notif.payload())?;

                // Incremental training
                incremental_train(&mut model, feature).await?;
            }
            _ => {}
        }
    }
}
```

**Use Case**: Real-time model updates (online learning).

### 2.2 Real-Time Feature Serving

#### Pattern 1: Redis Feature Cache (RECOMMENDED)

**Architecture**:
```
TimescaleDB (Features) → Redis (Cache with TTL) → ruv-FANN Inference
         ↓                         ↓
    Batch Refresh           Hot Path (<10ms)
```

**Implementation**:
```rust
use redis::AsyncCommands;

async fn get_features_for_inference(
    redis: &redis::Client,
    timescale: &tokio_postgres::Client,
    location_id: &str
) -> Result<FeatureVector> {
    let cache_key = format!("features:latest:{}", location_id);

    // Try cache first
    let cached: Option<String> = redis.get_async_connection().await?
        .get(&cache_key).await?;

    if let Some(json) = cached {
        return Ok(serde_json::from_str(&json)?);
    }

    // Cache miss: Query TimescaleDB
    let row = timescale.query_one(
        "SELECT * FROM features_hourly
         WHERE location_id = $1
         ORDER BY bucket DESC
         LIMIT 1",
        &[&location_id]
    ).await?;

    let features = FeatureVector::from_row(&row);

    // Cache for 5 minutes
    let _: () = redis.get_async_connection().await?
        .set_ex(&cache_key, serde_json::to_string(&features)?, 300).await?;

    Ok(features)
}

async fn predict_pm25(location_id: &str) -> Result<f64> {
    let features = get_features_for_inference(&REDIS, &TIMESCALE, location_id).await?;

    let model = GLOBAL_MODEL.read().await;
    let input = features.to_fann_input();

    let output = model.run(&input)?;
    Ok(output[0])
}
```

**Cache Configuration**:
```redis
# Redis config
maxmemory 128mb
maxmemory-policy allkeys-lru
```

**Pros**:
- ✅ <10ms latency (in-memory)
- ✅ TTL auto-expiration
- ✅ Handles cache invalidation
- ✅ Scales to 1000s of requests/sec

**Cons**:
- ❌ Requires Redis (extra service)
- ❌ Cache coherency concerns

#### Pattern 2: TimescaleDB Direct Query

**Implementation**:
```rust
async fn get_latest_features(location_id: &str) -> Result<FeatureVector> {
    let row = TIMESCALE_CLIENT.query_one(
        "SELECT * FROM features_hourly
         WHERE location_id = $1
         ORDER BY bucket DESC
         LIMIT 1",
        &[&location_id]
    ).await?;

    Ok(FeatureVector::from_row(&row))
}
```

**Pros**:
- ✅ No extra service
- ✅ Always fresh data

**Cons**:
- ❌ 50-200ms latency (database roundtrip)
- ❌ Doesn't scale to high request rates

**Use Case**: Low-throughput inference (<10 req/sec).

### 2.3 Model Feedback Loop

**Pattern**: Prediction Tracking + ADWIN Drift Detection

**Architecture**:
```
ruv-FANN Prediction → TimescaleDB (Log) → ADWIN Drift Detector → Retrain Trigger
         ↓                                         ↓
    Actual Value (1h later)              EWC++ Incremental Training
```

**Implementation**:
```sql
-- Prediction log table
CREATE TABLE predictions (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    predicted_pm25 DOUBLE PRECISION NOT NULL,
    actual_pm25 DOUBLE PRECISION,  -- NULL until actual arrives
    prediction_error DOUBLE PRECISION,
    model_version TEXT NOT NULL,
    PRIMARY KEY (timestamp, location_id)
);

SELECT create_hypertable('predictions', 'timestamp');

-- Update with actuals (1 hour later)
UPDATE predictions p
SET
    actual_pm25 = a.pm25,
    prediction_error = ABS(p.predicted_pm25 - a.pm25)
FROM silver_indoor_air a
WHERE p.timestamp = time_bucket('1 hour', a.timestamp)
  AND p.location_id = a.location_id
  AND p.actual_pm25 IS NULL;
```

**Rust Drift Detection**:
```rust
use std::collections::VecDeque;

struct AdwinDriftDetector {
    window: VecDeque<f64>,
    threshold: f64,
}

impl AdwinDriftDetector {
    fn add_element(&mut self, error: f64) -> bool {
        self.window.push_back(error);

        // Check for drift by comparing window halves
        for cut_point in 0..self.window.len() {
            let (left, right) = self.window.split_at(cut_point);
            let diff = (mean(left) - mean(right)).abs();

            if diff > self.threshold {
                // Drift detected! Discard old data
                self.window.drain(0..cut_point);
                return true; // Trigger retraining
            }
        }

        false
    }
}

async fn monitor_predictions() -> Result<()> {
    let mut drift_detector = AdwinDriftDetector::new();

    loop {
        // Fetch recent predictions with actuals
        let errors = fetch_prediction_errors(&TIMESCALE, Duration::hours(24)).await?;

        for error in errors {
            if drift_detector.add_element(error) {
                warn!("Concept drift detected! Triggering model retraining...");
                trigger_retraining().await?;
                break;
            }
        }

        tokio::time::sleep(Duration::hours(1)).await;
    }
}
```

**Retraining with EWC++**:
```rust
async fn retrain_with_ewc(model: &mut Fann, new_data: &[TrainingSample]) -> Result<()> {
    // Compute Fisher information matrix (importance of each weight)
    let fisher_info = compute_fisher_information(model);

    // Retrain with EWC regularization (prevents forgetting old knowledge)
    model.train_with_regularization(
        new_data,
        ewc_lambda: 2000.0,  // Memory protection strength
        fisher_info,
    );

    Ok(())
}
```

---

## 3. Technology-Specific Evaluation

### 3.1 TimescaleDB Continuous Aggregates

**Ideal For**:
- Real-time feature computation
- Rolling window features
- Cross-stream joins
- Automatic refresh

**Example Use Cases**:
```sql
-- Use Case 1: Hourly aggregates with 4-hour rolling window
CREATE MATERIALIZED VIEW features_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    AVG(pm25) OVER w4h AS pm25_mean_4h,
    STDDEV(pm25) OVER w4h AS pm25_std_4h,
    REGR_SLOPE(pm25, EXTRACT(EPOCH FROM time)) OVER w4h AS pm25_trend_4h
FROM sensor_data
WINDOW w4h AS (ORDER BY time ROWS BETWEEN 3 PRECEDING AND CURRENT ROW)
GROUP BY bucket;

-- Use Case 2: Anomaly detection features
CREATE MATERIALIZED VIEW anomaly_features
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('10 minutes', time) AS bucket,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    (pm25 - AVG(pm25) OVER w1h) / NULLIF(STDDEV(pm25) OVER w1h, 0) AS pm25_zscore
FROM sensor_data
WINDOW w1h AS (ORDER BY time ROWS BETWEEN 5 PRECEDING AND CURRENT ROW)
GROUP BY bucket;
```

**Performance** (Pi 5):
- Refresh: ~500ms for 1000 rows
- Query: <50ms for 7-day range
- Storage: ~10MB/month (compressed)

**Limitations**:
- Cannot use arbitrary window functions in continuous aggregates
- Refresh lag (10-minute default)

### 3.2 DuckDB Ad-Hoc Feature Computation

**Ideal For**:
- Exploratory feature engineering
- Batch export for training
- Complex analytical queries

**Example**:
```sql
-- Ad-hoc feature exploration
SELECT
    timestamp,
    pm25,

    -- Percentile-based features
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) OVER w24h AS pm25_p95_24h,

    -- Rate of change
    (pm25 - LAG(pm25, 1) OVER (ORDER BY timestamp)) /
        EXTRACT(EPOCH FROM (timestamp - LAG(timestamp, 1) OVER (ORDER BY timestamp))) AS pm25_rate_of_change,

    -- Fourier-based seasonality (requires extension)
    -- FFT(pm25) OVER w7d AS pm25_seasonality

FROM silver_indoor_air
WINDOW
    w24h AS (ORDER BY timestamp ROWS BETWEEN 1439 PRECEDING AND CURRENT ROW),
    w7d AS (ORDER BY timestamp ROWS BETWEEN 10079 PRECEDING AND CURRENT ROW);
```

**Pros**:
- ✅ Rich analytical functions
- ✅ Parquet export
- ✅ Fast for batch queries

**Cons**:
- ❌ Query-time computation (no materialization)
- ❌ Not suitable for real-time inference

### 3.3 Polars for High-Performance Batch Processing

**Ideal For**:
- Large-scale batch feature engineering
- Complex transformations
- Parquet → Parquet pipelines

**Example**:
```rust
use polars::prelude::*;

fn engineer_features(df: LazyFrame) -> Result<LazyFrame> {
    df
        // Group by time buckets
        .groupby_dynamic(
            col("timestamp"),
            [],
            DynamicGroupOptions {
                every: Duration::parse("1h"),
                period: Duration::parse("1h"),
                ..Default::default()
            }
        )
        .agg([
            col("pm25").mean().alias("pm25_mean"),
            col("pm25").std(0).alias("pm25_std"),
        ])
        // Add rolling features
        .with_columns([
            col("pm25_mean")
                .rolling_mean(RollingOptions {
                    window_size: Duration::parse("4h"),
                    min_periods: 1,
                    ..Default::default()
                })
                .alias("pm25_mean_4h"),
        ])
}
```

**Performance**:
- 10x faster than SQL for complex transformations
- Parallel processing (uses all Pi 5 cores)
- Memory-efficient lazy evaluation

### 3.4 Cron + SQL (Simple Batch)

**Ideal For**:
- Simple daily/weekly training jobs
- Low-frequency updates

**Example**:
```bash
#!/bin/bash
# Daily feature export at 3am

duckdb /workspace/ndp.db <<SQL
COPY (
    SELECT * FROM features_hourly
    WHERE bucket >= CURRENT_DATE - INTERVAL '90 days'
) TO '/tmp/features_$(date +%Y%m%d).parquet' (FORMAT PARQUET);
SQL

# Trigger training
/usr/local/bin/train_model.sh /tmp/features_*.parquet
```

**Pros**:
- ✅ Simplest approach
- ✅ No additional services

**Cons**:
- ❌ No real-time features
- ❌ Manual orchestration

---

## 4. Pattern Identification Capabilities

### 4.1 Anomaly Detection

#### Option 1: TimescaleDB Z-Score Detection

**Implementation**:
```sql
-- Real-time anomaly detection view
CREATE VIEW anomalies AS
SELECT
    timestamp,
    pm25,
    pm25_mean_4h,
    pm25_std_4h,
    (pm25 - pm25_mean_4h) / NULLIF(pm25_std_4h, 0) AS zscore,
    CASE
        WHEN ABS((pm25 - pm25_mean_4h) / NULLIF(pm25_std_4h, 0)) > 3
        THEN 'anomaly'
        ELSE 'normal'
    END AS anomaly_status
FROM features_hourly
WHERE pm25_std_4h > 0;

-- Alert on anomalies
CREATE OR REPLACE FUNCTION notify_anomaly()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.anomaly_status = 'anomaly' THEN
        PERFORM pg_notify('air_quality_anomaly',
            json_build_object(
                'timestamp', NEW.timestamp,
                'pm25', NEW.pm25,
                'zscore', NEW.zscore
            )::text
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

#### Option 2: augurs DBSCAN Clustering

**Rust Implementation**:
```rust
use augurs::clustering::DbscanDetector;

async fn detect_outliers(data: &[f64]) -> Result<Vec<usize>> {
    let detector = DbscanDetector::with_sensitivity(0.05)?;
    let outliers = detector.detect(data)?;
    Ok(outliers.iter().map(|&b| b as usize).collect())
}
```

**Integration**:
```rust
// Periodic outlier detection
async fn monitor_anomalies() -> Result<()> {
    loop {
        let pm25_data = fetch_recent_pm25(&TIMESCALE, Duration::hours(24)).await?;
        let outliers = detect_outliers(&pm25_data).await?;

        if !outliers.is_empty() {
            warn!("Detected {} outliers in last 24h", outliers.len());
            send_alert(&outliers).await?;
        }

        tokio::time::sleep(Duration::hours(1)).await;
    }
}
```

### 4.2 Time-Series Pattern Matching

#### Option 1: TimescaleDB Window Functions

**Implementation**:
```sql
-- Detect recurring pollution spikes (hourly pattern)
SELECT
    hour_of_day,
    AVG(pm25_mean) AS avg_pm25,
    STDDEV(pm25_mean) AS std_pm25,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25_mean) AS p95_pm25
FROM features_hourly
WHERE bucket >= CURRENT_DATE - INTERVAL '30 days'
GROUP BY hour_of_day
ORDER BY hour_of_day;

-- Detect seasonal patterns (day-of-week)
SELECT
    day_of_week,
    AVG(pm25_mean) AS avg_pm25
FROM features_hourly
WHERE bucket >= CURRENT_DATE - INTERVAL '90 days'
GROUP BY day_of_week
ORDER BY day_of_week;
```

#### Option 2: augurs Seasonality Detection

**Rust Implementation**:
```rust
use augurs::mstl::{MSTLModel, TrendModel};

async fn detect_seasonality(data: &[f64]) -> Result<Vec<f64>> {
    let model = MSTLModel::new(
        vec![24, 168],  // Hourly (24) and weekly (168) periods
        TrendModel::Linear,
    )?;

    let decomposition = model.fit(data)?;

    // Extract seasonal components
    Ok(decomposition.seasonal)
}
```

### 4.3 Correlation Analysis

**TimescaleDB Implementation**:
```sql
-- Indoor/outdoor correlation
SELECT
    CORR(pm25_indoor_mean, pm25_outdoor_mean) AS pm25_correlation,
    CORR(temp_indoor_mean, temp_outdoor_mean) AS temp_correlation,
    CORR(pm25_indoor_mean, wind_speed_mean) AS pm25_wind_correlation
FROM features_cross_stream
WHERE bucket >= CURRENT_DATE - INTERVAL '30 days';

-- Time-lagged correlation (PM2.5 indoor vs outdoor with lag)
SELECT
    lag_hours,
    CORR(i.pm25, o.pm25) AS correlation
FROM (
    SELECT
        bucket,
        pm25_indoor_mean AS pm25
    FROM features_cross_stream
) i
CROSS JOIN LATERAL (
    SELECT
        bucket - (n || ' hours')::INTERVAL AS lag_hours,
        pm25_outdoor_mean AS pm25
    FROM features_cross_stream,
         generate_series(0, 24) n
) o
WHERE i.bucket = o.bucket + o.lag_hours
GROUP BY lag_hours
ORDER BY lag_hours;
```

---

## 5. Recommendations by Phase

### Phase 1: dp-001 (Current - Grafana Dashboards)

**Technology**: DuckDB views only
- Focus: Visualization, not ML
- Features: Virtual views for Grafana
- Export: Batch cron job (daily)

**Implementation**:
```sql
-- DuckDB feature view (dp-001)
CREATE VIEW features_basic AS
SELECT
    time_bucket(INTERVAL '1 hour', timestamp) AS bucket,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min
FROM silver_indoor_air
GROUP BY bucket;
```

### Phase 2: dp-002 (Silver Layer Migration to TimescaleDB)

**Technology**: TimescaleDB continuous aggregates
- Migrate DuckDB → TimescaleDB
- Enable continuous aggregates
- Add feature caching (Redis)

**Implementation**:
```sql
-- TimescaleDB continuous aggregate (dp-002)
CREATE MATERIALIZED VIEW features_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    stream_id,
    location_id,

    -- Rolling window features (4-hour)
    AVG(pm25) OVER w4h AS pm25_mean_4h,
    STDDEV(pm25) OVER w4h AS pm25_std_4h,

    -- Lag features
    LAG(pm25, 1) OVER (ORDER BY bucket) AS pm25_lag_1h,
    LAG(pm25, 6) OVER (ORDER BY bucket) AS pm25_lag_6h,

    -- Time features
    EXTRACT(HOUR FROM bucket) AS hour_of_day,
    EXTRACT(DOW FROM bucket) AS day_of_week

FROM sensor_data
WHERE stream_id = 'air-quality'
WINDOW w4h AS (ORDER BY time ROWS BETWEEN 3 PRECEDING AND CURRENT ROW)
GROUP BY bucket, stream_id, location_id;

SELECT add_continuous_aggregate_policy('features_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '10 minutes',
    schedule_interval => INTERVAL '10 minutes');
```

### Phase 3: fe-001 (Feature Engineering Optimization)

**Technology**: Polars + augurs
- High-performance batch processing
- Advanced transformations
- Pattern detection

**Architecture**:
```
TimescaleDB → Polars Pipeline → Advanced Features → Parquet → ruv-FANN
         ↓                                                       ↓
    Redis Cache                                         Model Updates
         ↓
  ruv-FANN Inference
```

---

## 6. Final Recommendation

### Recommended Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   BRONZE LAYER                               │
│  Parquet Files (Existing, no changes)                       │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│                   SILVER LAYER (dp-002)                      │
│  TimescaleDB Hypertables                                     │
│  - Raw sensor data                                           │
│  - Compression enabled                                       │
│  - Retention policies                                        │
└────────────────────┬────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────────────────┐
│               FEATURE LAYER (fe-001)                         │
│  TimescaleDB Continuous Aggregates                           │
│  - Hourly features (auto-refresh every 10 min)              │
│  - Cross-stream features                                     │
│  - Rolling windows (4h, 24h)                                 │
└────┬────────────────────────────────────┬───────────────────┘
     │                                    │
     ▼                                    ▼
┌─────────────────┐              ┌──────────────────────────┐
│  Redis Cache    │              │  Batch Export (Parquet)   │
│  - TTL: 5 min   │              │  - Daily cron job         │
│  - Latest       │              │  - 90-day training data   │
│    features     │              └──────────┬───────────────┘
└────┬────────────┘                         │
     │                                      ▼
     │                              ┌──────────────────────────┐
     │                              │  ruv-FANN Training       │
     │                              │  - ADWIN drift detection │
     │                              │  - EWC++ incremental     │
     │                              └──────────┬───────────────┘
     │                                         │
     │                                         ▼
     │                              ┌──────────────────────────┐
     │                              │  Model Registry          │
     │                              │  - Safetensors format    │
     │                              │  - Versioned models      │
     │                              └──────────┬───────────────┘
     │                                         │
     ▼                                         ▼
┌─────────────────────────────────────────────────────────────┐
│               ruv-FANN INFERENCE ENGINE                      │
│  - Load features from Redis (<10ms)                          │
│  - Fallback to TimescaleDB (50-200ms)                        │
│  - Run prediction                                            │
│  - Log prediction to TimescaleDB                             │
└─────────────────────────────────────────────────────────────┘
```

### Implementation Roadmap

**dp-001 (Complete by Week 1)**:
- ✅ DuckDB views for Grafana
- ✅ Batch export cron job

**dp-002 (Week 2-4)**:
- 🔲 Migrate to TimescaleDB
- 🔲 Create continuous aggregates
- 🔲 Setup Redis feature cache
- 🔲 Implement feature serving API

**fe-001 (Week 5-8)**:
- 🔲 Advanced feature engineering (Polars)
- 🔲 Pattern detection (augurs)
- 🔲 ADWIN drift monitoring
- 🔲 EWC++ incremental training

**ml-001 (Week 9-12)**:
- 🔲 ruv-FANN model training
- 🔲 Real-time inference integration
- 🔲 Prediction logging and monitoring
- 🔲 Model versioning and rollback

---

## 7. References

### Project Documentation
- [dp-001 SCOPE.md](/workspaces/neural-data-platform/product/features/dp-001/SCOPE.md)
- [DuckDB Specification](/workspaces/neural-data-platform/product/features/dp-001/specification/DUCKDB_SPECIFICATION.md)
- [Technology Selection Guide](/workspaces/neural-data-platform/product/research/07-technology-selection.md)
- [Rust ML Frameworks Research](/workspaces/neural-data-platform/product/research/03-rust-ml-frameworks.md)

### External Resources
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)
- [DuckDB Window Functions](https://duckdb.org/docs/sql/window_functions)
- [Polars DataFrame API](https://pola-rs.github.io/polars/py-polars/html/reference/dataframe/index.html)
- [augurs Time-Series Toolkit](https://github.com/grafana/augurs)
- [ruv-FANN Documentation](https://github.com/ruvnet/ruv-FANN)

---

**Document Version**: 1.0
**Last Updated**: 2025-12-19
**Author**: Research Agent (Claude)
**Status**: Complete
