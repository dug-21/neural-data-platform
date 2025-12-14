# SPARC Specification: Neural Data Platform - Air Quality Module (air-001)

**Version:** 1.2.0
**Date:** December 13, 2025
**Status:** Validated - Actual Sensor Data Reviewed
**Author:** SPARC Specification Agent (Revised)

---

## Executive Summary

### Project Vision

The Neural Data Platform - Air Quality Module (air-001) represents the **first domain implementation** of a generic time-series data platform designed for real-time sensor data ingestion, storage, querying, and ML-powered forecasting. This module focuses on indoor air quality (IAQ) monitoring using AirGradient ONE sensors connected via MQTT, providing health-based alerting, ML-driven predictions, and actionable recommendations for residential and commercial environments.

**Core Philosophy:** Build a **GENERIC time-series platform** where air quality is the first domain adapter. Future domains (energy monitoring, financial data, industrial IoT sensors) should take ~8 hours to add via the domain adapter pattern.

### Scope

**In Scope:**
- **Dockerized deployment** - Single Docker image runs on Mac (dev), Pi5 (prod), and cloud (future)
- **Dual data ingestion** from AirGradient ONE sensors:
  - MQTT subscription: `airgradient/readings/{SERIAL_NUMBER}` topic
  - Local HTTP API polling: `http://airgradient_{SERIAL}.local/measures/current`
- **Complete sensor data capture** (29+ fields from AirGradient ONE v9 firmware 3.1.4+):
  - Particulate Matter: pm01, pm02, pm10 (atmospheric + standard), pm02Compensated
  - Particle Counts: pm003Count, pm005Count, pm01Count, pm02Count, pm50Count, pm10Count
  - Gases: rco2 (CO2 ppm), tvocIndex, tvocRaw, noxIndex, noxRaw
  - Environmental: atmp, atmpCompensated, rhum, rhumCompensated
  - Device: wifi, serialno, boot, bootCount, firmware, model, ledMode
- Parquet-based time-series storage with daily partitioning (S3-portable)
- Polars query engine for time-range queries and aggregations
- ruv-FANN forecasting engine (NHITS/NBEATSx models from vendor/ruv-fann/neuro-divergent)
- Health threshold alerting (CO2, PM2.5, VOC-based)
- Predictive alerting (forecast-driven)
- MCP tool integration for Claude-based interactions
- Domain extensibility framework (traits + adapters)

**Out of Scope (v1.0):**
- Outdoor ambient air quality monitoring (except I/O ratio calculations in future versions)
- Advanced analytics layer (QuestDB/TimescaleDB integration deferred)
- HomeKit/MQTT publishing for home automation (future enhancement)
- Multi-sensor aggregation (single sensor deployment initially)

### Success Criteria

1. **Functional Success:**
   - Ingest AirGradient MQTT messages at 1-minute intervals with <1s latency
   - Store 1 year of data (525,600 readings) in <500MB Parquet files
   - Query 24-hour range in <100ms (Polars in-memory)
   - Generate 6-hour PM2.5/CO2 forecasts with <30s cold-start latency
   - Detect health threshold violations within 1 minute
   - Expose 5+ MCP tools for Claude interactions

2. **Non-Functional Success:**
   - Platform portability: Single codebase runs on Mac (development) and Pi5 16GB (production)
   - Crash recovery: Resume from last committed Parquet partition without data loss
   - New domain adapter implementation: <8 hours for experienced developer
   - Zero external service dependencies (no Redis/PostgreSQL required)

3. **Technical Success:**
   - Rust-only implementation (no Python/Node.js runtime dependencies)
   - ruv-FANN integration with 27+ neural models from vendor/ruv-fann/neuro-divergent
   - Generic core abstractions (TimeSeriesEvent, DataSource, Predictor traits)
   - Air quality domain isolated in adapter layer

---

## 1. Functional Requirements

### FR-1: Data Ingestion (MQTT from AirGradient ONE)

**FR-1.1: MQTT Client Connection**
- **Description:** Establish persistent MQTT connection to AirGradient ONE sensor
- **Acceptance Criteria:**
  - Connect to MQTT broker (mosquitto or airgradient cloud) with TLS support
  - Subscribe to sensor topic pattern: `airgradient/readings/{SERIAL_NUMBER}`
  - Auto-reconnect on connection loss with exponential backoff (1s, 2s, 4s, 8s, max 30s)
  - Log connection events (connect, disconnect, reconnect)
- **Priority:** HIGH
- **Dependencies:** rumqttc crate (Rust MQTT client)

**FR-1.2: Message Parsing and Validation**
- **Description:** Parse incoming MQTT/HTTP JSON payloads and validate against AirGradient schema
- **Acceptance Criteria:**
  - Parse complete AirGradient ONE JSON payload (29+ fields from firmware 3.1.4+):
    ```json
    {
      "wifi": -46,
      "serialno": "ecda3b1eaaaf",
      "rco2": 447,
      "pm01": 3,
      "pm02": 7,
      "pm10": 8,
      "pm02Compensated": 6,
      "pm01Standard": 3,
      "pm02Standard": 7,
      "pm10Standard": 8,
      "pm003Count": 442,
      "pm005Count": 380,
      "pm01Count": 98,
      "pm02Count": 12,
      "pm50Count": 2,
      "pm10Count": 1,
      "atmp": 25.87,
      "atmpCompensated": 24.47,
      "rhum": 43,
      "rhumCompensated": 49,
      "tvocIndex": 100,
      "tvocRaw": 33051,
      "noxIndex": 1,
      "noxRaw": 16307,
      "boot": 6,
      "bootCount": 6,
      "ledMode": "pm",
      "firmware": "3.1.3",
      "model": "I-9PSL"
    }
    ```
  - Validate field types per AirGradient schema (floats for PM/counts/raw values, integers for indices, floats for temp/humidity)
  - Validate ranges per sensor specs:
    - CO2 (rco2): 380-10,000 ppm (Senseair S8)
    - PM2.5 (pm02): 0-500 µg/m³ (Plantower PMS5003)
    - TVOC Index: 1-500 (Sensirion SGP41)
    - NOx Index: 1-500 (Sensirion SGP41)
    - Temperature: -10 to 50°C (SHT4x)
    - Humidity: 0-100% (SHT4x)
  - Support both MQTT topic format and Local API response format
  - Reject malformed messages to Dead Letter Queue (DLQ)
  - Apply quality scoring (completeness, freshness, validity) per existing `data-staging/quality_scorer.rs` patterns
- **Priority:** HIGH
- **Dependencies:** serde_json, chrono
- **References:**
  - MQTT: https://www.airgradient.com/support/kb-mqtt-conf/
  - Local API: https://github.com/airgradienthq/arduino/blob/master/docs/local-server.md

**FR-1.3: Data Quality Assessment**
- **Description:** Calculate reading quality score based on completeness, sensor calibration status, and freshness
- **Acceptance Criteria:**
  - Quality score = `completeness × calibration_status × freshness_factor` (0.0-1.0 scale)
  - Completeness: Percentage of expected fields present (5/5 fields = 1.0)
  - Calibration status: CO2 sensor warmup period (<3 weeks) = 0.7x penalty, PM high humidity (>80% RH) = 0.9x penalty
  - Freshness: Age of reading (<5s = 1.0, 5-30s = 0.9, >30s = 0.7)
  - Attach quality flags: `["co2_warmup_period", "pm_high_humidity"]` for downstream filtering
