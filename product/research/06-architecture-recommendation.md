# Architecture Recommendation: Air Quality Intelligence System

**Document Version:** 1.0
**Date:** 2025-12-13
**Status:** DRAFT for Review
**Target Deployment:** Pi (sensor ingestion) + M4 Mac (ML inference, dashboards)

---

## 1. Executive Summary

Based on comprehensive analysis of the existing neural-data-platform codebase and industry reference architectures, we recommend a **Hexagonal Architecture with Actor-Based Concurrency** pattern for the air quality intelligence system. This approach enables:

1. **Domain isolation**: Reuse of 70%+ of existing platform infrastructure while cleanly separating air quality domain logic
2. **Scalable concurrency**: Actor model handles high-frequency sensor data ingestion without race conditions
3. **Testability**: Hexagonal architecture allows domain logic testing independent of infrastructure
4. **Extensibility**: Plugin architecture enables future domain additions (weather, energy, health monitoring)

**Key Decision:** Build on existing platform rather than rewrite from scratch. Estimated effort to adapt platform: 2-3 months vs 6-9 months for new system.

**Recommended Technology Stack:**

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| **Core Domain** | Existing neural-core (refactored) | 95/100 domain-agnostic storage traits, proven event system |
| **Forecasting** | augurs (Grafana) | Production-ready time-series toolkit (ETS, MSTL, Prophet, DBSCAN) |
| **Deep Learning** | burn + burn-tch | PyTorch interop for complex multi-variate forecasting if needed |
| **Online Learning** | ADWIN (custom Rust) + EWC++ | Drift detection + catastrophic forgetting prevention |
| **Concurrency** | Tokio actors + existing eventbus | Proven pattern in trading platform, maps well to sensor streams |
| **Storage** | QuestDB (default) | Fastest ingestion, schema-agnostic, high-cardinality support |
| **MCP Integration** | rmcp (official SDK) | Clean tool registration API for Claude Code integration |
| **Agentic** | Andrew Ng patterns (custom) | Reflection, Tool Use, Planning, Multi-Agent coordination |

---

## 2. Recommended Architecture Pattern

### 2.1 Hexagonal Architecture with Actor Model

**Visual Structure:**

```
┌─────────────────────────────────────────────────────────────────┐
│                   PRIMARY ADAPTERS (Driving)                     │
│  REST API   │   MCP Server   │   CLI   │   Web Dashboard        │
└──────────────────┬───────────────────────────────────────────────┘
                   │ Driving Ports (Application Layer)
         ┌─────────▼──────────┐
         │  CORE DOMAIN LOGIC  │ ◄─── Domain-Agnostic Platform Core
         │  (Hexagon)          │
         │  - TimeSeriesEvent  │
         │  - Predictor        │
         │  - Storage          │
         │  - EventBus         │
         └─────────┬──────────┘
                   │ Driven Ports (Infrastructure Needs)
┌──────────────────▼───────────────────────────────────────────────┐
│                SECONDARY ADAPTERS (Driven)                        │
│  AirQuality  │  QuestDB   │  Redis    │  HomeKit  │  MQTT       │
│  Adapter     │  Storage   │  Streams  │  Bridge   │  Publisher  │
└───────────────────────────────────────────────────────────────────┘
```

**Actor Model Integration:**

```
Data Flow: AirGradient Sensor → HTTP Handler → Ingestion Actor → Transform Actor → Storage Actor
                                    ↓               ↓                  ↓                ↓
                                 Validate       Enqueue            Process          Persist
                                                   ↓
                                            (Bounded Channels)
                                                   ↓
                                            Backpressure Control
```

**Key Benefits:**

1. **Isolation**: Domain logic (TimeSeriesEvent, Predictor) has zero dependencies on air quality specifics
2. **Testability**: Mock adapters for unit testing (InMemorySensorRepository vs PostgresSensorRepository)
3. **Swappability**: Replace QuestDB with InfluxDB by swapping adapter (no domain changes)
4. **Concurrency**: Per-sensor actors eliminate race conditions, natural backpressure via bounded channels

### 2.2 Why This Pattern for Air Quality?

**Air Quality Specific Characteristics:**

| Challenge | Pattern Benefit |
|-----------|----------------|
| High-frequency sensor data (1-60s intervals) | Actor model handles concurrent ingestion without locks |
| Multiple sensor types (CO2, PM2.5, temp, humidity) | Hexagonal ports abstract TimeSeriesEvent (any metric) |
| Future extensibility (weather, energy, occupancy) | New adapters plug in without core changes |
| Local (Pi) + Cloud (M4 Mac) deployment | Actor distribution via tokio enables hybrid deployment |
| Integration needs (HomeKit, Home Assistant, MQTT) | Secondary adapters for each integration point |

**Comparison to Alternatives:**

- **Microservices**: Too complex for single-building deployment, operational overhead
- **Monolithic CRUD**: Lacks concurrency model for high-frequency data, tight coupling
- **Lambda Architecture**: Overkill for moderate data volumes (1 sensor = ~3,600 points/hour)
- **CQRS + Event Sourcing**: Valuable for commands but excessive for every sensor reading

**Decision:** Start with modular monolith (hexagonal + actors), extract microservices only if scaling requires (e.g., multi-building network).

---

## 3. Component Reuse Analysis

### 3.1 Components to REUSE AS-IS (85-95/100 Domain-Agnostic)

#### **neural-core/traits/Storage** (95/100 - EXCELLENT)

**Justification:**
- Already supports TimeSeriesStorage trait with `store_point()`, `query_range()`, `aggregate()`
- Multi-backend support (Redis, PostgreSQL, TimescaleDB, InfluxDB, S3)
- No trading-specific logic in trait definitions

**Reuse Plan:**
```rust
// Air quality adapter uses existing trait
pub struct AirQualitySensorData {
    location_id: String,
    pm25_ugm3: f64,
    co2_ppm: f64,
    timestamp: DateTime<Utc>,
}

// Implement existing trait
impl TimeSeriesStorage for QuestDBStorage {
    async fn store_point(&self, series: &str, timestamp: DateTime<Utc>, value: f64) -> Result<()> {
        // QuestDB-specific implementation
        // series = "sensor-123.pm25", value = 35.2
    }
}
```

**Estimated Effort:** 0 hours (use as-is)

#### **neural-core/eventbus/** (90/100 - EXCELLENT)

**Justification:**
- Proto-only event system is fully domain-agnostic
- ProtoEventBus trait abstracts pubsub (InMemory, Redis implementations exist)
- Event envelope includes routing, quality metadata, tracing context
- DLQ, backpressure, batching already implemented

**Reuse Plan:**
```rust
// Define air quality proto events (new file)
// proto/air_quality.proto
message AirQualityEvent {
    string location_id = 1;
    float pm25 = 2;
    float co2 = 3;
    google.protobuf.Timestamp timestamp = 4;
    QualityFlags quality = 5;
}

// Use existing eventbus
let event_bus: Arc<dyn ProtoEventBus> = Arc::new(RedisEventBus::new(config));
event_bus.publish("air-quality.sensor-123", air_quality_event).await?;
```

**Estimated Effort:** 10 hours (define air quality proto schemas)

#### **neural-core/traits/Predictor** (85/100 - GOOD)

**Justification:**
- Generic async predict/train/evaluate/save_model interface
- No trading assumptions in trait
- Works with any time-series prediction task

**Reuse Plan:**
```rust
// Implement for air quality forecasting
pub struct AirQualityPredictor {
    model: Arc<RwLock<augurs::forecasting::Model>>,
}

impl Predictor for AirQualityPredictor {
    type Input = TimeSeriesData;  // Reuse existing type
    type Output = PredictionResult;  // Reuse existing type

    async fn predict(&self, input: &Self::Input) -> Result<Self::Output> {
        // Use augurs ETS/MSTL for forecasting
        let forecast = self.model.read().unwrap().forecast(input.values)?;
        Ok(PredictionResult { values: forecast, confidence: 0.95 })
    }

    async fn train(&mut self, data: &[Self::Input]) -> Result<()> {
        // Train with EWC++ for online learning
        self.model.write().unwrap().train_incremental(data)?;
        Ok(())
    }
}
```

**Estimated Effort:** 15 hours (wrap augurs models with Predictor trait)

#### **neural-ml-ops/** (80/100 - GOOD)

**Justification:**
- Generic training pipeline, model storage, feature engineering
- Training coordinator supports `model_type` parameter (domain-agnostic)
- Only limitation: some symbol-based organization (easily refactored)

**Reuse Plan:**
- Replace "symbol" with "location_id" or "sensor_id"
- Use existing training coordinator for scheduled model retraining
- Leverage feature engineering framework for air quality features (rolling averages, rate of change)

**Estimated Effort:** 20 hours (refactor symbol → sensor_id, adapt feature engineering)

#### **config-store/** (95/100 - EXCELLENT)

**Justification:**
- Configuration management is fully generic
- No domain-specific logic

**Reuse Plan:**
```yaml
# air_quality_config.yaml
sensors:
  - id: sensor-123
    location: living-room
    polling_interval_seconds: 60
    alert_thresholds:
      co2_ppm: 1000
      pm25_ugm3: 35.0

forecasting:
  model_type: augurs_ets
  horizon_hours: 24
  retrain_interval_hours: 168  # Weekly

storage:
  backend: questdb
  retention_days: 365
```

**Estimated Effort:** 5 hours (define air quality config schema)

### 3.2 Components to REFACTOR (70-80/100 - NEEDS WORK)

#### **neural-core/types/market.rs** (70/100 - TRADING CONTAMINATION)

**Issues:**
- MarketData, MarketContext, MarketTrend types have trading terminology
- Generic TimeSeriesData is good but wrapped in market-specific types

**Refactoring Plan:**

**Phase 1: Extract Generic Types**
```rust
// NEW: neural-core/types/timeseries.rs
pub struct TimeSeriesRecord<T> {
    pub timestamp: DateTime<Utc>,
    pub value: T,
    pub metadata: HashMap<String, String>,
}

pub trait TimeSeriesEvent {
    fn timestamp(&self) -> DateTime<Utc>;
    fn series_id(&self) -> &str;
    fn value(&self) -> f64;
}
```

**Phase 2: Move Trading Types to neural-trading**
```rust
// MOVE: neural-trading/src/types/market.rs
pub struct MarketData {
    pub timeseries: TimeSeriesRecord<f64>,
    pub market_context: MarketContext,  // Trading-specific
}
```

**Phase 3: Create Air Quality Adapter**
```rust
// NEW: air-quality-core/src/types/sensor.rs
pub struct AirQualityReading {
    pub location_id: String,
    pub pm25: f64,
    pub co2: f64,
    pub temperature: f64,
    pub humidity: f64,
    pub timestamp: DateTime<Utc>,
}

impl TimeSeriesEvent for AirQualityReading {
    fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    fn series_id(&self) -> &str { &self.location_id }
    fn value(&self) -> f64 { self.pm25 }  // Primary metric
}
```

**Estimated Effort:** 40 hours (extract generics, move trading types, create air quality types)

#### **data-staging/** (85/100 - MINOR REFACTORING)

**Issues:**
- Quality scorer field names reference market concepts (minor)
- JSON validation framework is generic (good)

**Refactoring Plan:**

**Current:**
```rust
// data-staging/src/quality_scorer.rs
fn score_completeness(data: &MarketData) -> f64 {
    // Check if price, volume, timestamp present
}
```

**Refactored:**
```rust
// NEW: data-staging/src/quality_scorer.rs (parameterized)
pub trait DataQualityScorer<T> {
    fn score_completeness(&self, data: &T) -> f64;
    fn score_freshness(&self, data: &T) -> f64;
    fn score_validity(&self, data: &T) -> f64;
}

// Air quality implementation
impl DataQualityScorer<AirQualityReading> for AirQualityScorer {
    fn score_completeness(&self, data: &AirQualityReading) -> f64 {
        let mut score = 0.0;
        if data.pm25 > 0.0 { score += 0.25; }
        if data.co2 > 0.0 { score += 0.25; }
        if data.temperature != 0.0 { score += 0.25; }
        if data.humidity > 0.0 { score += 0.25; }
        score
    }
}
```

**Estimated Effort:** 30 hours (parameterize quality scorer, adapt for air quality)

#### **mcp-trading-server/** (70/100 - TRADING-SPECIFIC TOOLS)

**Issues:**
- market_data.rs, trading.rs tools are 100% trading-specific
- Infrastructure (cache, health, neural, training_triggers) is generic

