# Implementation Roadmap to E2E Testing

**Version:** 1.0.0
**Date:** December 14, 2025
**Purpose:** Prioritized implementation plan to achieve E2E test capability

---

## Executive Summary

To run a complete E2E test demonstrating the air-001 vision in Docker:

| Phase | Focus | Duration | Status |
|-------|-------|----------|--------|
| Phase 1 | MQTT Ingestion Pipeline | Week 1 | NOT STARTED |
| Phase 2 | Alert System | Week 2 | NOT STARTED |
| Phase 3 | Forecasting Integration | Week 3 | NOT STARTED |
| Phase 4 | E2E Test Infrastructure | Week 4 | NOT STARTED |

**Total Estimated Effort:** 200-280 hours (5-7 developer weeks)

---

## Phase 1: MQTT Ingestion Pipeline (CRITICAL)

### Goal
Data flows from AirGradient sensor → MQTT → Storage → REST API

### Tasks

#### 1.1 MQTT Client Integration (20-30 hours)

**File:** `apps/air-quality-app/src/ingestion/mqtt.rs`

```rust
pub struct MqttIngestion {
    client: AsyncClient,
    eventloop: EventLoop,
    config: MqttConfig,
}

impl MqttIngestion {
    pub async fn connect(&mut self) -> Result<()>;
    pub async fn subscribe(&self, topic: &str) -> Result<()>;
    pub async fn run(&mut self, tx: Sender<AirQualityReading>) -> Result<()>;
}
```

**Dependencies:**
- Add `rumqttc = { workspace = true }` to Cargo.toml (already in workspace)
- Create `src/ingestion/` module directory

**Acceptance Criteria:**
- [ ] Connect to MQTT broker with auto-reconnect
- [ ] Subscribe to `airgradient/readings/+` topic pattern
- [ ] Parse incoming messages using existing `parse_mqtt_payload()`
- [ ] Send parsed readings to processing channel

#### 1.2 Data Flow Pipeline (15-20 hours)

**File:** `apps/air-quality-app/src/pipeline.rs`

```rust
pub struct IngestionPipeline {
    mqtt: MqttIngestion,
    validator: AirQualityValidator,
    adapter: AirQualityAdapter,
    storage: ParquetStore,
}

impl IngestionPipeline {
    pub async fn run(&mut self) -> Result<()> {
        // 1. Receive reading from MQTT channel
        // 2. Validate reading
        // 3. Convert to TimeSeriesPoints
        // 4. Write to Parquet via WAL
        // 5. Update in-memory cache for API
    }
}
```

**Acceptance Criteria:**
- [ ] Background task runs continuously
- [ ] Validated readings written to Parquet
- [ ] WAL ensures durability
- [ ] Metrics emitted for observability

#### 1.3 Main.rs Integration (5-10 hours)

**Modify:** `apps/air-quality-app/src/main.rs`

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Existing: tracing, config loading

    // NEW: Initialize real storage
    let storage = ParquetStore::new(&config.storage)?;

    // NEW: Initialize MQTT ingestion
    let mqtt = MqttIngestion::new(&config.mqtt)?;

    // NEW: Create pipeline
    let pipeline = IngestionPipeline::new(mqtt, storage.clone());

    // NEW: Spawn background ingestion task
    tokio::spawn(async move {
        pipeline.run().await
    });

    // Existing: Start HTTP server (now with real storage)
    let app = create_router(storage);
    // ...
}
```

**Acceptance Criteria:**
- [ ] Server starts with real MQTT connection
- [ ] Background ingestion runs in separate task
- [ ] Graceful shutdown commits WAL

### Phase 1 Deliverables
- [ ] MQTT client connects to broker
- [ ] Readings flow to Parquet storage
- [ ] REST API returns real data
- [ ] Health endpoint shows accurate status

---

## Phase 2: Alert System (HIGH PRIORITY)

### Goal
Threshold violations generate alerts accessible via API

### Tasks

#### 2.1 Health Thresholds (10-15 hours)

**File:** `domains/air-quality/src/thresholds.rs`

```rust
pub enum AirQualityLevel {
    Good,
    Moderate,
    UnhealthyForSensitive,
    Unhealthy,
    VeryUnhealthy,
    Hazardous,
}

