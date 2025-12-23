# Silver Platform Implementation Roadmap
## Neural Data Platform - Actionable Recommendations

**Synthesis Date**: 2025-12-23
**Context**: 5 operational streams in Bronze layer (Parquet), DuckDB attempt failed, planning Silver layer build

---

## Executive Summary

After comprehensive research across 6 domains (medallion architecture, data quality, agentic patterns, component inventory, platform capabilities, and build order), this document provides a **prioritized, actionable roadmap** for building NDP's Silver layer and beyond.

### Key Decisions

| Decision | Recommendation | Rationale |
|----------|---------------|-----------|
| **Silver Layer DB** | TimescaleDB | Purpose-built for time-series, PostgreSQL ecosystem, small-scale friendly |
| **ETL Approach** | Custom Rust | Matches existing codebase, high performance, control |
| **Data Quality** | Custom Rust validators | Lightweight, no Python dependency, fits NDP patterns |
| **Build Strategy** | ONE stream first | Validate before scaling, reduce risk |
| **First Stream** | `air-quality` | Highest value, most data, MQTT-based |
| **Gold Layer** | TimescaleDB continuous aggregates | No separate system needed initially |

### Why NOT DuckDB (Lessons Learned)

1. **No native time_bucket()** - Requires complex window functions
2. **Batch-oriented** - Not designed for continuous ingestion
3. **Single-process** - Conflicts with multi-stream concurrent writes
4. **No auto-refresh aggregates** - Manual materialized view management
5. **No Grafana data source** - Requires custom integration

---

## Current Platform State

```
┌─────────────────────────────────────────────────────────────────────┐
│ BRONZE LAYER (✅ OPERATIONAL)                                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐        │
│  │  air-quality   │  │ outdoor-weather│  │outdoor-air-qual│        │
│  │    (MQTT)      │  │   (HTTP)       │  │    (HTTP)      │        │
│  │  PM2.5,CO2,etc │  │  METAR data    │  │   AirNow API   │        │
│  └────────┬───────┘  └────────┬───────┘  └────────┬───────┘        │
│           │                   │                   │                 │
│  ┌────────┴───────┐  ┌────────┴───────┐  ┌────────┴───────┐        │
│  │ nws-observ     │  │ nws-forecast   │  │                │        │
│  │   (HTTP)       │  │   (HTTP)       │  │                │        │
│  │  Station KSGJ  │  │  156-hour grid │  │                │        │
│  └────────┬───────┘  └────────┬───────┘  └────────────────┘        │
│           │                   │                                      │
│           └───────────────────┴──────────────────────────────────►  │
│                                                                       │
│                          Parquet Files                               │
│                      (Append-only storage)                           │
└─────────────────────────────────────────────────────────────────────┘

Platform Maturity: Stage 2 (Data Integration)
Target: Stage 3 (Data Intelligence)
```

---

## Recommended Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ TARGET STATE                                                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                       │
│  BRONZE (Existing)          SILVER (Phase 1-2)      GOLD (Phase 3+) │
│  ─────────────────         ─────────────────        ───────────────  │
│                                                                       │
│  ┌─────────────┐           ┌─────────────────┐     ┌─────────────┐  │
│  │   Parquet   │  ──ETL──► │   TimescaleDB   │ ──► │ Continuous  │  │
│  │   Files     │           │   Hypertables   │     │ Aggregates  │  │
│  │ (5 streams) │           │                 │     │             │  │
│  └─────────────┘           │  - Cleaned      │     │ - Hourly    │  │
│                            │  - Validated    │     │ - Daily     │  │
│                            │  - Indexed      │     │ - Features  │  │
│                            └────────┬────────┘     └──────┬──────┘  │
│                                     │                      │         │
│                                     └──────────────────────┘         │
│                                               │                       │
│                                        Grafana Dashboards            │
│                                                                       │
│  ┌─────────────────────────────────────────────────────────────────┐│
│  │                     CROSS-CUTTING                                ││
│  │  - Data Quality (Rust validators)                                ││
│  │  - Monitoring (Prometheus + Grafana)                             ││
│  │  - Config (etcd GitOps - existing)                               ││
│  └─────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Silver Foundation (Quick Win)
### Duration: 2-3 weeks