**Refactoring Plan:**

**Keep Generic:**
- cache.rs → Reuse for air quality forecast caching
- health.rs → Reuse for sensor health checks
- neural.rs → Reuse for air quality model inference
- training_triggers.rs → Reuse for scheduled model retraining

**Replace with Air Quality Tools:**
```rust
// NEW: mcp-air-quality-server/src/tools/air_quality.rs
#[tool]
/// Get current air quality readings from all sensors
async fn get_current_readings() -> Result<AirQualityReadings, Error> { ... }

#[tool]
/// Forecast air quality for specified hours ahead
async fn forecast_air_quality(hours_ahead: u32) -> Result<Forecast, Error> { ... }

#[tool]
/// Analyze ventilation effectiveness and recommend adjustments
async fn analyze_ventilation(current_rate: f64, target_co2: f64) -> Result<Analysis, Error> { ... }

#[tool]
/// Get health recommendations based on current and forecasted air quality
async fn get_health_recommendations() -> Result<Recommendations, Error> { ... }
```

**Estimated Effort:** 25 hours (implement air quality MCP tools using rmcp SDK)

### 3.3 Components to REPLACE (0-65/100 - INCOMPATIBLE)

#### **neural-core/types/trading.rs** (0/100 - 100% TRADING)

**Issue:**
- TradingAction, Signal, Position, TradingDecision are entirely trading-specific
- No reuse for air quality

**Replacement:**
```rust
// NEW: air-quality-core/src/types/action.rs
pub enum AirQualityAction {
    IncreaseVentilation { rate_cfm: f64 },
    DecreaseVentilation { rate_cfm: f64 },
    EnablePurifier,
    DisablePurifier,
    Alert { level: AlertLevel, message: String },
    NoAction,
}

pub struct VentilationDecision {
    pub action: AirQualityAction,
    pub confidence: f64,
    pub rationale: String,
}
```

**Estimated Effort:** 15 hours (define air quality action types)

#### **neural-trading/** (0/100 - ENTIRE CRATE TRADING-SPECIFIC)

**Issue:**
- DAA coordinator, inference predictor, risk manager are all trading-specific

**Replacement:**
```rust
// NEW: air-quality-core/src/agents/
//  - forecaster_agent.rs (predict air quality 6-48 hours ahead)
//  - analyst_agent.rs (identify pollution sources, trends)
//  - optimizer_agent.rs (ventilation scheduling, energy optimization)
//  - health_agent.rs (generate health recommendations)
//  - coordinator.rs (OODA loop, multi-agent orchestration)
```

**Estimated Effort:** 60 hours (implement agentic architecture from scratch using Andrew Ng patterns)

### 3.4 NEW Components Required (Gaps to Fill)

#### **air-quality-adapters/** (NEW CRATE)

**Purpose:** Domain-specific adapters for air quality ecosystem

**Structure:**
```
air-quality-adapters/
├── src/
│   ├── sources/
│   │   ├── airgradient.rs     # AirGradient API adapter
│   │   ├── mqtt.rs             # MQTT sensor adapter
│   │   └── http_poller.rs      # Generic HTTP polling adapter
│   ├── storage/
│   │   ├── questdb.rs          # QuestDB implementation
│   │   └── influxdb.rs         # InfluxDB implementation
│   ├── integrations/
│   │   ├── homekit_bridge.rs   # HomeKit accessory integration
│   │   ├── home_assistant.rs   # Home Assistant MQTT discovery
│   │   └── mqtt_publisher.rs   # Generic MQTT publishing
│   └── transformations/
│       ├── unit_converter.rs   # Convert units (F→C, ppm→ugm3)
│       └── aqi_calculator.rs   # Calculate Air Quality Index
```

**Estimated Effort:** 80 hours

#### **air-quality-forecasting/** (NEW CRATE)

**Purpose:** Wrap augurs models with Predictor trait, implement online learning

**Structure:**
```
air-quality-forecasting/
├── src/
│   ├── models/
│   │   ├── ets.rs              # Exponential smoothing (augurs)
│   │   ├── mstl.rs             # Multiple seasonal decomposition
│   │   ├── prophet.rs          # Facebook Prophet
│   │   └── ensemble.rs         # Combine multiple models
│   ├── online_learning/
│   │   ├── adwin.rs            # Concept drift detection
│   │   ├── ewc.rs              # Elastic Weight Consolidation
│   │   └── model_hotswap.rs    # Shadow model training & swap
│   └── evaluation/
│       ├── metrics.rs          # MAE, RMSE, coverage
│       └── backtesting.rs      # Historical validation
```

**Estimated Effort:** 100 hours

#### **air-quality-agents/** (NEW CRATE)

**Purpose:** Agentic intelligence (Reflection, Tool Use, Planning, Multi-Agent)

**Structure:**
```
air-quality-agents/
├── src/
│   ├── forecaster.rs           # Forecasting with reflection loop
│   ├── analyst.rs              # Trend analysis, correlation
│   ├── optimizer.rs            # Ventilation scheduling (RL/PBRS)
│   ├── health.rs               # Health recommendations
│   ├── coordinator.rs          # OODA loop, multi-agent orchestration
│   └── patterns/
│       ├── reflection.rs       # Self-critique and refinement
│       ├── tool_use.rs         # API calls, database queries
│       └── planning.rs         # Multi-step task decomposition
```

**Estimated Effort:** 120 hours

#### **mcp-air-quality-server/** (NEW CRATE)

**Purpose:** MCP server exposing air quality tools to Claude Code

**Structure:**
```
mcp-air-quality-server/
├── src/
│   ├── main.rs                 # rmcp server setup
│   ├── tools/
│   │   ├── data_access.rs      # get_current_readings, get_historical
│   │   ├── forecasting.rs      # forecast_air_quality, explain_forecast
│   │   ├── analysis.rs         # analyze_trends, detect_events
│   │   ├── optimization.rs     # optimize_ventilation, recommend_actions
│   │   └── health.rs           # get_health_recommendations, check_alerts
│   └── types/
│       └── mcp_schemas.rs      # Tool input/output schemas
```

**Estimated Effort:** 40 hours

**TOTAL NEW DEVELOPMENT:** 340 hours (~8.5 weeks @ 40 hrs/week)

### 3.5 Component Reuse Summary

| Category | Component | Reuse % | Effort (hrs) | Status |
|----------|-----------|---------|--------------|--------|
| **REUSE AS-IS** | neural-core/traits/Storage | 95% | 0 | Ready |
| **REUSE AS-IS** | neural-core/eventbus | 90% | 10 | Proto schemas only |
| **REUSE AS-IS** | neural-core/traits/Predictor | 85% | 15 | Wrap augurs |
| **REUSE AS-IS** | neural-ml-ops | 80% | 20 | Minor refactor |
| **REUSE AS-IS** | config-store | 95% | 5 | Config schemas |
| **REFACTOR** | neural-core/types/market.rs | 70% | 40 | Extract generics |
| **REFACTOR** | data-staging | 85% | 30 | Parameterize scorer |
| **REFACTOR** | mcp-trading-server (infra) | 70% | 25 | Replace tools |
| **REPLACE** | neural-core/types/trading.rs | 0% | 15 | New action types |
| **REPLACE** | neural-trading | 0% | 60 | New agents |
| **NEW** | air-quality-adapters | N/A | 80 | Sources, integrations |
| **NEW** | air-quality-forecasting | N/A | 100 | augurs + online learning |
| **NEW** | air-quality-agents | N/A | 120 | Agentic patterns |
| **NEW** | mcp-air-quality-server | N/A | 40 | MCP tools |
| **TOTAL** | | **72%** | **560 hrs** | **~14 weeks** |

**Key Insight:** 72% platform reuse saves ~6 months development time vs building from scratch.

---

## 4. Layer Architecture

### 4.1 Four-Layer Hexagonal Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                   LAYER 1: APPLICATION LAYER                         │
│  (Primary Adapters - Inbound Requests)                               │
│                                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │ REST API │  │   MCP    │  │   CLI    │  │   Web    │            │
│  │          │  │  Server  │  │          │  │ Dashboard│            │
│  └─────┬────┘  └─────┬────┘  └─────┬────┘  └─────┬────┘            │
│        │             │             │             │                  │
└────────┼─────────────┼─────────────┼─────────────┼──────────────────┘
         │             │             │             │
         └─────────────┴─────────────┴─────────────┘
                       │ (Driving Ports)
┌──────────────────────▼──────────────────────────────────────────────┐
│                   LAYER 2: CORE DOMAIN LAYER                         │
│  (Domain-Agnostic Abstractions - Reused from neural-core)            │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Core Traits (Interfaces/Ports)                         │          │
│  │  - TimeSeriesEvent<T>  (generic time-series record)    │          │
│  │  - Predictor           (async predict/train/evaluate)  │          │
│  │  - Storage             (KV store)                       │          │
│  │  - TimeSeriesStorage   (store_point, query_range)      │          │
│  │  - ProtoEventBus       (publish/subscribe)             │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Core Entities (Domain Models)                          │          │
│  │  - TimeSeriesRecord<T> (timestamp, value, metadata)    │          │
│  │  - PredictionResult    (values, confidence)            │          │
│  │  - EventEnvelope       (routing, quality, tracing)     │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Core Services (Business Logic)                         │          │
│  │  - TrainingCoordinator (schedule model retraining)     │          │
│  │  - FeatureEngineering  (rolling avg, rate of change)   │          │
│  │  - EventBus            (DLQ, backpressure, batching)   │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ (Driven Ports)
┌──────────────────────▼──────────────────────────────────────────────┐
│                   LAYER 3: DOMAIN ADAPTER LAYER                      │
│  (Air Quality Specific - NEW Development)                            │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Air Quality Types                                       │          │
│  │  - AirQualityReading   (pm25, co2, temp, humidity)     │          │
│  │  - AirQualityAction    (ventilation, purifier, alert)  │          │
│  │  - VentilationDecision (action, confidence, rationale) │          │
│  │  - HealthRecommendation (level, advice, timing)        │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Air Quality Services (Domain Logic)                    │          │
│  │  - AirQualityPredictor (wraps augurs ETS/MSTL/Prophet) │          │
│  │  - AQICalculator       (compute Air Quality Index)     │          │
│  │  - VentilationOptimizer (RL-based scheduling)          │          │
│  │  - HealthAdvisor       (personalized recommendations)  │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
│  ┌────────────────────────────────────────────────────────┐          │
│  │ Agentic Agents (Reflection, Planning, Multi-Agent)     │          │
│  │  - ForecasterAgent     (predict with reflection loop)  │          │
│  │  - AnalystAgent        (trend analysis, correlation)   │          │
│  │  - OptimizerAgent      (PBRS reward shaping)           │          │
│  │  - HealthAgent         (recommendations)               │          │
│  │  - CoordinatorAgent    (OODA loop orchestration)       │          │
│  └────────────────────────────────────────────────────────┘          │
│                                                                       │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ (Driven Ports Implementation)
┌──────────────────────▼──────────────────────────────────────────────┐
│                   LAYER 4: INFRASTRUCTURE LAYER                      │
│  (Secondary Adapters - Outbound Integration)                         │
│                                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │ QuestDB  │  │  Redis   │  │ HomeKit  │  │  MQTT    │            │
│  │ Storage  │  │ Streams  │  │  Bridge  │  │Publisher │            │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │
│                                                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │
│  │AirGradient│ │ Weather  │  │  Home    │  │Prometheus│            │
│  │   API    │  │   API    │  │Assistant │  │ Metrics  │            │
│  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Layer Responsibilities

#### **Layer 1: Application Layer (Primary Adapters)**

**Responsibility:** Handle inbound requests, translate to domain operations

**Components:**
- REST API (Axum): HTTP endpoints for data access, forecasting, control
- MCP Server (rmcp): Tools for Claude Code integration
- CLI (clap): Command-line interface for admin operations
- Web Dashboard (SvelteKit): Real-time visualization, configuration

**Key Principle:** No business logic, just translation (HTTP → domain calls)

**Example:**
```rust
// REST API endpoint (primary adapter)
async fn get_forecast(
    Extension(predictor): Extension<Arc<dyn Predictor>>,
    Query(params): Query<ForecastParams>,
) -> Result<Json<ForecastResponse>, ApiError> {
    // 1. Translate HTTP request to domain operation
    let input = TimeSeriesData { ... };

    // 2. Call domain service
    let prediction = predictor.predict(&input).await?;

    // 3. Translate domain result to HTTP response
    Ok(Json(ForecastResponse::from(prediction)))
}
```

