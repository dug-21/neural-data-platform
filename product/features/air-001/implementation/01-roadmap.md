# SPARC Implementation Roadmap: Neural Data Platform - Air Quality (air-001)

**Feature ID**: air-001
**Document Version**: 1.1.0
**Date**: 2025-12-13
**Phase**: SPARC - Implementation Planning
**Revision**: Docker-First Deployment with Complete AirGradient Integration

---

## Executive Summary

This roadmap outlines the phased implementation of the Neural Data Platform Air Quality feature (air-001). The implementation follows a Docker-first approach enabling seamless deployment across development machines, Raspberry Pi 5, and cloud environments.

**Key Deliverables:**
- Docker container ingesting data from AirGradient ONE sensors (29+ fields)
- Parquet-based time-series storage with Polars queries
- ruv-FANN forecasting models for air quality prediction
- REST API for queries and MCP server for Claude integration
- Multi-architecture images (amd64/arm64)

---

## Implementation Phases Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        IMPLEMENTATION ROADMAP                                │
│                                                                             │
│  Phase 1: Foundation                                                        │
│  ├─ Core traits & types ────────────────────────────────── Week 1-2        │
│  ├─ AirGradient parser (29+ fields) ────────────────────── Week 2          │
│  └─ Docker development environment ─────────────────────── Week 2          │
│                                                                             │
│  Phase 2: Data Pipeline                                                     │
│  ├─ MQTT ingestion ─────────────────────────────────────── Week 3          │
│  ├─ Local HTTP API ingestion ───────────────────────────── Week 3          │
│  ├─ Parquet storage implementation ─────────────────────── Week 3-4        │
│  └─ Validation & quality scoring ───────────────────────── Week 4          │
│                                                                             │
│  Phase 3: Intelligence Layer                                                │
│  ├─ AQI calculations ───────────────────────────────────── Week 5          │
│  ├─ Alert engine ───────────────────────────────────────── Week 5          │
│  ├─ ruv-FANN integration ───────────────────────────────── Week 5-6        │
│  └─ Forecasting pipeline ───────────────────────────────── Week 6          │
│                                                                             │
│  Phase 4: API & Integration                                                 │
│  ├─ REST API (Axum) ────────────────────────────────────── Week 7          │
│  ├─ MCP server ─────────────────────────────────────────── Week 7          │
│  └─ Health & metrics endpoints ─────────────────────────── Week 7          │
│                                                                             │
│  Phase 5: Production Deployment                                             │
│  ├─ Multi-arch Docker build ────────────────────────────── Week 8          │
│  ├─ Pi 5 deployment testing ────────────────────────────── Week 8          │
│  └─ Documentation & release ────────────────────────────── Week 8          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: Foundation (Weeks 1-2)

### Objectives
- Establish core domain-agnostic traits
- Implement AirGradient message parser with complete 29+ field support
- Set up Docker development environment

### 1.1 Core Traits Implementation

**Priority**: P0 (Critical)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Create core/ crate structure
  ├── □ core/Cargo.toml (dependencies)
  ├── □ core/src/lib.rs (module exports)
  ├── □ core/src/traits.rs (TimeSeriesPoint, Store, Source, Forecast)
  └── □ core/src/error.rs (CoreError, Result type)

□ Implement TimeSeriesPoint trait
  ├── □ timestamp() -> DateTime<Utc>
  ├── □ series_name() -> String
  ├── □ value() -> f64
  ├── □ tags() -> HashMap<String, String>
  └── □ quality_score() -> f64

□ Implement Store trait
  ├── □ append(point) -> Result<()>
  ├── □ append_batch(points) -> Result<()>
  ├── □ query_range(series, start, end, filters) -> Result<Vec<T>>
  ├── □ aggregate(series, start, end, agg, interval) -> Result<Vec<AggregatedPoint>>
  └── □ health() -> Result<HealthStatus>

