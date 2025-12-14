# Implementation Roadmap: Air Quality Intelligence Platform

## Executive Summary

This roadmap defines a phased approach from MVP to a full agentic self-learning system. Total estimated effort: **14-18 weeks** across 7 phases, with each phase delivering usable functionality.

| Phase | Focus | Duration | Key Deliverable |
|-------|-------|----------|-----------------|
| 0 | Foundation | 1 week | Dev environment ready |
| 1 | MVP | 2 weeks | End-to-end data flow |
| 2 | Storage & Events | 2 weeks | Real-time streaming |
| 3 | ML Forecasting | 3 weeks | 24-hour predictions |
| 4 | Home Automation | 2 weeks | HomeKit/MQTT integration |
| 5 | MCP Integration | 2 weeks | Claude can query data |
| 6 | Agentic Learning | 3 weeks | Self-improving system |
| 7 | Domain Agnostic | 2 weeks | Multi-domain support |

---

## Phase 0: Foundation (Week 1)

### Prerequisites

**Hardware Setup:**
```bash
# Pi: Install 64-bit Raspberry Pi OS
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential pkg-config libssl-dev

# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-unknown-linux-gnu  # Pi cross-compile

# AirGradient ONE: Configure WiFi, note local IP
# Sensor exposes: http://<ip>/measures/current
```

**M4 Mac Setup:**
```bash
# Homebrew dependencies
brew install questdb redis grafana mosquitto

# Start services
brew services start questdb
brew services start redis
brew services start grafana
```

**Repository Structure:**
```
neural-data-platform/
├── Cargo.toml                    # Add air-quality-core to workspace
├── air-quality-core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── types/
│       │   ├── mod.rs
│       │   ├── reading.rs        # AirQualityReading
│       │   ├── thresholds.rs     # Health thresholds
│       │   └── events.rs         # AirQualityEvent
│       ├── sources/
│       │   └── airgradient.rs    # HTTP polling
│       ├── analysis/
│       │   ├── aqi.rs            # AQI calculation
│       │   ├── ventilation.rs    # ACH calculation
│       │   └── events.rs         # Event detection
│       └── actions/
│           ├── alerts.rs         # Notification dispatch
│           └── homekit.rs        # HomeKit bridge
└── proto/
    └── air_quality.proto
```

**Success Criteria:**
- [ ] Pi running 64-bit OS with Rust installed
- [ ] AirGradient ONE responding at `http://<ip>/measures/current`
- [ ] M4 Mac services (QuestDB, Redis, Grafana) running
- [ ] `air-quality-core` crate compiles

---

## Phase 1: MVP - Basic Ingestion & Alerts (Weeks 2-3)

### Deliverables

**1.1 AirGradient HTTP Polling Service**

```rust
// air-quality-core/src/sources/airgradient.rs
use reqwest::Client;
use tokio::time::{interval, Duration};

pub struct AirGradientSource {
    client: Client,
    endpoint: String,
    poll_interval: Duration,
}

impl AirGradientSource {
    pub async fn poll(&self) -> Result<AirQualityReading> {
        let response: AirGradientResponse = self.client
            .get(&self.endpoint)
            .send()
            .await?
            .json()
            .await?;
        Ok(response.into())
    }

    pub async fn run(&self, tx: mpsc::Sender<AirQualityReading>) {
        let mut ticker = interval(self.poll_interval);
        loop {
            ticker.tick().await;
            if let Ok(reading) = self.poll().await {
                let _ = tx.send(reading).await;
            }
        }
    }
}
```

**1.2 Local SQLite Storage (Pi)**

```rust
// air-quality-core/src/storage/sqlite.rs
use rusqlite::{Connection, params};

pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    pub fn new(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS readings (
                id INTEGER PRIMARY KEY,
                timestamp TEXT NOT NULL,
                co2 REAL,
                pm25 REAL,
                pm10 REAL,
                voc_index REAL,
                nox_index REAL,
                temperature REAL,
                humidity REAL,
                aqi INTEGER
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    pub fn insert(&self, reading: &AirQualityReading) -> Result<()> {
        self.conn.execute(
            "INSERT INTO readings (timestamp, co2, pm25, pm10, voc_index, nox_index, temperature, humidity, aqi)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                reading.timestamp.to_rfc3339(),
                reading.co2,
                reading.pm25,
                reading.pm10,
                reading.voc_index,
                reading.nox_index,
                reading.temperature,
                reading.humidity,
                reading.calculated_aqi(),
            ],
        )?;
        Ok(())
    }
}
```

**1.3 Threshold-Based Alerting**