- **Priority:** MEDIUM
- **References:** Existing `data-staging/src/quality_scorer.rs` (adapt for air quality domain)

**FR-1.4: Ingestion Rate Limits**
- **Description:** Handle variable sensor polling rates (1-minute default, configurable)
- **Acceptance Criteria:**
  - Support 1-second to 10-minute polling intervals
  - Buffer up to 1000 readings in memory before backpressure
  - Drop oldest readings if buffer full (FIFO policy)
  - Emit metrics: `readings_received_total`, `readings_dropped_total`, `ingestion_latency_ms`
- **Priority:** MEDIUM

**FR-1.5: Configuration Endpoint Retrieval**
- **Description:** Retrieve sensor configuration to properly interpret readings
- **Acceptance Criteria:**
  - Fetch config from `http://airgradient_{SERIAL}.local/config` on startup
  - Cache config for 1 hour (config rarely changes)
  - Extract critical fields:
    - `temperatureUnit`: "c" or "f" (determines if atmp needs conversion)
    - `pmStandard`: "ugm3" (confirms PM units)
    - `corrections.pm02.correctionAlgorithm`: "epa_2021", "lrapa", or "none"
    - `abcDays`: CO2 ABC calibration period (quality scoring)
    - `tvocLearningOffset`: VOC sensor learning period in hours
    - `noxLearningOffset`: NOx sensor learning period in hours
  - Config schema:
    ```json
    {
      "country": "US",
      "pmStandard": "ugm3",
      "temperatureUnit": "f",
      "abcDays": 8,
      "tvocLearningOffset": 12,
      "noxLearningOffset": 12,
      "corrections": {
        "pm02": {
          "correctionAlgorithm": "epa_2021",
          "slr": null
        }
      },
      "model": "I-9PSL-DE"
    }
    ```
  - Log warning if unexpected config values detected
  - Validate config version compatibility
- **Priority:** HIGH
- **Dependencies:** None
- **References:** Actual sensor config from `/config` endpoint

---

### FR-2: Storage (Parquet, Partitioned by Day)

**FR-2.1: Parquet File Format**
- **Description:** Store time-series data in Apache Parquet columnar format for efficient compression and querying
- **Acceptance Criteria:**
  - Schema (complete AirGradient ONE fields):
    ```
    timestamp: Timestamp(Microsecond, UTC)
    location_id: Utf8 (serialno)

    # Particulate Matter (µg/m³) - Float32 per actual sensor output
    pm01: Float32, pm02: Float32, pm10: Float32
    pm02_compensated: Float32
    pm01_standard: Float32, pm02_standard: Float32, pm10_standard: Float32

    # Particle Counts (per dL) - Float32 per actual sensor output (averaged values)
    pm003_count: Float32, pm005_count: Float32, pm01_count: Float32
    pm02_count: Float32, pm50_count: Float32, pm10_count: Float32

    # Gases
    rco2: UInt16 (CO2 ppm)
    tvoc_index: UInt16, tvoc_raw: Float32
    nox_index: UInt16, nox_raw: Float32

    # Environmental
    atmp: Float32, atmp_compensated: Float32 (°C - verify temperatureUnit from /config)
    rhum: Float32, rhum_compensated: Float32 (%)

    # Device Metadata
    wifi: Int8 (dBm), boot: UInt32, firmware: Utf8, model: Utf8
    led_mode: Utf8 (current LED display mode)

    # Quality
    quality_score: Float32, quality_flags: List<Utf8>
    ```
  - Compression: Snappy (fast encode/decode) or Zstd (better compression for archival)
  - File size target: ~15MB per day (1440 readings/day × 29+ fields)
  - Portable across Mac, Pi5, and S3 (no platform-specific dependencies)
- **Priority:** HIGH
- **Dependencies:** arrow, parquet crates

**FR-2.2: Daily Partitioning**
- **Description:** Organize Parquet files by date for efficient time-range queries and archival
- **Acceptance Criteria:**
  - File path pattern: `data/air_quality/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet`
  - Example: `data/air_quality/living_room/year=2025/month=12/day=13/readings.parquet`
  - Create new partition at midnight UTC
  - Commit previous day's partition atomically (write to temp file, then rename)
- **Priority:** HIGH

**FR-2.3: Write-Ahead Log (WAL) for Crash Recovery**
- **Description:** Buffer in-memory readings to WAL before Parquet commit
- **Acceptance Criteria:**
  - Append-only WAL file: `data/air_quality/{location_id}/wal/{date}.wal`
  - Write format: Newline-delimited JSON (NDJSON) for simplicity
  - Commit WAL to Parquet every 5 minutes or 100 readings (whichever first)
  - On startup: Replay uncommitted WAL entries to Parquet
  - Delete WAL after successful Parquet commit
- **Priority:** HIGH
- **Edge Cases:**
  - Power loss during Parquet write → Replay WAL on restart
  - Corrupted WAL entry → Skip and log error, continue with valid entries

**FR-2.4: Storage Capacity Management**
- **Description:** Manage disk usage with configurable retention policies
- **Acceptance Criteria:**
  - Default retention: 1 year (365 days × ~700KB = ~250MB per location)
  - Configurable retention: 30/90/180/365 days
  - Auto-delete partitions older than retention period (weekly background task)
  - Emit metrics: `storage_bytes_total`, `partitions_total`, `oldest_partition_days`
- **Priority:** MEDIUM

---

### FR-3: Querying (Polars, Time-Range, Aggregations)

**FR-3.1: Time-Range Queries**
- **Description:** Query air quality data for specific time windows
- **Acceptance Criteria:**
  - API: `query_range(location_id, start_time, end_time, metrics: Vec<Metric>) -> DataFrame`
  - Example: `query_range("living_room", "2025-12-13T00:00:00Z", "2025-12-13T23:59:59Z", vec![CO2, PM25])`
  - Load only relevant Parquet partitions (predicate pushdown)
  - Return Polars DataFrame with selected columns
  - Performance: <100ms for 24-hour query (1440 rows), <500ms for 7-day query (10,080 rows)
- **Priority:** HIGH
- **Dependencies:** polars crate

**FR-3.2: Aggregations**
- **Description:** Compute statistical aggregations over time windows
- **Acceptance Criteria:**
  - Supported aggregations: `mean`, `min`, `max`, `p50`, `p95`, `p99`, `std_dev`, `count`
  - Supported intervals: 1min, 5min, 15min, 1hour, 1day
  - API: `aggregate(location_id, start_time, end_time, metric: Metric, aggregation: Agg, interval: Duration) -> Vec<(Timestamp, f64)>`
  - Example: `aggregate("living_room", ..., PM25, Mean, 1hour)` → hourly PM2.5 averages
  - Use Polars `groupby_dynamic()` for efficient resampling
- **Priority:** HIGH

**FR-3.3: Multi-Location Queries**
- **Description:** Query across multiple sensor locations (future-proofing)
- **Acceptance Criteria:**
  - API: `query_multi_location(location_ids: Vec<String>, ...) -> HashMap<String, DataFrame>`
  - Parallelize partition reads using Rayon (1 thread per location)
  - Performance: Linear scaling with location count
- **Priority:** LOW (deferred to v1.1)

**FR-3.4: In-Memory Caching**
- **Description:** Cache recent data (last 24 hours) in memory for fast queries
- **Acceptance Criteria:**
  - Cache last 1440 readings (24 hours × 60 readings/hour) per location
  - Evict oldest entries when exceeding 10,000 total cached readings (LRU policy)
  - Cache hit rate >80% for dashboard queries (last 1-6 hours)
  - Emit metrics: `cache_hits_total`, `cache_misses_total`, `cache_size_bytes`
