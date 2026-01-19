# Platform Design: Real-Time Processing Layer

**Purpose**: Config-driven, platform-level solution for processing data within seconds of arrival.

**Design Principle**: Same philosophy as Bronze→Silver ETL - declarative YAML config, no code changes for new processors.

---

## 1. Conceptual Architecture

```
                              ┌─────────────────────────────────────┐
                              │      CONFIGURATION (etcd)           │
                              │  /streams/{id}/processors           │
                              │  /processors/{id}/config            │
                              └──────────────────┬──────────────────┘
                                                 │
                                                 ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           INGESTION LAYER                                    │
│                                                                              │
│   ┌─────────────┐         ┌─────────────────────────────────────────────┐   │
│   │ MQTT Source │────────►│            EVENT BUS                         │   │
│   └─────────────┘         │  (broadcast channel for real-time fan-out)  │   │
│   ┌─────────────┐         └─────────────┬───────────────┬───────────────┘   │
│   │ HTTP Source │──────────────────────►│               │                   │
│   └─────────────┘                       │               │                   │
└─────────────────────────────────────────┼───────────────┼───────────────────┘
                                          │               │
                    ┌─────────────────────┘               └─────────────────────┐
                    │                                                           │
                    ▼                                                           ▼
┌─────────────────────────────────────────┐       ┌─────────────────────────────────────────┐
│         STORAGE PATH (existing)          │       │       PROCESSING PATH (new)             │
│                                          │       │                                         │
│   RawStorageWriter                       │       │   ProcessingCoordinator                 │
│   - Batching (configurable)              │       │   - Routes to registered processors     │
│   - WAL + Parquet                         │       │   - Config-driven processor loading     │
│   - Bronze layer                         │       │   - Backpressure handling               │
│                                          │       │                                         │
│   (unchanged)                            │       │   ┌─────────────────────────────────┐   │
│                                          │       │   │  Processor Registry             │   │
└──────────────────────────────────────────┘       │   │  - ThresholdProcessor           │   │
                                                   │   │  - AggregationProcessor         │   │
                                                   │   │  - MLInferenceProcessor         │   │
                                                   │   │  - WebhookProcessor             │   │
                                                   │   │  - CustomProcessor (plugin)     │   │
                                                   │   └─────────────────────────────────┘   │
                                                   │                                         │
                                                   │   Output Sinks:                         │
                                                   │   - TimescaleDB (direct write)          │
                                                   │   - Webhook/HTTP callback               │
                                                   │   - MQTT publish                        │
                                                   │   - Alert channel                       │
                                                   └─────────────────────────────────────────┘
```

---

## 2. Configuration Schema

### 2.1 Stream-Level Processor Binding

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
description: "AirGradient indoor air quality sensors"

# Existing Bronze/Silver config...
sources: [...]
silver_etl: {...}

# NEW: Real-time processing configuration
processing:
  enabled: true

  # Which processors to run on this stream's data
  processors:
    - processor_id: air-quality-threshold-alerts
      enabled: true
      priority: 1  # Lower = runs first

    - processor_id: air-quality-ml-predictions
      enabled: true
      priority: 2

    - processor_id: rolling-averages
      enabled: true
      priority: 3
```

### 2.2 Processor Definitions

```yaml
# config/base/processors/air-quality-threshold-alerts.yaml
processor_id: air-quality-threshold-alerts
type: threshold
description: "Alert when air quality exceeds safe levels"
version: "1.0.0"

# Processor-specific configuration
config:
  rules:
    - name: pm25_unhealthy
      field: raw_payload.pm02Compensated
      condition: "> 35.4"
      severity: warning
      message: "PM2.5 exceeds EPA 'Unhealthy for Sensitive Groups' threshold"

    - name: pm25_hazardous
      field: raw_payload.pm02Compensated
      condition: "> 250.4"
      severity: critical
      message: "PM2.5 at hazardous levels"

    - name: co2_high
      field: raw_payload.rco2
      condition: "> 1000"
      severity: warning
      message: "CO2 elevated - consider ventilation"

    - name: co2_very_high
      field: raw_payload.rco2
      condition: "> 2000"
      severity: critical
      message: "CO2 very high - ventilation required"

# Where to send alerts
outputs:
  - type: webhook
    url: "${ALERT_WEBHOOK_URL}"

  - type: mqtt
    topic: "ndp/alerts/{stream_id}/{severity}"

  - type: timescale
    table: silver.alerts