pub struct HealthThresholds {
    pub co2: ThresholdSet,
    pub pm25: ThresholdSet,
    pub voc: ThresholdSet,
}

impl HealthThresholds {
    pub fn evaluate(&self, reading: &AirQualityReading) -> Vec<ThresholdViolation>;
}
```

**Threshold Values (per spec):**
- CO2: >1000 Moderate, >1500 Poor, >2000 VeryPoor
- PM2.5: >12 USG, >35 Unhealthy, >55 VeryUnhealthy
- VOC: >150 Moderate, >200 Poor, >300 VeryPoor

#### 2.2 Alert Engine (15-20 hours)

**File:** `apps/air-quality-app/src/alerting/engine.rs`

```rust
pub struct AlertEngine {
    thresholds: HealthThresholds,
    store: Arc<AlertStore>,
    dedup_cache: HashMap<String, AlertState>,
}

impl AlertEngine {
    pub fn evaluate(&mut self, reading: &AirQualityReading) -> Vec<Alert> {
        // 1. Check thresholds
        // 2. Generate alerts for violations
        // 3. Deduplicate (require 10% drop to clear)
        // 4. Store new alerts
        // 5. Return for delivery
    }
}
```

#### 2.3 Pipeline Integration (5-10 hours)

**Modify:** `apps/air-quality-app/src/pipeline.rs`

```rust
impl IngestionPipeline {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            let reading = self.receive_reading().await?;

            // Existing: validate, adapt, store

            // NEW: Evaluate alerts
            let alerts = self.alert_engine.evaluate(&reading);
            for alert in alerts {
                self.alert_store.add(alert).await?;
                tracing::warn!(?alert, "Threshold violation");
            }
        }
    }
}
```

### Phase 2 Deliverables
- [ ] Health thresholds defined per spec
- [ ] Alert engine evaluates each reading
- [ ] Alerts stored and queryable via API
- [ ] Deduplication prevents alert storms

---

## Phase 3: Forecasting Integration (HIGH PRIORITY)

### Goal
ML-powered predictions available via REST API

### Tasks

#### 3.1 ruv-FANN Integration (20-25 hours)

**File:** `apps/air-quality-app/src/forecasting/mod.rs`

```rust
use neuro_divergent::prelude::*;

pub struct AirQualityForecaster {
    nf: NeuralForecast<f32>,
    model_path: PathBuf,
}

impl AirQualityForecaster {
    pub async fn load(path: &Path) -> Result<Self>;

    pub async fn forecast(
        &self,
        data: &[TimeSeriesPoint],
        horizon_hours: u8,
    ) -> Result<Vec<ForecastPoint>>;
}
```

**Dependencies:**
- Add ruv-FANN dependencies to Cargo.toml
- Include pre-trained LSTM model in `/models` volume

#### 3.2 Feature Engineering (15-20 hours)

**File:** `apps/air-quality-app/src/forecasting/features.rs`

```rust
pub struct FeaturePipeline {
    scaler: StandardScaler,
}