□ Implement Source trait
  ├── □ id() -> &str
  ├── □ start() -> Result<()>
  ├── □ stream() -> Stream<Result<T>>
  └── □ stop() -> Result<()>

□ Write unit tests for traits (TDD)
  └── □ Test coverage > 90%
```

**Deliverables:**
- [ ] `core/` crate compiles with all traits defined
- [ ] Unit tests pass with 90%+ coverage
- [ ] Documentation for all public types

### 1.2 AirGradient Domain Types & Parser

**Priority**: P0 (Critical)
**Estimated Effort**: 4 days

**Tasks:**
```
□ Create domains/air-quality/ crate structure
  ├── □ domains/air-quality/Cargo.toml
  ├── □ domains/air-quality/src/lib.rs
  ├── □ domains/air-quality/src/types.rs
  ├── □ domains/air-quality/src/parser.rs
  ├── □ domains/air-quality/src/adapter.rs
  └── □ domains/air-quality/src/validation.rs

□ Implement AirQualityReading struct (29+ fields)
  ├── □ Device metadata (serialno, wifi, firmware, model, boot, ledMode)
  ├── □ CO2 (rco2)
  ├── □ PM Mass (pm01, pm02, pm10, pm02Compensated)
  ├── □ PM Standard (pm01Standard, pm02Standard, pm10Standard)
  ├── □ Particle counts (pm003Count..pm10Count - 6 fields)
  ├── □ Environmental (atmp, atmpCompensated, rhum, rhumCompensated)
  ├── □ VOC/NOx (tvocIndex, tvocRaw, noxIndex, noxRaw)
  └── □ Calculated fields (aqi, quality_score)

□ Implement parser for MQTT payload (12 fields)
  └── □ Handle subset of fields gracefully

□ Implement parser for Local API payload (29+ fields)
  └── □ Handle all fields including Compensated and Raw values

□ Implement dual-source merge logic
  └── □ Merge MQTT real-time with Local API extended fields

□ Write comprehensive parser tests
  ├── □ Complete payload parsing
  ├── □ MQTT subset parsing
  ├── □ Missing field handling
  ├── □ Invalid JSON handling
  └── □ Property tests for edge cases
```

**Test Data:**
```json
// Complete Local API payload for testing
{
  "wifi": -46,
  "serialno": "ecda3b1eaaaf",
  "rco2": 447,
  "pm01": 3, "pm02": 7, "pm10": 8,
  "pm02Compensated": 6,
  "pm01Standard": 3, "pm02Standard": 7, "pm10Standard": 8,
  "pm003Count": 442, "pm005Count": 380, "pm01Count": 98,
  "pm02Count": 12, "pm50Count": 2, "pm10Count": 1,
  "atmp": 25.87, "atmpCompensated": 24.47,
  "rhum": 43, "rhumCompensated": 49,
  "tvocIndex": 100, "tvocRaw": 33051,
  "noxIndex": 1, "noxRaw": 16307,
  "boot": 6, "bootCount": 6,
  "ledMode": "pm", "firmware": "3.1.4", "model": "I-9PSL"
}
```

**Deliverables:**
- [ ] `domains/air-quality/` crate compiles
- [ ] Parser handles both MQTT and Local API payloads
- [ ] Unit tests with 95%+ coverage
- [ ] Field availability matrix documented

### 1.3 Docker Development Environment

**Priority**: P0 (Critical)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Create Dockerfile (multi-stage)
  ├── □ Stage 1: deps (dependency caching)
  ├── □ Stage 2: builder (compilation)
  └── □ Stage 3: runtime (minimal image)

□ Create docker-compose.yml
  ├── □ mosquitto service (MQTT broker)
  ├── □ neural-air-quality service (main app)
  ├── □ Volumes for data persistence
  └── □ Network configuration

□ Create development helper scripts
  ├── □ scripts/dev-up.sh (start dev environment)
  ├── □ scripts/dev-down.sh (stop dev environment)
  ├── □ scripts/test-local.sh (run tests in container)
  └── □ scripts/build-multiarch.sh (multi-architecture build)

□ Create mosquitto configuration
  ├── □ mosquitto/config/mosquitto.conf
  └── □ mosquitto/config/acl.conf (optional)

□ Create example configuration
  └── □ config.example.toml

□ Test Docker environment
  ├── □ Container builds successfully
  ├── □ Container starts and health check passes
  └── □ MQTT connectivity works
```