**Goal**: Prove end-to-end Bronze → Silver → Dashboard with ONE stream

### Components

#### 1.1 TimescaleDB Setup
```yaml
# Docker Compose addition
services:
  timescaledb:
    image: timescale/timescaledb:latest-pg16
    container_name: ndp-timescaledb
    environment:
      POSTGRES_USER: ndp
      POSTGRES_PASSWORD: ${TIMESCALE_PASSWORD}
      POSTGRES_DB: neural_data_platform
    ports:
      - "5432:5432"
    volumes:
      - timescale_data:/var/lib/postgresql/data
    restart: unless-stopped

volumes:
  timescale_data:
```

#### 1.2 Silver Schema (Air Quality)
```sql
-- Create extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Silver layer table for air-quality stream
CREATE TABLE silver.air_quality (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,

    -- Sensor readings (cleaned/validated)
    pm25 DOUBLE PRECISION,
    pm10 DOUBLE PRECISION,
    co2 INTEGER,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    tvoc INTEGER,
    nox INTEGER,

    -- Metadata
    stream_id TEXT DEFAULT 'air-quality',
    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    source_file TEXT,

    -- Data quality flags
    dq_pm25_valid BOOLEAN DEFAULT true,
    dq_temp_valid BOOLEAN DEFAULT true,

    PRIMARY KEY (timestamp, location_id)
);

-- Convert to hypertable (auto-partitions by time)
SELECT create_hypertable('silver.air_quality', 'timestamp');

-- Indexes for common queries
CREATE INDEX idx_aq_location ON silver.air_quality (location_id, timestamp DESC);
CREATE INDEX idx_aq_pm25 ON silver.air_quality (timestamp DESC, pm25) WHERE pm25 IS NOT NULL;

-- Enable compression after 7 days
ALTER TABLE silver.air_quality SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'location_id'
);
SELECT add_compression_policy('silver.air_quality', INTERVAL '7 days');

-- Retention: drop data older than 1 year
SELECT add_retention_policy('silver.air_quality', INTERVAL '365 days');
```

#### 1.3 Gold Layer (Continuous Aggregates)
```sql
-- Hourly aggregates (auto-refreshing)
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) AS hour,
    location_id,

    -- PM2.5 aggregates
    AVG(pm25) AS pm25_avg,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,
    PERCENTILE_CONT(0.5) WITHIN GROUP (ORDER BY pm25) AS pm25_median,

    -- Temperature
    AVG(temperature) AS temp_avg,
    MAX(temperature) AS temp_max,
    MIN(temperature) AS temp_min,

    -- CO2
    AVG(co2) AS co2_avg,
    MAX(co2) AS co2_max,

    -- Sample count (data quality indicator)
    COUNT(*) AS sample_count
FROM silver.air_quality
GROUP BY hour, location_id;

-- Auto-refresh policy: every 15 minutes
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);

-- Daily aggregates
CREATE MATERIALIZED VIEW gold.air_quality_daily
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 day', timestamp) AS day,
    location_id,

    AVG(pm25) AS pm25_avg,
    MAX(pm25) AS pm25_max,
    MIN(pm25) AS pm25_min,

    AVG(temperature) AS temp_avg,
    MAX(temperature) AS temp_max,
    MIN(temperature) AS temp_min,

    SUM(CASE WHEN pm25 > 35 THEN 1 ELSE 0 END) AS unhealthy_hours,
    COUNT(*) AS sample_count
FROM silver.air_quality
GROUP BY day, location_id;

SELECT add_continuous_aggregate_policy('gold.air_quality_daily',
    start_offset => INTERVAL '2 days',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

#### 1.4 ETL Pipeline (Rust)

**New crate**: `silver-etl` in `/apps/silver-etl/`

```rust
// apps/silver-etl/src/main.rs (simplified structure)
use tokio_postgres::{NoTls, Client};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

