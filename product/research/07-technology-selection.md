# Technology Selection Guide: Neural Data Platform Air Quality Intelligence System

**Document Date:** 2025-12-13
**Target Platform:** Raspberry Pi 4 (2-4GB RAM) + M4 Mac
**Purpose:** Comprehensive technology selection for production deployment

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [ML & Forecasting Stack](#ml--forecasting-stack)
3. [Storage Stack](#storage-stack)
4. [Messaging & Events](#messaging--events)
5. [MCP Integration](#mcp-integration)
6. [Dashboard & Visualization](#dashboard--visualization)
7. [Home Automation](#home-automation)
8. [Observability](#observability)
9. [Development Tools](#development-tools)
10. [Version Matrix](#version-matrix)
11. [Cargo.toml Reference](#cargotoml-reference)

---

## Executive Summary

### Top-Level Technology Choices

| Component | Primary Choice | Rationale |
|-----------|---------------|-----------|
| **ML Framework** | augurs (Grafana) | Production-ready time-series forecasting, purpose-built for monitoring |
| **Neural Models** | burn + burn-tch | PyTorch interop, quantization support, custom architectures |
| **Online Learning** | ADWIN (custom impl) + EWC++ | Drift detection + catastrophic forgetting prevention |
| **Classical ML** | linfa | Preprocessing, feature engineering, interpretable models |
| **Time-Series DB (Pi)** | QuestDB | 2.94M rows/sec, 64-bit Pi OS, schema-agnostic |
| **Time-Series DB (Mac)** | TimescaleDB | PostgreSQL ecosystem, complex queries, Hypercore storage |
| **Cache** | Redis (optional) | In-memory cache for ML features, may be too heavy for Pi |
| **Messaging** | Tokio channels + MQTT | Internal async messaging + external IoT protocol |
| **MCP SDK** | rmcp | Official Rust SDK, stdio/SSE transport, clean API |
| **Dashboard** | Grafana (on M4 Mac) | Remote dashboard server, offload Pi resources |
| **Home Automation** | MQTT → Home Assistant | Most flexible, Homebridge for HomeKit |
| **Alerting** | ntfy (self-hosted) | Privacy, unlimited notifications, open-source |
| **Metrics** | Prometheus + OpenTelemetry | Industry standard, vendor-neutral |
| **Tracing** | Tempo + OpenTelemetry | Distributed tracing, Grafana integration |

---

## ML & Forecasting Stack

### Time-Series Forecasting

| Category | Primary Choice | Alternative | Justification |
|----------|---------------|-------------|---------------|
| **Forecasting (Production)** | augurs | ruv-swarm-ml | Purpose-built for monitoring, Grafana-backed, production-focused |
| **Deep Learning** | burn + burn-tch | tch-rs | Native Rust design, PyTorch interop, quantization support |
| **Online Learning** | ADWIN (custom) | ruvector-sona | Gold standard drift detection, custom impl avoids ecosystem lock-in |
| **Forgetting Prevention** | EWC++ (custom) | ruvector-sona | Proven method (45.7% reduction), custom impl for control |
| **Classical ML** | linfa | smartcore | scikit-learn equivalent, preprocessing focus, mature |

### Detailed Stack Decisions

#### 1. **augurs (Grafana) - Primary Forecasting Engine**

**Why augurs:**
- **Production-Ready**: Built by Grafana Labs for monitoring use cases
- **Purpose-Built**: Designed specifically for time-series monitoring scenarios
- **Model Variety**: ETS, MSTL, Prophet, DBSCAN for outlier detection
- **Performance**: Optimized for real-time monitoring workloads
- **Seasonality**: Automatic seasonal pattern detection
- **Changepoint Detection**: Identify abrupt shifts in behavior

**Use Cases:**
- Air quality forecasting (6-48 hours ahead)
- Anomaly detection (unusual sensor readings)
- Seasonal pattern identification (daily/weekly cycles)
- Outlier detection (sensor malfunctions)

**Cargo.toml:**
```toml
augurs = "0.4"
augurs-ets = "0.4"
augurs-mstl = "0.4"
augurs-prophet = "0.4"
augurs-clustering = "0.4"
```

**Trade-offs:**
- **Pros**: Mature, well-tested, monitoring-focused, good performance
- **Cons**: Early days (expect API changes), not official Grafana project (slower updates)

#### 2. **burn + burn-tch - Deep Learning (If Needed)**

**Why burn:**
- **PyTorch Interop**: Load pretrained models, leverage ecosystem
- **Type System**: Rust's type system enables optimizations unavailable in dynamic frameworks
- **Flexibility**: Supports custom architectures for complex time-series
- **Quantization**: 8-bit, 4-bit, 2-bit representations for edge deployment
- **Backends**: CPU, CUDA (GPU), MPS (macOS), WebAssembly

**Use Cases:**
- Multi-variate time-series with complex dependencies
- Long-range temporal dependencies (transformer models)
- Transfer learning from pretrained time-series models
- Custom architectures when classical methods insufficient

**Cargo.toml:**
```toml
burn = "0.14"
burn-tch = "0.14"  # PyTorch backend
burn-ndarray = "0.14"  # CPU backend for inference
```

**Trade-offs:**
- **Pros**: Flexibility, PyTorch ecosystem, performance, quantization
- **Cons**: Overengineered for simple forecasting, steeper learning curve
- **Decision**: Use only when augurs insufficient (complex dependencies, deep learning required)

#### 3. **ADWIN - Concept Drift Detection**

**Why ADWIN:**
- **Gold Standard**: Mathematical guarantees on false positives/negatives
- **Automatic**: No manual threshold tuning required
- **Adaptive**: Handles both abrupt and gradual drift
- **Proven**: Widely used in online learning research

**Implementation:**
- Custom Rust implementation (no native crate available)
- Reference: River (Python) and scikit-multiflow
- Algorithm well-documented in literature

**Use Cases:**
- Detect seasonal changes (winter heating → summer cooling)
- Identify sensor calibration drift
- Trigger model retraining when forecast accuracy degrades
- Detect new pollution sources affecting air quality

**Pseudocode:**
```rust
struct AdwinDriftDetector {
    window: VecDeque<f64>,
    threshold: f64,
}

impl AdwinDriftDetector {
    fn add_element(&mut self, error: f64) -> bool {
        // Returns true if drift detected
    }
}
```

#### 4. **EWC++ - Catastrophic Forgetting Prevention**

**Why EWC++:**
- **Research-Backed**: 45.7% reduction in catastrophic forgetting
- **Online Version**: Optimized for streaming data (OnlineEWC)
- **Proven**: Widely accepted method in continual learning

**Implementation:**
- Custom Rust implementation based on research papers
- Alternative: Use ruvector-sona if ecosystem acceptable
- Integrate with ADWIN for retraining triggers

**Use Cases:**
- Prevent model degradation during online learning
- Retrain models without forgetting historical patterns
- Adapt to new pollution sources while maintaining accuracy on existing data

**Configuration:**
```rust
struct EWCConfig {
    ewc_lambda: f64,  // Memory protection strength (default: 2000)
    learning_rate: f64,
    quality_threshold: f64,  // Minimum confidence to skip learning
}
```

#### 5. **linfa - Classical ML & Preprocessing**

**Why linfa:**
- **scikit-learn Equivalent**: Familiar API for Python users
- **Mature**: Stable, well-tested, comprehensive
- **Preprocessing**: Excellent feature engineering tools
- **Pure Rust**: Optional BLAS backend for performance

**Use Cases:**
- Feature engineering (derive air quality indices from raw sensor data)
- Clustering (identify pollution zones, group similar days)
- Regression (simple forecasting, calibration curves)
- Preprocessing pipelines (normalization, scaling, missing data)

**Cargo.toml:**
```toml
linfa = "0.7"
linfa-preprocessing = "0.7"
linfa-clustering = "0.7"
linfa-linear = "0.7"
```

**Trade-offs:**
- **Pros**: Mature, comprehensive, scikit-learn compatible
- **Cons**: Less specialized than augurs for time-series forecasting
- **Decision**: Use for preprocessing and classical ML, not primary forecasting

### Drift Detection Approach

**Strategy: ADWIN + EWC++ Integration**

```rust
// Forecasting loop with drift detection
loop {
    let prediction = model.forecast(current_data);
    let actual = wait_for_actual_reading();
    let error = (prediction - actual).abs();

    // Check for concept drift
    if drift_detector.add_element(error) {
        println!("Drift detected! Retraining model...");

        // Retrain with EWC++ regularization
        model.retrain_incremental(
            recent_data,
            &ewc_regularizer,
            ewc_lambda: 2000.0,
        );

        // Update EWC importance weights
        ewc_regularizer.update_fisher_information(&model);
    }
}
```

### Model Selection Matrix

| Scenario | Recommended Approach |
|----------|---------------------|
| **Simple forecasting (single sensor)** | augurs ETS or MSTL |
| **Multi-sensor with seasonality** | augurs Prophet |
| **Anomaly detection** | augurs DBSCAN + MAD |
| **Complex dependencies (weather, traffic)** | burn Transformer or LSTM |
| **Interpretability required** | linfa Linear Regression or augurs ETS |
| **High-frequency real-time** | augurs ETS (fast inference) |
| **Long-term historical analysis** | augurs MSTL + Prophet |

---

## Storage Stack

### Raspberry Pi Choice

| Component | Pi Choice (64-bit OS) | Pi Choice (32-bit OS) | Justification |
|-----------|----------------------|---------------------|---------------|
| **Time-Series DB** | QuestDB | InfluxDB 1.x | QuestDB: 2.94M rows/sec, schema-agnostic; InfluxDB 1.x: Avoid 2.x crash bug |
| **Cache Layer** | Redis (optional) | SQLite cache | Redis may be too heavy; SQLite lightweight for Pi |
| **Feature Store** | QuestDB dedicated table | SQLite | Pre-computed ML features with TTL |
| **Model Storage** | Local filesystem | Local filesystem | Serialized models (safetensors, burn format) |

### M4 Mac Choice

| Component | Mac Choice | Justification |
|-----------|-----------|---------------|
| **Time-Series DB** | TimescaleDB | PostgreSQL ecosystem, Hypercore storage, complex queries |
| **Cache Layer** | Redis | Abundant RAM, in-memory cache for ML features |
| **Feature Store** | TimescaleDB + Redis | TimescaleDB for persistence, Redis for fast access |
| **Long-Term Storage** | TimescaleDB | Compression, retention policies, ACID guarantees |

### Detailed Storage Decisions

#### 1. **QuestDB (Raspberry Pi - 64-bit OS)**

**Why QuestDB:**
- **Performance**: 2.94M rows/sec ingestion (5x faster than InfluxDB)
- **Cardinality**: Handles 2.3M rows/sec with 10M unique series
- **Schema-Agnostic**: No upfront schema configuration needed
- **Protocols**: InfluxDB line protocol, PostgreSQL wire protocol, HTTP REST
- **Minimal Footprint**: <10MB without Java runtime
- **SQL Support**: Standard SQL queries (easier learning curve)

**Installation:**
```bash
# Docker on Pi (64-bit OS)
docker run -p 9000:9000 -p 9009:9009 -p 8812:8812 \
  -v "$(pwd)/questdb-data:/var/lib/questdb" \
  questdb/questdb
```

**Configuration:**
```conf
# questdb.conf
http.enabled=true
pg.enabled=true
line.tcp.enabled=true
cairo.sql.copy.root=/var/lib/questdb/data

# Performance tuning for Pi
cairo.max.uncommitted.rows=50000
cairo.commit.lag=30000  # 30 seconds
```

**Retention Policy:**
```sql
-- Keep raw data for 30 days
ALTER TABLE sensor_data DROP PARTITION
WHERE timestamp < dateadd('d', -30, now());
```

**Trade-offs:**
- **Pros**: Fastest ingestion, high cardinality, schema-agnostic, multi-protocol
- **Cons**: Requires 64-bit OS, smaller community than InfluxDB
- **Decision**: Primary choice for 64-bit Pi deployments

#### 2. **TimescaleDB (M4 Mac)**

**Why TimescaleDB:**
- **PostgreSQL**: Full SQL compatibility, ACID guarantees
- **Hypercore**: Hybrid row-columnar storage (recent data row-based, old data columnar)
- **Complex Queries**: Excellent for analytical workloads
- **Hypertables**: Automatic time-based partitioning
- **Ecosystem**: Leverage PostgreSQL extensions (PostGIS for geospatial)

**Installation:**
```bash
# Homebrew on macOS
brew install timescaledb

# Enable extension
CREATE EXTENSION IF NOT EXISTS timescaledb;
```

**Hypertable Creation:**
```sql
-- Create hypertable for sensor data
CREATE TABLE sensor_data (
    time TIMESTAMPTZ NOT NULL,
    sensor_id TEXT NOT NULL,
    metric TEXT NOT NULL,
    value DOUBLE PRECISION,
    tags JSONB
);

SELECT create_hypertable('sensor_data', 'time');

-- Add compression policy
ALTER TABLE sensor_data SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'sensor_id,metric'
);

-- Automatically compress data older than 7 days
SELECT add_compression_policy('sensor_data', INTERVAL '7 days');
```

**Continuous Aggregates:**
```sql
-- Materialized view for 1-hour aggregates
CREATE MATERIALIZED VIEW sensor_data_1h
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    sensor_id,
    metric,
    AVG(value) as avg_value,
    MIN(value) as min_value,
    MAX(value) as max_value
FROM sensor_data
GROUP BY bucket, sensor_id, metric;

-- Refresh policy
SELECT add_continuous_aggregate_policy('sensor_data_1h',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour');
```

**Trade-offs:**
- **Pros**: PostgreSQL ecosystem, complex queries, ACID guarantees, compression
- **Cons**: Slower ingestion than QuestDB, requires schema configuration
- **Decision**: Primary choice for M4 Mac (powerful hardware, PostgreSQL familiarity)

#### 3. **Redis - Cache Layer**

**Why Redis:**
- **Performance**: In-memory storage, microsecond latency
- **TTL Support**: Automatic expiration for cached features
- **Data Structures**: Lists, sets, sorted sets, hashes
- **Persistence**: Optional RDB/AOF for durability

**Use Cases:**
- Cache computed ML features (sliding window aggregations)
- Cache recent forecasts to avoid recomputation
- Store model embeddings (intermediate layers)
- Rate limiting and backpressure signaling

**Configuration (M4 Mac):**
```bash
# Install via Homebrew
brew install redis

# Start server
redis-server --maxmemory 2gb --maxmemory-policy allkeys-lru
```

**Feature Caching Pattern:**
```rust
use redis::AsyncCommands;

async fn get_cached_features(sensor_id: &str) -> Option<Features> {
    let mut conn = redis_client.get_async_connection().await.ok()?;
    let key = format!("features:{}", sensor_id);

    let cached: Option<String> = conn.get(&key).await.ok()?;
    cached.and_then(|json| serde_json::from_str(&json).ok())
}

async fn cache_features(sensor_id: &str, features: &Features, ttl_secs: u64) {
    let mut conn = redis_client.get_async_connection().await.ok()?;
    let key = format!("features:{}", sensor_id);
    let json = serde_json::to_string(features).ok()?;

    let _: () = conn.set_ex(&key, json, ttl_secs).await.ok()?;
}
```

**Trade-offs:**
- **Pros**: Ultra-fast, TTL support, rich data structures
- **Cons**: Memory-heavy (may be too much for Pi), persistence trade-offs
- **Decision**: M4 Mac only; Pi uses SQLite for lightweight caching

#### 4. **SQLite - Pi Cache Layer**

**Why SQLite (Pi):**
- **Minimal Footprint**: No external dependencies
- **Zero Configuration**: Single file database
- **Good Enough Performance**: Sufficient for feature caching on Pi

**Feature Cache Table:**
```sql
CREATE TABLE feature_cache (
    sensor_id TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    features BLOB NOT NULL,
    expires_at INTEGER NOT NULL,
    PRIMARY KEY (sensor_id, timestamp)
);

CREATE INDEX idx_expires ON feature_cache(expires_at);

-- Cleanup expired entries
DELETE FROM feature_cache WHERE expires_at < strftime('%s', 'now');
```

**Trade-offs:**
- **Pros**: Lightweight, no dependencies, simple
- **Cons**: Slower than Redis, no advanced data structures
- **Decision**: Pi only; M4 Mac uses Redis

### Storage Architecture Patterns

#### **Hybrid Storage (Pi + Mac)**

```
┌─────────────────────┐
│   Raspberry Pi      │
├─────────────────────┤
│ QuestDB (hot data)  │  30-day retention
│ SQLite (cache)      │  Feature cache with TTL
└──────────┬──────────┘
           │ MQTT/HTTP
           ↓
┌─────────────────────┐
│   M4 Mac            │
├─────────────────────┤
│ TimescaleDB (cold)  │  Unlimited retention, compressed
│ Redis (cache)       │  ML features, forecasts
└─────────────────────┘
```

**Benefits:**
- Pi focuses on real-time data collection (30-day window)
- Mac handles long-term storage and complex analytics
- Pi continues working if network fails
- Reduced Pi storage requirements

#### **Retention Strategy**

| Data Type | Pi Retention | Mac Retention | Downsampling |
|-----------|-------------|--------------|--------------|
| Raw sensor data | 30 days | 1 year | No |
| 1-minute aggregates | N/A | 2 years | From raw after 1 year |
| 1-hour aggregates | N/A | 5 years | From 1-min after 2 years |
| 1-day aggregates | N/A | Forever | From 1-hr after 5 years |

**TimescaleDB Retention Policies:**
```sql
-- Drop raw data older than 1 year
SELECT add_retention_policy('sensor_data', INTERVAL '1 year');

-- Automatically create continuous aggregates
SELECT add_continuous_aggregate_policy('sensor_data_1h',
    start_offset => INTERVAL '1 year',
    end_offset => INTERVAL '1 day',
    schedule_interval => INTERVAL '1 day');
```

### Model Artifact Storage

**Storage Format:** Safetensors (Hugging Face format)
- Cross-framework compatibility
- Memory-mapped loading (fast inference)
- Secure (prevents arbitrary code execution)

**Directory Structure:**
```
/var/lib/neural-platform/models/
├── forecasting/
│   ├── ets_model_v1.safetensors
│   ├── prophet_model_v1.safetensors
│   └── metadata.json
├── anomaly/
│   ├── dbscan_model_v1.safetensors
│   └── metadata.json
└── drift/
    ├── adwin_state_v1.json
    └── ewc_fisher_v1.safetensors
```

**Versioning Strategy:**
- Semantic versioning (v1, v2, ...)
- Metadata file tracks training date, performance metrics, hyperparameters
- Rollback support (keep previous version until new model validated)

---

## Messaging & Events

### Internal Messaging (Rust)

| Component | Choice | Justification |
|-----------|--------|---------------|
| **Async Channels** | Tokio mpsc | Native async support, bounded/unbounded, zero-cost |
| **MPMC Channels** | Flume | Zero unsafe code, fast, async support |
| **Actor Framework** | Actix (optional) | If complex actor system needed; prefer plain Tokio for simplicity |

### External Messaging

| Protocol | Use Case | Implementation |
|----------|----------|----------------|
| **MQTT** | IoT sensors, Home Assistant | rumqttc (async MQTT client) |
| **Redis Streams** | Event streaming, pub/sub | redis-rs with streams support |
| **HTTP/REST** | External APIs, webhooks | axum or actix-web |

### Serialization

| Format | Use Case | Crate |
|--------|----------|-------|
| **Protobuf** | Internal messages (performance) | prost |
| **JSON** | External APIs (compatibility) | serde_json |
| **MessagePack** | Binary JSON alternative | rmp-serde |

### Detailed Messaging Decisions

#### 1. **Tokio Channels - Internal Messaging**

**Why Tokio:**
- **Native Async**: Seamless integration with Tokio runtime
- **Bounded Channels**: Automatic backpressure
- **Types**: mpsc (multi-producer single-consumer), oneshot, broadcast, watch
- **Zero Cost**: No external dependencies

**Usage Pattern:**
```rust
use tokio::sync::mpsc;

// Create bounded channel with backpressure
let (tx, mut rx) = mpsc::channel(100);

// Producer
tokio::spawn(async move {
    for data_point in sensor_stream {
        tx.send(data_point).await.unwrap();  // Blocks if channel full
    }
});

// Consumer
tokio::spawn(async move {
    while let Some(data_point) = rx.recv().await {
        process_data_point(data_point).await;
    }
});
```

**Cargo.toml:**
```toml
tokio = { version = "1.35", features = ["full"] }
```

#### 2. **Flume - MPMC Channels**

**Why Flume:**
- **MPMC**: Multiple producers, multiple consumers
- **Zero Unsafe**: Safe Rust implementation
- **Async Support**: Works with Tokio/async-std
- **Performance**: Competitive with crossbeam, often faster than std::sync::mpsc

**Usage Pattern:**
```rust
use flume::bounded;

// Create MPMC channel
let (tx, rx) = bounded(100);

// Multiple producers
for i in 0..4 {
    let tx = tx.clone();
    tokio::spawn(async move {
        tx.send_async(data).await.unwrap();
    });
}

// Multiple consumers
for i in 0..2 {
    let rx = rx.clone();
    tokio::spawn(async move {
        while let Ok(data) = rx.recv_async().await {
            process(data).await;
        }
    });
}
```

**Cargo.toml:**
```toml
flume = "0.11"
```

**Trade-offs:**
- **Pros**: MPMC, safe, async-friendly, performant
- **Cons**: Casual maintenance mode (stable, but slower updates)
- **Decision**: Use when MPMC required; otherwise Tokio mpsc sufficient

#### 3. **MQTT - External IoT Protocol**

**Why MQTT:**
- **Standard**: Industry-standard IoT protocol
- **Lightweight**: Minimal bandwidth overhead
- **Pub/Sub**: Decoupled producers/consumers
- **QoS Levels**: At-most-once, at-least-once, exactly-once

**Crate: rumqttc (Async MQTT Client)**

**Cargo.toml:**
```toml
rumqttc = "0.23"
```

**Usage Pattern:**
```rust
use rumqttc::{AsyncClient, MqttOptions, QoS};

let mut mqttoptions = MqttOptions::new("neural-platform", "localhost", 1883);
mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));

let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

// Subscribe to sensor topics
client.subscribe("sensors/+/air_quality", QoS::AtLeastOnce).await?;

// Publish data
tokio::spawn(async move {
    loop {
        let data = read_sensor_data().await;
        let payload = serde_json::to_vec(&data)?;
        client.publish("sensors/living_room/air_quality", QoS::AtLeastOnce, false, payload).await?;
    }
});

// Receive messages
while let Ok(notification) = eventloop.poll().await {
    if let Event::Incoming(Packet::Publish(p)) = notification {
        process_mqtt_message(p).await;
    }
}
```

**MQTT Broker:** Mosquitto (lightweight, Pi-friendly)

```bash
# Install on Pi
sudo apt install mosquitto mosquitto-clients

# Configure
cat > /etc/mosquitto/conf.d/neural.conf <<EOF
listener 1883
allow_anonymous true
EOF

sudo systemctl restart mosquitto
```

#### 4. **Redis Streams - Event Streaming**

**Why Redis Streams:**
- **Consumer Groups**: Multiple consumers, load balancing
- **Persistence**: Optional durability (RDB/AOF)
- **Range Queries**: Read historical events
- **Acknowledgment**: Ensure events processed

**Usage Pattern:**
```rust
use redis::AsyncCommands;

// Producer: Add events to stream
let _: () = conn.xadd(
    "sensor:events",
    "*",
    &[("sensor_id", "living_room"), ("co2", "850")]
).await?;

// Consumer: Read from stream
let results: Vec<(String, HashMap<String, String>)> = conn.xread(
    &["sensor:events"],
    &["0-0"]  // From beginning
).await?;
```

**Trade-offs:**
- **Pros**: Persistence, consumer groups, range queries
- **Cons**: Requires Redis server, memory-heavy
- **Decision**: M4 Mac only; Pi uses MQTT for external messaging

#### 5. **Protobuf - Internal Serialization**

**Why Protobuf:**
- **Performance**: Binary format, compact
- **Schema Evolution**: Forward/backward compatibility
- **Code Generation**: Type-safe Rust structs

**Crate: prost (Rust Protobuf Implementation)**

**Cargo.toml:**
```toml
prost = "0.12"
prost-types = "0.12"

[build-dependencies]
prost-build = "0.12"
```

**Schema Definition (proto/sensor_data.proto):**
```protobuf
syntax = "proto3";

message SensorReading {
  string sensor_id = 1;
  int64 timestamp = 2;
  string metric = 3;
  double value = 4;
  map<string, string> tags = 5;
}

message SensorBatch {
  repeated SensorReading readings = 1;
}
```

**Build Script (build.rs):**
```rust
fn main() {
    prost_build::compile_protos(&["proto/sensor_data.proto"], &["proto/"]).unwrap();
}
```

**Usage:**
```rust
mod proto {
    include!(concat!(env!("OUT_DIR"), "/sensor_data.rs"));
}

use proto::SensorReading;

// Serialize
let reading = SensorReading {
    sensor_id: "living_room".to_string(),
    timestamp: 1640000000,
    metric: "co2".to_string(),
    value: 850.0,
    tags: HashMap::new(),
};

let bytes = reading.encode_to_vec();

// Deserialize
let decoded = SensorReading::decode(&bytes[..])?;
```

**Trade-offs:**
- **Pros**: Performance, schema evolution, type safety
- **Cons**: Requires code generation, less human-readable
- **Decision**: Internal high-throughput messaging; JSON for external APIs

### Message Bus Architecture

```
┌─────────────┐
│   Sensors   │
└──────┬──────┘
       │ MQTT (external)
       ↓
┌─────────────┐
│ Ingestion   │
│   Actor     │
└──────┬──────┘
       │ Tokio mpsc (internal)
       ↓
┌─────────────┐
│ Transform   │
│   Actor     │
└──────┬──────┘
       │ Flume (MPMC, parallel workers)
       ↓
┌─────────────┐
│  Storage    │
│   Actor     │
└─────────────┘
```

**Backpressure Handling:**
- Bounded channels automatically apply backpressure
- Sender blocks when channel full (async await)
- Prevents memory overflow in high-throughput scenarios

---

## MCP Integration

### SDK Choice

| Component | Choice | Justification |
|-----------|--------|---------------|
| **MCP SDK** | rmcp | Official Rust SDK, clean API, stdio/SSE support |
| **Transport** | stdio (primary) | Local Claude Code integration, simple |
| **Alternative Transport** | SSE | Cloud hosting, remote access |
| **Protocol Version** | 2025-06-18 | Latest stable version |

### Detailed MCP Decisions

#### 1. **rmcp - Official Rust MCP SDK**

**Why rmcp:**
- **Official**: Maintained by Model Context Protocol team
- **Clean API**: `#[tool]` macro for easy tool definition
- **Multi-Transport**: stdio, SSE server, SSE client, child process
- **Type-Safe**: Leverages Rust's type system for validation

**Cargo.toml:**
```toml
rmcp = { version = "0.3", features = ["server", "transport-io", "protocol-2025-06-18"] }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

#### 2. **Transport: stdio (Primary Choice)**

**Why stdio:**
- **Local Integration**: Perfect for Claude Code local installation
- **Simple**: No network configuration required
- **Secure**: No authentication needed (same-machine only)
- **Fast**: No network overhead

**Server Implementation:**
```rust
use rmcp::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct AirQualityReadings {
    co2_ppm: f64,
    pm25_ugm3: f64,
    temperature_c: f64,
    humidity_percent: f64,
    timestamp: i64,
}

#[tool]
/// Get current air quality readings from all sensors
async fn get_current_readings() -> Result<AirQualityReadings, String> {
    let readings = fetch_sensor_data().await
        .map_err(|e| e.to_string())?;

    Ok(AirQualityReadings {
        co2_ppm: readings.co2,
        pm25_ugm3: readings.pm25,
        temperature_c: readings.temperature,
        humidity_percent: readings.humidity,
        timestamp: readings.timestamp,
    })
}

#[tool]
/// Forecast air quality for specified hours ahead
async fn forecast_air_quality(hours_ahead: u32) -> Result<Vec<ForecastPoint>, String> {
    let forecaster = get_forecaster().await;
    let predictions = forecaster.predict(hours_ahead as usize).await
        .map_err(|e| e.to_string())?;

    Ok(predictions.into_iter().map(|p| ForecastPoint {
        timestamp: p.timestamp,
        co2_ppm: p.co2,
        pm25_ugm3: p.pm25,
        confidence: p.confidence,
    }).collect())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = ServerBuilder::new()
        .name("Air Quality MCP Server")
        .version("1.0.0")
        .tool(get_current_readings)
        .tool(forecast_air_quality)
        .stdio_transport()
        .build()?;

    server.run().await?;
    Ok(())
}
```

**Claude Code Integration:**
```bash
# Add to Claude Code MCP configuration
claude mcp add air-quality /path/to/neural-platform-mcp-server
```

#### 3. **Transport: SSE (Cloud Deployment)**

**Why SSE (Alternative):**
- **Remote Access**: Accessible from anywhere via URL
- **Cloud Hosting**: Deploy on cloud server
- **Authentication**: OAuth2 support
- **Multiple Clients**: Many clients can connect simultaneously

**Cargo.toml (SSE):**
```toml
rmcp = { version = "0.3", features = ["server", "transport-sse-server", "auth"] }
axum = "0.7"
tokio = { version = "1.35", features = ["full"] }
```

**SSE Server Implementation:**
```rust
use rmcp::prelude::*;
use axum::{Router, routing::get};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mcp_server = ServerBuilder::new()
        .name("Air Quality MCP Server")
        .version("1.0.0")
        .tool(get_current_readings)
        .tool(forecast_air_quality)
        .build()?;

    let app = Router::new()
        .route("/sse", get(mcp_server.sse_handler()))
        .route("/health", get(|| async { "OK" }));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

**Trade-offs:**
- **stdio**: Simple, local-only, fast
- **SSE**: Remote access, cloud-ready, authentication
- **Decision**: stdio for Pi/Mac local deployments, SSE for cloud offering

#### 4. **Tool Definitions Pattern**

**Recommended MCP Tools for Air Quality Platform:**

**1. Data Access Tools:**
- `get_current_readings()`: Fetch latest sensor data
- `get_historical_data(start, end, sensors)`: Query time-series database
- `get_sensor_status()`: Check sensor health, last calibration

**2. Forecasting Tools:**
- `forecast_air_quality(hours_ahead, confidence_level)`: Multi-horizon forecasts
- `forecast_with_scenarios(hours_ahead, weather_scenarios)`: What-if analysis
- `explain_forecast(forecast_id)`: Interpretability (feature importance)

**3. Analysis Tools:**
- `analyze_trends(time_range)`: Identify patterns, seasonality, anomalies
- `detect_pollution_events()`: Identify unusual pollution episodes
- `correlate_with_external(data_source)`: Link air quality to traffic, weather

**4. Optimization Tools:**
- `optimize_ventilation(constraints)`: Best ventilation schedule
- `recommend_actions(current_conditions)`: Actionable advice
- `estimate_impact(action)`: Predict effect of intervention

**5. Health & Alerts Tools:**
- `get_health_recommendations(user_profile)`: Personalized health advice
- `check_alert_thresholds()`: Current alert status
- `configure_alerts(thresholds, channels)`: Customize alerting

**Example Implementation:**
```rust
#[derive(Debug, Serialize, Deserialize)]
struct VentilationAnalysis {
    current_rate_cfm: f64,
    recommended_rate_cfm: f64,
    estimated_co2_reduction: f64,
    energy_impact_kwh: f64,
    recommendation: String,
}

#[tool]
/// Analyze ventilation effectiveness and recommend adjustments
async fn analyze_ventilation(
    current_rate_cfm: f64,
    target_co2_ppm: f64
) -> Result<VentilationAnalysis, String> {
    let optimizer = get_optimizer().await;
    let analysis = optimizer.analyze_ventilation(current_rate_cfm, target_co2_ppm).await
        .map_err(|e| e.to_string())?;

    Ok(VentilationAnalysis {
        current_rate_cfm,
        recommended_rate_cfm: analysis.optimal_rate,
        estimated_co2_reduction: analysis.co2_delta,
        energy_impact_kwh: analysis.energy_cost,
        recommendation: analysis.recommendation_text,
    })
}
```

### MCP Tool Design Best Practices

1. **Clear Descriptions**: Use doc comments for LLM understanding
2. **Typed Inputs/Outputs**: Leverage Rust's type system for validation
3. **Error Handling**: Return `Result<T, Error>` with descriptive errors
4. **Async Operations**: Use async/await for I/O-bound operations
5. **Idempotency**: Design tools to be safely callable multiple times
6. **Observability**: Log tool calls for debugging and monitoring

---

## Dashboard & Visualization

### Primary Dashboard

| Component | Choice | Location | Justification |
|-----------|--------|----------|---------------|
| **Dashboard Server** | Grafana | M4 Mac | Offload Pi resources, powerful hardware |
| **Data Source** | Prometheus, QuestDB, TimescaleDB | Both | Multiple data sources, flexible |
| **Alerting** | Grafana Alerting | M4 Mac | Integrated with dashboard |

### Detailed Dashboard Decisions

#### 1. **Grafana on M4 Mac**

**Why Grafana on Mac:**
- **Resource Offload**: Dashboard rendering on powerful M4 Mac instead of Pi
- **Centralized**: Single dashboard for multiple Pi sensors
- **Pre-Built Dashboards**: Raspberry Pi monitoring, air quality templates
- **Multi-Source**: Connect to QuestDB (Pi), TimescaleDB (Mac), Prometheus

**Installation (M4 Mac):**
```bash
# Homebrew
brew install grafana

# Start service
brew services start grafana

# Access: http://localhost:3000
```

**Configuration:**
```yaml
# /opt/homebrew/etc/grafana/grafana.ini
[server]
http_port = 3000

[database]
type = sqlite3
path = grafana.db

[security]
admin_user = admin
admin_password = <change_me>

[auth.anonymous]
enabled = true
org_role = Viewer
```

#### 2. **Data Source Connectors**

**QuestDB Connector (PostgreSQL Plugin):**
```yaml
# Grafana data source config
apiVersion: 1
datasources:
  - name: QuestDB (Pi)
    type: postgres
    url: raspberry-pi.local:8812
    database: qdb
    user: admin
    jsonData:
      sslmode: disable
      postgresVersion: 1100
```

**TimescaleDB Connector (PostgreSQL Plugin):**
```yaml
  - name: TimescaleDB (Mac)
    type: postgres
    url: localhost:5432
    database: neural_platform
    user: postgres
    jsonData:
      sslmode: disable
      timescaledb: true
```

**Prometheus Connector:**
```yaml
  - name: Prometheus (Pi)
    type: prometheus
    url: http://raspberry-pi.local:9090
```

#### 3. **Dashboard Templates**

**Air Quality Dashboard:**
```json
{
  "dashboard": {
    "title": "Air Quality Monitoring",
    "panels": [
      {
        "title": "CO2 (ppm)",
        "type": "timeseries",
        "targets": [
          {
            "datasource": "QuestDB (Pi)",
            "rawSql": "SELECT time, value FROM sensor_data WHERE metric='co2' AND $__timeFilter(time)"
          }
        ],
        "fieldConfig": {
          "defaults": {
            "thresholds": {
              "mode": "absolute",
              "steps": [
                { "value": null, "color": "green" },
                { "value": 1000, "color": "yellow" },
                { "value": 1500, "color": "red" }
              ]
            }
          }
        }
      },
      {
        "title": "PM2.5 (µg/m³)",
        "type": "timeseries",
        "targets": [
          {
            "datasource": "QuestDB (Pi)",
            "rawSql": "SELECT time, value FROM sensor_data WHERE metric='pm25' AND $__timeFilter(time)"
          }
        ]
      }
    ]
  }
}
```

**Raspberry Pi System Dashboard:**
- Import pre-built template: Dashboard ID 10578
- Metrics: CPU temp, load, memory, disk, network
- Works with node_exporter and rpi_exporter

#### 4. **Alerting Integration**

**Grafana Alerting Rules:**
```yaml
# Alert: High CO2
- uid: high_co2
  title: High CO2 Levels
  condition: C
  data:
    - refId: A
      datasourceUid: questdb_pi
      model:
        rawSql: |
          SELECT avg(value) as co2
          FROM sensor_data
          WHERE metric='co2'
          AND time > now() - interval '5 minutes'
    - refId: C
      reducer: last
      expression: A
      conditions:
        - evaluator:
            params: [1000]
            type: gt
  notifications:
    - uid: ntfy_channel
```

**Notification Channels:**
- ntfy (self-hosted)
- Pushover (critical alerts)
- Webhook (custom integrations)
- Email (digest reports)

### Lightweight Local Dashboard (Pi Fallback)

**Use Case:** Local access when network to Mac fails

**Choice:** Flask (Minimal Footprint)

**Cargo.toml (Rust Alternative: Axum):**
```toml
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs"] }
```

**Simple Dashboard Server:**
```rust
use axum::{Router, routing::get, Json};
use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/api/current", get(get_current_data))
        .nest_service("/", ServeDir::new("static"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_current_data() -> Json<SensorData> {
    let data = fetch_sensor_data().await.unwrap();
    Json(data)
}
```

**Static HTML (static/index.html):**
```html
<!DOCTYPE html>
<html>
<head>
    <title>Air Quality</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
</head>
<body>
    <h1>Air Quality Dashboard</h1>
    <canvas id="co2Chart"></canvas>
    <script>
        fetch('/api/current')
            .then(r => r.json())
            .then(data => {
                // Render chart with Chart.js
            });
    </script>
</body>
</html>
```

**Trade-offs:**
- **Grafana (Mac)**: Feature-rich, centralized, offloads Pi
- **Axum (Pi)**: Minimal, local access, fallback
- **Decision**: Grafana primary, Axum fallback for Pi-only access

---

## Home Automation

### Integration Strategy

| Component | Choice | Justification |
|-----------|--------|---------------|
| **Primary Protocol** | MQTT | Standard, Home Assistant auto-discovery |
| **HomeKit Bridge** | Homebridge | homebridge-http plugin for custom sensors |
| **MQTT Broker** | Mosquitto | Lightweight, Pi-friendly |
| **Home Assistant** | Optional | Powerful automation, MQTT native support |

### Detailed Home Automation Decisions

#### 1. **MQTT Publishing Pattern**

**Topic Structure (Home Assistant Auto-Discovery):**
```
homeassistant/sensor/neural_platform/living_room_co2/config
homeassistant/sensor/neural_platform/living_room_co2/state
homeassistant/sensor/neural_platform/living_room_pm25/config
homeassistant/sensor/neural_platform/living_room_pm25/state
```

**Config Message (Auto-Discovery):**
```json
{
  "name": "Living Room CO2",
  "state_topic": "neural/living_room/co2",
  "unit_of_measurement": "ppm",
  "device_class": "carbon_dioxide",
  "unique_id": "neural_lr_co2_001",
  "device": {
    "identifiers": ["neural_platform_pi"],
    "name": "Neural Platform Air Quality",
    "model": "Raspberry Pi 4",
    "manufacturer": "Neural Data Platform"
  }
}
```

**State Message:**
```json
{
  "value": 850,
  "timestamp": 1640000000,
  "quality": "good"
}
```

**Publishing Code:**
```rust
use rumqttc::{AsyncClient, MqttOptions, QoS};

async fn publish_air_quality(client: &AsyncClient, sensor_data: &SensorData) -> Result<()> {
    // Publish auto-discovery config (once at startup)
    let config = serde_json::json!({
        "name": "Living Room CO2",
        "state_topic": "neural/living_room/co2",
        "unit_of_measurement": "ppm",
        "device_class": "carbon_dioxide",
        "unique_id": "neural_lr_co2_001"
    });

    client.publish(
        "homeassistant/sensor/neural_platform/living_room_co2/config",
        QoS::AtLeastOnce,
        true,  // Retain
        serde_json::to_vec(&config)?
    ).await?;

    // Publish state updates (every 30-60 seconds)
    let state = serde_json::json!({
        "value": sensor_data.co2,
        "timestamp": sensor_data.timestamp
    });

    client.publish(
        "neural/living_room/co2",
        QoS::AtLeastOnce,
        false,  // Don't retain state
        serde_json::to_vec(&state)?
    ).await?;

    Ok(())
}
```

#### 2. **HomeKit Integration (Homebridge)**

**Plugin:** homebridge-http

**Installation:**
```bash
# Install Homebridge (on Pi or Mac)
sudo npm install -g homebridge

# Install HTTP plugin
sudo npm install -g homebridge-http
```

**Configuration (~/.homebridge/config.json):**
```json
{
  "accessories": [
    {
      "accessory": "HTTP-SENSOR",
      "name": "Living Room Air Quality",
      "service": "AirQualitySensor",
      "url": "http://localhost:8080/api/air_quality",
      "http_method": "GET",
      "pullInterval": 60000,
      "debug": true,
      "characteristics": {
        "AirQuality": {
          "mapping": {
            "type": "xpath",
            "parameters": {
              "xpath": "//air_quality"
            }
          }
        },
        "CarbonDioxideLevel": {
          "mapping": {
            "type": "xpath",
            "parameters": {
              "xpath": "//co2"
            }
          }
        },
        "PM2_5Density": {
          "mapping": {
            "type": "xpath",
            "parameters": {
              "xpath": "//pm25"
            }
          }
        }
      }
    }
  ]
}
```

**HTTP API (Axum):**
```rust
#[derive(Serialize)]
struct HomeKitResponse {
    air_quality: u8,  // 1=Excellent, 2=Good, 3=Fair, 4=Inferior, 5=Poor
    co2: f64,
    pm25: f64,
    temperature: f64,
    humidity: f64,
}

async fn homekit_api() -> Json<HomeKitResponse> {
    let data = fetch_sensor_data().await.unwrap();

    // Calculate overall air quality index
    let air_quality = calculate_aqi(data.co2, data.pm25);

    Json(HomeKitResponse {
        air_quality,
        co2: data.co2,
        pm25: data.pm25,
        temperature: data.temperature,
        humidity: data.humidity,
    })
}

fn calculate_aqi(co2: f64, pm25: f64) -> u8 {
    match (co2, pm25) {
        (co2, pm25) if co2 < 600.0 && pm25 < 12.0 => 1,  // Excellent
        (co2, pm25) if co2 < 800.0 && pm25 < 35.0 => 2,  // Good
        (co2, pm25) if co2 < 1000.0 && pm25 < 55.0 => 3, // Fair
        (co2, pm25) if co2 < 1500.0 && pm25 < 150.0 => 4, // Inferior
        _ => 5,  // Poor
    }
}
```

#### 3. **Home Assistant Automation Examples**

**YAML Configuration:**
```yaml
# Alert on high CO2
automation:
  - alias: "High CO2 Alert"
    trigger:
      platform: numeric_state
      entity_id: sensor.living_room_co2
      above: 1000
      for:
        minutes: 5
    action:
      - service: notify.mobile_app_iphone
        data:
          message: "CO2 exceeds 1000 ppm in living room ({{ states('sensor.living_room_co2') }} ppm)"
          title: "Air Quality Alert"
      - service: switch.turn_on
        entity_id: switch.ventilation_fan

  - alias: "Nighttime Air Quality Check"
    trigger:
      platform: time
      at: "22:00:00"
    action:
      - service: notify.mobile_app_iphone
        data:
          message: |
            Air Quality Summary:
            - CO2: {{ states('sensor.living_room_co2') }} ppm
            - PM2.5: {{ states('sensor.living_room_pm25') }} µg/m³
            - Temperature: {{ states('sensor.living_room_temperature') }}°C
          title: "Nightly Air Quality Report"
```

#### 4. **MQTT Broker Setup (Mosquitto)**

**Installation (Pi):**
```bash
sudo apt update
sudo apt install mosquitto mosquitto-clients

# Configure
sudo cat > /etc/mosquitto/conf.d/neural.conf <<EOF
listener 1883
allow_anonymous false
password_file /etc/mosquitto/passwd
EOF

# Create password file
sudo mosquitto_passwd -c /etc/mosquitto/passwd neural

# Restart
sudo systemctl restart mosquitto
```

**Trade-offs:**
- **MQTT**: Standard, Home Assistant native, flexible
- **HomeKit**: Apple ecosystem only, requires Homebridge
- **Decision**: MQTT primary (HA integration), Homebridge optional (HomeKit users)

---

## Observability

### Metrics & Monitoring

| Component | Choice | Location | Justification |
|-----------|--------|----------|---------------|
| **Metrics Collector** | Prometheus | Pi + Mac | Pull-based scraping, industry standard |
| **Metrics Exporter** | node_exporter + custom | Pi | System metrics + air quality metrics |
| **Tracing** | OpenTelemetry + Tempo | Mac | Distributed tracing, request flow |
| **Logging** | tracing crate + Loki | Both | Structured logging with trace correlation |
| **Visualization** | Grafana | Mac | Unified observability dashboard |

### Detailed Observability Decisions

#### 1. **Prometheus (Metrics Collection)**

**Why Prometheus:**
- **Industry Standard**: Widely adopted, large ecosystem
- **Pull Model**: Prometheus scrapes targets (simple)
- **PromQL**: Powerful query language
- **Alerting**: Alertmanager integration

**Installation (Pi):**
```bash
# Download Prometheus for ARM
wget https://github.com/prometheus/prometheus/releases/download/v2.48.0/prometheus-2.48.0.linux-arm64.tar.gz
tar xvfz prometheus-*.tar.gz
cd prometheus-*

# Create systemd service
sudo cat > /etc/systemd/system/prometheus.service <<EOF
[Unit]
Description=Prometheus
After=network.target

[Service]
Type=simple
User=prometheus
ExecStart=/opt/prometheus/prometheus --config.file=/etc/prometheus/prometheus.yml --storage.tsdb.path=/var/lib/prometheus
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable prometheus
sudo systemctl start prometheus
```

**Configuration (prometheus.yml):**
```yaml
global:
  scrape_interval: 60s
  evaluation_interval: 60s

scrape_configs:
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']

  - job_name: 'node'
    static_configs:
      - targets: ['localhost:9100']  # node_exporter

  - job_name: 'air_quality'
    static_configs:
      - targets: ['localhost:9200']  # custom exporter

  - job_name: 'rpi_hardware'
    static_configs:
      - targets: ['localhost:9111']  # rpi_exporter

# Remote write to M4 Mac
remote_write:
  - url: "http://m4-mac.local:9090/api/v1/write"
    queue_config:
      capacity: 10000
      max_shards: 5
      min_shards: 1
```

#### 2. **Custom Air Quality Exporter**

**Cargo.toml:**
```toml
prometheus = "0.13"
prometheus-hyper = "0.2"
hyper = { version = "1.0", features = ["full"] }
```

**Implementation:**
```rust
use prometheus::{Encoder, TextEncoder, Registry, Gauge, Opts};
use hyper::{Server, Request, Response, Body};

struct AirQualityExporter {
    registry: Registry,
    co2_ppm: Gauge,
    pm25_ugm3: Gauge,
    temperature_c: Gauge,
    humidity_percent: Gauge,
}

impl AirQualityExporter {
    fn new() -> Self {
        let registry = Registry::new();

        let co2_ppm = Gauge::with_opts(Opts::new("air_quality_co2_ppm", "CO2 level in ppm")).unwrap();
        let pm25_ugm3 = Gauge::with_opts(Opts::new("air_quality_pm25_ugm3", "PM2.5 in µg/m³")).unwrap();
        let temperature_c = Gauge::with_opts(Opts::new("air_quality_temperature_celsius", "Temperature in Celsius")).unwrap();
        let humidity_percent = Gauge::with_opts(Opts::new("air_quality_humidity_percent", "Relative humidity %")).unwrap();

        registry.register(Box::new(co2_ppm.clone())).unwrap();
        registry.register(Box::new(pm25_ugm3.clone())).unwrap();
        registry.register(Box::new(temperature_c.clone())).unwrap();
        registry.register(Box::new(humidity_percent.clone())).unwrap();

        Self { registry, co2_ppm, pm25_ugm3, temperature_c, humidity_percent }
    }

    fn update_metrics(&self, data: &SensorData) {
        self.co2_ppm.set(data.co2);
        self.pm25_ugm3.set(data.pm25);
        self.temperature_c.set(data.temperature);
        self.humidity_percent.set(data.humidity);
    }

    fn metrics_handler(&self) -> Response<Body> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();

        Response::new(Body::from(buffer))
    }
}

#[tokio::main]
async fn main() {
    let exporter = Arc::new(AirQualityExporter::new());

    // Background task to update metrics
    let exporter_clone = exporter.clone();
    tokio::spawn(async move {
        loop {
            let data = fetch_sensor_data().await.unwrap();
            exporter_clone.update_metrics(&data);
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });

    // HTTP server for /metrics endpoint
    let addr = ([0, 0, 0, 0], 9200).into();
    let make_svc = make_service_fn(move |_| {
        let exporter = exporter.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |_req| {
                let exporter = exporter.clone();
                async move {
                    Ok::<_, hyper::Error>(exporter.metrics_handler())
                }
            }))
        }
    });

    Server::bind(&addr).serve(make_svc).await.unwrap();
}
```

#### 3. **OpenTelemetry (Tracing & Metrics)**

**Why OpenTelemetry:**
- **Vendor-Neutral**: Avoid lock-in
- **Unified**: Metrics, traces, logs in one framework
- **Correlation**: Link metrics to traces to logs
- **Ecosystem**: Wide adoption, many exporters

**Cargo.toml:**
```toml
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
opentelemetry-prometheus = "0.14"
tracing = "0.1"
tracing-opentelemetry = "0.22"
tracing-subscriber = "0.3"
```

**Setup:**
```rust
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

fn init_telemetry() -> Result<()> {
    // Tracing (spans)
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://m4-mac.local:4317")
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    // Logging
    let subscriber = Registry::default()
        .with(telemetry)
        .with(tracing_subscriber::fmt::layer());

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

// Usage
#[tracing::instrument]
async fn ingest_sensor_data(sensor_id: &str, data: SensorData) -> Result<()> {
    tracing::info!("Ingesting data from sensor {}", sensor_id);

    // This span will be traced
    let span = tracing::span!(tracing::Level::INFO, "validate_data");
    let _enter = span.enter();

    validate_data(&data)?;

    let span = tracing::span!(tracing::Level::INFO, "store_data");
    let _enter = span.enter();

    store_to_database(&data).await?;

    Ok(())
}
```

#### 4. **Grafana Tempo (Distributed Tracing)**

**Installation (M4 Mac):**
```bash
brew install grafana/grafana/tempo

# Start service
brew services start tempo
```

**Configuration (tempo.yaml):**
```yaml
server:
  http_listen_port: 3200

distributor:
  receivers:
    otlp:
      protocols:
        grpc:
          endpoint: 0.0.0.0:4317

storage:
  trace:
    backend: local
    local:
      path: /opt/homebrew/var/tempo/traces
```

**Grafana Data Source:**
```yaml
apiVersion: 1
datasources:
  - name: Tempo
    type: tempo
    url: http://localhost:3200
    jsonData:
      tracesToLogs:
        datasourceUid: loki
        tags: ['trace_id']
      tracesToMetrics:
        datasourceUid: prometheus
```

#### 5. **Grafana Loki (Log Aggregation)**

**Installation (M4 Mac):**
```bash
brew install grafana/grafana/loki

# Start service
brew services start loki
```

**Configuration (loki-config.yaml):**
```yaml
auth_enabled: false

server:
  http_listen_port: 3100

ingester:
  lifecycler:
    ring:
      kvstore:
        store: inmemory
      replication_factor: 1

schema_config:
  configs:
    - from: 2020-10-24
      store: boltdb-shipper
      object_store: filesystem
      schema: v11
      index:
        prefix: index_
        period: 24h

storage_config:
  boltdb_shipper:
    active_index_directory: /opt/homebrew/var/loki/boltdb-shipper-active
    cache_location: /opt/homebrew/var/loki/boltdb-shipper-cache
  filesystem:
    directory: /opt/homebrew/var/loki/chunks
```

**Rust Logging (tracing crate):**
```rust
use tracing::{info, warn, error, span, Level};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .json()  // JSON output for Loki
        .init();

    info!("Starting air quality platform");

    let span = span!(Level::INFO, "ingestion");
    let _enter = span.enter();

    match ingest_data().await {
        Ok(_) => info!("Data ingested successfully"),
        Err(e) => error!("Ingestion failed: {}", e),
    }
}
```

**Send Logs to Loki (Promtail on Pi):**
```bash
# Install Promtail
wget https://github.com/grafana/loki/releases/download/v2.9.0/promtail-linux-arm64.zip
unzip promtail-linux-arm64.zip
sudo mv promtail-linux-arm64 /usr/local/bin/promtail
```

**Promtail Configuration:**
```yaml
server:
  http_listen_port: 9080

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://m4-mac.local:3100/loki/api/v1/push

scrape_configs:
  - job_name: neural_platform
    static_configs:
      - targets:
          - localhost
        labels:
          job: neural_platform
          __path__: /var/log/neural-platform/*.log
```

### Observability Architecture

```
┌─────────────────┐
│  Raspberry Pi   │
├─────────────────┤
│ Prometheus      │  Scrapes local exporters (node, air_quality)
│ Promtail        │  Ships logs to Loki
│ OTel SDK        │  Sends traces to Tempo
└────────┬────────┘
         │ Remote Write (metrics)
         │ OTLP/gRPC (traces)
         │ HTTP Push (logs)
         ↓
┌─────────────────┐
│   M4 Mac        │
├─────────────────┤
│ Prometheus      │  Long-term metrics storage
│ Tempo           │  Distributed tracing backend
│ Loki            │  Log aggregation
│ Grafana         │  Unified visualization
└─────────────────┘
```

---

## Development Tools

### Testing Frameworks

| Component | Choice | Justification |
|-----------|--------|---------------|
| **Unit Testing** | Built-in `#[test]` | Native Rust testing, zero dependencies |
| **Integration Testing** | tokio-test | Async testing for Tokio-based code |
| **Benchmarking** | Criterion.rs | Statistical benchmarking, profiling |
| **Property Testing** | proptest | Generative testing, edge case discovery |
| **Mocking** | mockall | Mock trait implementations |

### CI/CD Approach

| Component | Choice | Justification |
|-----------|--------|---------------|
| **CI Platform** | GitHub Actions | Free for public repos, Rust toolchain support |
| **Cross-Compilation** | cross | Cross-compile for ARM (Pi) from x86 (Mac) |
| **Linting** | Clippy | Official Rust linter, catch common mistakes |
| **Formatting** | rustfmt | Consistent code style |
| **Security Audit** | cargo-audit | Vulnerability scanning |

### Documentation Tools

| Component | Choice | Justification |
|-----------|--------|---------------|
| **API Docs** | rustdoc | Generate docs from code comments |
| **User Docs** | mdBook | Rust-native documentation tool |
| **Diagrams** | Mermaid | Markdown-embedded diagrams |

### Detailed Development Tool Decisions

#### 1. **Testing with Criterion.rs**

**Cargo.toml:**
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }

[[bench]]
name = "forecasting"
harness = false
```

**Benchmark (benches/forecasting.rs):**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_ets_forecast(c: &mut Criterion) {
    let data = generate_test_data(1000);

    c.bench_function("ETS forecast 24h", |b| {
        b.iter(|| {
            let forecaster = ETSForecaster::new();
            forecaster.fit(black_box(&data));
            forecaster.predict(black_box(24))
        });
    });
}

criterion_group!(benches, benchmark_ets_forecast);
criterion_main!(benches);
```

**Run Benchmarks:**
```bash
cargo bench --bench forecasting
# Results in target/criterion/
```

#### 2. **CI/CD with GitHub Actions**

**.github/workflows/ci.yml:**
```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: clippy, rustfmt

      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Check formatting
        run: cargo fmt --all -- --check

      - name: Clippy linting
        run: cargo clippy --all-targets --all-features -- -D warnings

      - name: Run tests
        run: cargo test --all-features

      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit

  cross-compile:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install cross
        run: cargo install cross

      - name: Build for ARM (Raspberry Pi)
        run: cross build --release --target aarch64-unknown-linux-gnu

      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: neural-platform-arm64
          path: target/aarch64-unknown-linux-gnu/release/neural-platform
```

#### 3. **Documentation with mdBook**

**Installation:**
```bash
cargo install mdbook
```

**Initialize:**
```bash
mdbook init docs
cd docs
```

**Structure (docs/src/):**
```
SUMMARY.md
introduction.md
getting-started/
  installation.md
  configuration.md
architecture/
  overview.md
  storage.md
  ml-pipeline.md
api/
  rest-api.md
  mcp-tools.md
```

**Build:**
```bash
mdbook build
mdbook serve  # Live preview at http://localhost:3000
```

#### 4. **Property Testing with proptest**

**Cargo.toml:**
```toml
[dev-dependencies]
proptest = "1.4"
```

**Test (tests/property_tests.rs):**
```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_forecast_never_negative(
        data in prop::collection::vec(0.0f64..1000.0, 10..100),
        horizon in 1usize..48
    ) {
        let forecaster = ETSForecaster::new();
        forecaster.fit(&data);
        let predictions = forecaster.predict(horizon);

        // Property: Forecasts should never be negative
        for pred in predictions {
            assert!(pred >= 0.0, "Forecast should not be negative");
        }
    }
}
```

---

## Version Matrix

### Core Dependencies

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **tokio** | 1.35 | full | Async runtime |
| **serde** | 1.0 | derive | Serialization |
| **serde_json** | 1.0 | - | JSON support |
| **anyhow** | 1.0 | - | Error handling |
| **thiserror** | 1.0 | - | Custom errors |

### ML & Forecasting

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **augurs** | 0.4 | - | Time-series forecasting |
| **augurs-ets** | 0.4 | - | Exponential smoothing |
| **augurs-mstl** | 0.4 | - | Seasonal decomposition |
| **augurs-prophet** | 0.4 | - | Prophet forecasting |
| **augurs-clustering** | 0.4 | - | DBSCAN clustering |
| **burn** | 0.14 | - | Deep learning framework |
| **burn-tch** | 0.14 | - | PyTorch backend |
| **burn-ndarray** | 0.14 | - | CPU backend |
| **linfa** | 0.7 | - | Classical ML |
| **linfa-preprocessing** | 0.7 | - | Feature engineering |
| **linfa-clustering** | 0.7 | - | Clustering algorithms |

### Storage

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **questdb-rs** | 0.1 (unofficial) | - | QuestDB client (use HTTP or PostgreSQL wire) |
| **sqlx** | 0.7 | runtime-tokio-native-tls, postgres | TimescaleDB/PostgreSQL |
| **redis** | 0.24 | tokio-comp, connection-manager | Redis client |
| **rusqlite** | 0.30 | - | SQLite for Pi cache |

### Messaging

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **rumqttc** | 0.23 | - | Async MQTT client |
| **flume** | 0.11 | - | MPMC channels |
| **prost** | 0.12 | - | Protobuf serialization |
| **prost-types** | 0.12 | - | Protobuf well-known types |

### MCP Integration

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **rmcp** | 0.3 | server, transport-io, protocol-2025-06-18 | Official Rust MCP SDK |
| **axum** | 0.7 | - | HTTP server (for SSE transport) |

### Observability

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **prometheus** | 0.13 | - | Metrics collection |
| **opentelemetry** | 0.21 | - | Tracing framework |
| **opentelemetry-otlp** | 0.14 | - | OTLP exporter |
| **tracing** | 0.1 | - | Logging/tracing |
| **tracing-subscriber** | 0.3 | json | Log subscriber |
| **tracing-opentelemetry** | 0.22 | - | OTel integration |

### Web & API

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **axum** | 0.7 | - | HTTP server |
| **tower** | 0.4 | - | Middleware |
| **tower-http** | 0.5 | fs, cors | Static files, CORS |

### Development Tools

| Crate | Version | Features | Notes |
|-------|---------|----------|-------|
| **criterion** | 0.5 | html_reports | Benchmarking |
| **proptest** | 1.4 | - | Property testing |
| **mockall** | 0.12 | - | Mocking |
| **tokio-test** | 0.4 | - | Async testing |

---

## Cargo.toml Reference

### Complete Cargo.toml for Neural Platform

```toml
[package]
name = "neural-data-platform"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
prost = "0.12"
prost-types = "0.12"

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# ML & Forecasting
augurs = "0.4"
augurs-ets = "0.4"
augurs-mstl = "0.4"
augurs-prophet = "0.4"
augurs-clustering = "0.4"
burn = { version = "0.14", optional = true }
burn-tch = { version = "0.14", optional = true }
linfa = "0.7"
linfa-preprocessing = "0.7"
linfa-clustering = "0.7"

# Storage
sqlx = { version = "0.7", features = ["runtime-tokio-native-tls", "postgres", "chrono"] }
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }
rusqlite = { version = "0.30", optional = true }

# Messaging
rumqttc = "0.23"
flume = "0.11"

# MCP Integration
rmcp = { version = "0.3", features = ["server", "transport-io", "protocol-2025-06-18"] }

# Web & API
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "cors"] }

# Observability
prometheus = "0.13"
opentelemetry = "0.21"
opentelemetry-otlp = "0.14"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.22"

# Utilities
chrono = "0.4"
uuid = { version = "1.6", features = ["v4", "serde"] }
dotenv = "0.15"
clap = { version = "4.4", features = ["derive"] }

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.4"
mockall = "0.12"
tokio-test = "0.4"

[build-dependencies]
prost-build = "0.12"

[features]
default = ["pi"]
pi = ["rusqlite"]
deep-learning = ["burn", "burn-tch"]
all = ["pi", "deep-learning"]

[[bench]]
name = "forecasting"
harness = false

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

### Feature-Specific Builds

**Raspberry Pi Build (minimal):**
```bash
cargo build --release --target aarch64-unknown-linux-gnu
```

**M4 Mac Build (full features):**
```bash
cargo build --release --features all
```

**Development Build:**
```bash
cargo build
```

---

## Summary

This technology selection document provides a comprehensive guide for building the neural-data-platform air quality intelligence system. The choices balance performance, resource constraints, production readiness, and extensibility.

**Key Takeaways:**
1. **Forecasting**: augurs for production, burn for deep learning when needed
2. **Storage**: QuestDB (Pi), TimescaleDB (Mac), hybrid architecture
3. **Messaging**: Tokio channels (internal), MQTT (external), Flume (MPMC)
4. **MCP**: rmcp with stdio transport for local integration
5. **Dashboard**: Grafana on Mac to offload Pi resources
6. **Home Automation**: MQTT → Home Assistant, Homebridge for HomeKit
7. **Observability**: Prometheus + OpenTelemetry + Grafana stack
8. **Development**: Criterion (benchmarks), GitHub Actions (CI/CD), mdBook (docs)

**Next Steps:**
1. Set up development environment on both Pi and M4 Mac
2. Implement minimal viable storage layer (QuestDB on Pi)
3. Build basic forecasting pipeline with augurs
4. Create custom Prometheus exporter for air quality metrics
5. Deploy Grafana dashboard on M4 Mac
6. Integrate MQTT for Home Assistant auto-discovery
7. Implement MCP server with rmcp
8. Set up observability stack (Prometheus, Tempo, Loki, Grafana)

---

**Document Version:** 1.0
**Last Updated:** 2025-12-13
**Maintainer:** Neural Data Platform Team
