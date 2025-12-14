# AIR-002 Specification: MQTT Ingestion Pipeline

**Version:** 1.0.0
**Date:** December 14, 2025
**Status:** Draft
**Author:** SPARC Specification Agent
**Parent:** AIR-001 (Air Quality Module)

---

## Executive Summary

### Purpose

AIR-002 implements the **critical missing ingestion pipeline** for AirGradient sensor data via MQTT. This is the **#1 blocker** preventing end-to-end testing of the air quality platform. Without this pipeline, the system cannot receive real sensor data, making all downstream features (storage, querying, alerting, forecasting) untestable.

### Current State Analysis

**What Already Exists:**
- ✅ Domain parser: `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs`
  - `parse_mqtt_payload()` - Converts JSON to `AirQualityReading`
  - Handles 29 fields from AirGradient ONE firmware 3.1.4+
  - Graceful degradation for partial payloads
- ✅ Validation rules: `/workspaces/neural-data-platform/domains/air-quality/src/validation.rs`
  - Hardware-spec range validation (CO2: 380-10,000 ppm, PM: 0-500 µg/m³, etc.)
  - Multi-error collection
- ✅ Adapter: `/workspaces/neural-data-platform/domains/air-quality/src/adapter.rs`
  - `to_time_series_points()` - Converts `AirQualityReading` to `Vec<TimeSeriesPoint>`
  - Each metric becomes separate time series point
- ✅ Storage backend: Parquet with WAL (from neural-core)
- ✅ MQTT client skeleton: `/workspaces/neural-data-platform/core/src/sources/mqtt.rs`
  - MqttSource struct exists but not wired to domain code

**What's Missing:**
- ❌ Integration layer connecting: MQTT → Parser → Validator → Adapter → Storage
- ❌ Background task in main.rs to run ingestion loop
- ❌ Configuration management (broker URL, topics, QoS)
- ❌ Error handling and Dead Letter Queue (DLQ)
- ❌ Quality scoring (completeness, freshness, calibration)
- ❌ Metrics instrumentation (latency, throughput, errors)

### Scope

**In Scope for AIR-002:**
1. MQTT client initialization and connection management
2. Topic subscription with wildcard support
3. Message parsing using existing domain parser
4. Validation using existing domain validation rules
5. Quality scoring (extends validation with scoring logic)
6. Conversion to TimeSeriesPoints using existing adapter
7. Storage via Parquet backend
8. Dead Letter Queue for malformed/invalid messages
9. Reconnection logic with exponential backoff
10. Health status reporting (MQTT connection state)
11. Metrics instrumentation (Prometheus-compatible)

**Out of Scope (Deferred):**
- Local HTTP API polling (FR-1.1 from AIR-001) - Deferred to AIR-003
- Configuration endpoint retrieval (FR-1.5 from AIR-001) - Deferred to AIR-004
- Alerting (handled by AIR-005)
- Forecasting (handled by AIR-006)
- MCP tools (handled by AIR-007)

---

## 1. Functional Requirements

### FR-2.1: MQTT Client Initialization

**Description:** Initialize rumqttc MQTT client with configurable broker settings

**Acceptance Criteria:**
- ✅ Load MQTT configuration from config.yaml:
  ```yaml
  mqtt:
    broker_url: "mqtt://mosquitto:1883"  # or airgradient cloud broker
    client_id: "neural-air-quality-{HOSTNAME}"
    keep_alive_seconds: 30
    connection_timeout_seconds: 10
    clean_session: true
  ```