/// High-water mark tracking for incremental loads
struct HighWaterMark {
    stream_id: String,
    last_timestamp: DateTime<Utc>,
    last_file: String,
}

/// Main ETL pipeline for air-quality stream
async fn run_air_quality_etl(client: &Client, bronze_path: &Path) -> Result<EtlStats> {
    // 1. Get high-water mark from metadata table
    let hwm = get_high_water_mark(client, "air-quality").await?;

    // 2. Find Parquet files newer than HWM
    let new_files = find_new_parquet_files(bronze_path, &hwm)?;

    // 3. Process each file
    let mut stats = EtlStats::default();
    for file_path in new_files {
        let records = read_parquet_file(&file_path)?;
        let validated = validate_records(records)?;
        let inserted = insert_to_timescale(client, validated).await?;

        stats.files_processed += 1;
        stats.records_inserted += inserted;
    }

    // 4. Update high-water mark
    update_high_water_mark(client, "air-quality", stats.last_timestamp).await?;

    Ok(stats)
}

/// Data validation (custom Rust validators)
fn validate_records(records: Vec<AirQualityRecord>) -> Vec<ValidatedRecord> {
    records.into_iter()
        .filter_map(|r| {
            let mut valid = ValidatedRecord::from(r);

            // PM2.5 range validation (0-500 µg/m³)
            valid.dq_pm25_valid = valid.pm25.map_or(true, |v| v >= 0.0 && v <= 500.0);

            // Temperature range (-40°C to 60°C)
            valid.dq_temp_valid = valid.temperature.map_or(true, |v| v >= -40.0 && v <= 60.0);

            // Reject obviously invalid records
            if valid.pm25.map_or(false, |v| v < 0.0 || v > 1000.0) {
                return None; // Quarantine
            }

            Some(valid)
        })
        .collect()
}
```

#### 1.5 Basic Grafana Dashboard

**Dashboard JSON** (save to `deploy/grafana/dashboards/air-quality-silver.json`):

Key panels:
1. **Real-time PM2.5** - Last 24 hours line chart
2. **Hourly Aggregates** - From gold.air_quality_hourly
3. **Data Freshness** - Time since last record
4. **Sample Count** - Records per hour (data quality)
5. **AQI Category** - Color-coded current status

### Success Criteria (Phase 1)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Dashboard updates | Every 15 min | Automatic |
| Query latency | < 100ms | Grafana panel load time |
| Data lag | < 1 hour | `NOW() - MAX(timestamp)` |
| ETL reliability | 100% batches | No failed runs in 48h |
| Data quality | > 99% valid | DQ flag counts |

### Phase 1 Deliverables

- [ ] TimescaleDB Docker deployment
- [ ] Silver schema (air-quality hypertable)
- [ ] Gold continuous aggregates (hourly/daily)
- [ ] Rust ETL crate (`silver-etl`)
- [ ] Basic Grafana dashboard
- [ ] Data quality validators
- [ ] Integration tests
- [ ] SPARC documentation in `product/features/dp-001/`

---

## Phase 2: Scale Silver Layer
### Duration: 2-3 weeks

**Goal**: Apply proven pattern to remaining 4 streams

### Schema Templates

```sql
-- Template for all weather streams
CREATE TABLE silver.weather_observations (
    timestamp TIMESTAMPTZ NOT NULL,
    location_id TEXT NOT NULL,
    stream_id TEXT NOT NULL,  -- 'outdoor-weather', 'nws-observations'

    temperature DOUBLE PRECISION,
    dewpoint DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    pressure DOUBLE PRECISION,
    wind_speed DOUBLE PRECISION,
    wind_direction DOUBLE PRECISION,
    visibility DOUBLE PRECISION,
    precipitation DOUBLE PRECISION,

    ingested_at TIMESTAMPTZ DEFAULT NOW(),
    source_file TEXT,

    PRIMARY KEY (timestamp, location_id, stream_id)
);

SELECT create_hypertable('silver.weather_observations', 'timestamp');

