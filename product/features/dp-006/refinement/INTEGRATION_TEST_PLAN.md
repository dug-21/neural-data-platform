# DP-006: Integration Test Plan - Silver Layer ETL

**Feature**: dp-006 (Silver Layer Implementation)
**Phase**: Refinement
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Tester
**Status**: Draft

---

## Executive Summary

This document defines the comprehensive integration test plan for dp-006 Silver Layer ETL. It covers 6 core test scenarios from the specification, test infrastructure setup, fixture generation, and verification procedures.

### Test Philosophy

- **London School TDD**: Outside-in testing with mock infrastructure where appropriate
- **Real Integration**: Use actual TimescaleDB and DuckDB for end-to-end verification
- **Config-Driven**: Tests verify config-only stream addition works correctly
- **DQ Transparency**: Verify all DQ actions produce expected dq_flags

### Test Scenarios

| ID | Scenario | Priority | Duration Est. |
|----|----------|----------|---------------|
| IT-001 | Happy Path ETL | P0 (Critical) | 30s |
| IT-002 | DQ Violations | P0 (Critical) | 45s |
| IT-003 | Late Arrivals | P1 (High) | 30s |
| IT-004 | Recovery | P1 (High) | 60s |
| IT-005 | New Stream | P1 (High) | 45s |
| IT-006 | Schema Evolution | P2 (Medium) | 60s |

---

## 1. Test Environment Setup

### 1.1 Docker Compose Test Infrastructure

```yaml
# tests/docker/docker-compose.test.yml
version: '3.8'

services:
  # TimescaleDB for Silver layer
  timescaledb-test:
    image: timescale/timescaledb:latest-pg15
    container_name: ndp-timescale-test
    ports:
      - "5433:5432"  # Different port to avoid conflicts
    environment:
      POSTGRES_USER: ndp_test
      POSTGRES_PASSWORD: ndp_test_password
      POSTGRES_DB: ndp_silver_test
    volumes:
      - timescale-test-data:/var/lib/postgresql/data
      - ./init-scripts:/docker-entrypoint-initdb.d:ro
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ndp_test -d ndp_silver_test"]
      interval: 5s
      timeout: 5s
      retries: 5
    deploy:
      resources:
        limits:
          memory: 256M
    networks:
      - ndp-test-network

  # etcd for configuration
  etcd-test:
    image: quay.io/coreos/etcd:v3.5.9
    container_name: ndp-etcd-test
    ports:
      - "2380:2379"  # Different port to avoid conflicts
    environment:
      ETCD_NAME: etcd-test
      ETCD_LISTEN_CLIENT_URLS: http://0.0.0.0:2379
      ETCD_ADVERTISE_CLIENT_URLS: http://etcd-test:2379
      ETCD_DATA_DIR: /etcd-data
    volumes:
      - etcd-test-data:/etcd-data
    healthcheck:
      test: ["CMD", "etcdctl", "endpoint", "health"]
      interval: 5s
      timeout: 5s
      retries: 5
    deploy:
      resources:
        limits:
          memory: 128M
    networks:
      - ndp-test-network

volumes:
  timescale-test-data:
  etcd-test-data:

networks:
  ndp-test-network:
    driver: bridge
```

### 1.2 TimescaleDB Initialization Script

```sql
-- tests/docker/init-scripts/001-create-silver-schema.sql

-- Create Silver schema
CREATE SCHEMA IF NOT EXISTS silver;

-- Extension for TimescaleDB
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Air Quality Observations Table
CREATE TABLE silver.air_quality_observations (
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    source_id           TEXT,
    stream_id           TEXT NOT NULL DEFAULT 'air-quality',

    -- Measurements
    pm25                DOUBLE PRECISION,
    pm10                DOUBLE PRECISION,
    co2                 SMALLINT,
    temperature_c       DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    tvoc_index          SMALLINT,
    nox_index           SMALLINT,

    -- Context
    context             JSONB,

    -- DQ transparency
    dq_flags            TEXT[] DEFAULT '{}'::TEXT[],

    -- ETL metadata
    ingestion_time      TIMESTAMPTZ DEFAULT NOW(),
    batch_id            TEXT,

    PRIMARY KEY (observation_time, ndp_id)
);

-- Convert to hypertable
SELECT create_hypertable(
    'silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

-- Indexes
CREATE INDEX idx_aq_obs_ndp_id ON silver.air_quality_observations (ndp_id, observation_time DESC);
CREATE INDEX idx_aq_obs_dq_flags ON silver.air_quality_observations USING GIN (dq_flags);

-- Weather Observations Table
CREATE TABLE silver.weather_observations (
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    source_provider     TEXT NOT NULL,  -- 'nws' or 'owm'

    -- Measurements
    temperature_c       DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    pressure_hpa        DOUBLE PRECISION,
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    wind_gust_kmh       DOUBLE PRECISION,
    visibility_m        DOUBLE PRECISION,
    cloud_cover_pct     DOUBLE PRECISION,

    -- Context
    context             JSONB,

    -- DQ transparency
    dq_flags            TEXT[] DEFAULT '{}'::TEXT[],

    -- ETL metadata
    ingestion_time      TIMESTAMPTZ DEFAULT NOW(),
    batch_id            TEXT,

    PRIMARY KEY (observation_time, ndp_id, source_provider)
);

SELECT create_hypertable(
    'silver.weather_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

CREATE INDEX idx_weather_obs_ndp_id ON silver.weather_observations (ndp_id, observation_time DESC);
CREATE INDEX idx_weather_obs_provider ON silver.weather_observations (source_provider, observation_time DESC);
CREATE INDEX idx_weather_obs_dq_flags ON silver.weather_observations USING GIN (dq_flags);

-- Weather Forecasts Table
CREATE TABLE silver.weather_forecasts (
    issue_time          TIMESTAMPTZ NOT NULL,
    valid_time          TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    lead_time_hours     SMALLINT GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM (valid_time - issue_time)) / 3600
    ) STORED,

    -- Forecast values
    temperature_c       DOUBLE PRECISION,
    humidity_pct        DOUBLE PRECISION,
    wind_speed_kmh      DOUBLE PRECISION,
    wind_direction_deg  DOUBLE PRECISION,
    precipitation_prob_pct DOUBLE PRECISION,
    short_forecast      TEXT,

    -- Context
    context             JSONB,

    -- DQ transparency
    dq_flags            TEXT[] DEFAULT '{}'::TEXT[],

    -- ETL metadata
    ingestion_time      TIMESTAMPTZ DEFAULT NOW(),
    batch_id            TEXT,

    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

SELECT create_hypertable(
    'silver.weather_forecasts',
    'valid_time',
    chunk_time_interval => INTERVAL '1 day'
);

CREATE INDEX idx_forecasts_lead_time ON silver.weather_forecasts (lead_time_hours, valid_time DESC);
CREATE INDEX idx_forecasts_issue_time ON silver.weather_forecasts (issue_time DESC);

-- Outdoor Air Quality Table
CREATE TABLE silver.outdoor_air_quality (
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,

    -- AQI
    aqi                 SMALLINT,
    aqi_category        TEXT,

    -- Pollutants
    pm25                DOUBLE PRECISION,
    pm10                DOUBLE PRECISION,
    o3                  DOUBLE PRECISION,
    no2                 DOUBLE PRECISION,
    so2                 DOUBLE PRECISION,
    co                  DOUBLE PRECISION,

    -- Context
    context             JSONB,

    -- DQ transparency
    dq_flags            TEXT[] DEFAULT '{}'::TEXT[],

    -- ETL metadata
    ingestion_time      TIMESTAMPTZ DEFAULT NOW(),
    batch_id            TEXT,

    PRIMARY KEY (observation_time, ndp_id)
);

SELECT create_hypertable(
    'silver.outdoor_air_quality',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day'
);

-- DQ Transparency Table
CREATE TABLE silver.dq_transparency (
    id                  BIGSERIAL,
    check_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stream_id           TEXT NOT NULL,
    batch_id            TEXT,
    rule_name           TEXT NOT NULL,
    rule_level          TEXT NOT NULL,  -- 'row' or 'batch'
    field_name          TEXT,
    violation_type      TEXT NOT NULL,  -- 'flag', 'reject', 'clamp', 'drop'
    violation_reason    TEXT NOT NULL,
    row_count           INTEGER NOT NULL,
    original_value      TEXT,
    result_value        TEXT,
    sample_payload      JSONB,
    context             JSONB,

    PRIMARY KEY (check_time, id)
);

SELECT create_hypertable(
    'silver.dq_transparency',
    'check_time',
    chunk_time_interval => INTERVAL '7 days'
);

CREATE INDEX idx_dq_trans_stream_time ON silver.dq_transparency (stream_id, check_time DESC);
CREATE INDEX idx_dq_trans_rule ON silver.dq_transparency (rule_name, check_time DESC);

-- Grant permissions
GRANT ALL ON SCHEMA silver TO ndp_test;
GRANT ALL ON ALL TABLES IN SCHEMA silver TO ndp_test;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA silver TO ndp_test;
```