#### **Layer 2: Core Domain Layer (Domain-Agnostic)**

**Responsibility:** Generic time-series abstractions, reusable across domains

**Reused from neural-core:**
- Traits: TimeSeriesEvent, Predictor, Storage, TimeSeriesStorage, ProtoEventBus
- Entities: TimeSeriesRecord, PredictionResult, EventEnvelope
- Services: TrainingCoordinator, FeatureEngineering, EventBus (DLQ, backpressure)

**Key Principle:** Zero air quality knowledge, works for any time-series domain

**Example:**
```rust
// Domain trait (generic)
#[async_trait]
pub trait Predictor: Send + Sync {
    type Input;
    type Output;

    async fn predict(&self, input: &Self::Input) -> Result<Self::Output>;
    async fn train(&mut self, data: &[Self::Input]) -> Result<()>;
    async fn evaluate(&self, test_data: &[Self::Input]) -> Result<f64>;
    async fn save_model(&self, path: &Path) -> Result<()>;
}
```

#### **Layer 3: Domain Adapter Layer (Air Quality Specific)**

**Responsibility:** Air quality business logic, implements core domain traits

**Components:**
- Types: AirQualityReading, AirQualityAction, VentilationDecision
- Services: AirQualityPredictor (wraps augurs), AQICalculator, VentilationOptimizer
- Agents: ForecasterAgent, AnalystAgent, OptimizerAgent, HealthAgent, CoordinatorAgent

**Key Principle:** All air quality knowledge lives here, isolated from core

**Example:**
```rust
// Air quality domain service (implements core trait)
pub struct AirQualityPredictor {
    ets_model: augurs::forecasting::ETS,
    mstl_model: augurs::forecasting::MSTL,
    drift_detector: AdwinDriftDetector,
    ewc_regularizer: EWCRegularizer,
}

impl Predictor for AirQualityPredictor {
    type Input = AirQualityReading;
    type Output = AirQualityForecast;

    async fn predict(&self, input: &Self::Input) -> Result<Self::Output> {
        // 1. Use augurs for forecasting
        let ets_forecast = self.ets_model.forecast(&input.pm25_history)?;
        let mstl_forecast = self.mstl_model.forecast(&input.co2_history)?;

        // 2. Apply reflection pattern (self-critique)
        let refined_forecast = self.reflect_on_forecast(ets_forecast, mstl_forecast)?;

        Ok(refined_forecast)
    }

    async fn train(&mut self, data: &[Self::Input]) -> Result<()> {
        // Use EWC++ to prevent catastrophic forgetting
        self.ets_model.train_incremental(data, &self.ewc_regularizer)?;

        // Update ADWIN for drift detection
        for reading in data {
            let error = self.compute_error(reading);
            if self.drift_detector.add_element(error) {
                println!("Concept drift detected! Full retraining triggered.");
                self.ets_model.retrain_from_scratch(data)?;
            }
        }

        Ok(())
    }
}
```

#### **Layer 4: Infrastructure Layer (Secondary Adapters)**

**Responsibility:** External integrations, implement infrastructure ports

**Components:**
- Storage: QuestDBStorage (implements TimeSeriesStorage), RedisStreams (implements ProtoEventBus)
- Sources: AirGradientAPIAdapter, MQTTSensorAdapter, HTTPPollerAdapter
- Integrations: HomeKitBridge, HomeAssistantMQTT, MQTTPublisher
- Observability: PrometheusMetrics, GrafanaAlloy (OpenTelemetry)

**Key Principle:** Swappable implementations, no business logic

**Example:**
```rust
// Infrastructure adapter (implements core trait)
pub struct QuestDBStorage {
    pool: PgPool,  // QuestDB uses PostgreSQL wire protocol
}

impl TimeSeriesStorage for QuestDBStorage {
    async fn store_point(&self, series: &str, timestamp: DateTime<Utc>, value: f64) -> Result<()> {
        sqlx::query(
            "INSERT INTO air_quality (series, timestamp, value) VALUES ($1, $2, $3)"
        )
        .bind(series)
        .bind(timestamp)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn query_range(&self, series: &str, start: DateTime<Utc>, end: DateTime<Utc>)
        -> Result<Vec<(DateTime<Utc>, f64)>> {
        let rows = sqlx::query_as::<_, (DateTime<Utc>, f64)>(
            "SELECT timestamp, value FROM air_quality
             WHERE series = $1 AND timestamp BETWEEN $2 AND $3
             ORDER BY timestamp ASC"
        )
        .bind(series)
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }
}
```

### 4.3 Dependency Flow (Hexagonal Rule)

**Critical Principle:** Dependencies point INWARD only

```
Application Layer (REST, MCP, CLI)
    ↓ depends on
Core Domain Layer (Traits, Entities, Services)
    ↑ implemented by
Domain Adapter Layer (Air Quality Services, Agents)
    ↑ uses
Infrastructure Layer (QuestDB, Redis, HomeKit, MQTT)
```

**Anti-Pattern (AVOID):**
```rust
// BAD: Core domain depends on infrastructure
pub struct Predictor {
    questdb: QuestDBStorage,  // ❌ Core knows about QuestDB
}
```

**Correct Pattern:**
```rust
// GOOD: Core domain depends on trait (interface)
pub struct Predictor {
    storage: Box<dyn TimeSeriesStorage>,  // ✅ Core knows only trait
}

// Infrastructure implements trait
impl TimeSeriesStorage for QuestDBStorage { ... }
impl TimeSeriesStorage for InfluxDBStorage { ... }  // Swappable!
```

---

## 5. Concurrency Model

### 5.1 Tokio + Actor Pattern

**Rationale:**

| Requirement | Actor Model Benefit |
|-------------|---------------------|
| High-frequency sensor data (1-60s) | Per-sensor actors handle concurrent ingestion without locks |
| Stateful processing (per-sensor state) | Actors encapsulate state (no shared memory race conditions) |
| Backpressure management | Bounded channels naturally apply backpressure when overloaded |
| Fault isolation | Per-sensor actors crash independently (no cascading failures) |
| Distribution (Pi + M4 Mac) | Actor model distributes naturally (local message passing → network RPC) |

**vs Alternative Concurrency Models:**

- **Shared State + Locks**: Race conditions, deadlock risk, complex to debug
- **Thread Pool**: No backpressure, no per-sensor state isolation
- **Green Threads (Go-style)**: Not idiomatic Rust, lacks ownership guarantees
- **Actors (Actix framework)**: More features than needed, simpler to use Tokio directly

**Decision:** Use Tokio channels + manual actor pattern (proven in neural-trading crate)

### 5.2 Actor Architecture

**Actor Types:**

```
┌────────────────────────────────────────────────────────────────┐
│                        Sensor Actors                            │
│  (One actor per sensor, handles ingestion + validation)        │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐      │
│  │ SensorActor   │  │ SensorActor   │  │ SensorActor   │      │
│  │  (sensor-123) │  │  (sensor-456) │  │  (sensor-789) │      │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘      │
│          │ (bounded channel)│                   │              │
└──────────┼──────────────────┼───────────────────┼──────────────┘
           │                  │                   │
           └──────────────────┴───────────────────┘
                              │
┌─────────────────────────────▼──────────────────────────────────┐
│                     Transform Actors                            │
│  (Process and enrich data)                                      │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐      │
│  │ UnitConverter │  │ AQICalculator │  │  Validator    │      │
│  │     Actor     │  │     Actor     │  │     Actor     │      │
│  └───────┬───────┘  └───────┬───────┘  └───────┬───────┘      │
│          │                  │                   │              │
└──────────┼──────────────────┼───────────────────┼──────────────┘
           │                  │                   │
           └──────────────────┴───────────────────┘
                              │
┌─────────────────────────────▼──────────────────────────────────┐
│                      Storage Actors                             │
│  (Persist to time-series database)                              │
│                                                                 │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐      │
│  │   QuestDB     │  │     Redis     │  │   S3 Backup   │      │
│  │ StorageActor  │  │  StreamActor  │  │     Actor     │      │
│  └───────────────┘  └───────────────┘  └───────────────┘      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Message Flow Example:**

```
AirGradient Sensor → HTTP POST /ingest
    ↓
HTTP Handler (validate auth, rate limit)
    ↓
SensorActor (sensor-123) receives IngestMessage
    ↓ (validate data quality, check freshness)
TransformActor receives TransformMessage
    ↓ (convert units, calculate AQI, enrich metadata)
StorageActor receives StoreMessage
    ↓ (batch writes, handle errors, DLQ for failures)
QuestDB persisted
```

### 5.3 Actor Implementation Pattern

**SensorActor Example:**

```rust
use tokio::sync::mpsc;
use std::time::Duration;

// Actor state (isolated, no shared memory)
struct SensorActor {
    sensor_id: String,
    last_reading_timestamp: Option<DateTime<Utc>>,
    transform_tx: mpsc::Sender<TransformMessage>,
}

// Messages the actor handles
enum SensorMessage {
    Ingest { reading: AirQualityReading },
    GetStatus { reply_to: mpsc::Sender<SensorStatus> },
    Shutdown,
}

// Actor implementation
impl SensorActor {
    async fn run(mut self, mut rx: mpsc::Receiver<SensorMessage>) {
        println!("SensorActor {} started", self.sensor_id);

        while let Some(msg) = rx.recv().await {
            match msg {
                SensorMessage::Ingest { reading } => {
                    // Validate reading
                    if self.is_valid_reading(&reading) {
                        // Update state
                        self.last_reading_timestamp = Some(reading.timestamp);

                        // Forward to transform actor
                        let transform_msg = TransformMessage {
                            sensor_id: self.sensor_id.clone(),
                            reading,
                        };

                        if let Err(e) = self.transform_tx.send(transform_msg).await {
                            eprintln!("Failed to send to transform actor: {}", e);
                        }
                    } else {
                        eprintln!("Invalid reading from {}: {:?}", self.sensor_id, reading);
                    }
                }

                SensorMessage::GetStatus { reply_to } => {
                    let status = SensorStatus {
                        sensor_id: self.sensor_id.clone(),
                        last_reading: self.last_reading_timestamp,
                        health: self.compute_health(),
                    };
                    let _ = reply_to.send(status).await;
                }

                SensorMessage::Shutdown => {
                    println!("SensorActor {} shutting down", self.sensor_id);
                    break;
                }
            }
        }
    }

    fn is_valid_reading(&self, reading: &AirQualityReading) -> bool {
        // Physics constraints
        if reading.co2_ppm < 400.0 || reading.co2_ppm > 5000.0 { return false; }
        if reading.pm25_ugm3 < 0.0 || reading.pm25_ugm3 > 500.0 { return false; }

        // Freshness check
        if let Some(last_ts) = self.last_reading_timestamp {
            let elapsed = (reading.timestamp - last_ts).num_seconds();
            if elapsed < 0 || elapsed > 600 { return false; }  // Max 10 min gap
        }

        true
    }

    fn compute_health(&self) -> SensorHealth {
        match self.last_reading_timestamp {
            Some(ts) if Utc::now() - ts < Duration::from_secs(300) => SensorHealth::Healthy,
            Some(_) => SensorHealth::Stale,
            None => SensorHealth::Unknown,
        }
    }
}

// Spawn actor (returns handle for sending messages)
fn spawn_sensor_actor(
    sensor_id: String,
    transform_tx: mpsc::Sender<TransformMessage>,
) -> mpsc::Sender<SensorMessage> {
    let (tx, rx) = mpsc::channel(100);  // Bounded channel (backpressure)

    let actor = SensorActor {
        sensor_id,
        last_reading_timestamp: None,
        transform_tx,
    };

    tokio::spawn(actor.run(rx));

    tx  // Return sender for other components to communicate with actor
}
```

### 5.4 Backpressure Strategy

**Bounded Channels:**

```rust
// Bounded channel (100 messages max)
let (tx, rx) = mpsc::channel::<SensorMessage>(100);

// When full, sender.send() returns error (caller must handle)
match tx.try_send(msg) {
    Ok(_) => {},
    Err(mpsc::error::TrySendError::Full(_)) => {
        // Backpressure: queue full, reject request
        return Err(ApiError::ServiceOverloaded);
    }
    Err(mpsc::error::TrySendError::Closed(_)) => {
        // Actor crashed, return error
        return Err(ApiError::InternalError);
    }
}
```

**Benefits:**
- Prevents memory overflow under high load
- Natural rate limiting (reject requests when overloaded)
- Forces downstream consumers to keep up or fail fast

### 5.5 Fault Tolerance

**Supervision Strategy:**

```rust
// Supervisor actor watches SensorActor, restarts on crash
struct Supervisor {
    sensor_id: String,
    restart_count: usize,
    max_restarts: usize,
}