**Deliverables:**
- [ ] Docker image builds for amd64
- [ ] docker-compose stack starts successfully
- [ ] MQTT broker accessible from container
- [ ] Development workflow documented

### 1.4 Configuration Management (config-store Integration)

**Priority**: P0 (Critical)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Implement ConfigManager using config-store crate
  ├── □ core/src/config/manager.rs
  ├── □ Integrate with workspace config-store crate
  ├── □ Use SecureInMemoryConfigStore for production
  └── □ Use InMemoryConfigStore for testing

□ Create YAML configuration structure
  ├── □ config/base/air-quality.yaml (main config)
  ├── □ config/base/storage.yaml (Parquet settings)
  ├── □ config/base/alerting.yaml (thresholds, channels)
  ├── □ config/base/forecasting.yaml (model settings)
  └── □ config/base/observability.yaml (logging, metrics)

□ Create environment overlays
  ├── □ config/overlays/development/overrides.yaml
  ├── □ config/overlays/staging/overrides.yaml
  └── □ config/overlays/production/overrides.yaml

□ Implement GitOps configuration loading
  ├── □ Use GitOpsLoader for base + overlay pattern
  ├── □ Support GitHub repository sourcing (remote config)
  ├── □ Implement local file fallback
  └── □ Configure refresh interval (5 min default)

□ Implement environment variable substitution
  ├── □ Support ${VAR_NAME} pattern
  ├── □ Support ${VAR_NAME:default} pattern
  └── □ Mask sensitive values in logs

□ Create JSON Schema for validation
  ├── □ schemas/air-quality-config.json
  ├── □ Integrate SchemaValidator from config-store
  └── □ Add --validate-config CLI flag

□ Implement configuration hot-reload
  ├── □ File system watcher for local changes
  ├── □ Polling for GitHub remote changes
  ├── □ Define reloadable vs non-reloadable settings
  └── □ Log configuration changes with diff

□ Write configuration tests
  ├── □ YAML loading tests
  ├── □ Environment variable substitution tests
  ├── □ Base + overlay merging tests
  ├── □ Schema validation tests
  ├── □ Secret blocking tests (SecretBlocker)
  └── □ Hot-reload detection tests
```

**Configuration Schema (config/base/air-quality.yaml):**
```yaml
air_quality:
  sensors:
    - serial: "ecda3b1eaaaf"
      name: "Living Room"
      location_id: "living-room"
      data_source: "both"

  ingestion:
    mqtt:
      broker_url: "${MQTT_BROKER_URL:mqtt://mosquitto:1883}"
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

  alerting:
    enabled: true
    channels:
      - type: webhook
        url: "${ALERT_WEBHOOK_URL}"
      - type: log
        level: warn

  forecasting:
    enabled: true
    model: "nhits"
    horizon_hours: 24