```

### 2.3 ML Inference Processor

```yaml
# config/base/processors/air-quality-ml-predictions.yaml
processor_id: air-quality-ml-predictions
type: ml_inference
description: "Predict air quality trends"

config:
  model:
    type: onnx  # or: pytorch, tensorflow, custom
    path: "/models/air-quality-forecast.onnx"
    # OR load from URL/S3
    url: "${MODEL_URL}"

  # Feature extraction from raw payload
  features:
    - source: raw_payload.pm02Compensated
      name: pm25
      preprocessing: normalize

    - source: raw_payload.rco2
      name: co2
      preprocessing: normalize

    - source: raw_payload.atmpCompensated
      name: temperature

    - source: raw_payload.rhumCompensated
      name: humidity

  # What to predict
  predictions:
    - name: pm25_1h_forecast
      output_index: 0

    - name: pm25_trend
      output_index: 1

  # Inference settings
  batch_size: 1  # For real-time, process individually
  timeout_ms: 100  # Max inference time before skip

outputs:
  - type: timescale
    table: silver.predictions
    columns:
      observation_time: timestamp
      ndp_id: ndp_id
      pm25_1h_forecast: predictions.pm25_1h_forecast
      pm25_trend: predictions.pm25_trend

  - type: mqtt
    topic: "ndp/predictions/air-quality"
```

### 2.4 Aggregation Processor

```yaml
# config/base/processors/rolling-averages.yaml
processor_id: rolling-averages
type: aggregation
description: "Compute rolling statistics for dashboards"

config:
  windows:
    - name: 5min
      duration: 5m

    - name: 1hour
      duration: 1h

  aggregations:
    - field: raw_payload.pm02Compensated
      functions: [avg, min, max, p95]

    - field: raw_payload.rco2
      functions: [avg, min, max]

    - field: raw_payload.atmpCompensated
      functions: [avg]

  # Group by sensor
  group_by: [ndp_id]

outputs:
  - type: timescale
    table: silver.rolling_stats
```

### 2.5 Custom/Plugin Processor

```yaml
# config/base/processors/custom-ventilation-logic.yaml
processor_id: custom-ventilation-logic
type: custom
description: "Complex ventilation recommendation logic"

config:
  # Reference to compiled processor library
  library: "/plugins/ventilation_processor.so"
  # Or WASM for sandboxed execution
  wasm: "/plugins/ventilation_processor.wasm"

  # Config passed to the custom processor
  params:
    outdoor_temp_source: "outdoor-weather"
    comfort_range_min: 20.0
    comfort_range_max: 26.0

outputs:
  - type: mqtt
    topic: "ndp/recommendations/ventilation"
```

---

## 3. Core Library Changes

### 3.1 New Module Structure

```
core/src/
├── processing/                    # NEW MODULE
│   ├── mod.rs                     # Module exports
│   ├── traits.rs                  # Processor trait definitions
│   ├── coordinator.rs             # ProcessingCoordinator
│   ├── registry.rs                # ProcessorRegistry
│   ├── config.rs                  # ProcessorConfig types
│   └── processors/                # Built-in processors
│       ├── mod.rs
│       ├── threshold.rs           # ThresholdProcessor
│       ├── aggregation.rs         # AggregationProcessor
│       ├── ml_inference.rs        # MLInferenceProcessor
│       ├── webhook.rs             # WebhookProcessor
│       └── passthrough.rs         # PassthroughProcessor (for chaining)
├── outputs/                       # NEW MODULE
│   ├── mod.rs
│   ├── traits.rs                  # OutputSink trait
│   ├── timescale.rs               # Direct TimescaleDB writes
│   ├── mqtt.rs                    # MQTT publish
│   ├── webhook.rs                 # HTTP callback
│   └── channel.rs                 # Internal channel output
```

### 3.2 Core Traits

```rust
// core/src/processing/traits.rs

/// A processor that operates on streaming data
#[async_trait]
pub trait Processor: Send + Sync {
    /// Process a single data point
    /// Returns outputs to be sent to configured sinks
    async fn process(&self, input: &ProcessorInput) -> Result<ProcessorOutput, ProcessorError>;

    /// Process a batch of data points (for efficiency)
    async fn process_batch(&self, inputs: &[ProcessorInput]) -> Result<Vec<ProcessorOutput>, ProcessorError> {
        // Default: process individually
        let mut outputs = Vec::with_capacity(inputs.len());
        for input in inputs {
            outputs.push(self.process(input).await?);
        }
        Ok(outputs)
    }

    /// Called when processor is first loaded
    async fn initialize(&mut self, config: &ProcessorConfig) -> Result<(), ProcessorError>;