impl Supervisor {
    async fn supervise(&mut self, transform_tx: mpsc::Sender<TransformMessage>) {
        loop {
            let sensor_tx = spawn_sensor_actor(self.sensor_id.clone(), transform_tx.clone());

            // Monitor actor health (via periodic status checks)
            tokio::select! {
                _ = self.health_check_loop(&sensor_tx) => {
                    println!("SensorActor {} crashed", self.sensor_id);
                }
                _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                    println!("SensorActor {} healthy for 1 hour", self.sensor_id);
                }
            }

            // Restart actor (with exponential backoff)
            self.restart_count += 1;
            if self.restart_count > self.max_restarts {
                eprintln!("SensorActor {} exceeded max restarts, giving up", self.sensor_id);
                break;
            }

            let backoff = Duration::from_secs(2u64.pow(self.restart_count as u32));
            println!("Restarting SensorActor {} in {:?}", self.sensor_id, backoff);
            tokio::time::sleep(backoff).await;
        }
    }
}
```

### 5.6 Distribution (Pi + M4 Mac)

**Current (Single-Process):** All actors run locally via Tokio channels

**Future (Distributed):** Replace `mpsc::channel` with network transport

**Distribution Options:**

| Option | Use Case | Complexity |
|--------|----------|------------|
| **Tarpc (RPC)** | Low latency, typed | Medium |
| **gRPC** | Broad ecosystem, streaming | Medium |
| **MQTT** | IoT-native, pub/sub | Low |
| **Redis Streams** | Existing integration | Low |

**Recommended:** Start with MQTT for Pi → M4 communication (IoT-native, low complexity)

**Example MQTT Bridge:**

```rust
// Pi: Publish sensor readings to MQTT
let mqtt_client = rumqttc::AsyncClient::new(...);
mqtt_client.publish("sensors/sensor-123/readings", payload).await?;

// M4 Mac: Subscribe to sensor readings
let mut eventloop = mqtt_client.eventloop();
while let Ok(notification) = eventloop.poll().await {
    if let Event::Incoming(Packet::Publish(p)) = notification {
        let reading: AirQualityReading = serde_json::from_slice(&p.payload)?;
        sensor_actor_tx.send(SensorMessage::Ingest { reading }).await?;
    }
}
```

---

## 6. Data Flow Architecture

### 6.1 Ingestion Pipeline (AirGradient → Redis Streams)

**Pipeline Stages:**

```
┌──────────────┐
│ AirGradient  │
│   Sensor     │ (Polling: 60s interval)
└──────┬───────┘
       │ HTTP GET /measures/current
       ↓
┌──────▼───────┐
│     Pi       │ (Raspberry Pi - Edge Device)
│   Poller     │
│   Service    │
│              │ 1. HTTP GET from AirGradient local API
│              │ 2. Validate JSON schema
│              │ 3. Publish to Redis Stream
└──────┬───────┘
       │ XADD air-quality-raw {sensor-123, pm25:35.2, co2:850, ...}
       ↓
┌──────▼───────┐
│ Redis Streams│ (Durable event log, max length 10000)
│              │
│  Stream:     │
│  air-quality-│
│      raw     │
└──────┬───────┘
       │ XREADGROUP (Consumer Group: processors)
       ↓
┌──────▼───────┐
│  M4 Mac      │ (Processing Server)
│  Processor   │
│  Service     │
│              │ 1. Consume from Redis Stream
│              │ 2. Route to SensorActor
│              │ 3. Transform → Store → Analyze
└──────────────┘
```

**Implementation:**

```rust
// Pi: Poller Service
async fn poll_airgradient_sensor(
    sensor_config: &SensorConfig,
    redis_client: &redis::Client,
) -> Result<()> {
    let client = reqwest::Client::new();

    loop {
        // 1. Poll AirGradient local API
        let response = client
            .get(&format!("http://{}/measures/current", sensor_config.ip_address))
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if response.status().is_success() {
            // 2. Parse JSON response
            let data: AirGradientResponse = response.json().await?;

            // 3. Validate data quality
            if validate_reading(&data) {
                // 4. Publish to Redis Stream
                let payload = serde_json::to_string(&data)?;
                let mut conn = redis_client.get_async_connection().await?;
                redis::cmd("XADD")
                    .arg("air-quality-raw")
                    .arg("MAXLEN")
                    .arg("~")  // Approximate max length
                    .arg(10000)
                    .arg("*")  // Auto-generate ID
                    .arg(&[
                        ("sensor_id", sensor_config.id.as_str()),
                        ("payload", payload.as_str()),
                        ("timestamp", &Utc::now().to_rfc3339()),
                    ])
                    .query_async::<_, String>(&mut conn)
                    .await?;

                println!("Published reading from {}", sensor_config.id);
            } else {
                eprintln!("Invalid reading from {}: {:?}", sensor_config.id, data);
            }
        } else {
            eprintln!("HTTP error from {}: {}", sensor_config.id, response.status());
        }

        // Sleep until next poll interval
        tokio::time::sleep(Duration::from_secs(sensor_config.poll_interval_secs)).await;
    }
}

// M4 Mac: Processor Service
async fn consume_redis_stream(
    redis_client: &redis::Client,
    sensor_actors: &HashMap<String, mpsc::Sender<SensorMessage>>,
) -> Result<()> {
    let mut conn = redis_client.get_async_connection().await?;

    // Create consumer group (idempotent)
    let _ = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg("air-quality-raw")
        .arg("processors")
        .arg("0")
        .arg("MKSTREAM")
        .query_async::<_, ()>(&mut conn)
        .await;

    loop {
        // Read from stream (block for 1 second if no data)
        let result: StreamReadReply = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg("processors")
            .arg("consumer-1")
            .arg("BLOCK")
            .arg(1000)  // 1 second block
            .arg("COUNT")
            .arg(10)  // Batch size
            .arg("STREAMS")
            .arg("air-quality-raw")
            .arg(">")  // Only new messages
            .query_async(&mut conn)
            .await?;

        for stream_key in result.keys {
            for stream_id in stream_key.ids {
                // Parse payload
                let sensor_id: String = stream_id.get("sensor_id").unwrap();
                let payload: String = stream_id.get("payload").unwrap();
                let reading: AirQualityReading = serde_json::from_str(&payload)?;

                // Route to SensorActor
                if let Some(actor_tx) = sensor_actors.get(&sensor_id) {
                    actor_tx.send(SensorMessage::Ingest { reading }).await?;

                    // Acknowledge message
                    redis::cmd("XACK")
                        .arg("air-quality-raw")
                        .arg("processors")
                        .arg(&stream_id.id)
                        .query_async::<_, i32>(&mut conn)
                        .await?;
                } else {
                    eprintln!("Unknown sensor: {}", sensor_id);
                }
            }
        }
    }
}
```

**Benefits:**

- **Durability**: Redis Streams persist messages (survives processor crashes)
- **At-Least-Once Delivery**: Consumer groups track acknowledgments
- **Backpressure**: If processor slow, Redis Stream buffers messages (up to max length)
- **Decoupling**: Pi and M4 Mac run independently (network failures tolerated)

**Alternative:** MQTT (if Redis not desired)

### 6.2 Processing Pipeline (Feature Engineering, ML Inference)

**Pipeline Stages:**

```
SensorActor (validate, enrich)
    → TransformActor (unit conversion, AQI calculation)
        → FeatureActor (rolling averages, rate of change)
            → StorageActor (persist to QuestDB)
                → AnalysisActor (detect anomalies, trends)
                    → ForecasterActor (predict 24h ahead)
                        → OptimizerActor (recommend ventilation)
                            → ActionActor (execute controls, send alerts)
```

**Feature Engineering Actor Example:**

```rust
struct FeatureActor {
    window_size: usize,
    pm25_window: VecDeque<f64>,
    co2_window: VecDeque<f64>,
    storage_tx: mpsc::Sender<StoreMessage>,
}

impl FeatureActor {
    async fn run(mut self, mut rx: mpsc::Receiver<FeatureMessage>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                FeatureMessage::Compute { reading } => {
                    // 1. Add to rolling window
                    self.pm25_window.push_back(reading.pm25_ugm3);
                    self.co2_window.push_back(reading.co2_ppm);

                    if self.pm25_window.len() > self.window_size {
                        self.pm25_window.pop_front();
                        self.co2_window.pop_front();
                    }

                    // 2. Compute features
                    let features = Features {
                        pm25_current: reading.pm25_ugm3,
                        pm25_5min_avg: self.pm25_window.iter().sum::<f64>() / self.pm25_window.len() as f64,
                        pm25_rate_of_change: self.compute_rate_of_change(&self.pm25_window),
                        co2_current: reading.co2_ppm,
                        co2_15min_avg: self.co2_window.iter().sum::<f64>() / self.co2_window.len() as f64,
                        aqi: self.compute_aqi(reading.pm25_ugm3, reading.co2_ppm),
                    };

                    // 3. Forward to storage
                    let store_msg = StoreMessage {
                        sensor_id: reading.sensor_id,
                        features,
                        timestamp: reading.timestamp,
                    };
                    self.storage_tx.send(store_msg).await.ok();
                }
            }
        }
    }

    fn compute_rate_of_change(&self, window: &VecDeque<f64>) -> f64 {
        if window.len() < 2 { return 0.0; }
        let first = window.front().unwrap();
        let last = window.back().unwrap();
        (last - first) / window.len() as f64
    }

    fn compute_aqi(&self, pm25: f64, co2: f64) -> f64 {
        // Simplified AQI calculation (actual is more complex)
        let pm25_contribution = (pm25 / 35.0) * 50.0;  // EPA standard: 35 ugm3 = moderate
        let co2_contribution = ((co2 - 400.0) / 600.0) * 50.0;  // 1000 ppm = moderate
        pm25_contribution.max(co2_contribution).min(500.0)
    }
}
```

**ML Inference Actor Example:**

```rust
struct ForecasterActor {
    predictor: Arc<RwLock<AirQualityPredictor>>,
    alert_tx: mpsc::Sender<AlertMessage>,
}

impl ForecasterActor {
    async fn run(mut self, mut rx: mpsc::Receiver<ForecastRequest>) {
        while let Some(req) = rx.recv().await {
            // 1. Generate forecast
            let predictor = self.predictor.read().unwrap();
            let forecast = predictor.predict(&req.input).await.unwrap();

            // 2. Apply reflection loop (self-critique)
            let refined_forecast = self.reflect_on_forecast(&forecast, &req.input);

            // 3. Check for alerts
            for (i, value) in refined_forecast.values.iter().enumerate() {
                if value.pm25_ugm3 > 55.0 {  // Unhealthy threshold
                    let alert = AlertMessage {
                        level: AlertLevel::Warning,
                        message: format!(
                            "PM2.5 forecast exceeds healthy threshold in {}h: {:.1} ugm3",
                            i, value.pm25_ugm3
                        ),
                        timestamp: value.timestamp,
                    };
                    self.alert_tx.send(alert).await.ok();
                }
            }

            // 4. Return forecast
            req.reply_to.send(refined_forecast).await.ok();
        }
    }

    fn reflect_on_forecast(&self, forecast: &AirQualityForecast, input: &TimeSeriesData) -> AirQualityForecast {
        // Reflection pattern: self-critique and refine
        let mut refined = forecast.clone();

        // Check for unrealistic predictions (physics constraints)
        for value in &mut refined.values {
            // CO2 can't drop below outdoor baseline (400 ppm)
            if value.co2_ppm < 400.0 {
                eprintln!("Unrealistic CO2 forecast: {} ppm, clamping to 400", value.co2_ppm);
                value.co2_ppm = 400.0;
            }

            // PM2.5 can't spike without cause (check recent trends)
            let recent_avg = input.values.iter().rev().take(5).sum::<f64>() / 5.0;
            if value.pm25_ugm3 > recent_avg * 3.0 {
                eprintln!("Suspicious PM2.5 spike: {}, recent avg: {}", value.pm25_ugm3, recent_avg);
                value.pm25_ugm3 = recent_avg * 1.5;  // Conservative adjustment
            }
        }

        refined
    }
}
```

### 6.3 Action Pipeline (Alerts, HomeKit, MQTT)

**Pipeline Stages:**

```
ForecasterActor (predictions)
    → OptimizerActor (ventilation optimization)
        → ActionExecutor (dispatch actions)
            ├─→ AlertActor (SMS, push notifications)
            ├─→ HomeKitBridge (update HomeKit accessories)
            ├─→ MQTTPublisher (publish to Home Assistant)
            └─→ HVACControlActor (adjust ventilation, if integrated)