```

**Deliverables:**
- [ ] ConfigManager loads YAML from config-store
- [ ] Environment overlays merge correctly
- [ ] Environment variables substituted
- [ ] JSON Schema validation working
- [ ] Secret blocking prevents password storage
- [ ] Hot-reload updates thresholds without restart
- [ ] GitHub remote configuration supported
- [ ] Configuration tests pass with 90%+ coverage

---

## Phase 2: Data Pipeline (Weeks 3-4)

### Objectives
- Implement MQTT and HTTP ingestion sources
- Build Parquet storage backend
- Add data validation and quality scoring

### 2.1 MQTT Ingestion

**Priority**: P0 (Critical)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Implement MqttSource struct
  ├── □ Connection management with reconnect logic
  ├── □ Topic subscription (airgradient/readings/{SERIAL})
  ├── □ Message parsing integration
  └── □ Backpressure handling (bounded queue)

□ Add configuration support
  ├── □ broker_url, port
  ├── □ client_id
  ├── □ topic pattern
  ├── □ QoS level
  └── □ TLS (optional)

□ Implement error recovery
  ├── □ Exponential backoff on disconnect
  ├── □ Dead letter queue for invalid messages
  └── □ Metrics for connection status

□ Write integration tests
  ├── □ Test with testcontainers (MQTT broker)
  ├── □ Test reconnection behavior
  └── □ Test message throughput
```

**Deliverables:**
- [ ] MQTT source receives messages reliably
- [ ] Automatic reconnection on failure
- [ ] Integration tests pass

### 2.2 Local HTTP API Ingestion

**Priority**: P1 (High)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Implement HttpPollingSource struct
  ├── □ Configurable poll interval (default: 60s)
  ├── □ HTTP client with timeout
  └── □ mDNS discovery (optional)

□ Implement endpoint configuration
  ├── □ URL template: http://airgradient_{SERIAL}.local/measures/current
  ├── □ Multiple sensor support
  └── □ Fallback to IP address

□ Implement dual-source coordination
  ├── □ Merge MQTT + Local API readings
  ├── □ Deduplication by timestamp
  └── □ Priority handling (MQTT real-time, Local API for extended fields)

□ Write integration tests
  └── □ Mock HTTP server tests
```

**Deliverables:**
- [ ] HTTP polling retrieves full 29+ field payload
- [ ] Dual-source merge working correctly
- [ ] Integration tests pass

### 2.3 Parquet Storage Implementation

**Priority**: P0 (Critical)
**Estimated Effort**: 4 days

**Tasks:**
```
□ Implement ParquetStore struct
  ├── □ Base path configuration
  ├── □ Schema definition for AirQualityReading
  └── □ Write buffer management

□ Implement partition strategy
  ├── □ Path: data/{location_id}/{year}/{month}/{day}.parquet
  ├── □ Daily partitions
  └── □ Automatic directory creation

□ Implement write path
  ├── □ Buffered writes (batch size: 1000)
  ├── □ Periodic flush (interval: 60s)
  ├── □ Sort by timestamp before write
  └── □ Snappy compression

□ Implement read path
  ├── □ Partition pruning based on date range
  ├── □ Parallel partition scanning
  ├── □ Predicate pushdown
  └── □ Result limit (100k default)

□ Implement aggregation
  ├── □ Support Mean, Sum, Min, Max, Count, Percentile
  ├── □ Configurable interval (1m, 5m, 1h, 1d)
  └── □ Use Polars lazy evaluation

□ Implement compaction
  ├── □ Merge small files into larger ones
  ├── □ Deduplication by timestamp
  └── □ Background task scheduling

□ Write comprehensive tests
  ├── □ Unit tests for partition logic
  ├── □ Integration tests for write/read cycle
  └── □ Benchmark tests for performance