- **Priority:** MEDIUM

---

### FR-4: Forecasting (ruv-FANN, NHITS/NBEATSx)

**FR-4.1: Model Integration**
- **Description:** Integrate ruv-FANN neural forecasting models from vendor/ruv-fann/neuro-divergent
- **Acceptance Criteria:**
  - Load pre-trained NHITS model for PM2.5 forecasting (1-6 hour horizon)
  - Load pre-trained NBEATSx model for CO2 forecasting (1-6 hour horizon)
  - Model input: Last 24 hours of readings (1440 timesteps × features)
  - Model output: 360 timesteps (6 hours × 60 readings/hour) with confidence intervals (p10, p50, p90)
  - Cold-start latency: <30s (model load + inference)
  - Inference latency: <2s (warm cache)
- **Priority:** HIGH
- **Dependencies:** vendor/ruv-fann/neuro-divergent (27+ models available)

**FR-4.2: Feature Engineering**
- **Description:** Transform raw readings into model-ready features
- **Acceptance Criteria:**
  - Time features: `hour_of_day`, `day_of_week`, `is_weekend` (captures occupancy patterns)
  - Lag features: `pm25_lag_1h`, `pm25_lag_3h`, `pm25_lag_24h` (temporal dependencies)
  - Rolling statistics: `pm25_rolling_mean_1h`, `pm25_rolling_std_1h` (trend detection)
  - Multi-pollutant features: `pm25`, `co2`, `voc_index`, `temp_c`, `humidity_pct` (cross-correlations)
  - Normalization: Z-score normalization per feature (mean=0, std=1)
- **Priority:** HIGH
- **References:** Existing `neural-ml-ops/src/features/` framework

**FR-4.3: Forecast Storage**
- **Description:** Store forecast outputs alongside historical data
- **Acceptance Criteria:**
  - Parquet schema: `forecast_timestamp, target_timestamp, metric, p10, p50, p90, model_version`
  - Example: Forecast generated at 14:00 for 15:00-20:00 (6 hours ahead)
  - File path: `data/air_quality/{location_id}/forecasts/year={YYYY}/month={MM}/forecast_{timestamp}.parquet`
  - Retention: 7 days (forecasts older than actual data have no value)
- **Priority:** MEDIUM

**FR-4.4: Model Retraining**
- **Description:** Periodically retrain models with new data (future enhancement)
- **Acceptance Criteria:**
  - Deferred to v1.1 (use pre-trained models initially)
  - Trigger: Weekly or when forecast error degrades >20% (MAE increase)
  - Training data: Last 90 days of readings
  - Validation: Hold-out last 7 days, evaluate MAE/RMSE/MAPE
- **Priority:** LOW (deferred)

---

### FR-5: Alerting (Threshold-Based, Predictive)

**FR-5.1: Health Threshold Alerts**
- **Description:** Trigger alerts when pollutant levels exceed health thresholds
- **Acceptance Criteria:**
  - CO2 thresholds: >1000 ppm (Moderate), >1500 ppm (Poor), >2000 ppm (Very Poor)
  - PM2.5 thresholds: >12 µg/m³ (USG), >35 µg/m³ (Unhealthy), >55 µg/m³ (Very Unhealthy)
  - VOC index thresholds: >150 (Moderate), >200 (Poor), >300 (Very Poor)
  - Alert structure: `{timestamp, location_id, metric, value, threshold, severity, message}`
  - Example: `{timestamp: "2025-12-13T14:30:00Z", metric: "co2_ppm", value: 1650, threshold: 1500, severity: "Poor", message: "Drowsiness and fatigue likely. Ventilate immediately."}`
  - Deduplicate: Don't re-alert if condition persists (require 10% drop before clearing)
- **Priority:** HIGH

**FR-5.2: Predictive Alerts**
- **Description:** Trigger alerts based on forecast predictions (proactive notification)
- **Acceptance Criteria:**
  - Alert if forecast predicts threshold violation in next 1-3 hours
  - Example: "PM2.5 forecasted to exceed 35 µg/m³ in 2 hours. Consider pre-filtering or closing windows."
  - Confidence requirement: Only alert if p90 forecast exceeds threshold (high confidence)
  - Lead time: 1-6 hours (configurable)
- **Priority:** MEDIUM

**FR-5.3: Alert Delivery**
- **Description:** Deliver alerts via configured channels
- **Acceptance Criteria:**
  - v1.0: Log alerts to file (`data/alerts/{location_id}/alerts.jsonl`)
  - v1.0: Expose alerts via MCP tool (`get_recent_alerts`)
  - Future: Email, Slack, HomeKit notifications (deferred)
- **Priority:** HIGH (logging), LOW (external channels)

**FR-5.4: Alert History**
- **Description:** Store alert history for analysis
- **Acceptance Criteria:**
  - Parquet schema: `alert_id, timestamp, location_id, metric, value, threshold, severity, message, acknowledged, acknowledged_by, acknowledged_at`
  - Retention: 1 year
  - Query API: `get_alerts(location_id, start_time, end_time, severity_filter)`
- **Priority:** MEDIUM

---

### FR-6: MCP Integration (Claude Tools)

**FR-6.1: Air Quality Query Tool**
- **Description:** MCP tool for Claude to query current/historical air quality
- **Acceptance Criteria:**
  - Tool name: `air_quality_query`
  - Input schema: `{location_id: String, time_range: "current" | "last_hour" | "last_24h" | "last_7d", metrics: Vec<"co2" | "pm25" | "voc" | "temp" | "humidity">}`
  - Output: JSON with readings and health interpretations
  - Example: `{co2_ppm: 850, co2_level: "Acceptable", pm25_ugm3: 8.2, pm25_level: "Good", ...}`
- **Priority:** HIGH
- **References:** Existing `mcp-trading-server/src/tools/` framework

**FR-6.2: Forecast Tool**
- **Description:** MCP tool for Claude to retrieve forecasts
- **Acceptance Criteria:**
  - Tool name: `air_quality_forecast`
  - Input schema: `{location_id: String, metric: "co2" | "pm25", horizon_hours: 1-6}`
  - Output: Forecast time series with confidence intervals
  - Example: `[{time: "15:00", pm25_p50: 9.5, pm25_p10: 7.2, pm25_p90: 12.8}, ...]`
- **Priority:** HIGH

**FR-6.3: Alert Retrieval Tool**
- **Description:** MCP tool for Claude to retrieve active/recent alerts
- **Acceptance Criteria:**
  - Tool name: `air_quality_alerts`
  - Input schema: `{location_id: String, time_range: "active" | "last_24h" | "last_7d", severity_filter: Optional<Vec<"Moderate" | "Poor" | "VeryPoor">>}`
  - Output: List of alerts with timestamps and recommendations
- **Priority:** HIGH

**FR-6.4: Sensor Health Tool**
- **Description:** MCP tool for Claude to check sensor status
- **Acceptance Criteria:**
  - Tool name: `air_quality_sensor_health`
  - Input schema: `{location_id: String}`
  - Output: `{status: "online" | "offline" | "degraded", last_reading_age_seconds: 120, co2_calibration_status: "warming" | "active" | "stale", pm_quality: "good" | "high_humidity" | "saturated"}`
- **Priority:** MEDIUM