```

**Action Executor Example:**

```rust
struct ActionExecutor {
    alert_tx: mpsc::Sender<AlertMessage>,
    homekit_tx: mpsc::Sender<HomeKitMessage>,
    mqtt_tx: mpsc::Sender<MQTTMessage>,
}

impl ActionExecutor {
    async fn execute(&self, decision: VentilationDecision) {
        match decision.action {
            AirQualityAction::IncreaseVentilation { rate_cfm } => {
                println!("Increasing ventilation to {} CFM (confidence: {:.2})",
                         rate_cfm, decision.confidence);

                // Send to HomeKit (if integrated)
                self.homekit_tx.send(HomeKitMessage::UpdateVentilationRate { rate_cfm }).await.ok();

                // Publish to MQTT (for Home Assistant automation)
                let payload = json!({
                    "action": "increase_ventilation",
                    "rate_cfm": rate_cfm,
                    "timestamp": Utc::now().to_rfc3339(),
                });
                self.mqtt_tx.send(MQTTMessage {
                    topic: "air-quality/actions/ventilation",
                    payload: payload.to_string(),
                }).await.ok();
            }

            AirQualityAction::Alert { level, message } => {
                println!("Sending alert: {} - {}", level, message);

                // Send notification
                self.alert_tx.send(AlertMessage {
                    level,
                    message,
                    timestamp: Utc::now(),
                }).await.ok();

                // Update HomeKit accessory (trigger alert)
                self.homekit_tx.send(HomeKitMessage::TriggerAlert { level }).await.ok();
            }

            AirQualityAction::NoAction => {
                // No action needed, conditions acceptable
            }

            _ => {
                eprintln!("Unhandled action: {:?}", decision.action);
            }
        }
    }
}
```

**Alert Actor Example:**

```rust
struct AlertActor {
    twilio_client: TwilioClient,
    push_service: PushNotificationService,
}

impl AlertActor {
    async fn run(mut self, mut rx: mpsc::Receiver<AlertMessage>) {
        while let Some(alert) = rx.recv().await {
            match alert.level {
                AlertLevel::Critical => {
                    // Send SMS for critical alerts
                    self.twilio_client.send_sms(
                        &self.config.phone_number,
                        &format!("CRITICAL: {}", alert.message),
                    ).await.ok();

                    // Send push notification
                    self.push_service.send(
                        "Air Quality Alert",
                        &alert.message,
                        Some("critical"),
                    ).await.ok();
                }

                AlertLevel::Warning => {
                    // Push notification only for warnings
                    self.push_service.send(
                        "Air Quality Warning",
                        &alert.message,
                        Some("warning"),
                    ).await.ok();
                }

                AlertLevel::Info => {
                    // Log only for info
                    println!("INFO: {}", alert.message);
                }
            }
        }
    }
}
```

### 6.4 Data Flow Summary

```
AirGradient Sensor (60s polling)
    → Pi Poller Service
        → Redis Streams (durable buffer)
            → M4 Mac Processor
                → SensorActor (validate)
                    → TransformActor (units, AQI)
                        → FeatureActor (rolling avg, rate)
                            → StorageActor (QuestDB)
                                ├─→ AnalysisActor (anomaly detection)
                                ├─→ ForecasterActor (24h prediction)
                                │       ↓
                                │   OptimizerActor (PBRS reward)
                                │       ↓
                                └─→ ActionExecutor
                                        ├─→ AlertActor (SMS, push)
                                        ├─→ HomeKitBridge (accessories)
                                        ├─→ MQTTPublisher (Home Assistant)
                                        └─→ HVACControlActor (if integrated)
```

**Key Characteristics:**

- **Asynchronous**: All stages run concurrently (actors)
- **Backpressure**: Bounded channels prevent memory overflow
- **Fault-Tolerant**: Each actor crashes independently, supervised restart
- **Scalable**: Add more actors (horizontal scaling) or distribute (Pi + M4 Mac)
- **Observable**: Each stage emits metrics (OpenTelemetry)

---

## 7. Plugin/Extension Architecture

### 7.1 Extensibility Requirements

**Design Goal:** Enable future domain additions without core platform changes

**Target Domains:**

| Domain | Use Case | Extension Points |
|--------|----------|------------------|
| **Weather Monitoring** | Integrate weather data (temp, wind, precipitation) | DataSource plugin, TransformPlugin |
| **Energy Management** | Track energy consumption, solar generation | DataSource plugin, AnalysisPlugin |
| **Health Monitoring** | Wearable data (heart rate, sleep quality) | DataSource plugin, CorrelationPlugin |
| **Occupancy Tracking** | Room usage patterns (PIR, camera) | DataSource plugin, ForecastingPlugin |

### 7.2 Plugin Types

**Four Plugin Categories:**

```
┌─────────────────────────────────────────────────────────────────────┐
│                     PLUGIN ARCHITECTURE                              │
└─────────────────────────────────────────────────────────────────────┘
                              │
            ┌─────────────────┼─────────────────┐
            │                 │                 │
   ┌────────▼────────┐ ┌─────▼──────┐ ┌───────▼────────┐
   │ DataSource      │ │ Transform  │ │   Analysis     │
   │   Plugin        │ │   Plugin   │ │    Plugin      │
   ├─────────────────┤ ├────────────┤ ├────────────────┤
   │ - MQTT Source   │ │ - Unit     │ │ - Anomaly      │
   │ - HTTP Poller   │ │   Converter│ │   Detection    │
   │ - gRPC Stream   │ │ - Custom   │ │ - Correlation  │
   │ - Kafka         │ │   Transform│ │ - ML Models    │
   └─────────────────┘ └────────────┘ └────────────────┘
                              │
                     ┌────────▼────────┐
                     │   Output        │
                     │   Plugin        │
                     ├─────────────────┤
                     │ - Custom Alerts │
                     │ - Integrations  │
                     │ - Dashboards    │
                     └─────────────────┘
```

### 7.3 Plugin Trait Definitions

**DataSource Plugin:**

```rust
// Core trait (in platform-core)
#[async_trait]
pub trait DataSourcePlugin: Send + Sync {
    /// Plugin metadata
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn api_version(&self) -> u32 { PLUGIN_API_VERSION }

    /// Capability discovery
    fn supported_metrics(&self) -> Vec<MetricType>;

    /// Configuration schema (JSON Schema)
    fn config_schema(&self) -> JsonSchema;

    /// Initialize plugin with config
    async fn configure(&mut self, config: Value) -> Result<(), PluginError>;

    /// Ingest data (called periodically or on-demand)
    async fn ingest(&self) -> Result<Vec<TimeSeriesRecord<f64>>, PluginError>;

    /// Health check
    async fn health_check(&self) -> Result<PluginHealth, PluginError>;
}
```

**Example Implementation (Weather Plugin):**

```rust
pub struct OpenWeatherMapPlugin {
    api_key: String,
    location: (f64, f64),  // lat, lon
    client: reqwest::Client,
}

#[async_trait]
impl DataSourcePlugin for OpenWeatherMapPlugin {
    fn name(&self) -> &str { "openweathermap" }
    fn version(&self) -> &str { "1.0.0" }

    fn supported_metrics(&self) -> Vec<MetricType> {
        vec![
            MetricType::Temperature,
            MetricType::Humidity,
            MetricType::WindSpeed,
            MetricType::Precipitation,
        ]
    }

    fn config_schema(&self) -> JsonSchema {
        json!({
            "type": "object",
            "properties": {
                "api_key": { "type": "string" },
                "latitude": { "type": "number" },
                "longitude": { "type": "number" },
            },
            "required": ["api_key", "latitude", "longitude"]
        })
    }

    async fn configure(&mut self, config: Value) -> Result<(), PluginError> {
        self.api_key = config["api_key"].as_str()
            .ok_or(PluginError::ConfigurationError("Missing api_key".into()))?
            .to_string();

        self.location = (
            config["latitude"].as_f64().unwrap(),
            config["longitude"].as_f64().unwrap(),
        );

        Ok(())
    }

    async fn ingest(&self) -> Result<Vec<TimeSeriesRecord<f64>>, PluginError> {
        let url = format!(
            "https://api.openweathermap.org/data/2.5/weather?lat={}&lon={}&appid={}",
            self.location.0, self.location.1, self.api_key
        );

        let response = self.client.get(&url).send().await
            .map_err(|e| PluginError::RuntimeError(e.to_string()))?
            .json::<OpenWeatherResponse>().await
            .map_err(|e| PluginError::RuntimeError(e.to_string()))?;

        let timestamp = Utc::now();
        Ok(vec![
            TimeSeriesRecord {
                timestamp,
                value: response.main.temp - 273.15,  // Kelvin to Celsius
                metadata: hashmap! {
                    "metric_type" => "temperature",
                    "unit" => "celsius",
                    "source" => "openweathermap",
                },
            },
            TimeSeriesRecord {
                timestamp,
                value: response.main.humidity,
                metadata: hashmap! {
                    "metric_type" => "humidity",
                    "unit" => "percent",
                    "source" => "openweathermap",
                },
            },
        ])
    }

    async fn health_check(&self) -> Result<PluginHealth, PluginError> {
        // Simple health check: try to fetch current weather
        match self.ingest().await {
            Ok(_) => Ok(PluginHealth::Healthy),
            Err(e) => Ok(PluginHealth::Degraded(e.to_string())),
        }
    }
}
```

**Transform Plugin:**

```rust
#[async_trait]
pub trait TransformPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// Transform input to output (1:1 or 1:N)
    async fn transform(&self, input: &TimeSeriesRecord<f64>)
        -> Result<Vec<TimeSeriesRecord<f64>>, PluginError>;
}

// Example: Unit conversion plugin
pub struct UnitConverterPlugin {
    conversions: HashMap<String, ConversionFn>,
}

#[async_trait]
impl TransformPlugin for UnitConverterPlugin {
    fn name(&self) -> &str { "unit-converter" }
    fn version(&self) -> &str { "1.0.0" }

    async fn transform(&self, input: &TimeSeriesRecord<f64>)
        -> Result<Vec<TimeSeriesRecord<f64>>, PluginError> {
        let unit = input.metadata.get("unit").ok_or(
            PluginError::RuntimeError("Missing unit metadata".into())
        )?;

        if let Some(convert_fn) = self.conversions.get(unit.as_str()) {
            let mut output = input.clone();
            output.value = convert_fn(input.value);
            output.metadata.insert("unit".into(), "standard".into());
            Ok(vec![output])
        } else {
            // No conversion needed, pass through
            Ok(vec![input.clone()])
        }
    }
}
```

**Analysis Plugin:**

```rust
#[async_trait]
pub trait AnalysisPlugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    /// Analyze time-series data and return insights
    async fn analyze(&self, data: &[TimeSeriesRecord<f64>])
        -> Result<AnalysisResult, PluginError>;
}

// Example: Anomaly detection plugin (using augurs DBSCAN)
pub struct AnomalyDetectionPlugin {
    dbscan: augurs::anomaly_detection::DBSCAN,
}

#[async_trait]
impl AnalysisPlugin for AnomalyDetectionPlugin {
    fn name(&self) -> &str { "anomaly-detector" }
    fn version(&self) -> &str { "1.0.0" }

    async fn analyze(&self, data: &[TimeSeriesRecord<f64>])
        -> Result<AnalysisResult, PluginError> {
        let values: Vec<f64> = data.iter().map(|r| r.value).collect();
        let anomalies = self.dbscan.detect(&values)
            .map_err(|e| PluginError::RuntimeError(e.to_string()))?;

        let anomaly_indices: Vec<usize> = anomalies.iter()
            .enumerate()
            .filter(|(_, &is_anomaly)| is_anomaly)
            .map(|(i, _)| i)
            .collect();

        Ok(AnalysisResult {
            analysis_type: "anomaly_detection".into(),
            summary: format!("Detected {} anomalies", anomaly_indices.len()),
            details: json!({
                "anomaly_indices": anomaly_indices,
                "total_points": data.len(),
            }),
        })
    }
}
```

### 7.4 Plugin Loading Mechanism

**WebAssembly Plugins (Recommended):**

```rust
use wasmtime::{Engine, Module, Store, Instance};