```rust
// air-quality-core/src/actions/alerts.rs
pub struct ThresholdAlerts {
    thresholds: Thresholds,
    notifier: Box<dyn Notifier>,
    rate_limiter: RateLimiter,
}

impl ThresholdAlerts {
    pub async fn check(&self, reading: &AirQualityReading) -> Vec<Alert> {
        let mut alerts = Vec::new();

        if reading.co2 > self.thresholds.co2_warning {
            alerts.push(Alert::co2_high(reading.co2));
        }
        if reading.pm25 > self.thresholds.pm25_warning {
            alerts.push(Alert::pm25_high(reading.pm25));
        }
        // ... other thresholds

        for alert in &alerts {
            if self.rate_limiter.should_send(alert) {
                self.notifier.send(alert).await;
            }
        }
        alerts
    }
}
```

**1.4 CLI Dashboard**

```rust
// air-quality-core/src/bin/aq-cli.rs
use crossterm::{cursor, terminal, ExecutableCommand};

fn render_dashboard(reading: &AirQualityReading) {
    let mut stdout = std::io::stdout();
    stdout.execute(terminal::Clear(terminal::ClearType::All)).unwrap();
    stdout.execute(cursor::MoveTo(0, 0)).unwrap();

    println!("╔══════════════════════════════════════╗");
    println!("║       Air Quality Dashboard          ║");
    println!("╠══════════════════════════════════════╣");
    println!("║ CO2:      {:>6} ppm  {}             ║", reading.co2, status_icon(reading.co2, 1000.0));
    println!("║ PM2.5:    {:>6.1} µg/m³ {}           ║", reading.pm25, status_icon(reading.pm25, 12.0));
    println!("║ VOC:      {:>6} idx  {}             ║", reading.voc_index, status_icon(reading.voc_index as f64, 150.0));
    println!("║ Temp:     {:>6.1}°C                  ║", reading.temperature);
    println!("║ Humidity: {:>6.1}%                   ║", reading.humidity);
    println!("╠══════════════════════════════════════╣");
    println!("║ AQI:      {:>6} ({})           ║", reading.calculated_aqi(), aqi_label(reading.calculated_aqi()));
    println!("╚══════════════════════════════════════╝");
}
```

**1.5 Push Notifications (ntfy)**

```rust
// air-quality-core/src/actions/ntfy.rs
pub struct NtfyNotifier {
    topic: String,
    server: String,
}

impl Notifier for NtfyNotifier {
    async fn send(&self, alert: &Alert) -> Result<()> {
        let client = reqwest::Client::new();
        client.post(format!("{}/{}", self.server, self.topic))
            .header("Title", &alert.title)
            .header("Priority", alert.priority.to_string())
            .header("Tags", &alert.tags)
            .body(alert.message.clone())
            .send()
            .await?;
        Ok(())
    }
}
```

**Cargo.toml (Phase 1):**
```toml
[package]
name = "air-quality-core"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.35", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
rusqlite = { version = "0.30", features = ["bundled"] }
crossterm = "0.27"
thiserror = "1.0"

[[bin]]
name = "aq-cli"
path = "src/bin/aq-cli.rs"
```

**Success Criteria:**
- [ ] Sensor data polling every 30 seconds
- [ ] Readings stored in SQLite
- [ ] CLI shows real-time readings
- [ ] ntfy notifications on threshold breach
- [ ] 24-hour uptime without crashes

---

## Phase 2: Core Platform - Storage & Events (Weeks 4-5)

### Deliverables

**2.1 Proto Event Definitions**

```protobuf
// proto/air_quality.proto
syntax = "proto3";
package air_quality.v1;

import "google/protobuf/timestamp.proto";

message AirQualityEvent {
    string event_id = 1;
    string location_id = 2;
    google.protobuf.Timestamp timestamp = 3;
    AirQualityReading reading = 4;
    QualityMetadata quality = 5;
}

message AirQualityReading {
    float co2_ppm = 1;
    float pm25_ugm3 = 2;
    float pm10_ugm3 = 3;
    int32 voc_index = 4;
    int32 nox_index = 5;
    float temperature_c = 6;
    float humidity_pct = 7;
}

message QualityMetadata {
    float completeness = 1;
    float freshness = 2;
    bool sensor_warmed_up = 3;
}
```

**2.2 Redis Streams Integration**