-- Forecast table (separate due to future timestamps)
CREATE TABLE silver.weather_forecast (
    valid_time TIMESTAMPTZ NOT NULL,  -- When forecast is valid
    issue_time TIMESTAMPTZ NOT NULL,  -- When forecast was issued
    location_id TEXT NOT NULL,

    temperature DOUBLE PRECISION,
    dewpoint DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    wind_speed DOUBLE PRECISION,
    wind_direction DOUBLE PRECISION,
    pop DOUBLE PRECISION,  -- Probability of precipitation
    short_forecast TEXT,

    ingested_at TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (valid_time, issue_time, location_id)
);

SELECT create_hypertable('silver.weather_forecast', 'valid_time');
```

### Configuration-Driven ETL

```yaml
# config/base/silver-etl/config.yaml
streams:
  air-quality:
    enabled: true
    source_path: "/data/bronze/air-quality"
    target_table: "silver.air_quality"
    schedule: "*/15 * * * *"  # Every 15 minutes
    validators:
      - type: range
        field: pm25
        min: 0
        max: 500
      - type: range
        field: temperature
        min: -40
        max: 60

  outdoor-weather:
    enabled: true
    source_path: "/data/bronze/outdoor-weather"
    target_table: "silver.weather_observations"
    schedule: "*/5 * * * *"

  nws-observations:
    enabled: true
    source_path: "/data/bronze/nws-observations"
    target_table: "silver.weather_observations"
    schedule: "*/10 * * * *"

  nws-forecast-hourly:
    enabled: true
    source_path: "/data/bronze/nws-forecast-hourly"
    target_table: "silver.weather_forecast"
    schedule: "0 * * * *"  # Every hour

  outdoor-air-quality:
    enabled: true
    source_path: "/data/bronze/outdoor-air-quality"
    target_table: "silver.air_quality_external"
    schedule: "0 * * * *"
```

### Phase 2 Success Criteria

| Metric | Target |
|--------|--------|
| Streams migrated | 5/5 |
| Configuration coverage | 100% declarative |
| Schema consistency | Unified naming |
| Dashboard coverage | All streams visualized |

---

## Phase 3: Feature Engineering
### Duration: 2-3 weeks

**Goal**: Build ML-ready features for forecasting

### Feature Definitions

```sql
-- Multi-hour windows for ML features
CREATE MATERIALIZED VIEW gold.air_quality_features
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', timestamp) AS hour,
    location_id,

    -- Current values
    AVG(pm25) AS pm25_avg,

    -- Lag features (previous hours)
    LAG(AVG(pm25), 1) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp)) AS pm25_avg_lag_1h,
    LAG(AVG(pm25), 3) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp)) AS pm25_avg_lag_3h,
    LAG(AVG(pm25), 6) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp)) AS pm25_avg_lag_6h,
    LAG(AVG(pm25), 24) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp)) AS pm25_avg_lag_24h,

    -- Rolling windows
    AVG(AVG(pm25)) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp) ROWS BETWEEN 2 PRECEDING AND CURRENT ROW) AS pm25_rolling_3h,
    AVG(AVG(pm25)) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp) ROWS BETWEEN 5 PRECEDING AND CURRENT ROW) AS pm25_rolling_6h,

    -- Rate of change
    (AVG(pm25) - LAG(AVG(pm25), 1) OVER (PARTITION BY location_id ORDER BY time_bucket('1 hour', timestamp))) AS pm25_delta_1h,

    -- Temporal features
    EXTRACT(HOUR FROM time_bucket('1 hour', timestamp)) AS hour_of_day,
    EXTRACT(DOW FROM time_bucket('1 hour', timestamp)) AS day_of_week,

    -- Weather correlation features (join with weather data)
    -- Added in separate view with joins

    COUNT(*) AS sample_count