### 1.3 Test Configuration

```yaml
# tests/fixtures/configs/test_config.yaml
test_environment:
  timescaledb:
    host: localhost
    port: 5433
    database: ndp_silver_test
    user: ndp_test
    password: ndp_test_password

  etcd:
    endpoints:
      - http://localhost:2380

  bronze_path: /tmp/ndp-test/bronze

  timeouts:
    etl_max_seconds: 60
    setup_max_seconds: 30
    cleanup_max_seconds: 15
```

---

## 2. Test Fixtures

### 2.1 Directory Structure

```
tests/fixtures/
├── parquet/
│   ├── air-quality/
│   │   ├── valid_hour.parquet          # IT-001: Happy path data
│   │   ├── with_dq_violations.parquet  # IT-002: Out-of-range values
│   │   └── late_arrivals.parquet       # IT-003: Data with old timestamps
│   ├── outdoor-weather/
│   │   ├── valid_hour.parquet          # IT-001: Happy path
│   │   └── kelvin_conversion.parquet   # Unit conversion test
│   ├── nws-observations/
│   │   └── valid_hour.parquet
│   └── new-stream/
│       └── valid_data.parquet          # IT-005: New stream test
├── configs/
│   ├── air-quality-test.yaml           # Stream config for tests
│   ├── outdoor-weather-test.yaml
│   ├── new-stream-test.yaml            # IT-005: Config-only addition
│   └── evolved-schema.yaml             # IT-006: Schema evolution
└── expected/
    ├── it001_happy_path_counts.json
    ├── it002_dq_flags_expected.json
    └── it003_late_arrival_counts.json
```

### 2.2 Fixture Generation Module