- ✅ Support both TCP and TLS connections (mqtt:// and mqtts://)
- ✅ Generate unique client ID if not specified (append random suffix)
- ✅ Log connection parameters on startup (mask credentials)
- ✅ Return error if broker URL is invalid

**Priority:** HIGH
**Dependencies:** rumqttc crate (already in workspace Cargo.toml)
**Related Code:** `/workspaces/neural-data-platform/core/src/sources/mqtt.rs` (MqttSource exists)

---

### FR-2.2: Topic Subscription

**Description:** Subscribe to AirGradient MQTT topic pattern with wildcard support

**Acceptance Criteria:**
- ✅ Subscribe to topic pattern: `airgradient/readings/{SERIAL_NUMBER}`
- ✅ Support wildcard `+` for multi-sensor deployments: `airgradient/readings/+`
- ✅ Configurable QoS level (default: QoS 1 - At Least Once)
  ```yaml
  mqtt:
    topic_pattern: "airgradient/readings/+"
    qos: 1  # 0 = At Most Once, 1 = At Least Once, 2 = Exactly Once
  ```
- ✅ Subscribe on successful connection
- ✅ Re-subscribe automatically after reconnection
- ✅ Log subscription events (topic, QoS, success/failure)

**Priority:** HIGH
**Dependencies:** FR-2.1 (MQTT client must be connected)

---

### FR-2.3: Message Parsing

**Description:** Parse incoming MQTT payloads using existing domain parser

**Acceptance Criteria:**
- ✅ Use existing `air_quality::parser::parse_mqtt_payload()` from `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs`
- ✅ Handle JSON payloads with 29 fields (see AIR-001 spec section 7.2)
- ✅ Gracefully handle partial payloads (Option fields in `AirQualityReading`)
- ✅ Add timestamp if not present in payload (use message receipt time)
- ✅ Return `ParserError` for malformed JSON
- ✅ Log parsing errors with payload sample (truncated to 200 chars)

**Example Flow:**
```rust
// MQTT message received
let payload = r#"{"serialno": "ecda3b1eaaaf", "rco2": 450, "pm02": 12.5, ...}"#;

// Parse using domain parser
let reading = air_quality::parser::parse_mqtt_payload(payload)?;

// reading.device.serialno = "ecda3b1eaaaf"
// reading.metrics.rco2 = Some(450)
// reading.particles.pm02 = Some(12.5)
```

**Priority:** HIGH
**Dependencies:** FR-2.2 (subscription must provide messages)
**Reuses:** `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs`

---

### FR-2.4: Data Validation

**Description:** Validate parsed readings using existing domain validation rules

**Acceptance Criteria:**
- ✅ Use existing `air_quality::validation::validate_reading()` from `/workspaces/neural-data-platform/domains/air-quality/src/validation.rs`
- ✅ Validate hardware-spec ranges:
  - CO2: 380-10,000 ppm (SenseAir S8)
  - PM2.5: 0-500 µg/m³ (Plantower PMS5003)
  - TVOC/NOx index: 1-500 (Sensirion SGP41)
  - Temperature: -10 to 50°C (SHT4x)
  - Humidity: 0-100% (SHT4x)
  - WiFi: -100 to 0 dBm
- ✅ Collect all validation errors (don't fail on first error)
- ✅ Send invalid readings to Dead Letter Queue (FR-2.9)
- ✅ Log validation failures with error details
- ✅ Continue processing (don't crash on validation failure)

**Example Flow:**
```rust
// After parsing
let reading = parse_mqtt_payload(payload)?;

// Validate
match air_quality::validation::validate_reading(&reading) {
    Ok(()) => {
        // Valid - proceed to quality scoring
    }
    Err(ValidationError::Co2OutOfRange(value)) => {
        // Send to DLQ
        dlq.write(reading, format!("CO2 out of range: {}", value))?;
    }
    Err(ValidationError::MultipleErrors(errors)) => {
        // Send to DLQ with all errors
        dlq.write(reading, format!("{} validation errors", errors.len()))?;
    }
}
```

**Priority:** HIGH
**Dependencies:** FR-2.3 (parser must provide `AirQualityReading`)
**Reuses:** `/workspaces/neural-data-platform/domains/air-quality/src/validation.rs`

---

### FR-2.5: Quality Scoring

**Description:** Calculate data quality score based on completeness, freshness, and calibration status

**Acceptance Criteria:**
- ✅ Implement quality scoring module: `domains/air-quality/src/quality.rs`
- ✅ Quality score formula: `completeness × calibration × freshness` (0.0-1.0 scale)
- ✅ **Completeness score** (percentage of key fields present):
  ```rust
  // Key fields: rco2, pm02, atmp, rhum, tvocIndex
  let present_count = [rco2, pm02, atmp, rhum, tvocIndex]
      .iter()
      .filter(|f| f.is_some())
      .count();
  let completeness = present_count as f32 / 5.0;  // 5 key fields
  ```
- ✅ **Calibration penalties** (based on sensor specs):
  - CO2 warmup period (<21 days since boot): 0.7× penalty
  - High humidity (>80% RH): PM readings get 0.9× penalty
  - VOC/NOx learning period (first 12 hours): 0.7× penalty
- ✅ **Freshness factor** (age of reading if timestamp present):
  - <5s: 1.0
  - 5-30s: 0.9
  - 30-60s: 0.8
  - >60s: 0.7
- ✅ Attach quality flags: `Vec<String>` for downstream filtering
  ```rust
  quality_flags: vec![
      "co2_warmup_period",
      "pm_high_humidity",
      "voc_learning_period"
  ]
  ```
- ✅ Store quality score and flags in `TimeSeriesPoint` tags
- ✅ Accept readings with quality score ≥0.5 (configurable threshold)

**Example:**
```rust
// Reading: rco2=450, pm02=12.5, atmp=22.0, rhum=85.0, tvocIndex=None, boot=3 days
let completeness = 4.0 / 5.0 = 0.8  // 4 of 5 key fields
let calibration = 0.9               // PM penalty for high humidity (85% RH)
let freshness = 1.0                 // Just received
let quality_score = 0.8 × 0.9 × 1.0 = 0.72

quality_flags = ["pm_high_humidity"]
```

**Priority:** MEDIUM
**Dependencies:** FR-2.4 (validation must pass first)
**References:** AIR-001 FR-1.3 (quality assessment)

---

### FR-2.6: Conversion to TimeSeriesPoints

**Description:** Convert validated readings to generic TimeSeriesPoints using existing adapter

**Acceptance Criteria:**
- ✅ Use existing `air_quality::adapter::AirQualityAdapter::to_time_series_points()`
- ✅ Each metric becomes separate `TimeSeriesPoint`:
  - CO2 → TimeSeriesPoint { metric: "co2", value: rco2 as f64 }
  - PM2.5 → TimeSeriesPoint { metric: "pm25", value: pm02 as f64 }
  - Temperature → TimeSeriesPoint { metric: "temperature", value: atmp as f64 }
  - (13+ metrics total from 29 fields)
- ✅ Preserve metadata in tags:
  ```rust
  tags: {
      "metric": "pm25",
      "serialno": "ecda3b1eaaaf",
      "firmware": "3.1.3",
      "model": "I-9PSL",
      "quality_score": "0.85",
      "quality_flags": "pm_high_humidity"
  }
  ```
- ✅ Set `location_id` to `serialno` field
- ✅ Use reading timestamp (or message receipt time if missing)

**Example:**
```rust
let reading = parse_mqtt_payload(payload)?;
validate_reading(&reading)?;
let quality = calculate_quality(&reading)?;

// Convert to time series points
let mut points = AirQualityAdapter::to_time_series_points(&reading);

// Add quality metadata to all points
for point in &mut points {
    point.tags.insert("quality_score".to_string(), quality.score.to_string());
    point.tags.insert("quality_flags".to_string(), quality.flags.join(","));
}

// points = [
//     TimeSeriesPoint { timestamp, location_id: "ecda3b1eaaaf", value: 450.0, tags: {metric: "co2"} },
//     TimeSeriesPoint { timestamp, location_id: "ecda3b1eaaaf", value: 12.5, tags: {metric: "pm25"} },
//     ...
// ]
```

**Priority:** HIGH
**Dependencies:** FR-2.5 (quality scoring)
**Reuses:** `/workspaces/neural-data-platform/domains/air-quality/src/adapter.rs`

---

### FR-2.7: Storage Persistence

**Description:** Write TimeSeriesPoints to Parquet storage backend

**Acceptance Criteria:**
- ✅ Use existing `neural_core::Store` trait (already implemented for Parquet)
- ✅ Batch writes for efficiency (accumulate up to 100 points or 5 seconds)
- ✅ Write-ahead log (WAL) before Parquet commit (crash recovery)
- ✅ Atomic commits (temp file + rename pattern)
- ✅ Storage path pattern: `data/air_quality/{location_id}/year={YYYY}/month={MM}/day={DD}/readings.parquet`
- ✅ Rollover to new partition at midnight UTC
- ✅ Emit metrics:
  - `ingestion_points_written_total` (counter)
  - `ingestion_batch_write_duration_seconds` (histogram)
  - `ingestion_storage_errors_total` (counter)
- ✅ Log storage errors but don't crash (retry with backoff)

**Error Handling:**
```rust
match store.write_batch(points).await {
    Ok(()) => {
        metrics::counter!("ingestion_points_written_total", points.len());
    }
    Err(e) => {
        error!("Storage write failed: {}", e);
        metrics::counter!("ingestion_storage_errors_total", 1);
        // Retry logic with exponential backoff
        tokio::time::sleep(Duration::from_secs(2_u64.pow(retry_count))).await;
    }
}
```

**Priority:** HIGH
**Dependencies:** FR-2.6 (must have TimeSeriesPoints to store)
**Reuses:** Neural-core Store trait

---

### FR-2.8: Reconnection Logic

**Description:** Handle MQTT broker disconnections with exponential backoff

**Acceptance Criteria:**
- ✅ Detect disconnection events from rumqttc EventLoop
- ✅ Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 30s
- ✅ Unlimited reconnection attempts (don't give up)
- ✅ Re-subscribe to topics after successful reconnection
- ✅ Log reconnection attempts (attempt number, delay, reason)
- ✅ Emit metrics:
  - `ingestion_mqtt_disconnections_total` (counter)
  - `ingestion_mqtt_reconnections_total` (counter)
  - `ingestion_mqtt_reconnection_duration_seconds` (histogram)
- ✅ Update health status (connected/disconnected/reconnecting)

**Example:**
```rust
match event_loop.poll().await {
    Err(e) => {
        warn!("MQTT disconnected: {}", e);
        metrics::counter!("ingestion_mqtt_disconnections_total", 1);

        for attempt in 0.. {
            let delay = min(
                Duration::from_secs(2_u64.pow(attempt)),
                Duration::from_secs(30)
            );

            info!("Reconnecting in {}s (attempt {})", delay.as_secs(), attempt + 1);
            sleep(delay).await;

            match reconnect().await {
                Ok(()) => {
                    info!("Reconnected successfully");
                    metrics::counter!("ingestion_mqtt_reconnections_total", 1);
                    break;
                }
                Err(e) => {
                    error!("Reconnection failed: {}", e);
                }
            }
        }
    }
}
```

**Priority:** HIGH
**Dependencies:** FR-2.1 (MQTT client)
**References:** AIR-001 FR-1.1 (auto-reconnect requirement)

---

### FR-2.9: Dead Letter Queue (DLQ)

**Description:** Store malformed/invalid messages for debugging and reprocessing

**Acceptance Criteria:**
- ✅ Create DLQ directory: `data/air_quality/dlq/`
- ✅ Store failed messages as newline-delimited JSON (NDJSON):
  ```json
  {"timestamp": "2025-12-14T10:30:00Z", "error": "CO2 out of range: 15000", "payload": "{...}"}
  ```
- ✅ File naming: `dlq/year={YYYY}/month={MM}/day={DD}/errors.ndjson`
- ✅ Include error reason, original payload, timestamp, topic
- ✅ Limit DLQ size: max 10,000 entries per day (prevent disk fill)
- ✅ Emit metrics:
  - `ingestion_dlq_entries_total{reason}` (counter with reason label)
  - `ingestion_dlq_size_bytes` (gauge)
- ✅ Provide reprocessing tool (manual recovery): `air-quality-app reprocess-dlq --date 2025-12-14`

**DLQ Entry Format:**
```rust
#[derive(Serialize)]
struct DlqEntry {
    timestamp: DateTime<Utc>,
    topic: String,
    payload: String,  // Original JSON payload
    error_type: String,  // "parse_error", "validation_error", "quality_too_low"
    error_message: String,
    serial_no: Option<String>,  // Extracted if available
}
```

**Priority:** MEDIUM
**Dependencies:** FR-2.3, FR-2.4 (parsers/validators produce errors)
**References:** AIR-001 FR-1.2 (DLQ requirement)

---

### FR-2.10: Configuration Management

**Description:** Load ingestion pipeline settings from config.yaml

**Acceptance Criteria:**
- ✅ Configuration file: `apps/air-quality-app/config.yaml`
- ✅ Schema:
  ```yaml
  ingestion:
    mqtt:
      broker_url: "mqtt://mosquitto:1883"
      client_id: "neural-air-quality"
      topic_pattern: "airgradient/readings/+"
      qos: 1
      keep_alive_seconds: 30
      connection_timeout_seconds: 10
      reconnect_delay_seconds: 1
      max_reconnect_delay_seconds: 30

    quality:
      min_score_threshold: 0.5  # Reject readings below this
      enable_completeness_check: true
      enable_calibration_check: true
      enable_freshness_check: true

    storage:
      batch_size: 100  # Points per batch write
      batch_timeout_seconds: 5  # Max time before flush
      base_path: "data/air_quality"

    dlq:
      enabled: true
      max_entries_per_day: 10000
      base_path: "data/air_quality/dlq"
  ```
- ✅ Environment variable overrides: `MQTT_BROKER_URL`, `MQTT_TOPIC_PATTERN`, etc.
- ✅ Validate configuration on startup (fail fast if invalid)
- ✅ Log loaded configuration (mask secrets)

**Priority:** HIGH
**Dependencies:** None (foundation requirement)

---

### FR-2.11: Metrics Instrumentation

**Description:** Expose Prometheus metrics for monitoring ingestion pipeline

**Acceptance Criteria:**
- ✅ Use `metrics` crate with Prometheus exporter
- ✅ Expose metrics endpoint: `GET /metrics` (on port 8080)
- ✅ Counter metrics:
  - `ingestion_messages_received_total{topic}` - MQTT messages received
  - `ingestion_parse_errors_total` - JSON parse failures
  - `ingestion_validation_errors_total{reason}` - Validation failures by reason
  - `ingestion_points_written_total` - TimeSeriesPoints successfully stored
  - `ingestion_dlq_entries_total{reason}` - DLQ entries by reason
  - `ingestion_mqtt_disconnections_total` - Broker disconnections
  - `ingestion_mqtt_reconnections_total` - Successful reconnections
- ✅ Histogram metrics:
  - `ingestion_latency_seconds` - End-to-end (MQTT → storage) latency
  - `ingestion_batch_write_duration_seconds` - Storage write latency
  - `ingestion_mqtt_reconnection_duration_seconds` - Time to reconnect
- ✅ Gauge metrics:
  - `ingestion_mqtt_connected` - Connection status (0=disconnected, 1=connected)
  - `ingestion_buffer_size` - Pending points in memory
  - `ingestion_dlq_size_bytes` - DLQ disk usage

**Priority:** MEDIUM
**Dependencies:** All FRs (metrics instrument every step)

---

### FR-2.12: Health Status Reporting

**Description:** Expose health check endpoint showing ingestion pipeline status

**Acceptance Criteria:**
- ✅ Use existing `/health` endpoint from main.rs
- ✅ Add ingestion-specific checks:
  ```json
  {
    "healthy": true,
    "components": {
      "mqtt": {
        "status": "connected",
        "broker": "mosquitto:1883",
        "last_message_age_seconds": 45,
        "messages_received_total": 12543
      },
      "parser": {
        "status": "ok",
        "success_rate_pct": 99.8
      },
      "storage": {
        "status": "ok",
        "last_write_age_seconds": 3,
        "pending_batch_size": 47
      },
      "dlq": {
        "entries_today": 12,
        "size_bytes": 4096
      }
    }
  }
  ```
- ✅ Return HTTP 200 if healthy, 503 if degraded
- ✅ Consider degraded if:
  - MQTT disconnected for >60s
  - Parse error rate >5%
  - Storage write fails 3 consecutive times
  - Last message age >300s (5 minutes - sensor may be offline)

**Priority:** HIGH
**Dependencies:** FR-2.1, FR-2.7 (MQTT and storage components)

---

## 2. Integration Requirements

### IR-2.1: Wiring Components Together

**Description:** Connect existing components into functional pipeline

**Component Flow:**
```
MQTT Broker
    ↓ (rumqttc EventLoop)
MqttSource (core/src/sources/mqtt.rs)
    ↓ (raw payload: &[u8])
Parser (domains/air-quality/src/parser.rs)
    ↓ (AirQualityReading)
Validator (domains/air-quality/src/validation.rs)
    ↓ (validated reading)
Quality Scorer (NEW: domains/air-quality/src/quality.rs)
    ↓ (reading + quality metadata)
Adapter (domains/air-quality/src/adapter.rs)
    ↓ (Vec<TimeSeriesPoint>)
Storage (neural-core Store trait)
    ↓ (Parquet files)
Disk
```

**Integration Points:**
1. **MqttSource → Parser:**
   - MqttSource receives `Packet::Publish` with `payload: Bytes`
   - Call `air_quality::parser::parse_mqtt_payload(payload.as_ref())`
   - Handle `ParserError` → send to DLQ

2. **Parser → Validator:**
   - Receive `AirQualityReading` from parser
   - Call `air_quality::validation::validate_reading(&reading)`
   - Handle `ValidationError` → send to DLQ

3. **Validator → Quality Scorer:**
   - Receive validated `AirQualityReading`
   - Call `air_quality::quality::calculate_quality(&reading, &config?)` (NEW)
   - Reject if score < threshold → send to DLQ

4. **Quality Scorer → Adapter:**
   - Receive `AirQualityReading` + quality metadata
   - Call `AirQualityAdapter::to_time_series_points(&reading)`
   - Inject quality tags into each point

5. **Adapter → Storage:**
   - Receive `Vec<TimeSeriesPoint>`
   - Batch accumulator: collect until batch_size or timeout
   - Call `store.write_batch(points).await`

**Priority:** HIGH
**Dependencies:** All FRs (this is the integration)

---

### IR-2.2: Main Application Changes

**Description:** Modify `apps/air-quality-app/src/main.rs` to start ingestion pipeline

**Current State:**
```rust
// apps/air-quality-app/src/main.rs (lines 52-62)
fn create_mock_services() -> air_quality_app::api::routes::AppServices {
    // Mock implementations (no real MQTT)
}
```

**Required Changes:**
```rust
// NEW: apps/air-quality-app/src/main.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... existing tracing setup ...

    // Load configuration
    let config = AppConfig::from_yaml("config.yaml")?;

    // Create real services (not mocks)
    let services = create_real_services(&config).await?;

    // Start ingestion pipeline in background
    let ingestion_handle = tokio::spawn(async move {
        let mut mqtt_source = MqttSource::new(config.ingestion.mqtt);
        mqtt_source.start().await.expect("MQTT source failed");

        // Ingestion loop
        loop {
            let points = mqtt_source.fetch().await?;
            if !points.is_empty() {
                services.store.write_batch(points).await?;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Start API server
    let app = create_router(services);
    let listener = tokio::net::TcpListener::bind(&config.server.addr()).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// NEW: Create real implementations
async fn create_real_services(config: &AppConfig) -> AppServices {
    use neural_core::sources::mqtt::MqttSource;
    use neural_core::storage::parquet::ParquetStore;

    let mqtt_config = config.ingestion.mqtt.clone();
    let mqtt_source = Arc::new(MqttSource::new(mqtt_config));

    let storage_config = config.ingestion.storage.clone();
    let store = Arc::new(ParquetStore::new(storage_config)?);

    AppServices {
        source: mqtt_source,
        store,
        // ... other services (alerts, locations, forecast)
    }
}
```

**Priority:** HIGH
**Dependencies:** IR-2.1 (component wiring), FR-2.10 (config management)

---

### IR-2.3: New Module Structure

**Description:** Create new ingestion module to house pipeline logic

**New Files to Create:**

1. **`domains/air-quality/src/quality.rs`** (NEW)
   - Quality scoring logic (FR-2.5)
   - Functions: `calculate_quality(reading, config) -> QualityResult`
   - Exports: `QualityScore`, `QualityFlags`

2. **`apps/air-quality-app/src/ingestion/mod.rs`** (NEW)
   - Pipeline orchestration
   - Background task management
   - Error handling and retry logic

3. **`apps/air-quality-app/src/ingestion/pipeline.rs`** (NEW)
   - `IngestionPipeline` struct
   - Methods: `start()`, `stop()`, `process_message()`

4. **`apps/air-quality-app/src/ingestion/dlq.rs`** (NEW)
   - Dead Letter Queue implementation
   - Methods: `write(entry)`, `list(date)`, `reprocess(date)`

5. **`apps/air-quality-app/src/ingestion/metrics.rs`** (NEW)
   - Metrics registration and helper functions

**Module Dependencies:**
```rust
// apps/air-quality-app/src/lib.rs
pub mod api;      // Existing
pub mod config;   // Existing
pub mod ingestion; // NEW - add this

// domains/air-quality/src/lib.rs
pub mod adapter;    // Existing
pub mod parser;     // Existing
pub mod types;      // Existing
pub mod validation; // Existing
pub mod quality;    // NEW - add this
```

**Priority:** HIGH
**Dependencies:** None (foundation requirement)

---

## 3. Configuration Requirements

### CR-3.1: MQTT Broker Configuration

**Configuration:**
```yaml
ingestion:
  mqtt:
    # Broker connection
    broker_url: "mqtt://mosquitto:1883"  # TCP
    # broker_url: "mqtts://mqtt.airgradient.com:8883"  # TLS

    # Client settings
    client_id: "neural-air-quality"
    keep_alive_seconds: 30
    connection_timeout_seconds: 10
    clean_session: true

    # Credentials (optional)
    username: "${MQTT_USERNAME}"  # From env var
    password: "${MQTT_PASSWORD}"  # From env var

    # TLS settings (if mqtts://)
    tls:
      ca_cert_path: "/certs/ca.crt"
      client_cert_path: "/certs/client.crt"
      client_key_path: "/certs/client.key"
```

**Validation Rules:**
- broker_url must start with mqtt:// or mqtts://
- Port defaults: 1883 (TCP), 8883 (TLS)
- client_id max length: 23 characters (MQTT spec)
- If mqtts://, tls.ca_cert_path required

**Priority:** HIGH

---

### CR-3.2: Topic Configuration

**Configuration:**
```yaml
ingestion:
  mqtt:
    topic_pattern: "airgradient/readings/+"
    qos: 1  # 0, 1, or 2

    # Optional: Subscribe to multiple topics
    additional_topics:
      - pattern: "airgradient/config/+"
        qos: 0
      - pattern: "airgradient/status/+"
        qos: 1
```

**Topic Pattern Rules:**
- Single-level wildcard: `+` (e.g., `airgradient/readings/+`)
- Multi-level wildcard: `#` (e.g., `airgradient/#`)
- Literal serial number: `airgradient/readings/ecda3b1eaaaf`

**Priority:** HIGH

---

### CR-3.3: Quality Thresholds Configuration

**Configuration:**
```yaml
ingestion:
  quality:
    min_score_threshold: 0.5  # Reject readings below this

    # Enable/disable individual checks
    enable_completeness_check: true
    enable_calibration_check: true
    enable_freshness_check: true

    # Calibration penalty settings
    co2_warmup_days: 21  # CO2 sensor ABC period
    voc_learning_hours: 12  # VOC/NOx learning offset
    high_humidity_threshold_pct: 80  # PM penalty threshold

    # Freshness thresholds
    freshness_excellent_seconds: 5
    freshness_good_seconds: 30
    freshness_acceptable_seconds: 60
```

**Priority:** MEDIUM

---

### CR-3.4: Storage Configuration

**Configuration:**
```yaml
ingestion:
  storage:
    base_path: "data/air_quality"

    # Write batching
    batch_size: 100  # Points per batch
    batch_timeout_seconds: 5  # Max wait before flush

    # WAL settings
    wal_enabled: true
    wal_path: "data/air_quality/wal"
    wal_sync_interval_seconds: 1

    # Retention
    retention_days: 365
    cleanup_interval_hours: 24
```

**Priority:** HIGH

---

### CR-3.5: Dead Letter Queue Configuration

**Configuration:**
```yaml
ingestion:
  dlq:
    enabled: true
    base_path: "data/air_quality/dlq"
    max_entries_per_day: 10000
    retention_days: 7  # Auto-delete old DLQ entries
```

**Priority:** MEDIUM

---

## 4. Acceptance Criteria

### AC-4.1: End-to-End Data Flow

**Scenario:** Sensor publishes reading, system stores it

**Given:**
- MQTT broker running (Mosquitto)
- Air quality application running
- Configuration loaded

**When:**
- AirGradient sensor publishes to `airgradient/readings/ecda3b1eaaaf`:
  ```json
  {
    "serialno": "ecda3b1eaaaf",
    "rco2": 450,
    "pm02": 12.5,
    "atmp": 22.0,
    "rhum": 45.0,
    "wifi": -50
  }
  ```

**Then:**
- ✅ Message received within 1 second
- ✅ Parsed successfully to `AirQualityReading`
- ✅ Validated (all values in range)
- ✅ Quality score calculated (≥0.5)
- ✅ Converted to 5+ TimeSeriesPoints (co2, pm25, temperature, humidity, wifi)
- ✅ Written to Parquet: `data/air_quality/ecda3b1eaaaf/year=2025/month=12/day=14/readings.parquet`
- ✅ Queryable via REST API: `GET /api/v1/readings/latest?location_id=ecda3b1eaaaf`
- ✅ Health endpoint shows MQTT connected: `GET /health`

**Priority:** CRITICAL

---

### AC-4.2: Malformed Message Handling

**Scenario:** Invalid JSON sent to MQTT topic

**Given:**
- Application running

**When:**
- Publisher sends invalid JSON: `{"serialno": "test", invalid}`

**Then:**
- ✅ Parse error logged
- ✅ DLQ entry created: `data/air_quality/dlq/year=2025/month=12/day=14/errors.ndjson`
- ✅ DLQ contains error reason: "JSON parse error: ..."
- ✅ Metric incremented: `ingestion_parse_errors_total = 1`
- ✅ Application continues running (no crash)
- ✅ Next valid message processes successfully

**Priority:** HIGH

---

### AC-4.3: Out-of-Range Value Handling

**Scenario:** CO2 value exceeds sensor spec (>10,000 ppm)

**Given:**
- Application running

**When:**
- Publisher sends: `{"serialno": "test", "rco2": 15000, "pm02": 12.5}`

**Then:**
- ✅ Parsing succeeds
- ✅ Validation fails: `ValidationError::Co2OutOfRange(15000)`
- ✅ DLQ entry created with reason: "CO2 out of range: 15000 ppm"
- ✅ Metric incremented: `ingestion_validation_errors_total{reason="co2_out_of_range"} = 1`
- ✅ No data written to Parquet
- ✅ Application continues running

**Priority:** HIGH

---

### AC-4.4: Low Quality Score Handling

**Scenario:** Reading has incomplete data and low quality score

**Given:**
- Quality threshold configured: `min_score_threshold: 0.5`

**When:**
- Publisher sends: `{"serialno": "test", "pm02": 12.5}` (only 1 of 5 key fields)
- Completeness = 1/5 = 0.2
- Quality score = 0.2 < 0.5

**Then:**
- ✅ Parsing succeeds
- ✅ Validation succeeds (pm02 in range)
- ✅ Quality score fails threshold
- ✅ DLQ entry created with reason: "Quality score too low: 0.2"
- ✅ Metric incremented: `ingestion_dlq_entries_total{reason="quality_too_low"} = 1`
- ✅ No data written to Parquet

**Priority:** MEDIUM

---

### AC-4.5: MQTT Reconnection After Disconnect

**Scenario:** Broker goes offline then comes back

**Given:**
- Application connected to MQTT broker
- Ingesting data successfully

**When:**
1. Stop MQTT broker (simulate network outage)
2. Wait 60 seconds
3. Restart MQTT broker

**Then:**
- ✅ Disconnection detected within 30 seconds
- ✅ Health endpoint returns degraded: `{"mqtt": {"status": "disconnected"}}`
- ✅ Reconnection attempts logged: "Reconnecting in 1s", "Reconnecting in 2s", etc.
- ✅ Successful reconnection within 60 seconds of broker restart
- ✅ Re-subscription to topics automatic
- ✅ Data ingestion resumes
- ✅ Metric incremented: `ingestion_mqtt_reconnections_total = 1`
- ✅ Health endpoint returns healthy again

**Priority:** HIGH

---

### AC-4.6: Storage Write Retry on Failure

**Scenario:** Parquet storage temporarily unavailable

**Given:**
- Application running
- Disk full or storage service down

**When:**
- MQTT message received and processed
- Storage write fails: `CoreError::Storage("disk full")`

**Then:**
- ✅ Error logged: "Storage write failed: disk full"
- ✅ Metric incremented: `ingestion_storage_errors_total = 1`
- ✅ Retry after 1 second (exponential backoff)
- ✅ If still fails, retry after 2s, 4s, 8s, etc.
- ✅ Points buffered in memory (up to 1000)
- ✅ If buffer full, oldest points dropped (logged)
- ✅ When storage recovers, buffered points written

**Priority:** MEDIUM

---

### AC-4.7: Metrics Endpoint Availability

**Scenario:** Monitor ingestion pipeline via Prometheus

**Given:**
- Application running
- 100 messages processed (95 success, 3 parse errors, 2 validation errors)

**When:**
- `GET /metrics`

**Then:**
- ✅ HTTP 200 response
- ✅ Prometheus format:
  ```
  # TYPE ingestion_messages_received_total counter
  ingestion_messages_received_total{topic="airgradient/readings/+"} 100

  # TYPE ingestion_parse_errors_total counter
  ingestion_parse_errors_total 3

  # TYPE ingestion_validation_errors_total counter
  ingestion_validation_errors_total{reason="co2_out_of_range"} 1
  ingestion_validation_errors_total{reason="pm_out_of_range"} 1

  # TYPE ingestion_points_written_total counter
  ingestion_points_written_total 475  # 95 messages × 5 points each

  # TYPE ingestion_mqtt_connected gauge
  ingestion_mqtt_connected 1
  ```

**Priority:** MEDIUM

---

### AC-4.8: Health Check Accuracy

**Scenario:** Health endpoint reflects actual pipeline state

**Given:**
- Application running

**When:**
- All components healthy

**Then:**
- ✅ `GET /health` returns 200
- ✅ Response body:
  ```json
  {
    "healthy": true,
    "components": {
      "mqtt": {"status": "connected", "last_message_age_seconds": 45},
      "parser": {"status": "ok", "success_rate_pct": 99.8},
      "storage": {"status": "ok", "last_write_age_seconds": 3}
    }
  }
  ```

**When:**
- MQTT disconnected for 2 minutes

**Then:**
- ✅ `GET /health` returns 503
- ✅ Response: `{"healthy": false, "components": {"mqtt": {"status": "disconnected"}}}`

**Priority:** HIGH

---

## 5. Performance Requirements

### PR-5.1: Ingestion Latency

**Requirement:** Process MQTT message to storage within 1 second (p95)

**Measurement:**
- Metric: `ingestion_latency_seconds` histogram
- p95 < 1.0s
- p99 < 2.0s

**Rationale:** Real-time alerting requires <1 minute detection delay (budget 1s for ingestion)

---

### PR-5.2: Throughput

**Requirement:** Handle 60 readings/minute/location (1-minute sensor polling)

**Measurement:**
- 1 location: 1 msg/min = 5 points/min
- 10 locations: 10 msg/min = 50 points/min
- 100 locations: 100 msg/min = 500 points/min
- Target: 1000 msg/min = 5000 points/min (100× safety margin)

**Rationale:** Support future multi-sensor deployments

---

### PR-5.3: Memory Footprint

**Requirement:** <100MB RAM for ingestion pipeline (excluding storage cache)

**Measurement:**
- Metric: `process_resident_memory_bytes`
- Batch buffer: ~100 points × 1KB = 100KB
- MQTT client: ~10MB
- Total: <100MB

**Rationale:** Raspberry Pi 5 has 16GB RAM - leave room for other components

---

### PR-5.4: Reconnection Time

**Requirement:** Reconnect to MQTT broker within 60 seconds of broker restart

**Measurement:**
- Metric: `ingestion_mqtt_reconnection_duration_seconds` histogram
- p95 < 60s

**Rationale:** Minimize data loss window during network outages

---

## 6. Testing Requirements

### TR-6.1: Unit Tests

**Required Test Coverage:**

1. **Parser Tests** (already exist in `domains/air-quality/src/parser.rs`)
   - ✅ Valid complete payload
   - ✅ Valid partial payload
   - ✅ Invalid JSON
   - ✅ Missing required field

2. **Validator Tests** (already exist in `domains/air-quality/src/validation.rs`)
   - ✅ All metrics in range
   - ✅ CO2 out of range
   - ✅ PM out of range
   - ✅ Multiple errors

3. **Quality Scorer Tests** (NEW: `domains/air-quality/src/quality.rs`)
   - Completeness calculation
   - Calibration penalties (warmup, high humidity)
   - Freshness factors
   - Combined score calculation

4. **Adapter Tests** (already exist in `domains/air-quality/src/adapter.rs`)
   - ✅ Conversion to TimeSeriesPoints
   - ✅ Tag injection
   - ✅ Metric extraction

**Target:** 90% line coverage for new code

---

### TR-6.2: Integration Tests

**Required Test Scenarios:**

1. **End-to-End Happy Path**
   - Start embedded MQTT broker
   - Publish valid message
   - Assert data in Parquet
   - Query via REST API

2. **Error Handling**
   - Publish malformed JSON
   - Assert DLQ entry created
   - Assert metrics incremented

3. **Reconnection**
   - Stop MQTT broker
   - Assert disconnection detected
   - Restart broker
   - Assert reconnection successful

4. **Storage Failure Recovery**
   - Mock storage write failure
   - Assert retry logic works
   - Assert buffering works

**Location:** `apps/air-quality-app/tests/ingestion_integration_test.rs`

---

### TR-6.3: Performance Tests

**Required Benchmarks:**

1. **Latency Benchmark**
   - Measure: MQTT publish → Parquet write
   - Target: p95 < 1s

2. **Throughput Benchmark**
   - Measure: Messages/second sustained
   - Target: 1000 msg/min

3. **Memory Profiling**
   - Measure: RSS after 1 hour at 100 msg/min
   - Target: <100MB

**Location:** `apps/air-quality-app/benches/ingestion_bench.rs`

---

## 7. Dependencies and Constraints

### D-7.1: External Dependencies

**New Crate Dependencies:**
- `rumqttc = "0.24"` (MQTT client) - Already in workspace
- `metrics = "0.21"` (instrumentation)
- `metrics-exporter-prometheus = "0.13"` (Prometheus exporter)

**Existing Dependencies (Reused):**
- `tokio` (async runtime)
- `serde_json` (JSON parsing)
- `chrono` (timestamps)
- `tracing` (logging)

---

### D-7.2: Hardware Constraints

**Development:**
- Mac (x86_64 or ARM64)
- 8GB+ RAM
- 10GB+ disk

**Production:**
- Raspberry Pi 5 (16GB RAM, ARM64)
- 128GB+ storage
- Reliable network (WiFi or Ethernet)

---

### D-7.3: Network Requirements

**MQTT Broker Access:**
- Local: Mosquitto on localhost:1883
- Cloud: AirGradient cloud MQTT (mqtts://mqtt.airgradient.com:8883)
- Firewall: Allow outbound TCP 1883 (or 8883 for TLS)

**AirGradient Sensor:**
- Must be configured to publish to same broker
- Configuration via web UI: `http://airgradient_{SERIAL}.local/config`

---

## 8. Out of Scope

**Explicitly NOT in AIR-002:**

1. **Local HTTP API Polling** (FR-1.1 from AIR-001)
   - Deferred to AIR-003
   - Requires mDNS discovery and HTTP client

2. **Configuration Endpoint Retrieval** (FR-1.5 from AIR-001)
   - Deferred to AIR-004
   - Unit conversion (Fahrenheit → Celsius)
   - Correction algorithm detection

3. **Alerting Logic** (FR-5 from AIR-001)
   - Deferred to AIR-005
   - Threshold evaluation
   - Alert generation and delivery

4. **Forecasting** (FR-4 from AIR-001)
   - Deferred to AIR-006
   - ruv-FANN integration
   - Feature engineering

5. **MCP Tools** (FR-6 from AIR-001)
   - Deferred to AIR-007
   - Claude integration

---

## 9. Success Metrics

### Functional Success

**Must Achieve:**
- ✅ Real sensor data flows MQTT → Parquet
- ✅ REST API returns actual stored readings (not mocks)
- ✅ Health endpoint shows accurate MQTT connection status
- ✅ DLQ captures invalid messages without crashing

**Validation:**
- Run for 24 hours with real AirGradient sensor
- Ingest 1440 messages (1 per minute)
- Verify all data queryable via API
- Verify zero crashes or data loss

### Performance Success

**Metrics:**
- Ingestion latency p95 < 1s
- Parse success rate >99%
- Storage write success rate >99.9%
- Reconnection time <60s

### Code Quality

**Standards:**
- 90% unit test coverage (new code)
- Zero clippy warnings
- All integration tests passing
- Documentation for public APIs

---

## 10. Implementation Phases

### Phase 1: Foundation (Days 1-2)

**Deliverables:**
- ✅ Create new module structure (`ingestion/`, `quality.rs`)
- ✅ Implement configuration loading (FR-2.10)
- ✅ Implement quality scoring (FR-2.5)
- ✅ Unit tests for quality scorer

### Phase 2: Core Pipeline (Days 3-4)

**Deliverables:**
- ✅ Wire components: MQTT → Parser → Validator → Adapter → Storage (IR-2.1)
- ✅ Implement background task in main.rs (IR-2.2)
- ✅ Implement Dead Letter Queue (FR-2.9)
- ✅ Integration test: happy path

### Phase 3: Resilience (Days 5-6)

**Deliverables:**
- ✅ Implement reconnection logic (FR-2.8)
- ✅ Implement storage retry with backoff (FR-2.7)
- ✅ Integration tests: error handling, reconnection

### Phase 4: Observability (Day 7)

**Deliverables:**
- ✅ Implement metrics instrumentation (FR-2.11)
- ✅ Implement health status reporting (FR-2.12)
- ✅ Prometheus dashboard (grafana.json)

### Phase 5: Validation (Days 8-10)

**Deliverables:**
- ✅ Performance benchmarks (TR-6.3)
- ✅ 24-hour soak test with real sensor
- ✅ Documentation updates
- ✅ Demo video showing E2E flow

---

## 11. Risks and Mitigations

### R-11.1: MQTT Broker Unavailability

**Risk:** Sensor and app cannot communicate if broker is down

**Mitigation:**
- Use reliable broker (Mosquitto with persistent storage)
- Monitor broker health separately
- Consider HA setup (future): clustered brokers

**Probability:** Low
**Impact:** High

---

### R-11.2: Storage Write Failures

**Risk:** Disk full or Parquet corruption causes data loss

**Mitigation:**
- Implement WAL for crash recovery (already exists)
- Retry writes with exponential backoff (FR-2.7)
- Buffer in memory during outages
- Monitor disk usage via metrics

**Probability:** Medium
**Impact:** High

---

### R-11.3: Parser Breaking Changes

**Risk:** AirGradient firmware update changes JSON schema

**Mitigation:**
- Version detection from `firmware` field
- Graceful degradation (Option fields)
- DLQ captures incompatible messages
- Monitor parse error rate in production

**Probability:** Low
**Impact:** Medium

---

### R-11.4: Quality Scoring Edge Cases

**Risk:** Incorrect penalties reject valid data

**Mitigation:**
- Make quality checks configurable (enable/disable)
- Default threshold conservative (0.5)
- Log rejected readings with scores
- DLQ allows reprocessing with adjusted settings

**Probability:** Medium
**Impact:** Low

---

## 12. References

### 12.1: Existing Code

| Component | Path | Purpose |
|-----------|------|---------|
| Parser | `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs` | JSON → AirQualityReading |
| Validator | `/workspaces/neural-data-platform/domains/air-quality/src/validation.rs` | Range validation |
| Adapter | `/workspaces/neural-data-platform/domains/air-quality/src/adapter.rs` | AirQualityReading → TimeSeriesPoints |
| MQTT Source | `/workspaces/neural-data-platform/core/src/sources/mqtt.rs` | MQTT client skeleton |
| Main App | `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs` | Application entry point |

### 12.2: Parent Specifications

- **AIR-001 v1.2.0:** `/workspaces/neural-data-platform/product/features/air-001/specs/01-specification.md`
  - FR-1: Data Ingestion (MQTT requirements)
  - FR-2: Storage (Parquet format)
  - NFR-1: Performance (latency targets)

### 12.3: Gap Analysis

- **Critical Blockers:** `/workspaces/neural-data-platform/product/features/air-001/current-state/gaps/critical-blockers.md`
  - Blocker #1: No MQTT ingestion pipeline (this spec addresses it)

### 12.4: External Documentation

- **rumqttc:** https://docs.rs/rumqttc/
- **AirGradient MQTT:** https://www.airgradient.com/support/kb-mqtt-conf/
- **Prometheus Metrics:** https://prometheus.io/docs/concepts/metric_types/

---

## 13. Approval Checklist

- [ ] Technical Lead: Architecture and integration approach approved
- [ ] Domain Expert: Quality scoring logic validated
- [ ] Product Owner: Scope and success criteria agreed
- [ ] DevOps: Configuration and deployment plan reviewed
- [ ] Security: No secrets in config files, TLS optional validated

---

## 14. Next Steps

**After Approval:**

1. **Create AIR-002 Branch:**
   ```bash
   git checkout -b feature/air-002-ingestion-pipeline
   ```

2. **Phase 1 Implementation:**
   - Create module structure
   - Implement quality scoring
   - Write unit tests

3. **Phase 2 Implementation:**
   - Wire components together
   - Modify main.rs
   - First integration test

4. **Iterative Development:**
   - Phase 3 (resilience)
   - Phase 4 (observability)
   - Phase 5 (validation)

5. **Pull Request:**
   - Merge to main after all tests pass
   - Tag release: `v0.2.0-air-002`

---

**Document Status:** Ready for Review
**Estimated Effort:** 40-60 hours (8-10 days @ 5-6 hours/day)
**Blocking:** E2E testing of entire air quality platform