pub struct PluginManager {
    engine: Engine,
    plugins: HashMap<String, Box<dyn DataSourcePlugin>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            engine: Engine::default(),
            plugins: HashMap::new(),
        }
    }

    pub fn load_plugin(&mut self, path: &Path) -> Result<(), PluginError> {
        // Load WASM module
        let module = Module::from_file(&self.engine, path)
            .map_err(|e| PluginError::LoadError(e.to_string()))?;

        // Create instance
        let mut store = Store::new(&self.engine, ());
        let instance = Instance::new(&mut store, &module, &[])
            .map_err(|e| PluginError::LoadError(e.to_string()))?;

        // Get plugin metadata
        let get_name = instance.get_typed_func::<(), u32>(&mut store, "plugin_name")
            .map_err(|e| PluginError::LoadError(e.to_string()))?;
        let name_ptr = get_name.call(&mut store, ())
            .map_err(|e| PluginError::LoadError(e.to_string()))?;

        // Read name from memory
        let memory = instance.get_memory(&mut store, "memory")
            .ok_or(PluginError::LoadError("Missing memory export".into()))?;
        let name = read_string_from_memory(&memory, &store, name_ptr)?;

        // Wrap WASM plugin with trait
        let plugin = WasmDataSourcePlugin::new(instance, store);
        self.plugins.insert(name, Box::new(plugin));

        Ok(())
    }

    pub fn get_plugin(&self, name: &str) -> Option<&dyn DataSourcePlugin> {
        self.plugins.get(name).map(|p| p.as_ref())
    }
}
```

**Configuration-Based Discovery:**

```toml
# config/plugins.toml

[plugins.openweathermap]
path = "plugins/openweathermap.wasm"
type = "data_source"
enabled = true
config = { api_key = "${OPENWEATHER_API_KEY}", latitude = 37.7749, longitude = -122.4194 }

[plugins.anomaly_detector]
path = "plugins/anomaly_detector.wasm"
type = "analysis"
enabled = true
config = { sensitivity = 0.95, min_samples = 10 }

[plugins.unit_converter]
path = "plugins/unit_converter.wasm"
type = "transform"
enabled = true
```

### 7.5 Plugin Development Workflow

**1. Create Plugin Project:**

```bash
cargo new --lib weather-plugin
cd weather-plugin

# Add to Cargo.toml
[lib]
crate-type = ["cdylib"]

[dependencies]
platform-plugin-sdk = "1.0"  # Provides DataSourcePlugin trait
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

**2. Implement Plugin Trait:**

```rust
// src/lib.rs
use platform_plugin_sdk::*;

#[no_mangle]
pub extern "C" fn plugin_name() -> *const u8 {
    "weather-plugin\0".as_ptr()
}

#[no_mangle]
pub extern "C" fn plugin_version() -> *const u8 {
    "1.0.0\0".as_ptr()
}

// Implement DataSourcePlugin (as shown earlier)
```

**3. Compile to WASM:**

```bash
# Install wasm32 target
rustup target add wasm32-wasi

# Build plugin
cargo build --target wasm32-wasi --release

# Output: target/wasm32-wasi/release/weather_plugin.wasm
```

**4. Install Plugin:**

```bash
# Copy to plugins directory
cp target/wasm32-wasi/release/weather_plugin.wasm ~/.air-quality/plugins/

# Update config
cat >> ~/.air-quality/config/plugins.toml <<EOF
[plugins.weather]
path = "~/.air-quality/plugins/weather_plugin.wasm"
type = "data_source"
enabled = true
config = { api_key = "YOUR_API_KEY", latitude = 37.7749, longitude = -122.4194 }
EOF

# Restart platform
air-quality restart
```

**5. Test Plugin:**

```bash
# List installed plugins
air-quality plugins list

# Test plugin (dry-run)
air-quality plugins test weather

# Enable plugin
air-quality plugins enable weather
```

### 7.6 Plugin Security & Sandboxing

**WebAssembly Sandboxing Benefits:**

| Security Feature | WASM | Native (.so) |
|------------------|------|--------------|
| **Memory Isolation** | Yes (linear memory) | No (shared address space) |
| **System Call Access** | Limited (WASI) | Full access |
| **Resource Limits** | Configurable (CPU, memory) | OS-level only |
| **Determinism** | Yes | No |

**Resource Limits Example:**

```rust
use wasmtime::{Config, Engine, ResourceLimiter};

struct PluginLimits {
    max_memory_bytes: usize,
    max_table_elements: u32,
}

impl ResourceLimiter for PluginLimits {
    fn memory_growing(&mut self, current: usize, desired: usize, _maximum: Option<usize>)
        -> Result<bool, anyhow::Error> {
        Ok(desired <= self.max_memory_bytes)
    }

    fn table_growing(&mut self, current: u32, desired: u32, _maximum: Option<u32>)
        -> Result<bool, anyhow::Error> {
        Ok(desired <= self.max_table_elements)
    }
}

// Configure engine with limits
let mut config = Config::new();
config.max_wasm_stack(1024 * 1024);  // 1 MB stack

let engine = Engine::new(&config)?;
let mut store = Store::new(&engine, PluginLimits {
    max_memory_bytes: 10 * 1024 * 1024,  // 10 MB max memory
    max_table_elements: 1000,
});
store.limiter(|s| s);
```

---

## 8. Deployment Architecture

### 8.1 Target Environment

**Hardware:**

| Device | Role | Specs | Location |
|--------|------|-------|----------|
| **Raspberry Pi 4** | Sensor ingestion, local storage | 4GB RAM, 32GB SD | Near sensors (edge) |
| **M4 Mac Mini** | ML inference, dashboard hosting | 16GB RAM, 256GB SSD | Central location (home server) |

**Network:**

- Local WiFi (sensors → Pi)
- Ethernet (Pi → M4 Mac)
- Internet (weather API, notifications)

### 8.2 Deployment Model: Hybrid Edge + Central

**Visual Architecture:**

```
┌─────────────────────────────────────────────────────────────────────┐
│                    RASPBERRY PI (EDGE DEVICE)                        │
│  - AirGradient sensor polling (HTTP)                                 │
│  - Local data buffering (Redis Streams)                              │
│  - Edge filtering (invalid reading rejection)                        │
│  - Minimal processing (unit conversion, timestamp validation)        │
└──────────────────────┬──────────────────────────────────────────────┘
                       │ (Ethernet / WiFi)
                       │ MQTT: sensors/+/readings
                       │ Redis Stream: air-quality-raw
┌──────────────────────▼──────────────────────────────────────────────┐
│                       M4 MAC MINI (CENTRAL)                          │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │ Processing Layer                                          │       │
│  │  - Actor system (SensorActor, TransformActor, etc.)      │       │
│  │  - Feature engineering (rolling avg, rate of change)     │       │
│  │  - Data quality scoring                                  │       │
│  └──────────────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │ Storage Layer                                             │       │
│  │  - QuestDB (time-series database)                        │       │
│  │  - Redis (event bus, caching)                            │       │
│  │  - PostgreSQL (metadata, config)                         │       │
│  └──────────────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │ ML/Analytics Layer                                        │       │
│  │  - ForecasterAgent (augurs: ETS, MSTL, Prophet)          │       │
│  │  - AnalystAgent (anomaly detection, correlation)         │       │
│  │  - OptimizerAgent (ventilation scheduling)               │       │
│  │  - CoordinatorAgent (OODA loop)                          │       │
│  └──────────────────────────────────────────────────────────┘       │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │ Application Layer                                         │       │
│  │  - REST API (Axum)                                       │       │
│  │  - MCP Server (rmcp, stdio transport)                    │       │
│  │  - Web Dashboard (SvelteKit)                             │       │
│  └──────────────────────────────────────────────────────────┘       │
└──────────────────────┬──────────────────────────────────────────────┘
                       │
            ┌──────────┼──────────┐
            │          │          │
    ┌───────▼────┐ ┌──▼───────┐ ┌▼─────────┐
    │  HomeKit   │ │   MQTT   │ │  Alerts  │
    │  Accessory │ │ (Home    │ │ (SMS/    │
    │            │ │ Assistant│ │  Push)   │
    └────────────┘ └──────────┘ └──────────┘
```

### 8.3 Communication Patterns

**Pi → M4 Mac (Sensor Data):**

**Option 1: Redis Streams (Recommended)**

```rust
// Pi: Publish to Redis Stream
let mut conn = redis_client.get_async_connection().await?;
redis::cmd("XADD")
    .arg("air-quality-raw")
    .arg("MAXLEN")
    .arg("~")
    .arg(10000)
    .arg("*")
    .arg(&[("sensor_id", sensor_id), ("payload", payload)])
    .query_async::<_, String>(&mut conn)
    .await?;

// M4 Mac: Consume from Redis Stream
let result: StreamReadReply = redis::cmd("XREADGROUP")
    .arg("GROUP")
    .arg("processors")
    .arg("consumer-1")
    .arg("BLOCK")
    .arg(1000)
    .arg("STREAMS")
    .arg("air-quality-raw")
    .arg(">")
    .query_async(&mut conn)
    .await?;
```

**Benefits:**
- Durable (survives crashes)
- At-least-once delivery (consumer groups)
- Backpressure (buffer up to max length)

**Option 2: MQTT (Alternative)**

```rust
// Pi: Publish to MQTT
let mqtt_client = rumqttc::AsyncClient::new(...);
mqtt_client.publish(
    "sensors/sensor-123/readings",
    QoS::AtLeastOnce,
    false,
    payload,
).await?;

// M4 Mac: Subscribe to MQTT
let mut eventloop = mqtt_client.eventloop();
while let Ok(notification) = eventloop.poll().await {
    if let Event::Incoming(Packet::Publish(p)) = notification {
        let reading: AirQualityReading = serde_json::from_slice(&p.payload)?;
        handle_reading(reading).await?;
    }
}
```

**Benefits:**
- IoT-native protocol
- Pub/sub pattern (multiple subscribers)
- QoS levels (at-least-once, exactly-once)

**M4 Mac → Pi (Control Commands):**

**Option 1: REST API (Simple)**

```rust
// M4 Mac: Send command to Pi
let client = reqwest::Client::new();
client.post("http://pi.local:8080/commands/restart")
    .json(&CommandPayload { sensor_id: "sensor-123" })
    .send()
    .await?;

// Pi: REST API endpoint
async fn handle_restart_command(
    Json(payload): Json<CommandPayload>,
) -> Result<StatusCode, ApiError> {
    println!("Restarting sensor {}", payload.sensor_id);
    restart_sensor(&payload.sensor_id).await?;
    Ok(StatusCode::OK)
}
```

**Option 2: MQTT (Bi-directional)**

```rust
// M4 Mac: Publish command
mqtt_client.publish("commands/sensor-123/restart", QoS::AtLeastOnce, false, "").await?;

// Pi: Subscribe to commands
mqtt_client.subscribe("commands/+/restart", QoS::AtLeastOnce).await?;
```

**M4 Mac → Internet (External APIs):**

```rust
// Weather API
let weather = reqwest::get("https://api.openweathermap.org/...")
    .await?
    .json::<WeatherResponse>()
    .await?;

// Alert service (Twilio)
twilio_client.send_sms(phone_number, message).await?;

// HomeKit (local network)
homekit_bridge.update_characteristic(accessory_id, characteristic_id, value).await?;
```

### 8.4 Deployment: Docker Compose

**docker-compose.yml:**