```rust
// tests/fixtures/mod.rs
//! Test fixtures for dp-006 integration tests
//!
//! This module provides:
//! - Parquet file generation with various data scenarios
//! - Test stream configurations
//! - Expected result definitions

use chrono::{DateTime, Duration, Utc};
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use parquet::file::writer::SerializedFileWriter;
use parquet::schema::parser::parse_message_type;
use serde_json::{json, Value};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

/// Bronze Parquet schema matching existing Bronze layer structure
pub const BRONZE_SCHEMA: &str = r#"
message bronze_schema {
    REQUIRED INT64 timestamp (TIMESTAMP(MICROS,true));
    REQUIRED BINARY source_id (STRING);
    REQUIRED BINARY ndp_id (STRING);
    OPTIONAL BINARY stream_id (STRING);
    OPTIONAL BINARY context (JSON);
    REQUIRED BINARY raw_payload (JSON);
}
"#;

/// Test fixture configuration
#[derive(Debug, Clone)]
pub struct FixtureConfig {
    pub stream_id: String,
    pub ndp_id: String,
    pub base_time: DateTime<Utc>,
    pub row_count: usize,
    pub interval_minutes: i64,
}

impl Default for FixtureConfig {
    fn default() -> Self {
        Self {
            stream_id: "air-quality".to_string(),
            ndp_id: "test_device_001".to_string(),
            base_time: Utc::now() - Duration::hours(1),
            row_count: 60, // 1 hour of minute data
            interval_minutes: 1,
        }
    }
}

/// Create valid air quality Parquet file for happy path testing
pub fn create_air_quality_valid_hour(dir: &Path) -> PathBuf {
    let config = FixtureConfig {
        stream_id: "air-quality".to_string(),
        row_count: 60,
        ..Default::default()
    };

    let rows: Vec<Value> = (0..config.row_count)
        .map(|i| {
            let ts = config.base_time + Duration::minutes(i as i64 * config.interval_minutes);
            json!({
                "timestamp": ts.timestamp_micros(),
                "source_id": "mqtt://airgradient",
                "ndp_id": config.ndp_id,
                "stream_id": config.stream_id,
                "context": {
                    "device_type": "airgradient",
                    "location": {"type": "indoor", "path": "/test/room"}
                },
                "raw_payload": {
                    "pm02": 15.0 + (i as f64 * 0.5),   // Valid PM2.5: 15-45 range
                    "pm10": 25.0 + (i as f64 * 0.5),   // Valid PM10: 25-55 range
                    "rco2": 450 + (i * 5),             // Valid CO2: 450-750 range
                    "atmp": 22.0 + (i as f64 * 0.1),   // Valid temp: 22-28 C
                    "rhum": 45.0 + (i as f64 * 0.3),   // Valid humidity: 45-63%
                    "tvocIndex": 50 + (i % 20),        // Valid TVOC: 50-69
                    "noxIndex": 10 + (i % 15)          // Valid NOx: 10-24
                }
            })
        })
        .collect();

    write_bronze_parquet(dir, "valid_hour.parquet", &rows)
}

/// Create air quality data with DQ violations for IT-002
pub fn create_air_quality_with_dq_violations(dir: &Path) -> PathBuf {
    let config = FixtureConfig {
        stream_id: "air-quality".to_string(),
        row_count: 100,
        ..Default::default()
    };

    let rows: Vec<Value> = (0..config.row_count)
        .map(|i| {
            let ts = config.base_time + Duration::minutes(i as i64);

            // Introduce various DQ violations
            let (pm25, pm10, co2, temp, humidity) = match i % 10 {
                0 => (-5.0, 20.0, 450, 22.0, 50.0),      // Negative PM2.5 (out of range)
                1 => (1500.0, 25.0, 450, 22.0, 50.0),    // PM2.5 > 1000 (out of range)
                2 => (25.0, 15.0, 450, 22.0, 50.0),      // PM10 < PM2.5 (cross-field)
                3 => (25.0, 30.0, 300, 22.0, 50.0),      // CO2 < 380 (below atmospheric)
                4 => (25.0, 30.0, 450, -50.0, 50.0),     // Temp < -40 (out of range)
                5 => (25.0, 30.0, 450, 22.0, 120.0),     // Humidity > 100 (should clamp)
                6 => (25.0, 30.0, 450, 22.0, -10.0),     // Humidity < 0 (should clamp)
                _ => (25.0, 30.0, 450, 22.0, 50.0),      // Valid data
            };

            json!({
                "timestamp": ts.timestamp_micros(),
                "source_id": "mqtt://airgradient",
                "ndp_id": config.ndp_id,
                "stream_id": config.stream_id,
                "context": {"device_type": "airgradient"},
                "raw_payload": {
                    "pm02": pm25,
                    "pm10": pm10,
                    "rco2": co2,
                    "atmp": temp,
                    "rhum": humidity,
                    "tvocIndex": 50,
                    "noxIndex": 10
                }
            })
        })
        .collect();

    write_bronze_parquet(dir, "with_dq_violations.parquet", &rows)
}

/// Create late arrival data for IT-003
pub fn create_air_quality_late_arrivals(dir: &Path) -> PathBuf {
    let config = FixtureConfig {
        stream_id: "air-quality".to_string(),
        row_count: 30,
        base_time: Utc::now() - Duration::hours(3), // 3 hours old data
        ..Default::default()
    };

    let rows: Vec<Value> = (0..config.row_count)
        .map(|i| {
            let ts = config.base_time + Duration::minutes(i as i64);
            json!({
                "timestamp": ts.timestamp_micros(),
                "source_id": "mqtt://airgradient",
                "ndp_id": format!("{}_late", config.ndp_id),
                "stream_id": config.stream_id,
                "context": {"device_type": "airgradient", "late_arrival": true},
                "raw_payload": {
                    "pm02": 25.0,
                    "pm10": 30.0,
                    "rco2": 450,
                    "atmp": 22.0,
                    "rhum": 50.0,
                    "tvocIndex": 50,
                    "noxIndex": 10
                }
            })
        })
        .collect();

    write_bronze_parquet(dir, "late_arrivals.parquet", &rows)
}

/// Create valid outdoor weather data with unit conversions needed
pub fn create_outdoor_weather_valid(dir: &Path) -> PathBuf {
    let config = FixtureConfig {
        stream_id: "outdoor-weather".to_string(),
        ndp_id: "owm_sf_001".to_string(),
        row_count: 12,  // Hourly data for 12 hours
        interval_minutes: 60,
        ..Default::default()
    };

    let rows: Vec<Value> = (0..config.row_count)
        .map(|i| {
            let ts = config.base_time + Duration::minutes(i as i64 * config.interval_minutes);
            json!({
                "timestamp": ts.timestamp_micros(),
                "source_id": "http://openweathermap",
                "ndp_id": config.ndp_id,
                "stream_id": config.stream_id,
                "context": {
                    "provider": "openweathermap",
                    "location": {"lat": 37.7749, "lon": -122.4194}
                },
                "raw_payload": {
                    "main": {
                        "temp": 295.15 + (i as f64 * 0.5), // Kelvin (22C base)
                        "feels_like": 294.15 + (i as f64 * 0.5),
                        "humidity": 65 + (i % 10),
                        "pressure": 1013 + (i % 5)
                    },
                    "wind": {
                        "speed": 3.5 + (i as f64 * 0.2), // m/s
                        "deg": (180 + i * 15) % 360,
                        "gust": 5.0 + (i as f64 * 0.3)
                    },
                    "clouds": {
                        "all": 20 + (i * 3)
                    },
                    "visibility": 10000,
                    "dt": ts.timestamp()
                }
            })
        })
        .collect();

    write_bronze_parquet(dir, "valid_hour.parquet", &rows)
}

/// Create new stream data for IT-005 (config-only stream addition)
pub fn create_new_stream_data(dir: &Path) -> PathBuf {
    let config = FixtureConfig {
        stream_id: "test-new-stream".to_string(),
        ndp_id: "new_sensor_001".to_string(),
        row_count: 20,
        ..Default::default()
    };

    let rows: Vec<Value> = (0..config.row_count)
        .map(|i| {
            let ts = config.base_time + Duration::minutes(i as i64);
            json!({
                "timestamp": ts.timestamp_micros(),
                "source_id": "http://new-source",
                "ndp_id": config.ndp_id,
                "stream_id": config.stream_id,
                "context": {"device_type": "new_sensor"},
                "raw_payload": {
                    "metric_a": 100.0 + (i as f64),
                    "metric_b": 200.0 + (i as f64 * 2.0),
                    "status": "active"
                }
            })
        })
        .collect();

    write_bronze_parquet(dir, "valid_data.parquet", &rows)
}

/// Write rows to Bronze Parquet file
fn write_bronze_parquet(dir: &Path, filename: &str, rows: &[Value]) -> PathBuf {
    let file_path = dir.join(filename);

    // In actual implementation, use arrow/parquet crates
    // This is a placeholder showing the structure

    let schema = Arc::new(
        parse_message_type(BRONZE_SCHEMA).expect("Failed to parse schema")
    );

    let props = Arc::new(
        WriterProperties::builder()
            .set_compression(Compression::SNAPPY)
            .build(),
    );

    let file = File::create(&file_path).expect("Failed to create file");
    let mut writer = SerializedFileWriter::new(file, schema, props)
        .expect("Failed to create writer");

    // Write each row - implementation details depend on parquet crate version
    // ... (row writing logic)

    writer.close().expect("Failed to close writer");
    file_path
}

/// Setup complete test fixtures directory
pub fn setup_test_fixtures() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let base = temp_dir.path();

    // Create directory structure
    fs::create_dir_all(base.join("air-quality")).unwrap();
    fs::create_dir_all(base.join("outdoor-weather")).unwrap();
    fs::create_dir_all(base.join("new-stream")).unwrap();

    // Generate fixtures
    create_air_quality_valid_hour(&base.join("air-quality"));
    create_air_quality_with_dq_violations(&base.join("air-quality"));
    create_air_quality_late_arrivals(&base.join("air-quality"));
    create_outdoor_weather_valid(&base.join("outdoor-weather"));
    create_new_stream_data(&base.join("new-stream"));

    temp_dir
}
```