```rust
// air-quality-core/src/streaming/redis.rs
use redis::AsyncCommands;

pub struct RedisStreamPublisher {
    client: redis::Client,
    stream_key: String,
}

impl RedisStreamPublisher {
    pub async fn publish(&self, event: &AirQualityEvent) -> Result<String> {
        let mut conn = self.client.get_async_connection().await?;
        let proto_bytes = event.encode_to_vec();
        let id: String = conn.xadd(
            &self.stream_key,
            "*",
            &[("data", proto_bytes)],
        ).await?;
        Ok(id)
    }
}

pub struct RedisStreamConsumer {
    client: redis::Client,
    stream_key: String,
    group: String,
    consumer: String,
}

impl RedisStreamConsumer {
    pub async fn consume(&self) -> Result<Vec<AirQualityEvent>> {
        let mut conn = self.client.get_async_connection().await?;
        let results: Vec<StreamReadReply> = conn.xread_options(
            &[&self.stream_key],
            &[">"],
            &StreamReadOptions::default()
                .group(&self.group, &self.consumer)
                .count(100)
                .block(5000),
        ).await?;
        // Decode and return events
    }
}
```

**2.3 EventBus (Reuse from neural-core)**

```rust
// Reuse neural-core eventbus with air quality adapters
use neural_core::eventbus::{ProtoEventBus, RedisEventBus};
use crate::proto::AirQualityEvent;

pub type AirQualityEventBus = ProtoEventBus<AirQualityEvent>;

pub fn create_eventbus(config: &Config) -> Result<AirQualityEventBus> {
    match config.eventbus_backend {
        Backend::Redis => Ok(RedisEventBus::new(&config.redis_url)?),
        Backend::InMemory => Ok(InMemoryEventBus::new()),
    }
}
```

**2.4 QuestDB Storage (M4 Mac)**

```rust
// air-quality-core/src/storage/questdb.rs
use questdb::ingress::{Sender, Buffer};

pub struct QuestDBStore {
    sender: Sender,
}

impl QuestDBStore {
    pub async fn insert(&mut self, reading: &AirQualityReading) -> Result<()> {
        let mut buffer = Buffer::new();
        buffer
            .table("air_quality")?
            .symbol("location", &reading.location_id)?
            .column_f64("co2", reading.co2)?
            .column_f64("pm25", reading.pm25)?
            .column_f64("pm10", reading.pm10)?
            .column_i64("voc_index", reading.voc_index as i64)?
            .column_i64("nox_index", reading.nox_index as i64)?
            .column_f64("temperature", reading.temperature)?
            .column_f64("humidity", reading.humidity)?
            .column_i64("aqi", reading.calculated_aqi() as i64)?
            .at_now()?;
        self.sender.flush(&buffer).await?;
        Ok(())
    }
}
```

**2.5 Grafana Dashboard**

```json
// grafana/dashboards/air-quality.json
{
  "dashboard": {
    "title": "Air Quality Monitor",
    "panels": [
      {
        "title": "CO2 Levels",
        "type": "timeseries",
        "targets": [{
          "rawSql": "SELECT timestamp, co2 FROM air_quality WHERE $__timeFilter(timestamp)",
          "datasource": "QuestDB"
        }],
        "fieldConfig": {
          "defaults": {
            "thresholds": {
              "steps": [
                {"value": 0, "color": "green"},
                {"value": 800, "color": "yellow"},
                {"value": 1000, "color": "orange"},
                {"value": 1500, "color": "red"}
              ]
            }
          }
        }
      }
    ]
  }
}
```

**2.6 MQTT Publishing**

```rust
// air-quality-core/src/actions/mqtt.rs
use rumqttc::{AsyncClient, MqttOptions, QoS};

pub struct MqttPublisher {
    client: AsyncClient,
    base_topic: String,
}

impl MqttPublisher {
    pub async fn publish(&self, reading: &AirQualityReading) -> Result<()> {
        let payload = serde_json::to_vec(reading)?;
        self.client.publish(
            format!("{}/reading", self.base_topic),
            QoS::AtLeastOnce,
            false,
            payload,
        ).await?;
        Ok(())
    }
}
```

**Success Criteria:**
- [ ] Events flow through Redis Streams
- [ ] QuestDB storing all readings
- [ ] Grafana dashboard showing live data
- [ ] MQTT messages receivable by Home Assistant

---

## Phase 3: ML Forecasting (Weeks 6-8)

### Deliverables

**3.1 augurs Integration**