**FR-6.5: Recommendation Tool**
- **Description:** MCP tool for Claude to generate actionable recommendations
- **Acceptance Criteria:**
  - Tool name: `air_quality_recommendations`
  - Input schema: `{location_id: String}`
  - Output: Context-aware recommendations based on current conditions
  - Example: `["Open windows for 15 minutes to reduce CO2 from 1200 to <1000 ppm", "High PM2.5 detected - likely cooking event. Use range hood or air purifier", "Mold risk moderate (65% RH). Consider dehumidifier"]`
- **Priority:** MEDIUM

---

### FR-7: Domain Extensibility (New Domains via Traits)

**FR-7.1: Generic Core Traits**
- **Description:** Define domain-agnostic traits for time-series platform
- **Acceptance Criteria:**
  - `TimeSeriesEvent` trait: `timestamp()`, `location_id()`, `validate()`, `quality_score()`
  - `DataSource` trait: `connect()`, `subscribe()`, `poll()`, `disconnect()`
  - `DomainAdapter` trait: `parse_message()`, `transform_to_event()`, `health_thresholds()`, `recommendations()`
  - `Predictor` trait (reuse from neural-core): `predict()`, `train()`, `evaluate()`, `save_model()`
  - All air quality logic isolated in `air_quality_adapter.rs`
- **Priority:** HIGH
- **References:** Existing `neural-core/src/traits/` (Predictor, Storage already domain-agnostic)

**FR-7.2: Air Quality Domain Adapter**
- **Description:** Implement air quality domain as reference adapter
- **Acceptance Criteria:**
  - Module: `src/domains/air_quality/adapter.rs`
  - Implements: `DomainAdapter` trait
  - Contains: MQTT parsing, health thresholds (CO2/PM2.5/VOC), AQI calculations, mold risk, ventilation metrics
  - Isolated from core platform logic
- **Priority:** HIGH

**FR-7.3: New Domain Template**
- **Description:** Provide template/documentation for adding new domains
- **Acceptance Criteria:**
  - Template directory: `src/domains/_template/`
  - Documentation: `docs/adding-new-domain.md` with step-by-step guide
  - Estimated effort: <8 hours for experienced Rust developer
  - Example domains to test: Energy monitoring (kWh, voltage, current), Financial (OHLC, trades)
- **Priority:** MEDIUM

**FR-7.4: Domain Registry**
- **Description:** Runtime registry for loading domain adapters
- **Acceptance Criteria:**
  - Config-driven domain selection: `domain = "air_quality"` in TOML
  - Load adapter at startup: `DomainRegistry::load("air_quality")` → `Box<dyn DomainAdapter>`
  - Support multiple concurrent domains (future): `vec!["air_quality", "energy"]`
- **Priority:** MEDIUM

---

### FR-8: Configuration Management (config-store Integration)

**FR-8.1: Centralized Configuration via config-store**
- **Description:** All platform and domain configuration MUST use the existing `config-store` crate for centralized, versioned configuration management
- **Acceptance Criteria:**
  - Use `config-store` crate from workspace (`config-store/`)
  - Configuration paths follow hierarchical namespace: `/air-quality/{component}/{setting}`
  - Support runtime configuration updates via `ConfigStore::set()` and `ConfigStore::get()`
  - Use `ConfigNode` with metadata (owner, timestamps, tags) for audit trail
  - Support versioning (retain up to 10 versions per config path)
- **Priority:** HIGH
- **Dependencies:** `config-store` crate (workspace member)

**FR-8.2: YAML Configuration Files**
- **Description:** Primary configuration format MUST be YAML for human readability and GitOps compatibility
- **Acceptance Criteria:**
  - All static configuration in YAML format (`.yaml` or `.yml` extension)
  - Use `GitOpsLoader` from config-store for base/overlay pattern:
    ```
    config/
    ├── base/
    │   ├── air-quality.yaml      # Base air quality configuration
    │   ├── storage.yaml          # Storage backend settings
    │   ├── alerting.yaml         # Alert thresholds and channels
    │   └── forecasting.yaml      # ML model configuration
    └── overlays/
        ├── development/
        │   └── overrides.yaml    # Dev-specific overrides
        ├── staging/
        │   └── overrides.yaml
        └── production/
            └── overrides.yaml    # Pi5 production settings
    ```
  - Auto-detect format via file extension (YAML, TOML, JSON supported)
  - Validate YAML syntax on load with clear error messages
- **Priority:** HIGH
- **References:** `config-store/src/loaders/gitops.rs`

**FR-8.3: GitHub-Sourced Configuration**
- **Description:** Configuration files MUST support sourcing from GitHub repositories for GitOps workflows
- **Acceptance Criteria:**
  - Support GitHub raw file URLs for remote configuration:
    ```yaml
    config_sources:
      - type: github
        repo: "organization/air-quality-config"
        branch: "main"
        path: "config/"
      - type: local
        path: "/config/"  # Docker volume fallback
    ```
  - Periodic refresh of remote configuration (configurable interval, default 5 minutes)
  - Local cache with fallback on GitHub unavailability
  - Validate remote configuration before applying
  - Log configuration source and version on startup
- **Priority:** MEDIUM

**FR-8.4: Air Quality Configuration Schema**
- **Description:** Define YAML schema for air quality domain configuration
- **Acceptance Criteria:**
  - JSON Schema validation via `config-store/src/validation/schema.rs`
  - Schema file: `schemas/air-quality-config.json`
  - Configuration structure:
    ```yaml
    # air-quality.yaml
    air_quality:
      sensors:
        - serial: "ecda3b1eaaaf"
          name: "Living Room"
          location_id: "living-room"
          data_source: "both"  # mqtt | local_api | both

      ingestion:
        mqtt:
          broker_url: "mqtt://mosquitto:1883"
          topic_pattern: "airgradient/readings/{serial}"
          qos: 1
        local_api:
          poll_interval_seconds: 60
          timeout_seconds: 10

      thresholds:
        co2:
          good: 800
          moderate: 1000
          poor: 1500
          unhealthy: 2000
        pm25:
          good: 12.0
          moderate: 35.4
          unhealthy: 55.4
          hazardous: 150.4

      alerting:
        enabled: true
        channels:
          - type: webhook
            url: "${ALERT_WEBHOOK_URL}"
          - type: log
            level: "warn"
        rate_limit:
          cooldown_seconds: 300
          max_per_hour: 10

      forecasting:
        enabled: true
        model: "nhits"  # nhits | nbeats | ensemble
        horizon_hours: 24
        retrain_interval_days: 7
    ```
  - Support environment variable substitution (`${VAR_NAME}`)
  - Sensitive values blocked by `SecretBlocker` (passwords, API keys)
- **Priority:** HIGH

**FR-8.5: Configuration Hot-Reload**
- **Description:** Support runtime configuration updates without restart
- **Acceptance Criteria:**
  - Watch configuration for changes (file system or gRPC push)
  - Reload threshold values without restart
  - Reload alert channel configuration without restart
  - Log configuration changes with before/after diff
  - Non-reloadable settings (storage paths, MQTT broker) require restart with warning
- **Priority:** MEDIUM
- **References:** `config-store` gRPC `WatchConfig` streaming API

**FR-8.6: Configuration Validation**
- **Description:** Validate all configuration on load and update
- **Acceptance Criteria:**
  - Use `InputValidator` from config-store for injection prevention
  - Use `SchemaValidator` for JSON Schema validation
  - Reject invalid configuration with descriptive error messages
  - Support dry-run validation: `--validate-config` CLI flag
  - Health check endpoint reports configuration validity
- **Priority:** HIGH