### 2.3 Stream Test Configurations

```yaml
# tests/fixtures/configs/air-quality-test.yaml
stream_id: air-quality
stream_type: observations
enabled: true

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: source_id
      target: source_id

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      nullable: true

    - source_path: raw_payload.pm10
      target_column: pm10
      type: double_precision
      nullable: true

    - source_path: raw_payload.rco2
      target_column: co2
      type: smallint
      nullable: true

    - source_path: raw_payload.atmp
      target_column: temperature_c
      type: double_precision
      nullable: true

    - source_path: raw_payload.rhum
      target_column: humidity_pct
      type: double_precision
      nullable: true

    - source_path: raw_payload.tvocIndex
      target_column: tvoc_index
      type: smallint
      nullable: true

    - source_path: raw_payload.noxIndex
      target_column: nox_index
      type: smallint
      nullable: true

  dq_rules:
    - rule: range_check
      field: pm25
      min: 0.0
      max: 1000.0
      action: flag

    - rule: range_check
      field: pm10
      min: 0.0
      max: 2000.0
      action: flag

    - rule: range_check
      field: co2
      min: 380
      max: 10000
      action: flag

    - rule: range_check
      field: temperature_c
      min: -40.0
      max: 85.0
      action: flag

    - rule: range_check
      field: humidity_pct
      min: 0.0
      max: 100.0
      action: clamp

    - rule: cross_field_check
      name: pm10_gte_pm25
      expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
      message: pm10_less_than_pm25
      action: flag

  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 5 minutes
```

```yaml
# tests/fixtures/configs/new-stream-test.yaml
# IT-005: Config-only stream addition test
stream_id: test-new-stream
stream_type: observations
enabled: true

silver_etl:
  enabled: true
  target_table: silver.test_new_stream  # New table

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.metric_a
      target_column: metric_a
      type: double_precision
      nullable: true

    - source_path: raw_payload.metric_b
      target_column: metric_b
      type: double_precision
      nullable: true

    - source_path: raw_payload.status
      target_column: status
      type: text
      nullable: true

  dq_rules:
    - rule: range_check
      field: metric_a
      min: 0.0
      max: 1000.0
      action: flag

  dq_output:
    enabled: true
    target_column: dq_flags

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

---

## 3. Integration Test Cases

### 3.1 IT-001: Happy Path ETL

**Purpose**: Verify successful end-to-end ETL of valid data.

**Preconditions**:
- TimescaleDB test container running
- Empty silver.air_quality_observations table
- Valid Parquet fixture: `valid_hour.parquet` (60 rows)
- Air quality stream config loaded in etcd

**Test Steps**:
1. Setup: Clear Silver table, load config
2. Execute: Run silver-etl for air-quality stream
3. Verify: Query Silver table for expected rows

**Expected Outcomes**:
- All 60 rows inserted into Silver
- No dq_flags on any rows (clean data)
- Correct column types and values
- ETL completes in < 60 seconds
- Metrics logged: rows_processed = 60

```rust
// tests/integration/it001_happy_path.rs

#[tokio::test]
#[ignore] // Run with: cargo test --ignored
async fn test_happy_path_etl() {
    // ============ ARRANGE ============
    let test_env = TestEnvironment::setup().await;

    // Clear Silver table
    test_env.db.execute(
        "TRUNCATE silver.air_quality_observations",
        &[]
    ).await.unwrap();

    // Load fixture
    let fixture_path = test_env.load_fixture(
        "air-quality",
        "valid_hour.parquet"
    );

    // Load config to etcd
    test_env.etcd.put(
        "/streams/air-quality/config",
        include_str!("../fixtures/configs/air-quality-test.yaml")
    ).await.unwrap();

    // ============ ACT ============
    let start = std::time::Instant::now();

    let result = silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: fixture_path.to_string_lossy().to_string(),
        ..Default::default()
    }).await;

    let duration = start.elapsed();

    // ============ ASSERT ============
    assert!(result.is_ok(), "ETL should succeed: {:?}", result);

    // Verify row count
    let row_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(row_count, 60, "Should have 60 rows in Silver");

    // Verify no DQ flags (clean data)
    let flagged_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE array_length(dq_flags, 1) > 0",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(flagged_count, 0, "No rows should have DQ flags");

    // Verify column types via sample query
    let sample = test_env.db.query_one(
        "SELECT observation_time, ndp_id, pm25, temperature_c, humidity_pct
         FROM silver.air_quality_observations
         ORDER BY observation_time
         LIMIT 1",
        &[]
    ).await.unwrap();

    assert!(sample.get::<_, DateTime<Utc>>(0).timestamp() > 0);
    assert!(!sample.get::<_, String>(1).is_empty());
    assert!(sample.get::<_, f64>(2) >= 0.0);

    // Verify performance
    assert!(
        duration.as_secs() < 60,
        "ETL should complete in < 60s, took {:?}",
        duration
    );

    // Verify metrics (if available)
    let metrics = result.unwrap();
    assert_eq!(metrics.rows_processed, 60);
    assert_eq!(metrics.rows_with_dq_flags, 0);
}
```

**Verification Queries**:

```sql
-- IT-001.V1: Row count verification
SELECT COUNT(*) as total_rows
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id;
-- Expected: 60

-- IT-001.V2: No DQ flags verification
SELECT COUNT(*) as flagged_rows
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id
  AND array_length(dq_flags, 1) > 0;
-- Expected: 0

-- IT-001.V3: Value range verification
SELECT
    MIN(pm25) as min_pm25,
    MAX(pm25) as max_pm25,
    MIN(temperature_c) as min_temp,
    MAX(temperature_c) as max_temp
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id;
-- Expected: pm25 in [15, 45], temp in [22, 28]