```

**Schema Definition:**
```rust
// Parquet schema matching AirQualityReading
Schema {
    timestamp: Timestamp(Microsecond, UTC),
    location_id: Utf8,

    // Device metadata
    wifi: Int8,
    firmware: Utf8,
    model: Utf8,
    boot_count: UInt32,

    // CO2
    co2: UInt16,

    // PM Mass
    pm01: UInt16,
    pm25: UInt16,
    pm10: UInt16,
    pm25_compensated: UInt16,

    // PM Standard
    pm01_standard: UInt16,
    pm25_standard: UInt16,
    pm10_standard: UInt16,

    // Particle counts
    pm003_count: UInt32,
    pm005_count: UInt32,
    pm01_count: UInt32,
    pm02_count: UInt32,
    pm50_count: UInt32,
    pm10_count: UInt32,

    // Environmental
    temperature: Float32,
    temperature_compensated: Float32,
    humidity: Float32,
    humidity_compensated: Float32,

    // VOC/NOx
    tvoc_index: UInt16,
    tvoc_raw: UInt32,
    nox_index: UInt16,
    nox_raw: UInt32,

    // Quality
    quality_score: Float32,
    data_source: Utf8,
}
```

**Deliverables:**
- [ ] Parquet storage writes and reads correctly
- [ ] Partition strategy working
- [ ] Query performance meets targets
- [ ] Compaction reduces file count

### 2.4 Validation & Quality Scoring

**Priority**: P1 (High)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Implement Validator trait for AirQualityReading
  ├── □ Required field checks (serialno)
  ├── □ Range validation per sensor
  ├── □ Cross-field consistency (PM2.5 ≤ PM10)
  └── □ Temporal validation (no future timestamps)

□ Implement quality scoring algorithm
  ├── □ Base score from completeness
  ├── □ Penalties for warnings
  └── □ Penalties for missing fields

□ Implement sanitization
  ├── □ Clamp out-of-range values
  ├── □ Convert NaN to None
  └── □ Trim whitespace

□ Write validation tests
  └── □ Test all validation rules
```

**Validation Rules:**
| Field | Min | Max | Notes |
|-------|-----|-----|-------|
| pm25 | 0 | 1000 | μg/m³ |
| pm10 | 0 | 1000 | μg/m³ |
| co2 | 400 | 10000 | ppm |
| temperature | -40 | 85 | °C |
| humidity | 0 | 100 | % |
| tvoc_index | 0 | 500 | index |
| nox_index | 0 | 500 | index |

**Deliverables:**
- [ ] All validation rules implemented
- [ ] Quality scoring working correctly
- [ ] Invalid data handled gracefully

---

## Phase 3: Intelligence Layer (Weeks 5-6)

### Objectives
- Implement EPA AQI calculations
- Build alert engine with threshold monitoring
- Integrate ruv-FANN forecasting

### 3.1 AQI Calculations

**Priority**: P0 (Critical)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Implement EPA AQI breakpoint calculations
  ├── □ PM2.5 AQI (24-hour average)
  ├── □ PM10 AQI
  └── □ NowCast algorithm (recent-weighted)

□ Implement CO2 health index
  ├── □ Custom scale (not EPA standard)
  └── □ Cognitive impact thresholds

□ Implement TVOC index mapping
  └── □ Sensirion SGP41 index interpretation

□ Implement composite AQI
  ├── □ Maximum of individual pollutant AQIs
  └── □ Dominant pollutant identification

□ Implement AQI categories
  ├── □ Good (0-50)
  ├── □ Moderate (51-100)
  ├── □ Unhealthy for Sensitive (101-150)
  ├── □ Unhealthy (151-200)
  ├── □ Very Unhealthy (201-300)
  └── □ Hazardous (301-500)

□ Write comprehensive AQI tests
  └── □ Test all breakpoint boundaries
```

**Deliverables:**
- [ ] AQI calculations match EPA standards
- [ ] 100% test coverage on AQI functions
- [ ] Categories correctly assigned

### 3.2 Alert Engine

**Priority**: P1 (High)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Implement AlertEngine struct
  ├── □ Threshold rule configuration
  ├── □ Rate limiting (cooldown periods)
  └── □ Alert dispatch

□ Implement threshold types
  ├── □ Absolute threshold (PM2.5 > 35.5)
  ├── □ Relative threshold (PM2.5 +50% in 1h)
  └── □ Duration threshold (PM2.5 > 12 for 24h)

□ Implement alert severity levels
  ├── □ Info, Warning, Error, Critical
  └── □ Escalation logic

□ Implement notification channels
  ├── □ In-memory (for API polling)
  ├── □ Webhook dispatcher
  └── □ Future: Email, SMS, Push

□ Write alert system tests
  ├── □ Threshold triggering
  ├── □ Rate limiting behavior
  └── □ Escalation logic
```