**FR-8.7: Dynamic Unit Handling**
- **Description:** Interpret sensor readings based on sensor config from FR-1.5
- **Acceptance Criteria:**
  - If `temperatureUnit: "f"`, convert atmp/atmpCompensated to Celsius before storage
    - Conversion: `celsius = (fahrenheit - 32) × 5/9`
  - If `pmStandard` != "ugm3", apply appropriate conversion (future-proofing)
  - Record original unit in metadata for audit trail
  - Store all values in SI units (Celsius, µg/m³)
  - Document which corrections are pre-applied by sensor:
    - `pm02Compensated` uses algorithm from `corrections.pm02.correctionAlgorithm`
    - Platform should NOT re-apply EPA 2021 or LRAPA corrections
  - Quality scoring adjustments based on config:
    - VOC sensor learning period: First `tvocLearningOffset` hours = 0.7x penalty
    - NOx sensor learning period: First `noxLearningOffset` hours = 0.7x penalty
    - CO2 ABC period: Quality increases after `abcDays` days
- **Priority:** HIGH
- **Dependencies:** FR-1.5 (Config endpoint retrieval)

---

## 2. Non-Functional Requirements

### NFR-1: Performance

**NFR-1.1: Ingestion Throughput**
- **Requirement:** Support 60 readings/minute/location (1-minute polling) with <1s end-to-end latency
- **Measurement:** `histogram(ingestion_latency_ms)` - p95 <1000ms, p99 <2000ms
- **Rationale:** Real-time health alerts require <1 minute detection delay

**NFR-1.2: Query Latency**
- **Requirement:** 24-hour query in <100ms (cold), <20ms (cached)
- **Measurement:** `histogram(query_duration_ms)` - p95 <100ms
- **Rationale:** Dashboard responsiveness for user interactions

**NFR-1.3: Forecast Latency**
- **Requirement:** 6-hour forecast generation in <30s (cold start), <2s (warm cache)
- **Measurement:** `histogram(forecast_duration_ms)` - p95 <30000ms cold, <2000ms warm
- **Rationale:** Acceptable wait time for ML-powered features

**NFR-1.4: Storage Efficiency**
- **Requirement:** <500MB for 1 year of single-location data (1440 readings/day × 365 days)
- **Measurement:** `storage_bytes_total / days_stored` <1.5MB/day
- **Rationale:** Pi5 has 16GB RAM, 128GB+ storage - leave room for multiple locations and models

**NFR-1.5: Memory Footprint**
- **Requirement:** <500MB RAM under normal load (1 location, 24h cache, 1 active forecast)
- **Measurement:** `process_resident_memory_bytes` <500MB
- **Rationale:** Pi5 has 16GB RAM - leave room for OS and other services

---

### NFR-2: Reliability

**NFR-2.1: Data Durability**
- **Requirement:** No data loss on graceful shutdown or crash
- **Validation:** WAL replay test - kill -9 process mid-write, verify all WAL entries recovered
- **Acceptance:** 100% of WAL entries committed to Parquet on restart

**NFR-2.2: Crash Recovery Time**
- **Requirement:** Resume operation within 30 seconds of restart
- **Validation:** Kill -9, measure time to first MQTT message processed
- **Acceptance:** <30s (includes WAL replay, model load, MQTT reconnect)

**NFR-2.3: Connection Resilience**
- **Requirement:** Auto-reconnect to MQTT broker after network outage
- **Validation:** Disconnect broker, wait 60s, reconnect broker, verify messages resume
- **Acceptance:** Reconnect within 60s, no manual intervention required

**NFR-2.4: Partial Sensor Failures**
- **Requirement:** Continue operation if individual sensors fail (e.g., CO2 sensor offline)
- **Validation:** Send MQTT messages with missing `co2_ppm` field
- **Acceptance:** Store partial readings with quality flags, continue forecasting on available metrics

**NFR-2.5: Disk Full Handling**
- **Requirement:** Gracefully handle disk full conditions
- **Validation:** Fill disk to 95%, verify writes fail gracefully
- **Acceptance:** Log error, drop new readings, send critical alert, resume when space available

---

### NFR-3: Portability (Docker-Based)

**NFR-3.1: Multi-Architecture Docker Images**
- **Requirement:** Single Docker image supports amd64 (Mac Intel), arm64 (Mac M-series, Pi5)
- **Validation:** Build multi-arch image with `docker buildx`, test on each platform
- **Acceptance:** Same `docker-compose.yml` runs on Mac (dev) and Pi5 (prod) unchanged

**NFR-3.2: Container Self-Sufficiency**
- **Requirement:** Zero host dependencies beyond Docker runtime
- **Validation:** Run on fresh Docker install with no pre-installed packages
- **Acceptance:** `docker compose up` starts successfully on clean host

**NFR-3.3: Configuration Portability**
- **Requirement:** Same `config.toml` and `docker-compose.yml` work across platforms
- **Validation:** Copy entire deployment directory from Mac to Pi5
- **Acceptance:** Only change: environment-specific secrets (MQTT credentials)

**NFR-3.4: Data Volume Portability**
- **Requirement:** Parquet data volumes portable across platforms and to cloud storage
- **Validation:** Copy `/data` volume from Mac to Pi5, verify queries work
- **Acceptance:** Byte-identical schema, successful query after volume migration

**NFR-3.5: Model Volume Portability**
- **Requirement:** ruv-FANN models in `/models` volume portable across platforms
- **Validation:** Train on Mac, copy volume to Pi5, verify identical predictions
- **Acceptance:** <0.1% prediction delta (floating-point tolerance)

**NFR-3.6: Cloud Migration Path**
- **Requirement:** Docker deployment translatable to cloud services
- **Validation:** Deploy same image to AWS ECS/Fargate or GCP Cloud Run
- **Acceptance:** Deploy with <1 hour of cloud-specific configuration

---

### NFR-4: Extensibility

**NFR-4.1: New Domain Implementation Time**
- **Requirement:** Add new domain (energy, finance, etc.) in <8 hours
- **Validation:** Implement energy monitoring domain (kWh, voltage, current) using template
- **Acceptance:**
  - Developer unfamiliar with codebase: <16 hours
  - Developer familiar with codebase: <8 hours
  - 80% of code reused from core platform

**NFR-4.2: Domain Isolation**
- **Requirement:** Domain-specific logic isolated in adapter modules
- **Validation:** Grep core platform for air quality terms (CO2, PM2.5, AQI)
- **Acceptance:** Zero domain terms in `src/core/`, `src/storage/`, `src/query/` - all in `src/domains/air_quality/`

**NFR-4.3: Backward Compatibility**
- **Requirement:** New domain additions don't break existing domains
- **Validation:** Add energy domain, verify air quality still works
- **Acceptance:** 100% of air quality tests pass after new domain added

**NFR-4.4: Configuration Schema Stability**
- **Requirement:** Config schema versioned, old configs auto-migrate
- **Validation:** Use v1.0 config with v1.1 binary
- **Acceptance:** Auto-detect version, migrate to new schema, log migration

---

### NFR-5: Observability

**NFR-5.1: Metrics**
- **Requirement:** Expose Prometheus metrics for monitoring
- **Acceptance:**
  - Ingestion: `readings_received_total`, `readings_dropped_total`, `ingestion_latency_ms`
  - Storage: `storage_bytes_total`, `partitions_total`, `wal_commits_total`
  - Query: `query_duration_ms`, `cache_hits_total`, `cache_misses_total`
  - Forecast: `forecast_duration_ms`, `forecast_errors_total`, `model_load_duration_ms`
  - Alerts: `alerts_triggered_total{severity}`, `alerts_active_count`