-- IT-001.V4: Timestamp range verification
SELECT
    MIN(observation_time) as earliest,
    MAX(observation_time) as latest,
    MAX(observation_time) - MIN(observation_time) as span
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id;
-- Expected: span ~= 59 minutes
```

**Cleanup**:
```sql
DELETE FROM silver.air_quality_observations WHERE batch_id = $test_batch_id;
```

---

### 3.2 IT-002: DQ Violations

**Purpose**: Verify DQ rules trigger correctly and populate dq_flags.

**Preconditions**:
- Empty Silver table
- Fixture with violations: `with_dq_violations.parquet` (100 rows)
- Known violation pattern:
  - Row 0, 10, 20...: Negative PM2.5 (range_check)
  - Row 1, 11, 21...: PM2.5 > 1000 (range_check)
  - Row 2, 12, 22...: PM10 < PM2.5 (cross_field_check)
  - Row 3, 13, 23...: CO2 < 380 (range_check)
  - Row 4, 14, 24...: Temp < -40 (range_check)
  - Row 5, 15, 25...: Humidity > 100 (clamp action)
  - Row 6, 16, 26...: Humidity < 0 (clamp action)

**Test Steps**:
1. Setup: Clear table, load violation fixture
2. Execute: Run silver-etl
3. Verify: DQ flags populated correctly

**Expected Outcomes**:
- All 100 rows inserted (no drops)
- Rows with violations have dq_flags populated
- `action: clamp` rows have humidity clamped to [0, 100]
- Transparency table has violation records

```rust
// tests/integration/it002_dq_violations.rs

#[tokio::test]
#[ignore]
async fn test_dq_violations() {
    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    let fixture_path = test_env.load_fixture(
        "air-quality",
        "with_dq_violations.parquet"
    );

    // ============ ACT ============
    let result = silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: fixture_path.to_string_lossy().to_string(),
        ..Default::default()
    }).await.unwrap();

    // ============ ASSERT ============

    // All rows inserted (no drops)
    let total: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);
    assert_eq!(total, 100);

    // Rows with PM2.5 range violations (negative)
    let pm25_negative_flags: Vec<String> = test_env.db.query(
        "SELECT unnest(dq_flags) as flag
         FROM silver.air_quality_observations
         WHERE pm25 < 0 OR pm25 > 1000",
        &[]
    ).await.unwrap().iter().map(|r| r.get(0)).collect();

    assert!(
        pm25_negative_flags.iter().all(|f| f.contains("range_check:pm25")),
        "PM2.5 out-of-range should have range_check flag"
    );

    // Cross-field violation: PM10 < PM2.5
    let cross_field_flags: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE 'cross_field_check:pm10_less_than_pm25' = ANY(dq_flags)",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(cross_field_flags, 10, "Should have 10 cross-field violations");

    // Clamp verification: humidity should be [0, 100]
    let humidity_range = test_env.db.query_one(
        "SELECT MIN(humidity_pct), MAX(humidity_pct)
         FROM silver.air_quality_observations",
        &[]
    ).await.unwrap();

    let min_humidity: f64 = humidity_range.get(0);
    let max_humidity: f64 = humidity_range.get(1);

    assert!(min_humidity >= 0.0, "Humidity should be clamped to >= 0");
    assert!(max_humidity <= 100.0, "Humidity should be clamped to <= 100");

    // Clamped rows should have flags
    let clamped_flags: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE 'range_check:humidity_pct:clamped' = ANY(dq_flags)",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(clamped_flags, 20, "Should have 20 clamped humidity rows");

    // Transparency table verification
    let transparency_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.dq_transparency
         WHERE stream_id = 'air-quality'",
        &[]
    ).await.unwrap().get(0);

    assert!(transparency_count > 0, "Should have transparency records");

    // Metrics verification
    assert!(result.rows_with_dq_flags > 0);
}
```

**Verification Queries**:

```sql
-- IT-002.V1: DQ flag distribution
SELECT
    unnest(dq_flags) as flag,
    COUNT(*) as occurrences
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id
GROUP BY 1
ORDER BY 2 DESC;
-- Expected:
-- range_check:pm25:out_of_bounds       20
-- range_check:humidity_pct:clamped     20
-- cross_field_check:pm10_less_than_pm25 10
-- range_check:co2:out_of_bounds        10
-- range_check:temperature_c:out_of_bounds 10

-- IT-002.V2: Clamp action verification
SELECT
    COUNT(*) FILTER (WHERE humidity_pct = 0) as clamped_to_zero,
    COUNT(*) FILTER (WHERE humidity_pct = 100) as clamped_to_hundred
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id
  AND 'range_check:humidity_pct:clamped' = ANY(dq_flags);
-- Expected: clamped_to_zero = 10, clamped_to_hundred = 10

-- IT-002.V3: Multiple flags per row
SELECT COUNT(*) as multi_flag_rows
FROM silver.air_quality_observations
WHERE batch_id = $test_batch_id
  AND array_length(dq_flags, 1) > 1;
-- Expected: 0 (each row has at most one violation in test data)
```

---

### 3.3 IT-003: Late Arrivals

**Purpose**: Verify lag_interval catches late-arriving data.

**Preconditions**:
- Silver table with existing data (observation_time = T)
- New late data with observation_time = T - 3 hours
- lag_interval configured as 5 minutes (normal) vs 4 hours (for test)

**Test Steps**:
1. Setup: Insert initial data, then add late arrival fixture
2. Execute: Run ETL with normal lag_interval (5 min)
3. Verify: Late data NOT captured
4. Execute: Run ETL with extended lag_interval (4 hours)
5. Verify: Late data IS captured

```rust
// tests/integration/it003_late_arrivals.rs

#[tokio::test]
#[ignore]
async fn test_late_arrivals() {
    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    // ============ STEP 1: Load initial data ============
    let initial_fixture = test_env.load_fixture(
        "air-quality",
        "valid_hour.parquet"  // Recent data
    );

    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: initial_fixture.to_string_lossy().to_string(),
        ..Default::default()
    }).await.unwrap();

    let initial_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(initial_count, 60, "Initial load should have 60 rows");

    // ============ STEP 2: Add late arrival fixture ============
    let late_fixture = test_env.load_fixture(
        "air-quality",
        "late_arrivals.parquet"  // 3 hours old
    );

    // ============ STEP 3: Run with normal lag_interval (5 min) ============
    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: late_fixture.to_string_lossy().to_string(),
        lag_interval: Duration::minutes(5),
        ..Default::default()
    }).await.unwrap();

    let after_short_lag: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    // Late data should NOT be captured with 5 min lag
    assert_eq!(
        after_short_lag, 60,
        "Late data should NOT be captured with 5 min lag"
    );

    // ============ STEP 4: Run with extended lag_interval (4 hours) ============
    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: late_fixture.to_string_lossy().to_string(),
        lag_interval: Duration::hours(4),
        ..Default::default()
    }).await.unwrap();

    let after_long_lag: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    // Late data SHOULD be captured with 4 hour lag
    assert_eq!(
        after_long_lag, 90,  // 60 initial + 30 late
        "Late data SHOULD be captured with 4 hour lag"
    );

    // Verify late arrivals have freshness flag
    let late_flags: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE ndp_id LIKE '%_late'
           AND 'freshness_check:observation_time:stale' = ANY(dq_flags)",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(late_flags, 30, "All late arrivals should have freshness flag");
}
```

**Verification Queries**:

```sql
-- IT-003.V1: Watermark position
SELECT MAX(observation_time) as watermark
FROM silver.air_quality_observations;