```yaml
version: '3.8'

services:
  # Raspberry Pi Services
  pi-poller:
    image: air-quality/pi-poller:latest
    container_name: pi-poller
    restart: unless-stopped
    network_mode: host  # Access local network sensors
    environment:
      - REDIS_URL=redis://m4-mac.local:6379
      - SENSORS_CONFIG=/config/sensors.yaml
    volumes:
      - ./config:/config:ro
      - ./logs:/logs
    depends_on:
      - redis

  # M4 Mac Services
  redis:
    image: redis:7-alpine
    container_name: redis
    restart: unless-stopped
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
    command: redis-server --appendonly yes --maxmemory 2gb --maxmemory-policy allkeys-lru

  questdb:
    image: questdb/questdb:latest
    container_name: questdb
    restart: unless-stopped
    ports:
      - "9000:9000"    # Web console
      - "8812:8812"    # PostgreSQL wire protocol
      - "9009:9009"    # InfluxDB line protocol
    volumes:
      - questdb-data:/var/lib/questdb
    environment:
      - QDB_CAIRO_COMMIT_LAG=1000  # 1 second commit lag
      - QDB_PG_ENABLED=true

  processor:
    image: air-quality/processor:latest
    container_name: processor
    restart: unless-stopped
    ports:
      - "8080:8080"    # REST API
    environment:
      - REDIS_URL=redis://redis:6379
      - QUESTDB_URL=postgresql://admin:quest@questdb:8812/qdb
      - RUST_LOG=info
    volumes:
      - ./config:/config:ro
      - models:/models  # Trained ML models
    depends_on:
      - redis
      - questdb

  mcp-server:
    image: air-quality/mcp-server:latest
    container_name: mcp-server
    restart: unless-stopped
    ports:
      - "3000:3000"    # MCP stdio/SSE
    environment:
      - PROCESSOR_URL=http://processor:8080
    volumes:
      - ./config:/config:ro
    depends_on:
      - processor

  dashboard:
    image: air-quality/dashboard:latest
    container_name: dashboard
    restart: unless-stopped
    ports:
      - "5173:5173"    # SvelteKit dev server
    environment:
      - API_URL=http://processor:8080
    depends_on:
      - processor

  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    restart: unless-stopped
    ports:
      - "3001:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning:ro
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_INSTALL_PLUGINS=grafana-questdb-datasource
    depends_on:
      - questdb

  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    restart: unless-stopped
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
    depends_on:
      - processor

volumes:
  redis-data:
  questdb-data:
  models:
  grafana-data:
  prometheus-data:
```

**Deployment Steps:**

```bash
# 1. Build images
cd air-quality-platform
docker compose build

# 2. Deploy to M4 Mac
docker compose up -d

# 3. Deploy Pi poller to Raspberry Pi
ssh pi@pi.local
docker compose -f docker-compose.pi.yml up -d

# 4. Verify services
docker compose ps
docker compose logs -f processor

# 5. Access dashboards
open http://m4-mac.local:3001  # Grafana
open http://m4-mac.local:9000  # QuestDB console
open http://m4-mac.local:5173  # Air Quality Dashboard
```

### 8.5 Deployment: Native Binaries (Alternative)

**For lower overhead (no Docker):**

```bash
# M4 Mac: Install dependencies
brew install redis questdb postgresql

# Start services
brew services start redis
brew services start questdb
brew services start postgresql

# Build platform
cargo build --release

# Run processor
./target/release/air-quality-processor --config config/processor.yaml &

# Run MCP server
./target/release/mcp-air-quality-server --config config/mcp.yaml &

# Run dashboard
cd dashboard
npm run build
npm run preview &

# Raspberry Pi: Build for ARM
cargo build --release --target aarch64-unknown-linux-gnu
scp target/aarch64-unknown-linux-gnu/release/pi-poller pi@pi.local:/usr/local/bin/

# Pi: Run poller
ssh pi@pi.local
/usr/local/bin/pi-poller --config /etc/air-quality/pi-poller.yaml &
```

### 8.6 Monitoring & Observability

**OpenTelemetry Instrumentation:**

```rust
use opentelemetry::{global, metrics::*, trace::*};

// Initialize OpenTelemetry (in main)
let meter = global::meter("air-quality-platform");
let tracer = global::tracer("air-quality-platform");

// Metrics
let ingestion_counter = meter.u64_counter("data_points_ingested").init();
let query_histogram = meter.f64_histogram("query_duration_seconds").init();

// Record ingestion
ingestion_counter.add(
    batch.len() as u64,
    &[KeyValue::new("sensor_id", sensor_id)]
);

// Record query latency
let start = Instant::now();
let result = execute_query(query).await;
query_histogram.record(
    start.elapsed().as_secs_f64(),
    &[KeyValue::new("query_type", "range")]
);

// Tracing
let mut span = tracer.span_builder("ingest_data").start(&tracer);
span.set_attribute(KeyValue::new("sensor_id", sensor_id));
span.set_attribute(KeyValue::new("batch_size", batch.len() as i64));

let result = ingest_batch(batch).await;

span.end();
```

**Grafana Dashboard Metrics:**

- Ingestion rate (points/second)
- Query latency (p50, p95, p99)
- Storage utilization (QuestDB disk usage)
- Actor queue depths (backpressure monitoring)
- Forecast accuracy (MAE, RMSE)
- Alert frequency (alerts/hour)
- Drift detection events (retraining triggers)

**Health Checks:**

```rust
// /health endpoint
async fn health_check(
    Extension(storage): Extension<Arc<dyn TimeSeriesStorage>>,
    Extension(redis): Extension<redis::Client>,
) -> Result<Json<HealthStatus>, ApiError> {
    let mut status = HealthStatus { services: vec![] };

    // Check QuestDB
    match storage.health_check().await {
        Ok(_) => status.services.push(ServiceHealth {
            name: "questdb",
            status: "healthy",
        }),
        Err(e) => status.services.push(ServiceHealth {
            name: "questdb",
            status: &format!("unhealthy: {}", e),
        }),
    }

    // Check Redis
    let mut conn = redis.get_async_connection().await?;
    match redis::cmd("PING").query_async::<_, String>(&mut conn).await {
        Ok(_) => status.services.push(ServiceHealth {
            name: "redis",
            status: "healthy",
        }),
        Err(e) => status.services.push(ServiceHealth {
            name: "redis",
            status: &format!("unhealthy: {}", e),
        }),
    }

    Ok(Json(status))
}
```

---

## 9. Key Architectural Decisions (ADRs)

### ADR-001: Hexagonal Architecture with Actor Model

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Air quality intelligence system requires:
- Domain-agnostic core (reuse for future domains)
- High-frequency sensor data handling (1-60s intervals)
- Extensibility (plugins for new integrations)
- Testability (isolated unit tests)

**Decision:**

Adopt Hexagonal Architecture (Ports & Adapters) with Tokio actor-based concurrency.

**Rationale:**

| Requirement | Hexagonal Benefit | Actor Model Benefit |
|-------------|-------------------|---------------------|
| Domain agnosticism | Traits abstract time-series operations | N/A |
| High-frequency data | N/A | Per-sensor actors handle concurrent ingestion |
| Extensibility | Secondary adapters swappable | Easy to add new actor types |
| Testability | Mock adapters for tests | Actors isolated, deterministic message handling |
| Reusability | Core traits reused from neural-core | Proven pattern in trading platform |

**Alternatives Considered:**

1. **Microservices**: Too complex for single-building deployment, operational overhead
2. **Shared State + Locks**: Race conditions, deadlock risk, complex debugging
3. **Event Sourcing**: Valuable for commands but excessive for every sensor reading

**Consequences:**

- **Positive**: Clean domain isolation, reuse 70%+ of existing platform, excellent testability
- **Negative**: More layers/abstractions than simple CRUD, learning curve for Hexagonal pattern
- **Mitigation**: Provide architecture diagrams, code examples, documentation

**Compliance:**

All new code MUST follow dependency rule: Application → Core Domain ← Domain Adapter ← Infrastructure

---

### ADR-002: QuestDB as Default Time-Series Database

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need high-performance time-series database for:
- High-frequency sensor ingestion (1-60s intervals)
- Multi-dimensional data (CO2, PM2.5, temp, humidity per sensor)
- Complex analytical queries (aggregations, range queries)
- Schema flexibility (add new sensors without migrations)

**Decision:**

Use QuestDB as default time-series database, with pluggable storage adapters for alternatives (InfluxDB, TimescaleDB).

**Rationale:**

| Database | Ingestion (rows/s) | Query Speed | High Cardinality | Schema Flexibility |
|----------|-------------------|-------------|------------------|--------------------|
| **QuestDB** | **Millions** | **Fastest (SIMD)** | **Excellent** | **Schema-agnostic** |
| InfluxDB | High | Fast | Poor | Schema-enforced |
| TimescaleDB | Medium | Medium | Good | Schema-enforced |

**Benchmark Data:**

- QuestDB: 18.8% faster ingestion than InfluxDB (research finding)
- QuestDB: Schema-agnostic ingestion (no upfront DDL required)
- QuestDB: Three-tier storage (WAL → Columnar → Query optimization)

**Alternatives Considered:**

1. **InfluxDB**: Established ecosystem but high-cardinality performance issues, schema-enforced
2. **TimescaleDB**: PostgreSQL compatibility strong but slower ingestion, schema migrations required
3. **SQLite**: Too slow for high-frequency writes, not designed for time-series

**Consequences:**

- **Positive**: Fastest ingestion, schema flexibility, excellent for analytics
- **Negative**: Smaller community than InfluxDB, less integrations
- **Mitigation**: Abstract via TimeSeriesStorage trait, provide InfluxDB adapter as alternative

**Compliance:**

QuestDB adapter MUST implement TimeSeriesStorage trait. Users can swap to InfluxDB/TimescaleDB via configuration.

---

### ADR-003: augurs for Time-Series Forecasting

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need production-ready time-series forecasting for:
- Air quality predictions (6-48 hours ahead)
- Anomaly detection (unusual pollution events)
- Seasonal pattern recognition (daily/weekly cycles)

**Decision:**

Use augurs (Grafana Labs) as primary forecasting library, with augurs ETS (Exponential Smoothing), MSTL (Multiple Seasonal Decomposition), and Prophet models.

**Rationale:**

| Library | Production Ready | Time-Series Focus | Forecasting Models | Anomaly Detection |
|---------|------------------|-------------------|--------------------|-------------------|
| **augurs** | **Yes (Grafana)** | **Yes (monitoring)** | **ETS, MSTL, Prophet** | **DBSCAN, MAD** |
| ruv-swarm-ml | Experimental | Yes | 27+ models (unverified) | No |
| burn/burn-tch | Yes (inference) | No (general DL) | Custom (requires implementation) | No |
| linfa | Yes | No (general ML) | Limited | No |

**FOSDEM 2025 Endorsement:**

augurs featured as "new library for time-series analysis (forecasting, outlier detection, clustering)" at FOSDEM 2025, validating production focus.

**Alternatives Considered:**

1. **ruv-swarm-ml**: 27+ models appealing but maturity unclear, claims unverified (84.8% SWE-Bench)
2. **burn + burn-tch**: Excellent for custom deep learning but overkill for standard forecasting
3. **linfa**: Good for classical ML but not time-series specialized

**Consequences:**

- **Positive**: Production-proven (Grafana Cloud), purpose-built for monitoring use cases, comprehensive models
- **Negative**: Early library (expect API changes), not an official Grafana project (slower maintenance possible)
- **Mitigation**: Wrap augurs models with Predictor trait (abstraction allows future replacement)

**Compliance:**

All forecasting MUST use augurs via Predictor trait. Custom models allowed if wrapped with same interface.

---

### ADR-004: Andrew Ng Agentic Patterns for Intelligence

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need self-improving system with:
- Self-critique of forecasts (detect unrealistic predictions)
- Multi-source data integration (sensors, weather, occupancy)
- Multi-step optimization (ventilation scheduling)
- Specialized agents (forecasting, analysis, optimization, health)

**Decision:**

Implement Andrew Ng's four agentic AI design patterns: Reflection, Tool Use, Planning, Multi-Agent Collaboration.

**Rationale:**

| Pattern | Air Quality Application | Research Evidence |
|---------|-------------------------|-------------------|
| **Reflection** | Self-critique forecasts (physics constraints) | ~20% accuracy improvement (Madaan et al., 2023) |
| **Tool Use** | Call APIs (weather, sensor, database) | Industry standard (LangChain, AutoGen) |
| **Planning** | Decompose optimization (forecast → analyze → schedule) | Proven in robotics, complex workflows |
| **Multi-Agent** | Specialized agents (Forecaster, Analyst, Optimizer, Health) | Deloitte: 25% of GenAI companies piloting by 2025 |

**OODA Loop Implementation:**

- **Observe**: Sensor readings, weather data, occupancy
- **Orient**: Compare to baselines, seasonal patterns, anomalies
- **Decide**: Forecast air quality, determine intervention
- **Act**: Execute control (ventilation), send alerts, loop back

**Alternatives Considered:**

1. **Rule-Based System**: Simple but inflexible, no learning, manual threshold tuning
2. **Single Monolithic Agent**: No specialization, harder to test/debug
3. **Reinforcement Learning Only**: Requires extensive training data, safety concerns during exploration

**Consequences:**

- **Positive**: Self-improving (reflection loop), modular (specialized agents), proven patterns
- **Negative**: Complexity overhead vs simple rules, requires careful prompt engineering (if using LLM)
- **Mitigation**: Start with reflection + tool use (simple), add planning/multi-agent incrementally