**NFR-5.2: Structured Logging**
- **Requirement:** JSON-structured logs for machine parsing
- **Acceptance:**
  - Format: `{"timestamp": "2025-12-13T14:30:00Z", "level": "INFO", "target": "air_quality::ingestion", "message": "Reading ingested", "location_id": "living_room", "co2_ppm": 850}`
  - Levels: ERROR (alerts, crashes), WARN (quality issues), INFO (lifecycle events), DEBUG (verbose data flows)
  - Outputs: stdout (default), file (configurable)

**NFR-5.3: Health Checks**
- **Requirement:** HTTP health check endpoint for monitoring
- **Acceptance:**
  - Endpoint: `GET /health` → `{"status": "healthy", "mqtt": "connected", "storage": "ok", "last_reading_age_seconds": 120}`
  - Status codes: 200 (healthy), 503 (degraded/offline)
  - Response time: <10ms

---

## 3. Constraints

### 3.1 Hardware Constraints

**HC-1: Development Environment**
- **Platform:** macOS (x86_64 or ARM64)
- **Resources:** 8GB+ RAM, 10GB+ disk
- **Rationale:** Developer workstation for rapid iteration

**HC-2: Production Environment**
- **Platform:** Raspberry Pi 5 (16GB RAM, ARM64)
- **Storage:** 128GB+ microSD or SSD
- **Network:** WiFi or Ethernet with internet access (MQTT)
- **Power:** 5V/5A USB-C (27W) with UPS recommended for data durability
- **Rationale:** Low-cost edge deployment for home/small office

**HC-3: Sensor Hardware**
- **Device:** AirGradient ONE (ESP32-based)
- **Protocol:** MQTT over WiFi
- **Sensors:** SenseAir S8/S88 (CO2), Plantower PMS5003 (PM), Sensirion SGP41 (VOC/NOx), SHT4x (temp/humidity)
- **Polling Rate:** 1-minute default (configurable to 1s-10min)

---

### 3.2 Software Constraints

**SC-1: Programming Language**
- **Language:** Rust (stable channel, edition 2021)
- **Rationale:** Memory safety, performance, no runtime dependencies, excellent cross-compilation

**SC-2: External Dependencies**
- **Allowed:** Rust crates from crates.io, vendored dependencies (ruv-fann)
- **Required:** Docker (deployment platform)
- **Prohibited:** Python/Node.js runtimes within container, external databases (Redis, PostgreSQL) as required dependencies
- **Rationale:** Docker provides consistent deployment; no runtime dependency hell within container

**SC-3: Data Formats**
- **Storage:** Apache Parquet (columnar, compressed)
- **Interchange:** JSON (MQTT payloads, MCP responses)
- **Configuration:** TOML (human-readable, strongly typed)
- **Rationale:** Industry standards, tool compatibility, portability

**SC-4: Neural Models**
- **Framework:** ruv-FANN (vendor/ruv-fann/neuro-divergent)
- **Models:** NHITS, NBEATSx (27+ models available)
- **Format:** ONNX or native serialization (TBD based on vendor support)
- **Rationale:** Vendor lock-in to ruv-FANN ecosystem, pre-trained models available

---

### 3.3 Operational Constraints

**OC-1: Docker Deployment**
- **Target:** Single Docker image for all platforms
- **Base Image:** `rust:1.75-slim-bookworm` (multi-arch: amd64, arm64)
- **Image Size Target:** <100MB compressed
- **Container Runtime:** Docker or Podman
- **Orchestration:** Docker Compose (single-node), Kubernetes (future)
- **Volume Mounts:**
  - `/data` - Parquet storage (persistent volume)
  - `/config` - Configuration files (read-only bind mount)
  - `/models` - ruv-FANN models (persistent volume)
- **Health Check:** `GET /health` with 30s interval
- **Resource Limits (Pi5):** 2GB RAM, 2 CPU cores
- **Rationale:** Consistent deployment across Mac/Pi5/cloud, easy upgrades via image pull

**OC-2: Docker Compose Configuration**
```yaml
services:
  neural-air-quality:
    image: neural-data-platform/air-quality:latest
    restart: unless-stopped
    ports:
      - "8080:8080"   # Health/metrics
      - "9090:9090"   # MCP server (optional)
    volumes:
      - air-quality-data:/data
      - air-quality-models:/models
      - ./config.toml:/config/config.toml:ro
    environment:
      - MQTT_BROKER_URL=mqtt://broker:1883
      - AIRGRADIENT_SERIAL=ecda3b1eaaaf
      - DATA_SOURCE=mqtt  # or 'local_api' or 'both'
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      resources:
        limits:
          memory: 2G
          cpus: '2.0'
volumes:
  air-quality-data:
  air-quality-models:
```

**OC-3: Configuration**
- **Format:** Single TOML file mounted at `/config/config.toml`
- **Environment Overrides:** All settings overridable via `NEURAL_` prefixed env vars
- **Hot Reload:** Not supported in v1.0 (container restart required)
- **Secrets:** Environment variables (Docker secrets for production)

**OC-4: Network**
- **Inbound:**
  - Port 8080: HTTP health checks + Prometheus metrics
  - Port 9090: MCP server (stdio or HTTP)
- **Outbound:**
  - MQTT broker (port 1883 or 8883 for TLS)
  - AirGradient Local API (port 80, mDNS `airgradient_*.local`)
- **Docker Network:** Bridge (default) or host (for mDNS local API discovery)

**OC-5: Security**
- **MQTT:** TLS + username/password or client certificates
- **MCP:** stdio (container exec) or HTTP with API key
- **Storage:** Container user `1000:1000` (non-root)
- **Secrets:** Docker secrets or encrypted env files
- **No Auth:** Health check endpoint public (contains no sensitive data)

---

### 3.4 Timeline Constraints

**TC-1: Development Phases**
- **Phase 1 (Weeks 1-2):** Core ingestion + storage (MQTT → Parquet)
- **Phase 2 (Weeks 3-4):** Query engine + domain adapters (Polars + air quality logic)
- **Phase 3 (Weeks 5-6):** Forecasting + alerting (ruv-FANN integration)
- **Phase 4 (Week 7):** MCP tools + documentation
- **Phase 5 (Week 8):** Integration testing + deployment automation
- **Total:** 8 weeks to v1.0

**TC-2: External Dependencies**
- **Risk:** ruv-FANN vendor availability/support
- **Mitigation:** Fallback to statsmodels/ARIMA if ruv-FANN integration blocked
- **Deadline:** Must resolve by end of Phase 3 (Week 6)

---

## 4. Success Criteria (Measurable Outcomes)

### 4.1 Functional Validation

| Criterion | Measurement | Target | Priority |
|-----------|-------------|--------|----------|
| **F-1: Ingestion** | Successful MQTT message processing rate | >99% of received messages | HIGH |
| **F-2: Storage** | WAL→Parquet commit success rate | 100% (zero data loss) | HIGH |
| **F-3: Query** | 24-hour query latency (p95) | <100ms | HIGH |
| **F-4: Forecast** | 6-hour PM2.5 forecast MAE | <5 µg/m³ (vs. actual) | MEDIUM |
| **F-5: Alerts** | Threshold violation detection latency | <60s from reading to alert | HIGH |
| **F-6: MCP** | Claude query success rate | >95% (valid tool calls) | HIGH |
| **F-7: Domain Extension** | Hours to implement energy domain | <8h (experienced dev) | MEDIUM |