-- IT-003.V2: Late arrival identification
SELECT
    COUNT(*) as late_rows,
    MIN(observation_time) as earliest_late,
    MAX(observation_time) as latest_late
FROM silver.air_quality_observations
WHERE ndp_id LIKE '%_late';

-- IT-003.V3: Freshness flag verification
SELECT COUNT(*) as stale_flagged
FROM silver.air_quality_observations
WHERE 'freshness_check:observation_time:stale' = ANY(dq_flags);
```

---

### 3.4 IT-004: Recovery

**Purpose**: Verify ETL resumes correctly after process crash/restart.

**Preconditions**:
- Partial data in Silver (simulate interrupted ETL)
- Full data in Bronze

**Test Steps**:
1. Setup: Load partial data (simulate interrupted ETL at row 30)
2. Execute: Run ETL (simulating restart)
3. Verify: Remaining rows processed, no duplicates

```rust
// tests/integration/it004_recovery.rs

#[tokio::test]
#[ignore]
async fn test_recovery_after_crash() {
    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    // ============ STEP 1: Simulate partial ETL (crashed at row 30) ============
    // Insert first 30 rows directly to simulate partial completion
    let partial_fixture = test_env.load_fixture(
        "air-quality",
        "valid_hour.parquet"
    );

    // Manually insert first 30 rows to simulate crash
    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: partial_fixture.to_string_lossy().to_string(),
        max_rows: Some(30),  // Stop after 30 rows (simulates crash)
        ..Default::default()
    }).await.unwrap();

    let partial_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(partial_count, 30, "Should have 30 rows after partial ETL");

    // Record watermark
    let watermark_before: DateTime<Utc> = test_env.db.query_one(
        "SELECT MAX(observation_time) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    // ============ STEP 2: Restart ETL (recovery) ============
    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: partial_fixture.to_string_lossy().to_string(),
        ..Default::default()  // No max_rows limit
    }).await.unwrap();

    // ============ STEP 3: Verify recovery ============
    let final_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    // Should have all 60 rows, no duplicates
    assert_eq!(final_count, 60, "Should have all 60 rows after recovery");

    // Verify no duplicates
    let duplicate_check: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM (
            SELECT observation_time, ndp_id, COUNT(*) as cnt
            FROM silver.air_quality_observations
            GROUP BY 1, 2
            HAVING COUNT(*) > 1
         ) duplicates",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(duplicate_check, 0, "Should have no duplicate rows");

    // Verify watermark advanced
    let watermark_after: DateTime<Utc> = test_env.db.query_one(
        "SELECT MAX(observation_time) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert!(
        watermark_after > watermark_before,
        "Watermark should advance after recovery"
    );
}
```

**Verification Queries**:

```sql
-- IT-004.V1: Duplicate check
SELECT observation_time, ndp_id, COUNT(*) as cnt
FROM silver.air_quality_observations
GROUP BY 1, 2
HAVING COUNT(*) > 1;
-- Expected: 0 rows (no duplicates)

-- IT-004.V2: Continuity check (no gaps)
WITH time_diffs AS (
    SELECT
        observation_time,
        observation_time - LAG(observation_time) OVER (
            PARTITION BY ndp_id ORDER BY observation_time
        ) as gap
    FROM silver.air_quality_observations
)
SELECT COUNT(*) as large_gaps
FROM time_diffs
WHERE gap > INTERVAL '2 minutes';
-- Expected: 0 (no large gaps in continuous data)
```

---

### 3.5 IT-005: New Stream (Config-Only Addition)

**Purpose**: Verify a new stream can be added with YAML configuration only (no Rust code changes).

**Preconditions**:
- No silver.test_new_stream table exists
- New stream config: `new-stream-test.yaml`
- Fixture: `new-stream/valid_data.parquet`

**Test Steps**:
1. Setup: Create target table via migration
2. Load new stream config to etcd
3. Execute: Run silver-etl for new stream
4. Verify: Data appears in Silver

```rust
// tests/integration/it005_new_stream.rs

#[tokio::test]
#[ignore]
async fn test_new_stream_config_only() {
    let test_env = TestEnvironment::setup().await;

    // ============ STEP 1: Create new target table ============
    // This simulates running a schema migration for the new stream
    test_env.db.execute(
        "CREATE TABLE IF NOT EXISTS silver.test_new_stream (
            observation_time    TIMESTAMPTZ NOT NULL,
            ndp_id              TEXT NOT NULL,
            metric_a            DOUBLE PRECISION,
            metric_b            DOUBLE PRECISION,
            status              TEXT,
            dq_flags            TEXT[] DEFAULT '{}'::TEXT[],
            ingestion_time      TIMESTAMPTZ DEFAULT NOW(),
            batch_id            TEXT,
            PRIMARY KEY (observation_time, ndp_id)
        )",
        &[]
    ).await.unwrap();

    test_env.db.execute(
        "SELECT create_hypertable('silver.test_new_stream', 'observation_time',
         if_not_exists => TRUE)",
        &[]
    ).await.unwrap();

    // ============ STEP 2: Load config to etcd ============
    let new_config = include_str!("../fixtures/configs/new-stream-test.yaml");
    test_env.etcd.put("/streams/test-new-stream/config", new_config).await.unwrap();

    // ============ STEP 3: Load fixture ============
    let fixture_path = test_env.load_fixture(
        "new-stream",
        "valid_data.parquet"
    );

    // ============ STEP 4: Run ETL ============
    // CRITICAL: No Rust code changes required - config drives everything
    let result = silver_etl::run(SilverEtlConfig {
        stream_id: "test-new-stream".to_string(),
        bronze_path: fixture_path.to_string_lossy().to_string(),
        ..Default::default()
    }).await;

    assert!(result.is_ok(), "ETL should succeed for new stream: {:?}", result);

    // ============ STEP 5: Verify data in Silver ============
    let row_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.test_new_stream",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(row_count, 20, "Should have 20 rows from new stream");

    // Verify column values
    let sample = test_env.db.query_one(
        "SELECT metric_a, metric_b, status
         FROM silver.test_new_stream
         ORDER BY observation_time
         LIMIT 1",
        &[]
    ).await.unwrap();

    assert!(sample.get::<_, f64>(0) >= 100.0, "metric_a should be >= 100");
    assert!(sample.get::<_, f64>(1) >= 200.0, "metric_b should be >= 200");
    assert_eq!(sample.get::<_, String>(2), "active");

    // ============ CLEANUP ============
    test_env.db.execute("DROP TABLE silver.test_new_stream", &[]).await.unwrap();
}
```

**Verification Queries**:

```sql
-- IT-005.V1: New table populated
SELECT COUNT(*) as rows
FROM silver.test_new_stream;
-- Expected: 20