```rust
// air-quality-core/src/forecasting/mod.rs
use augurs::{ets::AutoETS, mstl::MSTLModel, Forecast};

pub struct AirQualityForecaster {
    ets_models: HashMap<String, AutoETS>,  // per-metric models
    mstl_model: Option<MSTLModel>,
}

impl AirQualityForecaster {
    pub fn forecast(&self, metric: &str, horizon: usize) -> Result<Forecast> {
        let model = self.ets_models.get(metric)
            .ok_or_else(|| Error::ModelNotFound(metric.to_string()))?;

        Ok(model.predict(horizon, 0.95)?)  // 95% confidence
    }

    pub fn fit(&mut self, metric: &str, history: &[f64]) -> Result<()> {
        let model = AutoETS::new(1, "ZZZ")  // Auto-select best ETS variant
            .fit(history)?;
        self.ets_models.insert(metric.to_string(), model);
        Ok(())
    }
}
```

**3.2 Feature Engineering Pipeline**

```rust
// air-quality-core/src/features/mod.rs
pub struct FeatureEngine {
    window_size: usize,
}

impl FeatureEngine {
    pub fn extract(&self, readings: &[AirQualityReading]) -> FeatureVector {
        FeatureVector {
            // Temporal features
            hour_of_day: readings.last().map(|r| r.timestamp.hour()).unwrap_or(0),
            day_of_week: readings.last().map(|r| r.timestamp.weekday().num_days_from_monday()).unwrap_or(0),

            // Rolling statistics
            co2_mean: self.rolling_mean(readings, |r| r.co2),
            co2_std: self.rolling_std(readings, |r| r.co2),
            pm25_mean: self.rolling_mean(readings, |r| r.pm25),
            pm25_max: self.rolling_max(readings, |r| r.pm25),

            // Trends
            co2_trend: self.linear_trend(readings, |r| r.co2),
            temperature_trend: self.linear_trend(readings, |r| r.temperature),

            // Derived
            ventilation_score: self.estimate_ach(readings),
            mold_risk: self.calculate_mold_risk(readings.last().unwrap()),
        }
    }
}
```

**3.3 Model Training Coordinator**

```rust
// air-quality-core/src/training/coordinator.rs
pub struct TrainingCoordinator {
    storage: Arc<dyn TimeSeriesStorage>,
    model_store: Arc<dyn ModelStore>,
    schedule: TrainingSchedule,
}

impl TrainingCoordinator {
    pub async fn run(&self) {
        let mut ticker = interval(self.schedule.check_interval);
        loop {
            ticker.tick().await;
            for metric in &self.schedule.metrics {
                if self.should_retrain(metric).await {
                    self.retrain(metric).await;
                }
            }
        }
    }

    async fn should_retrain(&self, metric: &str) -> bool {
        let last_train = self.model_store.last_trained(metric).await;
        let data_points = self.storage.count_since(metric, last_train).await;
        data_points >= self.schedule.min_new_points
    }

    async fn retrain(&self, metric: &str) {
        let history = self.storage.query_range(metric, ..now()).await?;
        let mut forecaster = AirQualityForecaster::default();
        forecaster.fit(metric, &history)?;
        self.model_store.save(metric, &forecaster).await?;
    }
}
```

**3.4 Prediction-Based Alerts**

```rust
// air-quality-core/src/actions/predictive_alerts.rs
pub struct PredictiveAlerter {
    forecaster: Arc<AirQualityForecaster>,
    thresholds: Thresholds,
}

impl PredictiveAlerter {
    pub async fn check(&self, current: &AirQualityReading) -> Vec<PredictiveAlert> {
        let mut alerts = Vec::new();

        // Forecast next 4 hours
        let forecast = self.forecaster.forecast("co2", 8)?;  // 30-min intervals

        for (i, point) in forecast.point.iter().enumerate() {
            if *point > self.thresholds.co2_warning as f64 {
                alerts.push(PredictiveAlert {
                    metric: "co2".into(),
                    predicted_value: *point,
                    time_until: Duration::from_secs(i as u64 * 1800),
                    confidence: forecast.intervals.as_ref()
                        .map(|i| 1.0 - (i.upper[i] - i.lower[i]) / *point)
                        .unwrap_or(0.8),
                });
                break;  // Alert only for first breach
            }
        }
        alerts
    }
}
```

**3.5 Model Storage & Versioning**

```rust
// air-quality-core/src/storage/models.rs
pub struct ModelStore {
    base_path: PathBuf,
}

impl ModelStore {
    pub async fn save(&self, name: &str, model: &impl Serialize) -> Result<ModelVersion> {
        let version = ModelVersion {
            name: name.into(),
            version: Utc::now().format("%Y%m%d_%H%M%S").to_string(),
            created_at: Utc::now(),
        };

        let path = self.base_path.join(&version.version).join(format!("{}.bin", name));
        fs::create_dir_all(path.parent().unwrap()).await?;

        let bytes = bincode::serialize(model)?;
        fs::write(&path, bytes).await?;

        // Update symlink to latest
        let latest = self.base_path.join("latest").join(format!("{}.bin", name));
        fs::remove_file(&latest).await.ok();
        fs::symlink(&path, &latest).await?;

        Ok(version)
    }
}
```

