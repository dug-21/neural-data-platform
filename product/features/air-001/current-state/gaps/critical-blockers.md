# Critical Blockers for E2E Testing

**Analysis Date:** December 14, 2025
**Assessment:** What MUST be implemented before E2E testing is possible

---

## Blocker #1: No MQTT Ingestion Pipeline

### Current State
- Server starts but cannot receive sensor data
- No rumqttc client initialization
- No topic subscription
- No data flow to storage

### What Exists
- Domain parser ready (`parse_mqtt_payload()`)
- Validation rules ready (hardware-spec ranges)
- Adapter ready (TimeSeriesPoint conversion)
- Parquet storage ready (with WAL)

### What's Missing

```rust
// Needed in apps/air-quality-app/src/main.rs or new ingestion module

pub struct MqttIngestionPipeline {
    mqtt_client: AsyncClient,
    parser: AirQualityParser,
    validator: AirQualityValidator,
    storage: ParquetStore,
}

impl MqttIngestionPipeline {
    pub async fn run(&self) -> Result<()> {
        // 1. Connect to MQTT broker
        // 2. Subscribe to airgradient/readings/{serial}
        // 3. For each message:
        //    a. Parse JSON payload
        //    b. Validate ranges
        //    c. Convert to TimeSeriesPoints
        //    d. Write to Parquet via WAL
        // 4. Handle reconnection on failure
    }
}
```

### Implementation Steps

1. Add `rumqttc` dependency (already in workspace Cargo.toml)
2. Create `src/ingestion/mqtt.rs` module
3. Implement `MqttSource` using existing core `Source` trait
4. Add background task in `main.rs` to run ingestion
5. Wire parser → validator → adapter → storage

### Estimated Effort
- **Hours:** 40-60
- **Complexity:** Medium
- **Dependencies:** None (all components exist)

---

## Blocker #2: No Forecasting Integration

### Current State
- Forecast endpoint returns empty predictions
- ruv-FANN library exists but not connected
- No feature engineering pipeline

### What Exists
- 27+ neural models in `vendor/ruv-fann/neuro-divergent/`
- LSTM, NBEATS, NHITS available
- Model loading API ready
- Inference API ready

### What's Missing

```rust
// Needed integration between air-quality-app and ruv-FANN

pub struct AirQualityForecaster {
    model: Box<dyn BaseModel>,
    feature_pipeline: FeatureEngineeringPipeline,
}

impl AirQualityForecaster {
    pub async fn forecast(
        &self,
        location_id: &str,
        metric: &str,
        horizon_hours: u8,
    ) -> Result<ForecastResult> {
        // 1. Query last 24 hours of data
        // 2. Apply feature engineering
        //    - Lag features (1h, 3h, 24h)
        //    - Rolling stats (mean, std)
        //    - Time features (hour, day_of_week)
        // 3. Run model inference
        // 4. Return predictions with confidence intervals
    }
}
```

### Implementation Steps

1. Add ruv-FANN dependencies to air-quality-app Cargo.toml
2. Create `src/forecasting/` module
3. Implement feature engineering pipeline
4. Load pre-trained LSTM/NBEATS models
5. Wire to forecast endpoint handler
6. Add forecast storage to Parquet

### Estimated Effort
- **Hours:** 50-70
- **Complexity:** High
- **Dependencies:** ruv-FANN model selection

---

## Blocker #3: No Alert Generation

### Current State
- Alerts endpoint exists but never generates alerts
- AlertStore is in-memory only
- No threshold monitoring loop

### What Exists
- Alert struct defined (severity, status)
- AlertStore with add/get methods
- Validation ranges (could be health thresholds)

### What's Missing

```rust
// Needed alert generation loop

pub struct AlertEngine {
    thresholds: HealthThresholds,
    alert_store: Arc<AlertStore>,
    dedup_cache: HashMap<String, DateTime<Utc>>,
}

pub struct HealthThresholds {
    co2: ThresholdSet,    // >1000 Moderate, >1500 Poor, >2000 VeryPoor
    pm25: ThresholdSet,   // >12 USG, >35 Unhealthy, >55 VeryUnhealthy
    voc: ThresholdSet,    // >150 Moderate, >200 Poor, >300 VeryPoor
}

impl AlertEngine {
    pub fn evaluate(&mut self, reading: &AirQualityReading) -> Vec<Alert> {
        // 1. Check each metric against thresholds
        // 2. Generate alert if threshold exceeded
        // 3. Deduplicate (require 10% drop before clearing)
        // 4. Store to alert history
        // 5. Return new alerts
    }
}
```

### Implementation Steps

1. Define `HealthThresholds` struct with spec values
2. Create `src/alerting/engine.rs` module
3. Implement threshold evaluation logic
4. Add deduplication cache
5. Integrate with ingestion pipeline (evaluate on each reading)
6. Add alert persistence to Parquet

### Estimated Effort
- **Hours:** 30-40
- **Complexity:** Medium
- **Dependencies:** FR-1.1 (needs readings to evaluate)

---

## Blocker Resolution Order

### Phase 1: Data Flow (Week 1)
```
MQTT → Parse → Validate → Store → Query
```
- Implement FR-1.1 MQTT client
- Connect existing components
- Verify data appears in Parquet
- Verify REST API returns real data

### Phase 2: Alerting (Week 2)
```
Reading → Threshold Check → Alert Generation → Storage
```
- Implement FR-5.1 health thresholds
- Add alert engine to ingestion pipeline
- Verify alerts appear in API

### Phase 3: Forecasting (Week 3)
```
Historical Data → Features → Model → Predictions
```
- Implement FR-4.1 model integration
- Implement FR-4.2 feature engineering
- Verify forecasts appear in API

### Phase 4: E2E Validation (Week 4)
```
Docker → MQTT Simulator → Full Pipeline → Assertions
```
- Create E2E test Docker environment
- Write integration tests
- Validate all components work together

---

## Minimum Viable E2E

To demonstrate the vision with absolute minimum work:

### Must Have
1. **MQTT → Parquet** - Data flows from sensor to storage
2. **REST API returns real data** - Query actual stored readings
3. **Health endpoint accurate** - Shows real MQTT/storage status

### Can Stub
1. **Forecasting** - Return mock predictions (already done)
2. **Alerting** - Log threshold violations (don't persist)
3. **MCP Tools** - Defer to v1.1

### E2E Test Scenarios (Minimum)

```yaml
Scenario 1: Data Ingestion
  Given: MQTT broker running
  When: Sensor publishes reading to airgradient/readings/ecda3b1eaaaf
  Then: Reading appears in Parquet storage within 5 seconds
  And: GET /api/v1/readings/latest returns the reading

Scenario 2: Data Persistence
  Given: 100 readings ingested
  When: Application restarts
  Then: All 100 readings still queryable
  And: WAL replay completes successfully

Scenario 3: Health Monitoring
  Given: Application running with MQTT connected
  When: GET /health called
  Then: Response shows mqtt: "connected", storage: "ok"
  When: MQTT broker stopped
  Then: Response shows mqtt: "disconnected", status: "degraded"
```

---

## Risk Assessment

| Blocker | Risk if Delayed | Mitigation |
|---------|-----------------|------------|
| MQTT Ingestion | System unusable | Highest priority |
| Forecasting | Feature incomplete | Use mock predictions for demo |
| Alerting | Feature incomplete | Log warnings instead of alerts |

### Recommended Focus

**Week 1 Goal:** MQTT ingestion working end-to-end

This unblocks:
- Real data in system
- Meaningful REST API responses
- Accurate health status
- Foundation for alerting
- Data for forecasting

All other blockers can be addressed incrementally after Week 1.