---

### 4.2 Non-Functional Validation

| Criterion | Measurement | Target | Priority |
|-----------|-------------|--------|----------|
| **NF-1: Latency** | Ingestion p95 latency | <1s | HIGH |
| **NF-2: Throughput** | Readings/second (single location) | 1 reading/min sustained | HIGH |
| **NF-3: Storage** | Bytes per day (compressed) | <1.5MB/location/day | MEDIUM |
| **NF-4: Memory** | RSS under load | <500MB | MEDIUM |
| **NF-5: Crash Recovery** | Time to resume after kill -9 | <30s | HIGH |
| **NF-6: Portability** | Cross-platform test pass rate | 100% (Mac, Pi5) | HIGH |
| **NF-7: Extensibility** | New domain lines of code | <500 LOC (excluding tests) | MEDIUM |

---

### 4.3 Acceptance Tests

**AT-1: End-to-End Smoke Test**
1. Start platform with clean state (no data)
2. Send 1440 MQTT messages (simulate 24 hours at 1-min intervals)
3. Query last 24 hours - verify 1440 rows returned
4. Trigger health alert (inject CO2 >1500 ppm message)
5. Generate 6-hour forecast
6. Query alerts - verify threshold alert present
7. Verify Parquet file created with correct schema
8. Kill -9 process, restart, verify WAL replay recovers in-flight data

**AT-2: Multi-Day Data Retention Test**
1. Ingest 365 days of synthetic data (525,600 readings)
2. Verify storage <500MB
3. Query random 7-day window - verify <500ms latency
4. Delete partitions >365 days old - verify auto-cleanup works

**AT-3: Sensor Failure Resilience Test**
1. Send MQTT messages with missing fields (e.g., no `co2_ppm`)
2. Verify partial reading stored with quality flags
3. Disconnect MQTT broker for 5 minutes
4. Reconnect broker
5. Verify auto-reconnect and message processing resumes

**AT-4: Forecast Accuracy Test**
1. Use 90 days of historical AirGradient data (real or synthetic)
2. Generate forecasts for days 91-97 (7-day test set)
3. Compare forecasts to actuals
4. Verify PM2.5 MAE <5 µg/m³, CO2 MAE <100 ppm

**AT-5: New Domain Implementation Test**
1. Implement energy monitoring domain (kWh, voltage, current)
2. Track developer time from template copy to first query
3. Verify <8 hours for experienced developer
4. Verify air quality domain still passes all tests

---

## 5. Out of Scope (What We're NOT Building)

### 5.1 Deferred to Future Versions

**v1.1 Features:**
- Multi-sensor aggregation (average across rooms)
- Outdoor air quality integration (for indoor/outdoor ratio calculations)
- HomeKit/Home Assistant integration
- Advanced analytics (trend analysis, anomaly detection beyond thresholds)
- Model retraining automation

**v1.2 Features:**
- Cloud deployment (Kubernetes, AWS Lambda)
- Multi-tenancy (multiple independent users)
- Advanced alerting (email, SMS, Slack)
- Mobile app (native iOS/Android)
- Data export (CSV, Excel)

**v2.0 Features:**
- QuestDB/TimescaleDB integration (SQL analytics layer)
- Real-time dashboards (web UI with live updates)
- Advanced forecasting (ensemble models, weather integration)
- Energy optimization recommendations (HVAC control via HomeKit)
- Regulatory reporting (ASHRAE, WELL Building Standard compliance)

---

### 5.2 Explicitly Not Supported

**Out of Scope Forever:**
- Outdoor ambient monitoring (use EPA AirNow or PurpleAir instead)
- Industrial/workplace compliance (OSHA, ISO standards)
- Laboratory-grade measurements (research equipment has different requirements)
- Chemical speciation (beyond VOC index - requires GC-MS)
- Video/audio monitoring (privacy concerns, out of domain)
- Real-time control loops (v1.0 is monitoring only, not actuation)

---

## 6. Glossary

### 6.1 Air Quality Terms

| Term | Definition |
|------|------------|
| **AQI** | Air Quality Index - EPA 0-500 scale for health impact communication |
| **PM2.5** | Particulate Matter ≤2.5 micrometers (fine particles, inhalable deep into lungs) |
| **PM10** | Particulate Matter ≤10 micrometers (coarse particles, nose/throat deposition) |
| **VOC** | Volatile Organic Compounds (gases from paints, cleaners, combustion) |
| **NOx** | Nitrogen Oxides (primarily NO2 from combustion) |
| **CO2** | Carbon Dioxide (respiration byproduct, ventilation proxy) |
| **TVOC** | Total Volatile Organic Compounds (absolute concentration, not available on SGP41) |
| **RH** | Relative Humidity (percentage of moisture saturation at current temperature) |
| **ACH** | Air Changes Per Hour (ventilation rate metric) |
| **ppm** | Parts Per Million (concentration unit for gases) |
| **µg/m³** | Micrograms per cubic meter (concentration unit for particles) |
| **USG** | Unhealthy for Sensitive Groups (AQI category 101-150) |
| **I/O Ratio** | Indoor/Outdoor ratio (source attribution for PM2.5) |

### 6.2 Sensor Terms

| Term | Definition |
|------|------------|
| **NDIR** | Non-Dispersive Infrared (CO2 sensing technology) |
| **ABC** | Automatic Baseline Calibration (CO2 sensor self-calibration assuming outdoor exposure) |
| **Laser Scattering** | Optical particle counting method (Plantower PMS5003) |
| **Metal Oxide** | Gas sensing technology (Sensirion SGP41) |
| **VOC Index** | Relative 1-500 scale (100 = 24h baseline, not absolute TVOC concentration) |
| **Hygroscopic Growth** | Water uptake by particles at high humidity (causes PM measurement error) |
| **Calibration Factor** | Sensor-specific correction multiplier (batch variation compensation) |
| **Warmup Period** | Time for sensor to stabilize after power-on (CO2: 3 weeks, SGP41: 10s) |

### 6.3 Platform Terms