**Default Thresholds:**
```toml
[[thresholds]]
id = "pm25-moderate"
field = "pm25"
operator = "greater_than"
value = 12.0
severity = "info"

[[thresholds]]
id = "pm25-unhealthy"
field = "pm25"
operator = "greater_than"
value = 35.5
severity = "warning"

[[thresholds]]
id = "co2-poor"
field = "co2"
operator = "greater_than"
value = 1000
severity = "info"

[[thresholds]]
id = "co2-unhealthy"
field = "co2"
operator = "greater_than"
value = 2000
severity = "warning"
```

**Deliverables:**
- [ ] Alert engine triggers on threshold exceedance
- [ ] Rate limiting prevents alert fatigue
- [ ] Webhook notifications working

### 3.3 ruv-FANN Integration

**Priority**: P1 (High)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Create FannAdapter implementing Forecast trait
  ├── □ Model loading from safetensors
  ├── □ Model training from historical data
  └── □ Prediction generation

□ Implement data preparation pipeline
  ├── □ Feature engineering (temporal, lag, rolling)
  ├── □ Normalization (StandardScaler)
  └── □ Train/validation split

□ Implement model selection logic
  ├── □ NHITS for long-range forecasting
  ├── □ NBEATSx for exogenous variables
  └── □ Configuration-based selection

□ Implement prediction output
  ├── □ Point predictions
  ├── □ Confidence intervals (Monte Carlo)
  └── □ Feature importance

□ Write forecasting tests
  ├── □ Model training completes
  ├── □ Predictions are reasonable
  └── □ Performance benchmarks
```

**Deliverables:**
- [ ] ruv-FANN models integrated
- [ ] 24-hour forecasts generating
- [ ] Confidence intervals included

---

## Phase 4: API & Integration (Week 7)

### Objectives
- Build REST API with Axum
- Implement MCP server for Claude
- Add health and metrics endpoints

### 4.1 REST API

**Priority**: P0 (Critical)
**Estimated Effort**: 3 days

**Tasks:**
```
□ Create apps/air-quality-app/ crate
  ├── □ main.rs (entry point)
  ├── □ config.rs (configuration loading)
  └── □ api/ (REST handlers)

□ Implement API routes
  ├── □ GET  /health              (health check)
  ├── □ GET  /metrics             (Prometheus metrics)
  ├── □ GET  /api/v1/readings/latest
  ├── □ GET  /api/v1/readings?start=&end=&location=
  ├── □ GET  /api/v1/aggregate?start=&end=&interval=&agg=
  ├── □ GET  /api/v1/forecast?location=&horizon=
  ├── □ GET  /api/v1/alerts
  └── □ GET  /api/v1/locations

□ Implement request/response types
  ├── □ OpenAPI spec generation
  └── □ JSON serialization

□ Add middleware
  ├── □ Request logging
  ├── □ Error handling
  ├── □ CORS (configurable)
  └── □ Request timeout

□ Write API tests
  ├── □ Integration tests for all endpoints
  └── □ Error handling tests
```

**API Response Format:**
```json
{
  "status": "success",
  "data": { ... },
  "meta": {
    "timestamp": "2025-12-13T15:30:00Z",
    "request_id": "abc123"
  }
}
```

**Deliverables:**
- [ ] All API endpoints implemented
- [ ] OpenAPI documentation generated
- [ ] Integration tests pass

### 4.2 MCP Server

**Priority**: P1 (High)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Implement MCP server (rmcp)
  ├── □ Tool registration
  ├── □ Transport (stdio or SSE)
  └── □ Response formatting

□ Implement MCP tools
  ├── □ get_current_readings(location_id)
  ├── □ query_readings(location_id, start, end)
  ├── □ get_forecast(location_id, horizon)
  ├── □ get_health_recommendations(location_id)
  └── □ list_locations()

□ Add Claude integration documentation
  └── □ Tool descriptions for Claude
```