    /// Called on config hot-reload
    async fn reconfigure(&mut self, config: &ProcessorConfig) -> Result<(), ProcessorError>;

    /// Health check
    async fn health_check(&self) -> HealthStatus;

    /// Processor metadata
    fn metadata(&self) -> ProcessorMetadata;
}

/// Input to a processor
pub struct ProcessorInput {
    pub timestamp: DateTime<Utc>,
    pub stream_id: String,
    pub source_id: String,
    pub ndp_id: Option<String>,
    pub raw_payload: serde_json::Value,
    pub context: Option<serde_json::Value>,
}

/// Output from a processor
pub struct ProcessorOutput {
    pub timestamp: DateTime<Utc>,
    pub output_type: OutputType,
    pub data: serde_json::Value,
    pub metadata: HashMap<String, String>,
}

pub enum OutputType {
    Prediction,
    Alert,
    Aggregation,
    Transformed,
    Passthrough,
}
```

### 3.3 Output Sink Trait

```rust
// core/src/outputs/traits.rs

#[async_trait]
pub trait OutputSink: Send + Sync {
    /// Write outputs to the sink
    async fn write(&self, outputs: &[ProcessorOutput]) -> Result<(), OutputError>;

    /// Flush any buffered data
    async fn flush(&self) -> Result<(), OutputError>;

    /// Health check
    async fn health_check(&self) -> HealthStatus;
}
```

### 3.4 Processing Coordinator

```rust
// core/src/processing/coordinator.rs

pub struct ProcessingCoordinator {
    registry: Arc<ProcessorRegistry>,
    stream_bindings: HashMap<String, Vec<ProcessorBinding>>,
    output_sinks: HashMap<String, Arc<dyn OutputSink>>,
    metrics: ProcessingMetrics,
}

impl ProcessingCoordinator {
    /// Route a data point to all bound processors for its stream
    pub async fn process(&self, input: ProcessorInput) -> Result<(), ProcessorError> {
        let bindings = self.stream_bindings.get(&input.stream_id)
            .ok_or(ProcessorError::NoBindings)?;

        // Process through each bound processor in priority order
        for binding in bindings {
            if !binding.enabled {
                continue;
            }

            let processor = self.registry.get(&binding.processor_id)?;

            let start = Instant::now();
            match processor.process(&input).await {
                Ok(outputs) => {
                    self.metrics.record_success(&binding.processor_id, start.elapsed());

                    // Route outputs to configured sinks
                    for output in outputs {
                        self.route_output(&binding.processor_id, output).await?;
                    }
                }
                Err(e) => {
                    self.metrics.record_failure(&binding.processor_id, &e);
                    // Continue to next processor (don't fail entire pipeline)
                    warn!("Processor {} failed: {}", binding.processor_id, e);
                }
            }
        }

        Ok(())
    }
}
```

---

## 4. Integration with Existing Pipeline

### 4.1 Event Bus (Broadcast Channel)

Add a broadcast channel that fans out to both storage and processing:

```rust
// In air-quality-app main.rs

// Create broadcast channel for real-time fan-out
let (event_tx, _) = tokio::sync::broadcast::channel::<RawDataPoint>(1000);

// Storage path subscribes
let storage_rx = event_tx.subscribe();
let storage_writer = RawStorageWriter::new(store, storage_rx, ...);

// Processing path subscribes
let processing_rx = event_tx.subscribe();
let processing_coordinator = ProcessingCoordinator::new(registry, processing_rx);

// Sources send to broadcast channel
source_manager.set_event_sender(event_tx);
```

### 4.2 Source Integration

Minimal change to sources - just send to event bus:

```rust
// In mqtt/mod.rs process_events()

// After parsing JSON:
let raw_point = RawDataPoint::new(source_id, json);

// Send to event bus (fans out to storage + processing)
if let Err(e) = event_tx.send(raw_point.clone()) {
    warn!("Event bus send failed: {}", e);
}
```

---

## 5. New Binary: Processing Daemon

Similar to `silver-etl`, create a processing daemon:

```
apps/
├── air-quality-app/       # Ingestion (existing)
├── silver-etl/            # Bronze→Silver ETL (existing)
└── ndp-processor/         # NEW: Real-time processing daemon
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── daemon.rs
        └── config.rs
```

Or integrate into existing `air-quality-app` with feature flag.

---

## 6. Configuration Loading

### 6.1 etcd Keys

```
/processors/{processor_id}/config     # Processor definitions
/streams/{stream_id}/processing       # Stream→Processor bindings
```

### 6.2 Hot Reload

```rust
// Watch for processor config changes
registry.watch_processors().await;