| Term | Definition |
|------|------------|
| **Parquet** | Apache Parquet - columnar storage format (compression + fast queries) |
| **Polars** | Rust DataFrame library (query engine for Parquet) |
| **MQTT** | Message Queue Telemetry Transport (lightweight pub/sub protocol for IoT) |
| **WAL** | Write-Ahead Log (append-only buffer for crash recovery) |
| **MCP** | Model Context Protocol (Claude's tool integration framework) |
| **ruv-FANN** | Vendor neural network library (27+ time-series models) |
| **NHITS** | Neural Hierarchical Interpolation for Time Series (SOTA forecasting model) |
| **NBEATSx** | Neural Basis Expansion Analysis for Time Series (exogenous variables) |
| **Domain Adapter** | Trait implementation isolating domain-specific logic (air quality, energy, etc.) |
| **TimeSeriesEvent** | Generic trait for time-stamped data points |
| **DLQ** | Dead Letter Queue (failed message storage for retry/debugging) |

### 6.4 Forecasting Terms

| Term | Definition |
|------|------------|
| **Horizon** | Forecast prediction window (e.g., 6 hours ahead) |
| **Lag Features** | Past values used as model inputs (e.g., value from 1h ago) |
| **Rolling Mean** | Moving average over time window (trend detection) |
| **p10/p50/p90** | Percentile forecasts (10th, 50th/median, 90th - quantify uncertainty) |
| **MAE** | Mean Absolute Error (forecast accuracy metric) |
| **RMSE** | Root Mean Squared Error (penalizes large errors) |
| **MAPE** | Mean Absolute Percentage Error (relative accuracy) |
| **Cold Start** | First inference after model load (slower due to initialization) |

### 6.5 Health Terms

| Term | Definition |
|------|------------|
| **Cognitive Decline** | Reduced mental performance (attention, decision-making) due to CO2 >1000 ppm |
| **CVD** | Cardiovascular Disease (PM2.5 increases heart attack/stroke risk) |
| **ASHRAE** | American Society of Heating, Refrigerating and Air-Conditioning Engineers (indoor air standards) |
| **EPA** | Environmental Protection Agency (outdoor air quality regulations) |
| **WHO** | World Health Organization (global health guidelines, PM2.5 <5 µg/m³ target) |
| **WELL** | WELL Building Standard (healthy building certification) |
| **PMV/PPD** | Predicted Mean Vote / Predicted Percentage Dissatisfied (thermal comfort metrics) |
| **Mold Risk** | Probability of fungal growth based on temperature + humidity |

---

## 7. References

### 7.1 Existing Codebase Components

| Component | Path | Reusability | Adaptation Required |
|-----------|------|-------------|---------------------|
| **Predictor Trait** | `neural-core/src/traits/` | 100% | None - already generic |
| **Storage Trait** | `neural-core/src/traits/` | 100% | None - multi-backend support |
| **EventBus** | `neural-core/src/eventbus/` | 90% | Change proto schemas for air quality |
| **Data Staging** | `data-staging/src/` | 85% | Adapt quality scorer field names |
| **ML Ops** | `neural-ml-ops/src/` | 80% | Change model types, keep training pipeline |
| **MCP Server** | `mcp-trading-server/src/` | 70% | Replace trading tools with air quality tools |
| **ruv-FANN** | `vendor/ruv-fann/` | 100% | Use NHITS/NBEATSx models as-is |

### 7.2 AirGradient Data Sources (Complete Field Reference)

**MQTT Topic:** `airgradient/readings/{SERIAL_NUMBER}`
**Local API:** `http://airgradient_{SERIAL}.local/measures/current`

| Field | Type | Unit | Description | Source |
|-------|------|------|-------------|--------|
| `wifi` | Int | dBm | WiFi signal strength | Both |
| `serialno` | String | - | Device serial number | Both |
| `rco2` | Int | ppm | CO₂ concentration (Senseair S8) | Both |
| `pm01` | Float | µg/m³ | PM1.0 atmospheric | Both |
| `pm02` | Float | µg/m³ | PM2.5 atmospheric | Both |
| `pm10` | Float | µg/m³ | PM10 atmospheric | Both |
| `pm02Compensated` | Float | µg/m³ | PM2.5 with humidity correction | Both |
| `pm01Standard` | Float | µg/m³ | PM1.0 standard particle | Both |
| `pm02Standard` | Float | µg/m³ | PM2.5 standard particle | Both |
| `pm10Standard` | Float | µg/m³ | PM10 standard particle | Both |
| `pm003Count` | Float | /dL | Particles ≥0.3µm count | Both |
| `pm005Count` | Float | /dL | Particles ≥0.5µm count | Both |
| `pm01Count` | Float | /dL | Particles ≥1.0µm count | Both |
| `pm02Count` | Float | /dL | Particles ≥2.5µm count | Both |
| `pm50Count` | Float | /dL | Particles ≥5.0µm count | Both |
| `pm10Count` | Float | /dL | Particles ≥10µm count | Both |
| `atmp` | Float | °C | Temperature raw (check /config for unit) | Both |
| `atmpCompensated` | Float | °C | Temperature corrected | Both |
| `rhum` | Float | % | Relative humidity raw | Both |
| `rhumCompensated` | Float | % | Relative humidity corrected | Both |
| `tvocIndex` | Int | 1-500 | VOC index (Sensirion SGP41) | Both |
| `tvocRaw` | Float | - | VOC raw sensor signal | Both |
| `noxIndex` | Int | 1-500 | NOx index (Sensirion SGP41) | Both |
| `noxRaw` | Float | - | NOx raw sensor signal | Both |
| `boot` | Int | - | Measurement cycle counter | Both |
| `bootCount` | Int | - | Same as boot (HA compat) | Both |
| `ledMode` | String | - | Current LED display mode | Both |
| `firmware` | String | - | Firmware version | Both |
| `model` | String | - | Hardware model (I-9PSL) | Both |

**Note:** Both MQTT and Local API return identical 29-field payloads (firmware 3.4.1+). Either source provides complete data.

### 7.3 External Documentation

- **AirGradient ONE:** https://www.airgradient.com/documentation/one-v9/
- **AirGradient API:** https://www.airgradient.com/air-quality-monitoring-toolkit/operating/airgradient-api/
- **AirGradient MQTT:** https://www.airgradient.com/support/kb-mqtt-conf/
- **AirGradient Local Server:** https://github.com/airgradienthq/arduino/blob/master/docs/local-server.md
- **EPA AQI:** https://www.airnow.gov/aqi/aqi-basics/
- **WHO Air Quality Guidelines:** https://www.who.int/news-room/fact-sheets/detail/ambient-(outdoor)-air-quality-and-health
- **ASHRAE 62.1:** Ventilation for Acceptable Indoor Air Quality
- **Polars Documentation:** https://pola-rs.github.io/polars/
- **Apache Parquet:** https://parquet.apache.org/docs/
- **MQTT Specification:** https://mqtt.org/mqtt-specification/
- **ruv-FANN Models:** vendor/ruv-fann/neuro-divergent (local documentation)

### 7.4 Research Papers

- Sensirion SGP41 Datasheet: VOC/NOx Index Algorithm (Sensirion, 2021)
- Low-cost PM Sensor Calibration (Zheng et al., 2018) - RH correction methods
- CO2 and Cognitive Function (Allen et al., 2016) - 1000 ppm threshold impacts
- VTT Mold Growth Model (Ojanen et al., 2010) - Temperature/humidity/material factors
- NHITS: Neural Hierarchical Interpolation for Time Series (Challu et al., 2023)

---

## 8. Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-13 | SPARC Specification Agent | Initial specification based on codebase analysis and air quality domain research |
| 1.1.0 | 2025-12-13 | Claude (Revised) | **Major revision:** Docker deployment model, complete AirGradient sensor fields (29+ fields from Local API), dual data source support (MQTT + Local HTTP API), validated against official AirGradient documentation |
| 1.2.0 | 2025-12-13 | Claude (Swarm Review) | **Critical fixes from actual sensor data validation:** (1) FR-1.1: Fixed MQTT topic pattern to `airgradient/readings/{SERIAL_NUMBER}`, (2) FR-2.1: Changed PM/count/raw field types from UInt16/UInt32 to Float32, (3) Section 7.2: Corrected all 29 field sources to "Both" and types to Float, (4) Added FR-1.5: Config endpoint retrieval, (5) Added FR-8.7: Dynamic unit handling, (6) Added `led_mode` field to schema, (7) Fixed CO2 validation range to 380-10,000 ppm |

---

**Next Steps:**
1. Review specification with stakeholders
2. Proceed to SPARC Pseudocode phase (algorithm design for core workflows)
3. Define system architecture (component diagrams, data flows)
4. Implement Phase 1: Core ingestion + storage

**Approval Required:**
- [ ] Technical Lead (architecture + constraints)
- [ ] Domain Expert (health thresholds + sensor specs)
- [ ] Product Owner (scope + success criteria)