**MCP Tool Example:**
```rust
#[mcp_tool]
async fn get_current_readings(location_id: String) -> Result<AirQualityReading> {
    /// Returns the most recent air quality reading for a sensor location.
    /// Includes PM2.5, CO2, temperature, humidity, AQI, and health recommendations.
}
```

**Deliverables:**
- [ ] MCP server responds to tool calls
- [ ] Claude can query air quality data
- [ ] Documentation complete

### 4.3 Health & Metrics

**Priority**: P1 (High)
**Estimated Effort**: 1 day

**Tasks:**
```
□ Implement health endpoint
  ├── □ Storage health check
  ├── □ MQTT connection status
  └── □ Model availability

□ Implement Prometheus metrics
  ├── □ readings_ingested_total
  ├── □ readings_stored_total
  ├── □ parse_errors_total
  ├── □ alerts_triggered_total
  ├── □ storage_size_bytes
  └── □ query_latency_histogram

□ Configure Prometheus scraping
  └── □ /metrics endpoint
```

**Deliverables:**
- [ ] Health endpoint returns meaningful status
- [ ] Prometheus metrics exposed
- [ ] Grafana dashboards (optional)

---

## Phase 5: Production Deployment (Week 8)

### Objectives
- Build multi-architecture Docker images
- Test on Raspberry Pi 5
- Complete documentation and release

### 5.1 Multi-Architecture Build

**Priority**: P0 (Critical)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Configure buildx for multi-arch
  ├── □ Create builder instance
  └── □ QEMU emulation setup

□ Optimize Dockerfile for both architectures
  ├── □ amd64 build verification
  └── □ arm64 build verification

□ Set up CI/CD for multi-arch builds
  ├── □ GitHub Actions workflow
  └── □ Container registry push

□ Create release tags
  └── □ Semantic versioning
```

**Build Command:**
```bash
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  --tag ghcr.io/neural-data-platform/air-quality:1.0.0 \
  --push \
  .
```

**Deliverables:**
- [ ] Multi-arch images build successfully
- [ ] Images pushed to container registry
- [ ] CI/CD pipeline complete

### 5.2 Raspberry Pi 5 Deployment Testing

**Priority**: P0 (Critical)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Test on physical Raspberry Pi 5
  ├── □ Docker installation
  ├── □ Image pull (arm64)
  ├── □ Container startup
  └── □ Health check verification

□ Test with real AirGradient sensor
  ├── □ Configure sensor MQTT
  ├── □ Verify data ingestion
  └── □ Check storage writes

□ Performance testing
  ├── □ Memory usage < 1.5GB
  ├── □ CPU usage < 80%
  └── □ Sustained operation (24h)

□ Create setup script
  └── □ scripts/setup-pi5.sh

□ Test auto-restart
  ├── □ systemd integration
  └── □ Container restart on failure
```

**Deliverables:**
- [ ] Platform runs stably on Pi 5
- [ ] Setup script works correctly
- [ ] Performance within targets

### 5.3 Documentation & Release

**Priority**: P1 (High)
**Estimated Effort**: 2 days

**Tasks:**
```
□ Complete README.md
  ├── □ Quick start guide
  ├── □ Configuration reference
  ├── □ API documentation
  └── □ Troubleshooting

□ Create deployment guides
  ├── □ Docker Compose deployment
  ├── □ Raspberry Pi 5 guide
  └── □ Cloud deployment (future)

□ Generate API documentation
  ├── □ OpenAPI spec
  └── □ MCP tool documentation

□ Create release notes
  ├── □ Features list
  ├── □ Known issues
  └── □ Migration notes

□ Tag release
  └── □ v1.0.0
```