**Compliance:**

All agents MUST implement OODA loop. Reflection MUST validate physics constraints (CO2 >= 400 ppm, PM2.5 >= 0).

---

### ADR-005: ADWIN + EWC++ for Online Learning

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need online learning for:
- Concept drift detection (seasonal changes, new pollution sources)
- Catastrophic forgetting prevention (retain old knowledge while learning new)
- Zero-downtime model updates (hot-swap models)

**Decision:**

Implement ADWIN (Adaptive Windowing) for drift detection + EWC++ (Elastic Weight Consolidation) for catastrophic forgetting prevention + shadow model training for hot-swap.

**Rationale:**

| Requirement | Solution | Research Evidence |
|-------------|----------|-------------------|
| **Drift Detection** | ADWIN | Gold standard, mathematical guarantees, auto-adapts window size |
| **Forgetting Prevention** | EWC++ | 45.7% reduction in catastrophic forgetting (2025 research) |
| **Hot-Swap** | Shadow model | 18.8% faster training, 15.6% better accuracy (IncLSTM) |

**ADWIN Benefits:**

- Automatic time-scale adaptation (no manual window size tuning)
- Rigorous performance guarantees (bounds on false positives/negatives)
- Handles abrupt and gradual drift

**EWC++ Benefits:**

- Identifies parameters crucial to previous tasks
- Prevents overwriting important weights during new learning
- Online version optimized for streaming data

**Alternatives Considered:**

1. **Periodic Full Retraining**: Discards old knowledge, expensive, downtime during retraining
2. **Fixed Window Drift Detection**: Requires manual tuning, misses slow drift
3. **No Drift Detection**: Model performance degrades over time, no adaptation

**Consequences:**

- **Positive**: Adaptive learning, no catastrophic forgetting, zero-downtime updates
- **Negative**: ADWIN not available in Rust (must implement), EWC++ limited to ruvector-sona (or custom)
- **Mitigation**: Implement ADWIN from research papers (well-documented), evaluate ruvector-sona or custom EWC++

**Compliance:**

Forecasting models MUST monitor drift via ADWIN. Retraining MUST use EWC++ regularization. Model swaps MUST validate on holdout set.

---

### ADR-006: rmcp for MCP Server Integration

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need to expose air quality intelligence to Claude Code via Model Context Protocol (MCP) for:
- Natural language queries ("What's the PM2.5 forecast for tomorrow?")
- Action execution ("Increase ventilation to 100 CFM")
- Analysis ("Why did CO2 spike at 3pm yesterday?")

**Decision:**

Use rmcp (official Rust MCP SDK) with stdio transport for local Claude Code integration.

**Rationale:**

| Library | Official SDK | Stdio Support | SSE Support | Tool Macro | Maturity |
|---------|--------------|---------------|-------------|------------|----------|
| **rmcp** | **Yes** | **Yes** | **Yes** | **Yes (#[tool])** | **Stable** |
| rust-mcp-sdk | No | Yes | Yes | No | Experimental |
| Custom | N/A | Custom | Custom | No | High effort |

**#[tool] Macro Benefit:**

```rust
#[tool]
/// Get current air quality readings from all sensors
async fn get_current_readings() -> Result<AirQualityReadings, Error> {
    // Implementation
}

// Automatically registers tool with MCP server (no manual boilerplate)
```

**Alternatives Considered:**

1. **rust-mcp-sdk**: Alternative implementation but not official, less mature
2. **Custom Implementation**: Too much effort, reinventing wheel, maintenance burden
3. **Python MCP Server (PyO3)**: Adds Python dependency, interop overhead

**Consequences:**

- **Positive**: Official SDK, clean API, stdio transport works with Claude Code, SSE for cloud deployment
- **Negative**: Early SDK (API may change), limited examples
- **Mitigation**: Pin to specific version, contribute examples/docs back to project

**Compliance:**

All MCP tools MUST use rmcp. Tools MUST have clear descriptions (doc comments) for LLM understanding.

---

### ADR-007: Redis Streams for Pi → M4 Communication

**Status:** ACCEPTED
**Date:** 2025-12-13
**Deciders:** Architecture Team

**Context:**

Need reliable message transport from Raspberry Pi (edge) to M4 Mac (central) for:
- High-frequency sensor data (1-60s intervals)
- Durability (survive crashes)
- At-least-once delivery (no data loss)
- Backpressure (buffer when M4 slow)

**Decision:**

Use Redis Streams for Pi → M4 Mac sensor data transport, with consumer groups for at-least-once delivery.

**Rationale:**

| Transport | Durability | At-Least-Once | Backpressure | Complexity | Existing Integration |
|-----------|------------|---------------|--------------|------------|---------------------|
| **Redis Streams** | **Yes** | **Yes (groups)** | **Yes (MAXLEN)** | **Low** | **Yes (neural-core)** |
| MQTT | Yes (QoS 1+) | Yes | Broker-dependent | Low | No |
| gRPC | No (in-flight only) | No | Yes (backpressure) | Medium | No |
| HTTP POST | No (retry logic) | Manual | No | Low | No |

**Redis Streams Benefits:**

- Durable event log (append-only, persisted to disk)
- Consumer groups track acknowledgments (at-least-once delivery)
- MAXLEN controls buffer size (automatic backpressure)
- Already integrated in neural-core (RedisEventBus)

**Alternatives Considered:**

1. **MQTT**: Good for IoT but adds broker dependency, QoS 2 overhead for exactly-once
2. **gRPC**: Excellent performance but no durability (in-flight messages lost on crash)
3. **HTTP POST + Retry**: Simple but manual retry logic, no consumer groups

**Consequences:**

- **Positive**: Durable, at-least-once delivery, backpressure, existing integration
- **Negative**: Redis dependency (additional service), not IoT-native like MQTT
- **Mitigation**: Redis already used for caching/eventbus, minimal added overhead

**Compliance:**

Pi poller MUST publish to Redis Stream "air-quality-raw". M4 processor MUST consume via consumer group "processors". MAXLEN set to 10,000.

---

## 10. Implementation Roadmap

### Phase 1: Foundation (Weeks 1-4)

**Goal:** Working ingestion → storage → basic forecasting

**Deliverables:**

| Task | Effort (hrs) | Owner | Status |
|------|--------------|-------|--------|
| Set up QuestDB + Redis (Docker Compose) | 4 | DevOps | Not Started |
| Implement Pi poller service (AirGradient HTTP → Redis Stream) | 16 | Backend | Not Started |
| Implement M4 processor service (Redis Stream → QuestDB) | 20 | Backend | Not Started |
| Wrap augurs ETS/MSTL with Predictor trait | 15 | ML | Not Started |
| Implement baseline forecasting (6h, 24h, 48h horizons) | 20 | ML | Not Started |
| Create REST API endpoints (get_current, get_forecast) | 12 | Backend | Not Started |
| Deploy to Pi + M4 Mac (Docker Compose) | 8 | DevOps | Not Started |
| **TOTAL** | **95 hrs** | | |

**Success Metrics:**

- Pi polls AirGradient every 60s, publishes to Redis Stream
- M4 consumes from Redis, stores to QuestDB
- Forecast accuracy: MAE < 10 ugm3 for PM2.5 (24h ahead)
- API latency: p95 < 100ms

### Phase 2: Agentic Intelligence (Weeks 5-8)

**Goal:** Multi-agent system with reflection, tool use, analysis

**Deliverables:**

| Task | Effort (hrs) | Owner | Status |
|------|--------------|-------|--------|
| Implement ForecasterAgent (OODA loop + reflection) | 30 | ML | Not Started |
| Implement AnalystAgent (trend analysis, anomaly detection) | 25 | ML | Not Started |
| Implement HealthAgent (health recommendations) | 20 | Domain | Not Started |
| Implement CoordinatorAgent (multi-agent orchestration) | 25 | Backend | Not Started |
| Create MCP server (rmcp + 5 core tools) | 40 | Backend | Not Started |
| Integrate with Claude Code (test tool calls) | 10 | QA | Not Started |
| **TOTAL** | **150 hrs** | | |

**Success Metrics:**

- Reflection loop improves forecast accuracy by 10%+
- AnalystAgent detects 90%+ of known anomalies
- MCP tools respond in < 2s
- Claude Code successfully calls all 5 tools

### Phase 3: Online Learning (Weeks 9-12)

**Goal:** Self-improving system with drift detection, hot-swap models

**Deliverables:**

| Task | Effort (hrs) | Owner | Status |
|------|--------------|-------|--------|
| Implement ADWIN drift detection (custom Rust) | 40 | ML | Not Started |
| Integrate EWC++ (ruvector-sona or custom) | 50 | ML | Not Started |
| Implement shadow model training (background task) | 30 | ML | Not Started |
| Implement model validation gate (A/B testing) | 20 | ML | Not Started |
| Create model performance dashboard (Grafana) | 15 | DevOps | Not Started |
| Log drift events, retraining triggers | 10 | Backend | Not Started |
| **TOTAL** | **165 hrs** | | |

**Success Metrics:**

- ADWIN detects drift within 1 hour of occurrence
- EWC++ prevents >80% of catastrophic forgetting
- Model hot-swap validation rejects <10% of swaps (high precision)
- Retraining frequency: 1-2x per week

### Phase 4: Optimization (Weeks 13-16)

**Goal:** RL-based ventilation optimization, reward shaping

**Deliverables:**

| Task | Effort (hrs) | Owner | Status |
|------|--------------|-------|--------|
| Design reward function (air quality + energy + alerts) | 20 | ML | Not Started |
| Implement PBRS (potential-based reward shaping) | 30 | ML | Not Started |
| Train RL agent (PyO3 + stable-baselines3) | 40 | ML | Not Started |
| Simulate in environment (validate safety constraints) | 25 | QA | Not Started |
| Deploy RL agent (A/B test vs rule-based) | 15 | DevOps | Not Started |
| Monitor safety, efficiency, user satisfaction | 10 | PM | Not Started |
| **TOTAL** | **140 hrs** | | |

**Success Metrics:**

- Alert frequency reduced by 30%+ vs rule-based
- Energy consumption reduced by 15%+ (same air quality targets)
- Zero safety violations (CO2 never exceeds critical threshold)
- User satisfaction score: 4.5/5+

**TOTAL PROJECT EFFORT:** 550 hours (~14 weeks @ 40 hrs/week)

---

## 11. Conclusion

This architecture recommendation synthesizes research findings from the existing neural-data-platform codebase, reference architectures, and Rust ML ecosystem to provide a pragmatic, production-ready path forward.

**Key Takeaways:**

1. **Reuse 72% of Existing Platform**: Proven storage, eventbus, prediction traits save 6 months development time
2. **Hexagonal Architecture**: Clean domain isolation enables future domain additions (weather, energy, health)
3. **Actor Model**: Handles high-frequency sensor data without race conditions, proven in neural-trading
4. **Production-Ready Stack**: augurs (forecasting), QuestDB (storage), rmcp (MCP), ADWIN + EWC++ (online learning)
5. **Agentic Intelligence**: Andrew Ng patterns (Reflection, Tool Use, Planning, Multi-Agent) for self-improving system
6. **Hybrid Deployment**: Pi (edge ingestion) + M4 Mac (ML inference, dashboards) balances cost and performance

**Next Steps:**

1. **Stakeholder Review**: Present architecture to team, gather feedback
2. **Prototype Phase 1**: Validate ingestion → storage → forecasting pipeline (4 weeks)
3. **Iterate Based on Learnings**: Refine architecture based on prototype findings
4. **Full Implementation**: Execute 16-week roadmap (Phases 1-4)

**Risks & Mitigations:**

| Risk | Impact | Mitigation |
|------|--------|------------|
| ruv-FANN ecosystem immaturity | High | Start with augurs, evaluate ruv-swarm-ml in parallel |
| Catastrophic forgetting during online learning | High | Implement EWC++, validate before model swap |
| RL reward hacking | Medium | Physics constraints, trip wires, human oversight |
| Concept drift missed | Medium | ADWIN monitoring, manual audits |

**Decision Authority:**

This document serves as **DRAFT for review**. Final approval required from:

- Technical Lead (architecture compliance)
- Product Manager (roadmap alignment)
- ML Lead (forecasting/learning approach)
- DevOps Lead (deployment feasibility)

**Document Version:** 1.0 (DRAFT)
**Last Updated:** 2025-12-13
**Next Review:** 2025-12-20