**Success Criteria:**
- [ ] 24-hour forecasts for CO2 and PM2.5
- [ ] Model retraining on schedule
- [ ] Predictive alerts 1-2 hours before breaches
- [ ] Model versioning and rollback capability

---

## Phase 4: Home Automation Integration (Weeks 9-10)

### Deliverables

**4.1 HomeKit Bridge (Homebridge)**

```json
// homebridge/config.json
{
  "bridge": {
    "name": "Air Quality Bridge",
    "username": "CC:22:3D:E3:CE:30",
    "port": 51826,
    "pin": "031-45-154"
  },
  "accessories": [
    {
      "accessory": "HttpAirQualitySensor",
      "name": "Living Room Air Quality",
      "url": "http://pi.local:8080/api/v1/current",
      "http_method": "GET",
      "field_path": {
        "air_quality": "aqi_level",
        "co2": "co2",
        "pm25": "pm25",
        "voc": "voc_index"
      }
    }
  ],
  "platforms": []
}
```

**4.2 HTTP API for HomeKit**

```rust
// air-quality-core/src/api/homekit.rs
use axum::{Router, Json, routing::get};

pub fn homekit_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/current", get(current_reading))
        .route("/api/v1/aqi", get(current_aqi))
        .with_state(state)
}

async fn current_reading(State(state): State<AppState>) -> Json<HomeKitReading> {
    let reading = state.storage.latest().await.unwrap();
    Json(HomeKitReading {
        aqi_level: aqi_to_homekit_level(reading.calculated_aqi()),
        co2: reading.co2,
        pm25: reading.pm25,
        voc_index: reading.voc_index,
        temperature: reading.temperature,
        humidity: reading.humidity,
    })
}

fn aqi_to_homekit_level(aqi: u32) -> u8 {
    // HomeKit: 0=Unknown, 1=Excellent, 2=Good, 3=Fair, 4=Inferior, 5=Poor
    match aqi {
        0..=50 => 1,
        51..=100 => 2,
        101..=150 => 3,
        151..=200 => 4,
        _ => 5,
    }
}
```

**4.3 Home Assistant MQTT Auto-Discovery**

```rust
// air-quality-core/src/actions/homeassistant.rs
pub struct HomeAssistantIntegration {
    mqtt: MqttPublisher,
    device_id: String,
}

impl HomeAssistantIntegration {
    pub async fn register(&self) -> Result<()> {
        // Auto-discovery config for each sensor
        for sensor in &["co2", "pm25", "temperature", "humidity", "aqi"] {
            let config = json!({
                "name": format!("Air Quality {}", sensor.to_uppercase()),
                "state_topic": format!("homeassistant/sensor/{}/state", self.device_id),
                "value_template": format!("{{{{ value_json.{} }}}}", sensor),
                "unique_id": format!("{}_{}", self.device_id, sensor),
                "device": {
                    "identifiers": [&self.device_id],
                    "name": "Air Quality Sensor",
                    "manufacturer": "AirGradient"
                },
                "unit_of_measurement": unit_for(sensor),
                "device_class": device_class_for(sensor),
            });

            self.mqtt.publish_json(
                &format!("homeassistant/sensor/{}_{}/config", self.device_id, sensor),
                &config,
            ).await?;
        }
        Ok(())
    }
}
```

**4.4 Automation Triggers**

```rust
// air-quality-core/src/actions/automation.rs
pub struct AutomationTriggers {
    mqtt: MqttPublisher,
}

impl AutomationTriggers {
    pub async fn trigger(&self, event: AutomationEvent) -> Result<()> {
        let topic = format!("air-quality/automation/{}", event.trigger_type);
        self.mqtt.publish_json(&topic, &event).await
    }
}

pub enum AutomationEvent {
    VentilationNeeded { co2: f64, recommendation: String },
    AirPurifierOn { pm25: f64 },
    AirPurifierOff,
    WindowOpenSuggested { reason: String },
}
```

**Success Criteria:**
- [ ] Air quality visible in Apple Home
- [ ] Home Assistant auto-discovers sensors
- [ ] Automation triggers working
- [ ] 99.9% uptime for HomeKit accessory

---

## Phase 5: MCP Integration (Weeks 11-12)

### Deliverables

**5.1 rmcp Server Implementation**