**Deliverables:**
- [ ] All documentation complete
- [ ] Release tagged and published
- [ ] Deployment guides tested

---

## Success Metrics

### Phase Completion Criteria

| Phase | Criteria | Measurement |
|-------|----------|-------------|
| Phase 1 | Core traits implemented, Docker dev env working | Unit tests pass |
| Phase 2 | Data ingestion and storage working | Integration tests pass |
| Phase 3 | AQI, alerts, forecasting operational | Integration tests pass |
| Phase 4 | API accessible, MCP working | E2E tests pass |
| Phase 5 | Running on Pi 5 with real sensor | Manual verification |

### Key Performance Indicators

| KPI | Target | Measurement |
|-----|--------|-------------|
| Message throughput | 1/sec sustained | Load test |
| Query latency (1 day) | < 100ms p99 | Benchmark |
| Forecast accuracy | MAPE < 15% | Model evaluation |
| Container startup | < 30s | Timing |
| Memory usage (Pi 5) | < 1.5GB | Monitoring |
| Test coverage | > 85% | CI report |

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| ruv-FANN ARM64 compatibility | High | Early testing, fallback to simpler models |
| AirGradient API changes | Medium | Version-specific parsers, field validation |
| Pi 5 resource constraints | Medium | Memory profiling, aggressive buffering |
| MQTT reliability | Low | Reconnection logic, message persistence |

---

## Dependencies

### External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| rumqttc | 0.23+ | MQTT client |
| polars | 0.35+ | DataFrame/Parquet |
| axum | 0.7+ | REST API |
| tokio | 1.0+ | Async runtime |
| serde | 1.0+ | Serialization |
| chrono | 0.4+ | Timestamps |
| ruv-fann | workspace | Forecasting |

### Internal Dependencies

| Crate | Depends On |
|-------|------------|
| `core` | `neural-core` (prediction types) |
| `domains/air-quality` | `core` |
| `apps/air-quality-app` | `core`, `domains/air-quality`, `neural-ml-ops` |

---

## Appendix: File Structure

```
neural-data-platform/
├── Cargo.toml                      # Workspace root
├── Dockerfile                       # Multi-stage Docker build
├── docker-compose.yml               # Development stack
├── config.example.toml              # Configuration template
│
├── core/                            # Generic time-series platform
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── traits.rs
│       ├── error.rs
│       ├── storage/
│       │   ├── mod.rs
│       │   └── parquet.rs
│       ├── sources/
│       │   ├── mod.rs
│       │   ├── mqtt.rs
│       │   └── http_poll.rs
│       └── forecast/
│           ├── mod.rs
│           └── fann_adapter.rs
│
├── domains/
│   └── air-quality/                 # Air quality domain
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── types.rs
│           ├── parser.rs
│           ├── adapter.rs
│           ├── validation.rs
│           ├── aqi.rs
│           └── alerts.rs
│
├── apps/
│   └── air-quality-app/             # Main application
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── config.rs
│           ├── api/
│           │   ├── mod.rs
│           │   ├── routes.rs
│           │   └── handlers/
│           └── mcp/
│               ├── mod.rs
│               └── tools.rs
│
├── tests/
│   ├── integration/
│   ├── container/
│   └── e2e/
│
├── benches/
│   └── air_quality_benchmarks.rs
│
├── scripts/
│   ├── dev-up.sh
│   ├── dev-down.sh
│   ├── setup-pi5.sh
│   └── build-multiarch.sh
│
└── product/features/air-001/
    ├── specs/01-specification.md
    ├── architecture/01-system-design.md
    ├── pseudocode/01-algorithms.md
    ├── refinement/01-criteria.md
    └── implementation/01-roadmap.md     # This document
```

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-13 | Claude | Initial roadmap |
| 1.1.0 | 2025-12-13 | Claude | Docker-first approach, 29+ AirGradient fields |