impl FeaturePipeline {
    pub fn transform(&self, readings: &[AirQualityReading]) -> DataFrame {
        // 1. Extract target metric (pm25 or co2)
        // 2. Add lag features (1h, 3h, 24h)
        // 3. Add rolling stats (mean, std over 1h)
        // 4. Add time features (hour, day_of_week)
        // 5. Normalize with z-score
    }
}
```

#### 3.3 API Handler Update (10-15 hours)

**Modify:** `apps/air-quality-app/src/api/handlers/forecast.rs`

```rust
pub async fn handle_forecast(
    State(state): State<AppState>,
    Query(params): Query<ForecastParams>,
) -> Result<Json<ForecastResponse>, ApiError> {
    // 1. Query last 24 hours of data
    let historical = state.storage.query_range(
        &params.location_id,
        Utc::now() - Duration::hours(24),
        Utc::now(),
    ).await?;

    // 2. Generate forecast
    let predictions = state.forecaster.forecast(
        &historical,
        params.horizon_hours,
    ).await?;

    // 3. Format response with confidence intervals
    Ok(Json(ForecastResponse { predictions }))
}
```

### Phase 3 Deliverables
- [ ] Pre-trained LSTM model loaded at startup
- [ ] Feature pipeline transforms raw readings
- [ ] Forecasts include p10/p50/p90 intervals
- [ ] <30s cold start, <2s warm inference

---

## Phase 4: E2E Test Infrastructure (MEDIUM PRIORITY)

### Goal
Automated E2E tests validate complete system in Docker

### Tasks

#### 4.1 Docker Compose E2E (10-15 hours)

**File:** `product/features/air-002/docker/docker-compose.e2e.yml`

- Mosquitto broker
- Air quality app
- Sensor simulator
- Test observer
- Test runner

(See docker-architecture.md for complete specification)

#### 4.2 Sensor Simulator (10-15 hours)

**Directory:** `tests/e2e/sensor-simulator/`

- Rust binary publishing mock AirGradient readings
- Configurable scenarios (normal, high_co2, high_pm25)
- Configurable publish interval

#### 4.3 Test Runner (15-20 hours)

**Directory:** `tests/e2e/test-runner/`

- Integration tests using reqwest
- Validate data flow end-to-end
- Assert health endpoint accuracy
- Assert data persistence
- Assert alert generation

#### 4.4 CI/CD Integration (5-10 hours)

**File:** `.github/workflows/e2e-tests.yml`

- Build all containers
- Run E2E test suite
- Upload test results
- Fail PR on test failure

### Phase 4 Deliverables
- [ ] `docker compose up` starts complete test environment
- [ ] Sensor simulator publishes mock readings
- [ ] Test runner validates all scenarios
- [ ] CI/CD runs E2E on every PR

---

## Dependency Graph

```
Phase 1 (MQTT)
    │
    ├─────────────────────┐
    │                     │
    ▼                     ▼
Phase 2 (Alerts)    Phase 3 (Forecasting)
    │                     │
    └─────────────────────┘
              │
              ▼
       Phase 4 (E2E Tests)
```

**Critical Path:** Phase 1 → Phase 4
**Parallel Track:** Phase 2 and Phase 3 can proceed in parallel after Phase 1

---

## Resource Allocation

### Option A: Single Developer (7 weeks)
```
Week 1-2: Phase 1 (MQTT)
Week 3:   Phase 2 (Alerts)
Week 4-5: Phase 3 (Forecasting)
Week 6-7: Phase 4 (E2E)
```

### Option B: Two Developers (4 weeks)
```
Developer 1:
  Week 1: Phase 1 (MQTT)
  Week 2-3: Phase 3 (Forecasting)
  Week 4: Phase 4 (E2E - test runner)

Developer 2:
  Week 1: Phase 1 support + Phase 2 prep
  Week 2: Phase 2 (Alerts)
  Week 3-4: Phase 4 (E2E - infrastructure)
```

### Option C: Three Developers (3 weeks)
```
Developer 1: Phase 1 (MQTT) → Phase 4 (integration)
Developer 2: Phase 2 (Alerts) → Phase 4 (test scenarios)
Developer 3: Phase 3 (Forecasting) → Phase 4 (CI/CD)
```

---

## Risk Mitigation

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| ruv-FANN integration issues | Medium | High | Use stub forecaster initially |
| MQTT connection instability | Low | Medium | Robust reconnect logic |
| Parquet schema evolution | Medium | Medium | Version schema, migration scripts |
| Docker build times | Low | Low | Use buildx cache |
| E2E test flakiness | Medium | Medium | Retries, deterministic data |

---

## Success Criteria

### Minimum Viable E2E
- [ ] Sensor data flows from MQTT to Parquet
- [ ] REST API returns real readings
- [ ] Health endpoint shows accurate status
- [ ] Docker Compose starts all services

### Full E2E
- [ ] All Phase 1-4 deliverables complete
- [ ] 45-minute E2E test suite passes
- [ ] CI/CD runs on every PR
- [ ] Multi-architecture images available

### Production Ready
- [ ] Pi5 deployment validated
- [ ] 24-hour stability test passes
- [ ] MCP tools functional
- [ ] Documentation complete

---

## Next Steps

1. **Immediate:** Begin Phase 1.1 (MQTT Client Integration)
2. **Week 1:** Complete Phase 1 deliverables
3. **Week 2:** Begin Phase 2 and Phase 3 in parallel
4. **Week 3:** Complete Phase 2/3, begin Phase 4
5. **Week 4:** Complete E2E infrastructure, run full test suite