FROM silver.air_quality
GROUP BY hour, location_id;
```

### Feature Access API

```rust
// Query interface for ML training
pub async fn get_training_features(
    client: &Client,
    location_id: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<Vec<FeatureRow>> {
    let query = r#"
        SELECT
            hour,
            pm25_avg,
            pm25_avg_lag_1h,
            pm25_avg_lag_3h,
            pm25_rolling_6h,
            pm25_delta_1h,
            hour_of_day,
            day_of_week
        FROM gold.air_quality_features
        WHERE location_id = $1
          AND hour BETWEEN $2 AND $3
        ORDER BY hour
    "#;

    // Execute and map to feature vectors
}
```

---

## Phase 4: Advanced Dashboards
### Duration: 2-3 weeks

**Goal**: Production-quality visualization

### Dashboard Structure

```
/deploy/grafana/dashboards/
├── overview/
│   └── home.json            # Platform health overview
├── air-quality/
│   ├── realtime.json        # Current conditions
│   ├── trends.json          # Historical analysis
│   └── alerts.json          # Threshold violations
├── weather/
│   ├── observations.json    # Current weather
│   └── forecast.json        # Forecast vs actual
└── system/
    ├── data-quality.json    # DQ metrics
    └── pipeline-health.json # ETL monitoring
```

### Key Visualizations

1. **AQI Gauge** - Real-time air quality index
2. **PM2.5 Heatmap** - Hour x Day pattern analysis
3. **Forecast Accuracy** - Predicted vs actual
4. **Data Freshness** - Lag monitoring per stream
5. **Anomaly Detection** - Statistical outliers

---

## Phase 5: ML Integration
### Duration: 4-6 weeks

**Goal**: Deploy ruv-FANN models for forecasting

### Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                    ML Pipeline                                  │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │  Feature Store  │────►│ Training Job    │                   │
│  │  (TimescaleDB)  │     │ (ruv-FANN)      │                   │
│  └─────────────────┘     └────────┬────────┘                   │
│                                   │                             │
│                          ┌────────▼────────┐                   │
│                          │  Model Store    │                   │
│                          │  (Parquet)      │                   │
│                          └────────┬────────┘                   │
│                                   │                             │
│  ┌─────────────────┐     ┌────────▼────────┐                   │
│  │  Live Features  │────►│ Inference Svc   │──► Predictions   │
│  │  (Real-time)    │     │ (Rust)          │                   │
│  └─────────────────┘     └─────────────────┘                   │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

### Model Training Flow

```rust
// ML training orchestration
pub async fn train_pm25_forecast_model(
    features: &FeatureDataset,
    config: &ModelConfig,
) -> Result<TrainedModel> {
    // 1. Prepare training data
    let (train, test) = features.split(0.8);

    // 2. Initialize ruv-FANN network
    let mut network = fann::Network::new(&[
        features.input_dim(),
        64,  // Hidden layer 1
        32,  // Hidden layer 2
        1,   // Output: PM2.5 prediction
    ])?;

    // 3. Train
    network.train_on_data(train, config.epochs, config.learning_rate)?;

    // 4. Evaluate
    let metrics = evaluate_model(&network, &test)?;

    // 5. Save model artifact
    let model = TrainedModel {
        network,
        metrics,
        version: chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string(),
    };

    model.save(&config.model_path)?;

    Ok(model)
}
```

---

## Phase 6: Alerting & Automation
### Duration: 2-3 weeks

**Goal**: Autonomous monitoring and notifications

### Alert Rules

```yaml
# config/base/alerts/air-quality.yaml
alerts:
  - name: pm25_unhealthy
    description: "PM2.5 exceeds unhealthy threshold"
    condition: "pm25_avg > 35"
    severity: warning
    channels: [slack, email]

  - name: pm25_very_unhealthy
    description: "PM2.5 exceeds very unhealthy threshold"
    condition: "pm25_avg > 150"
    severity: critical
    channels: [slack, email, sms]

  - name: data_stale
    description: "No data received in 30 minutes"
    condition: "NOW() - MAX(timestamp) > INTERVAL '30 minutes'"
    severity: warning
    channels: [slack]

  - name: etl_failure
    description: "ETL pipeline failed"
    condition: "etl_status = 'failed'"
    severity: critical
    channels: [slack, email]
```

### Rust Alert Engine

```rust
// Alert evaluation engine
pub struct AlertEngine {
    rules: Vec<AlertRule>,
    channels: HashMap<String, Box<dyn NotificationChannel>>,
    rate_limiter: RateLimiter,
}

impl AlertEngine {
    pub async fn evaluate(&self, client: &Client) -> Result<Vec<FiredAlert>> {
        let mut fired = Vec::new();

        for rule in &self.rules {
            if self.rate_limiter.should_evaluate(&rule.name) {
                let result = rule.evaluate(client).await?;
                if result.triggered {
                    let alert = self.fire_alert(rule, result).await?;
                    fired.push(alert);
                }
            }
        }

        Ok(fired)
    }
}
```

---

## Agentic Capabilities (Future)

Based on agentic patterns research, consider these future enhancements:

### 1. Self-Healing Pipelines
- Automatic retry with exponential backoff
- Schema drift detection and adaptation
- Anomaly detection triggers reprocessing

### 2. Conversational Analytics (Text-to-SQL)
```
User: "What was the average PM2.5 last week?"
Agent: "The average PM2.5 for the past 7 days was 12.3 µg/m³,
        which is in the 'Good' category."
```

### 3. Autonomous Data Quality
- Learn expected patterns from historical data
- Auto-generate validation rules
- Self-adjusting thresholds

### 4. Predictive Alerting
- Alert BEFORE threshold exceeded
- ML-based anomaly prediction
- Proactive maintenance suggestions

---

## Technology Stack Summary

| Component | Technology | Why |
|-----------|------------|-----|
| **Bronze Storage** | Parquet | Existing, proven |
| **Silver Database** | TimescaleDB | Time-series native, PostgreSQL |
| **Gold Layer** | Continuous Aggregates | Auto-refresh, no extra system |
| **ETL** | Custom Rust | Performance, existing expertise |
| **Data Quality** | Custom Rust validators | Lightweight, no Python |
| **Visualization** | Grafana | Existing, TimescaleDB native |
| **Monitoring** | Prometheus + Grafana | Standard stack |
| **ML Framework** | ruv-FANN | Existing, Rust native |
| **Configuration** | etcd + GitOps | Existing infrastructure |

---

## Risk Mitigation

| Risk | Mitigation | Owner |
|------|------------|-------|
| TimescaleDB performance | Load test with one stream first | ndp-architect |
| ETL bugs corrupt data | Keep Bronze immutable, validate | ndp-tester |
| Scope creep | MoSCoW prioritization, defer | ndp-scrum-master |
| Skills gap | Allocate learning time | Team |
| Pi resource limits | Monitor usage, backpressure | ndp-rust-dev |

---

## Immediate Next Steps

### This Week
1. **Create feature directory**: `product/features/dp-001/`
2. **Write SCOPE.md**: Phase 1 scope only
3. **ADR**: Document TimescaleDB decision
4. **Docker**: Add TimescaleDB to compose

### Week 2
1. **Schema**: Create air-quality hypertable
2. **ETL**: Implement basic Parquet → TimescaleDB
3. **Aggregates**: Set up hourly continuous aggregate

### Week 3
1. **Dashboard**: Basic air quality visualization
2. **Validation**: Test end-to-end pipeline
3. **Documentation**: SPARC completion phase

---

## Research Documents Reference

All supporting research is in `product/research/silverplatform/`:

| Document | Focus | Key Insight |
|----------|-------|-------------|
| `methodology/medallion-architecture.md` | Architecture patterns | TimescaleDB > DuckDB |
| `methodology/data-quality-frameworks.md` | DQ tools comparison | Custom Rust validators |
| `methodology/platform-capability-patterns.md` | Platform design | Configuration-driven |
| `agentic-patterns/autonomous-analysis.md` | Future capabilities | Self-healing, text-to-SQL |
| `components/component-inventory.md` | 10 component breakdown | Build vs buy analysis |
| `build-order/build-sequence-research.md` | Maturity & sequencing | ONE stream first |

---

**Document Status**: FINAL
**Ready for Implementation**: YES
**Next Action**: Create `product/features/dp-001/` and begin Phase 1