```rust
// mcp-air-quality-server/src/main.rs
use rmcp::{Server, ServerHandler, tool};

#[derive(Clone)]
struct AirQualityServer {
    storage: Arc<dyn TimeSeriesStorage>,
    forecaster: Arc<AirQualityForecaster>,
    analyzer: Arc<VentilationAnalyzer>,
}

#[tool(description = "Get current air quality readings from all sensors")]
async fn get_current_readings(&self) -> Result<CurrentReadings> {
    let reading = self.storage.latest().await?;
    Ok(CurrentReadings {
        co2_ppm: reading.co2,
        pm25_ugm3: reading.pm25,
        voc_index: reading.voc_index,
        temperature_c: reading.temperature,
        humidity_pct: reading.humidity,
        aqi: reading.calculated_aqi(),
        aqi_category: aqi_category(reading.calculated_aqi()),
        timestamp: reading.timestamp,
    })
}

#[tool(description = "Forecast air quality for the next N hours")]
async fn forecast_air_quality(&self, hours: u32) -> Result<Forecast> {
    let intervals = (hours * 2) as usize;  // 30-min intervals
    let forecast = self.forecaster.forecast("aqi", intervals)?;
    Ok(Forecast {
        predictions: forecast.point,
        confidence_intervals: forecast.intervals,
        model_version: self.forecaster.version(),
    })
}

#[tool(description = "Analyze ventilation adequacy and provide recommendations")]
async fn analyze_ventilation(&self) -> Result<VentilationAnalysis> {
    let history = self.storage.query_range("co2", last_24h()).await?;
    self.analyzer.analyze(&history)
}

#[tool(description = "Get health recommendations based on current conditions")]
async fn get_health_recommendations(&self) -> Result<Vec<Recommendation>> {
    let reading = self.storage.latest().await?;
    Ok(generate_recommendations(&reading))
}

#[tool(description = "Explain what a specific reading means for health")]
async fn explain_reading(&self, metric: String, value: f64) -> Result<Explanation> {
    Ok(explain_metric(&metric, value))
}

#[tokio::main]
async fn main() {
    let server = AirQualityServer::new().await;
    Server::new(server)
        .serve_stdio()
        .await
        .unwrap();
}
```

**5.2 Claude Desktop Configuration**

```json
// ~/Library/Application Support/Claude/claude_desktop_config.json
{
  "mcpServers": {
    "air-quality": {
      "command": "/path/to/mcp-air-quality-server",
      "args": ["--config", "/path/to/config.toml"]
    }
  }
}
```

**5.3 Example Conversations**

```
User: "What's the air quality like right now?"
Claude: [calls get_current_readings]
"The air quality is currently Good with an AQI of 45. CO2 is at 620 ppm
(excellent for cognitive function), PM2.5 is 8.3 µg/m³ (below the EPA
annual standard of 9 µg/m³), and humidity is comfortable at 48%."

User: "Will it get worse tonight?"
Claude: [calls forecast_air_quality(hours=8)]
"Based on historical patterns, CO2 is expected to rise to around 850 ppm
by midnight as the house is sealed overnight. PM2.5 should remain stable.
I'd recommend running your ventilation system for 15 minutes before bed."

User: "How's my ventilation?"
Claude: [calls analyze_ventilation]
"Your estimated air changes per hour (ACH) is 0.4, which is below the
ASHRAE recommendation of 0.35-0.5 for residential spaces. CO2 levels
take approximately 2.5 hours to decay from 1200 ppm to 600 ppm when
windows are open, suggesting good potential for natural ventilation."
```

**Success Criteria:**
- [ ] All 5+ MCP tools working
- [ ] Claude can query real-time data
- [ ] Natural language forecasts
- [ ] Contextual recommendations

---

## Phase 6: Agentic Self-Learning (Weeks 13-15)

### Deliverables

**6.1 ADWIN Drift Detection**

```rust
// air-quality-core/src/learning/drift.rs
use adwin::ADWIN;

pub struct DriftDetector {
    detectors: HashMap<String, ADWIN>,
    threshold: f64,
}

impl DriftDetector {
    pub fn check(&mut self, metric: &str, value: f64) -> Option<DriftEvent> {
        let detector = self.detectors.entry(metric.into())
            .or_insert_with(|| ADWIN::new(self.threshold));

        if detector.add(value) {
            Some(DriftEvent {
                metric: metric.into(),
                old_mean: detector.old_mean(),
                new_mean: detector.new_mean(),
                timestamp: Utc::now(),
            })
        } else {
            None
        }
    }
}
```

**6.2 Online Model Updates**