// Watch for binding changes
coordinator.watch_bindings().await;
```

---

## 7. Built-in Processor Types

| Type | Purpose | Config-Driven? |
|------|---------|----------------|
| `threshold` | Alert on value conditions | Yes - rules in YAML |
| `aggregation` | Rolling windows, stats | Yes - windows/functions in YAML |
| `ml_inference` | Run ONNX/custom models | Yes - model path, features in YAML |
| `transform` | Field mapping, unit conversion | Yes - reuse Silver ETL transforms |
| `webhook` | HTTP callback on events | Yes - URL, payload template |
| `filter` | Drop/pass based on conditions | Yes - filter expressions |
| `enrich` | Add data from other sources | Yes - join config |
| `custom` | Plugin/WASM extensibility | Partial - code + config |

---

## 8. Estimated Scope

### Core Library (`core/`)

| Component | New Lines | Complexity |
|-----------|-----------|------------|
| `processing/traits.rs` | ~150 | Medium |
| `processing/coordinator.rs` | ~300 | Medium |
| `processing/registry.rs` | ~200 | Low |
| `processing/config.rs` | ~250 | Low |
| `processing/processors/threshold.rs` | ~200 | Low |
| `processing/processors/aggregation.rs` | ~350 | Medium |
| `processing/processors/ml_inference.rs` | ~400 | High |
| `outputs/` module | ~400 | Medium |
| **Subtotal** | **~2,250** | |

### Application (`apps/`)

| Component | New Lines | Complexity |
|-----------|-----------|------------|
| Event bus integration | ~100 | Low |
| `ndp-processor/` binary | ~500 | Medium |
| Config client extensions | ~150 | Low |
| **Subtotal** | **~750** | |

### Configuration

| Component | Files |
|-----------|-------|
| Processor schemas | 5-10 YAML files |
| Stream binding examples | Update existing |
| JSON Schema for validation | 2-3 files |

### Total

| Category | Estimate |
|----------|----------|
| New Rust code | ~3,000 lines |
| Config/schema | ~500 lines YAML |
| Tests | ~1,000 lines |
| Documentation | ~500 lines |

---

## 9. Phased Implementation

### Phase 1: Foundation (Week 1-2)
- [ ] `Processor` and `OutputSink` traits
- [ ] `ProcessorRegistry` with hot-reload
- [ ] `ThresholdProcessor` (simplest built-in)
- [ ] MQTT output sink
- [ ] Basic coordinator

### Phase 2: Core Processors (Week 3-4)
- [ ] `AggregationProcessor` with windowing
- [ ] TimescaleDB output sink
- [ ] Webhook output sink
- [ ] Event bus integration

### Phase 3: ML & Advanced (Week 5-6)
- [ ] `MLInferenceProcessor` with ONNX runtime
- [ ] Custom/plugin processor support
- [ ] Full config schema and validation
- [ ] `ndp-processor` daemon binary

### Phase 4: Polish (Week 7-8)
- [ ] Metrics and observability
- [ ] Grafana dashboard for processing
- [ ] Documentation and examples
- [ ] Performance optimization

---

## 10. Alignment with Roadmap

This design directly addresses roadmap items:

| Roadmap Item | How This Addresses It |
|--------------|----------------------|
| **Gold Layer: ML-ready feature engineering** | `AggregationProcessor` computes features in real-time |
| **Neural Predictions: Time-series forecasting** | `MLInferenceProcessor` runs models on arrival |
| **Action Triggers: Threshold-based alerts** | `ThresholdProcessor` with config-driven rules |

---

## 11. Example End-to-End Flow

1. **MQTT message arrives** with PM2.5 = 45.0
2. **Event bus** broadcasts to storage + processing
3. **Storage path** (unchanged) batches and writes to Bronze
4. **Processing path**:
   - `ThresholdProcessor` evaluates rule `pm25 > 35.4` → TRUE
   - Creates alert output with severity=warning
   - Routes to configured sinks:
     - Webhook fires to Slack
     - MQTT publishes to `ndp/alerts/air-quality/warning`
     - TimescaleDB inserts to `silver.alerts`
5. **Total latency**: 50-200ms from message receipt to alert delivery

---

This is a platform-level solution that:
- Fits NDP's config-driven philosophy
- Requires no code changes for new processors/rules
- Integrates cleanly with existing architecture
- Scales from simple thresholds to ML inference