-- IT-005.V2: Schema matches config
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'silver'
  AND table_name = 'test_new_stream'
ORDER BY ordinal_position;
-- Expected columns: observation_time, ndp_id, metric_a, metric_b, status, dq_flags, ...

-- IT-005.V3: DQ rules applied
SELECT COUNT(*) as flagged
FROM silver.test_new_stream
WHERE array_length(dq_flags, 1) > 0;
-- Expected: 0 (all data valid in fixture)
```

---

### 3.6 IT-006: Schema Evolution

**Purpose**: Verify schema changes (adding columns) via config migration.

**Preconditions**:
- Existing air_quality_observations table with data
- Modified config with new column: `co2_compensated`

**Test Steps**:
1. Setup: Populate Silver with existing schema
2. Apply schema migration (add column)
3. Update config to include new field mapping
4. Execute: Run ETL with new fixture including the field
5. Verify: Existing data preserved, new column populated

```rust
// tests/integration/it006_schema_evolution.rs

#[tokio::test]
#[ignore]
async fn test_schema_evolution() {
    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    // ============ STEP 1: Load initial data ============
    let initial_fixture = test_env.load_fixture(
        "air-quality",
        "valid_hour.parquet"
    );

    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: initial_fixture.to_string_lossy().to_string(),
        ..Default::default()
    }).await.unwrap();

    let initial_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(initial_count, 60, "Initial data should be loaded");

    // ============ STEP 2: Apply schema migration ============
    test_env.db.execute(
        "ALTER TABLE silver.air_quality_observations
         ADD COLUMN IF NOT EXISTS co2_compensated SMALLINT",
        &[]
    ).await.unwrap();

    // ============ STEP 3: Update config with new field mapping ============
    let evolved_config = r#"
stream_id: air-quality
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  field_mappings:
    # ... existing mappings ...
    - source_path: raw_payload.co2Compensated
      target_column: co2_compensated
      type: smallint
      nullable: true
"#;

    test_env.etcd.put("/streams/air-quality/config", evolved_config).await.unwrap();

    // ============ STEP 4: Load new data with the new field ============
    // Create fixture with co2Compensated field
    let evolved_fixture = test_env.create_fixture_with_evolved_schema();

    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: evolved_fixture.to_string_lossy().to_string(),
        ..Default::default()
    }).await.unwrap();

    // ============ STEP 5: Verify ============

    // Existing data preserved
    let total_count: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations",
        &[]
    ).await.unwrap().get(0);

    assert!(total_count > 60, "Should have more rows after new data");

    // Old rows have NULL in new column
    let old_rows_null: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE co2_compensated IS NULL
           AND observation_time < NOW() - INTERVAL '30 minutes'",
        &[]
    ).await.unwrap().get(0);

    assert_eq!(old_rows_null, 60, "Old rows should have NULL in new column");

    // New rows have values in new column
    let new_rows_populated: i64 = test_env.db.query_one(
        "SELECT COUNT(*) FROM silver.air_quality_observations
         WHERE co2_compensated IS NOT NULL",
        &[]
    ).await.unwrap().get(0);

    assert!(new_rows_populated > 0, "New rows should have co2_compensated values");
}
```

**Verification Queries**:

```sql
-- IT-006.V1: Column exists
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'silver'
  AND table_name = 'air_quality_observations'
  AND column_name = 'co2_compensated';
-- Expected: 1 row

-- IT-006.V2: Old data preserved
SELECT COUNT(*) as old_rows
FROM silver.air_quality_observations
WHERE co2_compensated IS NULL;
-- Expected: 60 (original rows)

-- IT-006.V3: New data has new field
SELECT COUNT(*) as new_rows
FROM silver.air_quality_observations
WHERE co2_compensated IS NOT NULL;
-- Expected: > 0 (new rows with field populated)
```

---

## 4. Test Infrastructure Code

### 4.1 Test Environment Helper

```rust
// tests/common/test_environment.rs

use deadpool_postgres::{Config, Pool, Runtime};
use etcd_client::Client as EtcdClient;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct TestEnvironment {
    pub db: Pool,
    pub etcd: EtcdClient,
    pub temp_dir: TempDir,
    pub bronze_path: PathBuf,
}

impl TestEnvironment {
    pub async fn setup() -> Self {
        // Wait for containers to be ready
        wait_for_postgres("localhost", 5433).await;
        wait_for_etcd("localhost", 2380).await;

        // Create database pool
        let mut cfg = Config::new();
        cfg.host = Some("localhost".to_string());
        cfg.port = Some(5433);
        cfg.dbname = Some("ndp_silver_test".to_string());
        cfg.user = Some("ndp_test".to_string());
        cfg.password = Some("ndp_test_password".to_string());

        let pool = cfg.create_pool(Some(Runtime::Tokio1), tokio_postgres::NoTls)
            .expect("Failed to create pool");

        // Create etcd client
        let etcd = EtcdClient::connect(["localhost:2380"], None)
            .await
            .expect("Failed to connect to etcd");

        // Setup temp directory for fixtures
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let bronze_path = temp_dir.path().join("bronze");
        std::fs::create_dir_all(&bronze_path).unwrap();

        Self {
            db: pool,
            etcd,
            temp_dir,
            bronze_path,
        }
    }

    pub async fn clear_silver_tables(&self) {
        let client = self.db.get().await.unwrap();

        client.execute("TRUNCATE silver.air_quality_observations CASCADE", &[])
            .await.unwrap();
        client.execute("TRUNCATE silver.weather_observations CASCADE", &[])
            .await.unwrap();
        client.execute("TRUNCATE silver.weather_forecasts CASCADE", &[])
            .await.unwrap();
        client.execute("TRUNCATE silver.outdoor_air_quality CASCADE", &[])
            .await.unwrap();
        client.execute("TRUNCATE silver.dq_transparency CASCADE", &[])
            .await.unwrap();
    }

    pub fn load_fixture(&self, stream_id: &str, filename: &str) -> PathBuf {
        let stream_dir = self.bronze_path.join(stream_id);
        std::fs::create_dir_all(&stream_dir).unwrap();

        // Copy or generate fixture
        let fixture_path = stream_dir.join(filename);

        // In actual implementation, copy from tests/fixtures/parquet/
        // or generate using fixture module

        fixture_path
    }
}