```rust
// air-quality-core/src/learning/online.rs
pub struct OnlineLearner {
    model: IncrementalModel,
    buffer: RingBuffer<f64>,
    update_threshold: usize,
}

impl OnlineLearner {
    pub fn observe(&mut self, value: f64) {
        self.buffer.push(value);
        if self.buffer.len() >= self.update_threshold {
            self.update();
        }
    }

    fn update(&mut self) {
        let recent = self.buffer.as_slice();
        self.model.partial_fit(recent);
        self.buffer.clear();
    }
}
```

**6.3 Reflection Loop**

```rust
// air-quality-core/src/agents/reflection.rs
pub struct ReflectionAgent {
    observer: MetricsObserver,
    analyzer: PerformanceAnalyzer,
    adjuster: ThresholdAdjuster,
    history: Vec<ReflectionCycle>,
}

impl ReflectionAgent {
    pub async fn run_cycle(&mut self) -> ReflectionResult {
        // 1. OBSERVE: Collect performance metrics
        let observations = self.observer.collect().await;

        // 2. ANALYZE: Identify issues
        let analysis = self.analyzer.analyze(&observations);

        // 3. DECIDE: Determine adjustments
        let adjustments = match analysis {
            Analysis::TooManyAlerts { rate, .. } => {
                vec![Adjustment::RaiseThreshold { metric: "co2", delta: 50.0 }]
            }
            Analysis::MissedEvents { count, .. } => {
                vec![Adjustment::LowerThreshold { metric: "pm25", delta: 2.0 }]
            }
            Analysis::ModelDrift { metric, .. } => {
                vec![Adjustment::Retrain { metric }]
            }
            Analysis::Optimal => vec![],
        };

        // 4. APPLY: Make changes
        for adj in &adjustments {
            self.adjuster.apply(adj).await;
        }

        // 5. RECORD: Log for future learning
        let result = ReflectionResult {
            cycle: self.history.len(),
            observations,
            analysis,
            adjustments,
            timestamp: Utc::now(),
        };
        self.history.push(result.clone());

        result
    }
}
```

**6.4 Performance-Based Threshold Tuning**

```rust
// air-quality-core/src/agents/tuning.rs
pub struct ThresholdTuner {
    current: Thresholds,
    feedback: FeedbackCollector,
}

impl ThresholdTuner {
    pub fn tune(&mut self) -> ThresholdUpdate {
        let feedback = self.feedback.summarize();

        // Alert fatigue: too many alerts → raise thresholds
        if feedback.alerts_per_day > 10.0 {
            self.current.co2_warning += 50.0;
            self.current.pm25_warning += 2.0;
        }

        // Missed events: user reported issues not alerted → lower
        if feedback.user_reported_issues > 0 {
            self.current.co2_warning -= 25.0;
        }

        // False positives: user dismissed alerts → raise
        let dismissal_rate = feedback.dismissed / feedback.total_alerts;
        if dismissal_rate > 0.5 {
            self.current.co2_warning += 25.0;
        }

        ThresholdUpdate {
            new: self.current.clone(),
            reason: format!("Tuned based on {} feedback events", feedback.total()),
        }
    }
}
```

**6.5 Model Hot-Swapping**

```rust
// air-quality-core/src/learning/hotswap.rs
pub struct ModelManager {
    active: Arc<RwLock<Box<dyn Forecaster>>>,
    candidates: Vec<Box<dyn Forecaster>>,
    evaluator: ModelEvaluator,
}

impl ModelManager {
    pub async fn evaluate_and_swap(&self) {
        let current_score = self.evaluator.score(&*self.active.read().await).await;

        for candidate in &self.candidates {
            let candidate_score = self.evaluator.score(candidate).await;
            if candidate_score > current_score * 1.05 {  // 5% improvement threshold
                let mut active = self.active.write().await;
                *active = candidate.clone();
                tracing::info!(
                    "Hot-swapped model: {} -> {} (score: {:.3} -> {:.3})",
                    current_score, candidate_score
                );
                return;
            }
        }
    }
}
```

**Success Criteria:**
- [ ] Drift detection triggering retraining
- [ ] Thresholds auto-adjusting based on feedback
- [ ] Alert fatigue reduced by 50%
- [ ] Models improving without manual intervention

---

## Phase 7: Domain Agnostic Core (Weeks 16-17)

### Deliverables

**7.1 Generic TimeSeriesEvent Trait**

```rust
// neural-core/src/types/timeseries.rs
pub trait TimeSeriesEvent: Send + Sync + Clone {
    type Value: Clone + Default;

    fn timestamp(&self) -> DateTime<Utc>;
    fn source_id(&self) -> &str;
    fn value(&self) -> Self::Value;
    fn quality(&self) -> QualityMetadata;
}

// Air quality implements this
impl TimeSeriesEvent for AirQualityReading {
    type Value = AirQualityValues;

    fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    fn source_id(&self) -> &str { &self.sensor_id }
    fn value(&self) -> Self::Value {
        AirQualityValues { co2: self.co2, pm25: self.pm25, ... }
    }
    fn quality(&self) -> QualityMetadata { self.quality.clone() }
}
```

**7.2 Domain Adapter Pattern**

```rust
// neural-core/src/adapters/mod.rs
pub trait DomainAdapter {
    type Event: TimeSeriesEvent;
    type Action: DomainAction;
    type Config: DeserializeOwned;

    fn name(&self) -> &str;
    fn parse_event(&self, raw: &[u8]) -> Result<Self::Event>;
    fn validate(&self, event: &Self::Event) -> ValidationResult;
    fn available_actions(&self) -> Vec<ActionDescriptor>;
}

// Air quality adapter
pub struct AirQualityAdapter;

impl DomainAdapter for AirQualityAdapter {
    type Event = AirQualityReading;
    type Action = AirQualityAction;
    type Config = AirQualityConfig;

    fn name(&self) -> &str { "air-quality" }
    // ...
}
```

**7.3 Domain Registry**

```rust
// neural-core/src/registry.rs
pub struct DomainRegistry {
    adapters: HashMap<String, Box<dyn DomainAdapter>>,
}

impl DomainRegistry {
    pub fn register<A: DomainAdapter + 'static>(&mut self, adapter: A) {
        self.adapters.insert(adapter.name().into(), Box::new(adapter));
    }

    pub fn get(&self, name: &str) -> Option<&dyn DomainAdapter> {
        self.adapters.get(name).map(|a| a.as_ref())
    }
}

// Usage
let mut registry = DomainRegistry::new();
registry.register(AirQualityAdapter);
registry.register(EnergyAdapter);  // Future domain
```

**7.4 Example: Adding Energy Domain**

```rust
// energy-core/src/lib.rs (new domain in < 1 week)
pub struct EnergyReading {
    pub timestamp: DateTime<Utc>,
    pub meter_id: String,
    pub power_watts: f64,
    pub voltage: f64,
    pub current: f64,
    pub power_factor: f64,
}

impl TimeSeriesEvent for EnergyReading {
    type Value = EnergyValues;
    // ... implement trait
}

pub struct EnergyAdapter;
impl DomainAdapter for EnergyAdapter {
    type Event = EnergyReading;
    // ... implement adapter
}
```

**Success Criteria:**
- [ ] Generic traits extracted to neural-core
- [ ] Air quality adapter uses generic traits
- [ ] New domain (energy) added in <40 hours
- [ ] Documentation for domain extension

---

## Risk Mitigation

| Phase | Risk | Mitigation |
|-------|------|------------|
| 1 | Sensor connectivity issues | Local buffering, retry logic |
| 2 | Redis memory on Pi | Use streams with MAXLEN |
| 3 | Model accuracy | Start with simple ETS, add complexity |
| 4 | HomeKit stability | Separate process, watchdog restart |
| 5 | MCP protocol changes | Pin rmcp version, monitor updates |
| 6 | Runaway auto-tuning | Bounds on adjustments, manual override |
| 7 | Breaking changes | Semantic versioning, deprecation warnings |

---

## Resource Requirements

| Phase | Hardware | External Services | Effort |
|-------|----------|-------------------|--------|
| 0 | Pi + Mac | None | 10h |
| 1 | Pi (1GB RAM) | ntfy (free) | 30h |
| 2 | Pi + Mac | Redis, QuestDB | 35h |
| 3 | Mac (training) | None | 50h |
| 4 | Pi | Homebridge | 25h |
| 5 | Mac | Claude Desktop | 30h |
| 6 | Mac (learning) | None | 50h |
| 7 | None (refactor) | None | 30h |
| **Total** | | | **260h** |

---

## Success Metrics (Full System)

| Metric | Target |
|--------|--------|
| End-to-end latency | < 5 seconds |
| Forecast accuracy (MAPE) | < 15% |
| Alert precision | > 85% |
| Alert fatigue reduction | > 50% |
| System uptime | > 99.5% |
| New domain integration | < 1 week |

---

## Conclusion

This roadmap delivers incremental value at each phase:
- **Phase 1**: Working MVP in 2 weeks
- **Phase 4**: Full home automation by week 10
- **Phase 6**: Self-learning system by week 15
- **Phase 7**: Domain-agnostic platform by week 17

The phased approach minimizes risk while ensuring each milestone is independently useful.