async fn wait_for_postgres(host: &str, port: u16) {
    use std::time::Duration;
    use tokio::time::sleep;

    for _ in 0..30 {
        if tokio_postgres::connect(
            &format!("host={} port={} user=ndp_test password=ndp_test_password dbname=ndp_silver_test", host, port),
            tokio_postgres::NoTls
        ).await.is_ok() {
            return;
        }
        sleep(Duration::from_secs(1)).await;
    }
    panic!("PostgreSQL not ready after 30 seconds");
}

async fn wait_for_etcd(host: &str, port: u16) {
    use std::time::Duration;
    use tokio::time::sleep;

    for _ in 0..30 {
        if etcd_client::Client::connect([&format!("{}:{}", host, port)], None)
            .await.is_ok() {
            return;
        }
        sleep(Duration::from_secs(1)).await;
    }
    panic!("etcd not ready after 30 seconds");
}
```

### 4.2 Test Runner Script

```bash
#!/bin/bash
# tests/run_integration_tests.sh

set -e

echo "=== DP-006 Integration Tests ==="

# Start test infrastructure
echo "Starting test infrastructure..."
docker-compose -f tests/docker/docker-compose.test.yml up -d

# Wait for services
echo "Waiting for services to be ready..."
sleep 10

# Run integration tests
echo "Running integration tests..."
cargo test --package silver-etl --test '*' -- --ignored --test-threads=1

# Capture exit code
TEST_EXIT_CODE=$?

# Cleanup
echo "Cleaning up..."
docker-compose -f tests/docker/docker-compose.test.yml down -v

exit $TEST_EXIT_CODE
```

---

## 5. Performance Test Scenarios

### 5.1 Hourly Batch Performance

**Requirement**: ETL batch completes in < 60 seconds

```rust
#[tokio::test]
#[ignore]
async fn bench_hourly_batch() {
    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    // Generate realistic hourly data: 7 streams, 1 reading/minute each
    // Total: 7 * 60 = 420 rows
    let streams = ["air-quality", "outdoor-weather", "outdoor-air-quality",
                   "nws-observations", "nws-forecast-hourly", "nws-gridpoints-forecast",
                   "outdoor-weather-owm"];

    for stream in &streams {
        let fixture = test_env.create_fixture(stream, 60);
        test_env.etcd.put(
            &format!("/streams/{}/config", stream),
            &get_stream_config(stream)
        ).await.unwrap();
    }

    let start = std::time::Instant::now();

    for stream in &streams {
        silver_etl::run(SilverEtlConfig {
            stream_id: stream.to_string(),
            ..Default::default()
        }).await.unwrap();
    }

    let duration = start.elapsed();

    println!("Hourly batch duration: {:?}", duration);
    assert!(
        duration.as_secs() < 60,
        "Hourly batch should complete in < 60s, took {:?}",
        duration
    );
}
```

### 5.2 Memory Usage

**Requirement**: < 300MB peak memory

```rust
#[tokio::test]
#[ignore]
async fn bench_memory_usage() {
    // This test requires external memory monitoring
    // Run with: /usr/bin/time -v cargo test bench_memory_usage -- --ignored

    let test_env = TestEnvironment::setup().await;
    test_env.clear_silver_tables().await;

    // Large fixture: 24 hours of data
    let fixture = test_env.create_large_fixture("air-quality", 1440);  // 24 * 60

    silver_etl::run(SilverEtlConfig {
        stream_id: "air-quality".to_string(),
        bronze_path: fixture.to_string_lossy().to_string(),
        ..Default::default()
    }).await.unwrap();

    // Memory verification done via external tool
    // Check: Maximum resident set size < 300MB
}
```

---

## 6. Test Execution Summary

### 6.1 Test Categories and Durations

| Category | Test Count | Est. Duration | Notes |
|----------|------------|---------------|-------|
| Happy Path | 1 | 30s | P0, must pass |
| DQ Violations | 1 | 45s | P0, critical for quality |
| Late Arrivals | 1 | 30s | P1, data freshness |
| Recovery | 1 | 60s | P1, reliability |
| New Stream | 1 | 45s | P1, extensibility |
| Schema Evolution | 1 | 60s | P2, maintainability |
| **Total** | **6** | **~5 min** | |

### 6.2 CI/CD Integration

```yaml
# .github/workflows/dp006-integration-tests.yml
name: DP-006 Integration Tests

on:
  pull_request:
    paths:
      - 'apps/silver-etl/**'
      - 'core/src/config/silver_etl.rs'
      - 'config/base/streams/*/config.yaml'

jobs:
  integration-tests:
    runs-on: ubuntu-latest

    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg15
        ports:
          - 5433:5432
        env:
          POSTGRES_USER: ndp_test
          POSTGRES_PASSWORD: ndp_test_password
          POSTGRES_DB: ndp_silver_test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

      etcd:
        image: quay.io/coreos/etcd:v3.5.9
        ports:
          - 2380:2379
        env:
          ETCD_LISTEN_CLIENT_URLS: http://0.0.0.0:2379
          ETCD_ADVERTISE_CLIENT_URLS: http://localhost:2379

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Initialize TimescaleDB Schema
        run: |
          PGPASSWORD=ndp_test_password psql -h localhost -p 5433 -U ndp_test -d ndp_silver_test \
            -f tests/docker/init-scripts/001-create-silver-schema.sql

      - name: Run Integration Tests
        run: cargo test --package silver-etl --test '*' -- --ignored --test-threads=1
        env:
          TEST_POSTGRES_HOST: localhost
          TEST_POSTGRES_PORT: 5433
          TEST_ETCD_HOST: localhost
          TEST_ETCD_PORT: 2380
```

---

## 7. Expected Results Reference

### 7.1 IT-001 Happy Path Expected Counts

```json
{
  "test_id": "IT-001",
  "input_rows": 60,
  "expected": {
    "silver_row_count": 60,
    "rows_with_dq_flags": 0,
    "rows_dropped": 0,
    "transparency_records": 0
  },
  "timing": {
    "max_duration_seconds": 60
  }
}
```

### 7.2 IT-002 DQ Violations Expected Flags

```json
{
  "test_id": "IT-002",
  "input_rows": 100,
  "expected": {
    "silver_row_count": 100,
    "dq_flag_distribution": {
      "range_check:pm25:out_of_bounds": 20,
      "range_check:humidity_pct:clamped": 20,
      "cross_field_check:pm10_less_than_pm25": 10,
      "range_check:co2:out_of_bounds": 10,
      "range_check:temperature_c:out_of_bounds": 10
    },
    "rows_with_dq_flags": 70,
    "rows_dropped": 0
  }
}
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Tester | Initial integration test plan |

---

## References

1. `product/features/dp-006/specification/SPECIFICATION.md` - Feature specification
2. `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - DQ framework design
3. `docs/testing/AIR-005-TEST-DESIGN.md` - London School TDD patterns
4. `apps/air-quality-app/tests/silver_layer_integration_test.rs` - Existing DuckDB tests
