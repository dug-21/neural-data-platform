# AIR-002: Ingestion Pipeline Implementation Roadmap

**Feature:** MQTT to Parquet Data Ingestion Pipeline
**Priority:** CRITICAL - Blocks E2E Testing
**Estimated Total Effort:** 22-30 hours (REVISED: config scope reduced)
**Created:** 2025-12-14
**Last Updated:** 2025-12-14 (Config scope analysis)

**Note:** Configuration standardization (config-store integration) deferred to AIR-003 to minimize critical path. See `/workspaces/neural-data-platform/product/features/air-002/implementation/02-config-scope-analysis.md` for detailed analysis.

---

## Executive Summary

This roadmap delivers the MINIMUM viable ingestion pipeline to unblock E2E testing. All domain logic exists; we just need to wire components together and replace mock implementations with real ones.

**Success Criteria:**
- MQTT messages flow to Parquet storage
- REST API returns real sensor data
- Health endpoint shows accurate MQTT/storage status
- Integration tests validate data persistence

---

## Task Breakdown

### Task 1: Configuration Management
**ID:** AIR-002-T1
**Priority:** HIGH
**Estimated Hours:** 1-2 (REVISED: simplified, config-store deferred)
**Dependencies:** None
**Status:** NOT STARTED

#### Description
Create minimal YAML configuration for MQTT broker and storage paths with environment variable overrides. Uses simple serde-based loading. Config-store integration deferred to AIR-003.

#### Files to Create/Modify
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/config.yaml`
- **MODIFY:** `/workspaces/neural-data-platform/apps/air-quality-app/src/config.rs`

#### Implementation Details

**config.yaml:**
```yaml
server:
  host: "0.0.0.0"
  port: 8080

mqtt:
  broker_url: "localhost"
  port: 1883
  client_id: "air-quality-app"
  topic_pattern: "airgradient/readings/+"
  qos: 1
  reconnect_delay_secs: 1
  max_reconnect_delay_secs: 30
  buffer_capacity: 1000

storage:
  base_path: "/data/parquet"
  wal_enabled: true
```

**config.rs updates:**
- Add `MqttConfigYaml` struct (serializable version)
- Add `StorageConfigYaml` struct
- Implement `from_yaml()` using serde_yaml
- Implement `to_mqtt_config()` converter to platform-core types
- Environment variable overrides (MQTT_BROKER_URL, MQTT_PORT, STORAGE_PATH)
- Simple validation (non-empty URLs, valid ports)

#### Acceptance Criteria
- [ ] Configuration loads from config.yaml
- [ ] Environment variables override config file values (MQTT_BROKER_URL, STORAGE_PATH)
- [ ] Default config available if file missing
- [ ] Converts to platform-core MqttConfig struct
- [ ] ParquetStore can use storage.base_path

#### Out of Scope (Deferred to AIR-003)
- Schema validation beyond basic checks
- Config versioning
- config-store integration
- TOML format support
- Advanced validation rules

#### Manual Verification
```bash
# Test config loading
MQTT_BROKER_URL=test-broker cargo run --bin air-quality-app
# Should see log: "MQTT broker: test-broker"
```

---

### Task 2: MQTT Ingestion Module
**ID:** AIR-002-T2
**Priority:** CRITICAL
**Estimated Hours:** 6-8
**Dependencies:** T1 (config)
**Status:** NOT STARTED

#### Description
Create ingestion module that initializes MQTT source, connects to broker, and subscribes to topics.

#### Files to Create/Modify
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/src/ingestion/mod.rs`
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/src/ingestion/mqtt_handler.rs`

#### Implementation Details

**ingestion/mod.rs:**
```rust
pub mod mqtt_handler;

pub use mqtt_handler::MqttHandler;
```

**ingestion/mqtt_handler.rs:**
```rust
use air_quality::parser::parse_mqtt_payload;
use air_quality::validation::validate_reading;
use air_quality::adapter::AirQualityAdapter;
use platform_core::sources::mqtt::{MqttSource, MqttConfig};
use platform_core::traits::Source;
use tokio::sync::mpsc;
use tracing::{info, error, warn};

pub struct MqttHandler {
    source: MqttSource,
    channel: mpsc::Sender<Vec<TimeSeriesPoint>>,
}

impl MqttHandler {
    pub async fn new(config: MqttConfig, channel: mpsc::Sender<Vec<TimeSeriesPoint>>) -> Result<Self> {
        let mut source = MqttSource::new(config);
        source.start().await?;

        Ok(Self { source, channel })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("MQTT handler started, polling for messages");

        loop {
            // Poll for new data from MQTT
            match self.source.fetch().await {
                Ok(points) if !points.is_empty() => {
                    // Points already converted by MqttSource
                    info!("Received {} time series points from MQTT", points.len());

                    if let Err(e) = self.channel.send(points).await {
                        error!("Failed to send points to pipeline: {}", e);
                    }
                }
                Ok(_) => {
                    // No data, continue polling
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!("MQTT fetch error: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    pub async fn health_check(&self) -> Result<HealthStatus> {
        self.source.health_check().await
    }
}
```

**Key Points:**
- Use existing `MqttSource` from `platform_core::sources::mqtt`
- Parse messages using `air_quality::parser::parse_mqtt_payload`
- Validate using `air_quality::validation::validate_reading`
- Convert using `air_quality::adapter::AirQualityAdapter::to_time_series_points`
- Send to processing channel

#### Acceptance Criteria
- [ ] MQTT source connects to broker on startup
- [ ] Subscribes to `airgradient/readings/+` topic pattern
- [ ] Parses incoming JSON payloads
- [ ] Validates readings against spec ranges
- [ ] Converts to TimeSeriesPoints
- [ ] Auto-reconnects on connection failure
- [ ] Logs all operations with appropriate levels

#### Manual Verification
```bash
# Start mosquitto broker
mosquitto -v

# Publish test message
mosquitto_pub -t "airgradient/readings/test123" -m '{"serialno":"test123","pm02":12.5,"rco2":450}'

# Check logs for:
# - "MQTT handler started"
# - "Received 1 time series points from MQTT"
```

---

### Task 3: Storage Pipeline
**ID:** AIR-002-T3
**Priority:** CRITICAL
**Estimated Hours:** 5-6
**Dependencies:** T1 (config)
**Status:** NOT STARTED

#### Description
Create pipeline that receives TimeSeriesPoints from MQTT handler and writes to Parquet storage.

#### Files to Create/Modify
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/mod.rs`
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/src/pipeline/storage_writer.rs`

#### Implementation Details

**pipeline/mod.rs:**
```rust
pub mod storage_writer;

pub use storage_writer::StorageWriter;
```

**pipeline/storage_writer.rs:**
```rust
use platform_core::storage::parquet::ParquetStore;
use platform_core::traits::{Store, TimeSeriesPoint};
use tokio::sync::mpsc;
use tracing::{info, error};

pub struct StorageWriter {
    store: Arc<ParquetStore>,
    receiver: mpsc::Receiver<Vec<TimeSeriesPoint>>,
    batch_size: usize,
    batch_timeout: Duration,
}

impl StorageWriter {
    pub fn new(
        store: Arc<ParquetStore>,
        receiver: mpsc::Receiver<Vec<TimeSeriesPoint>>,
    ) -> Self {
        Self {
            store,
            receiver,
            batch_size: 100,
            batch_timeout: Duration::from_secs(5),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("Storage writer started");

        let mut buffer = Vec::new();
        let mut last_flush = Instant::now();

        loop {
            tokio::select! {
                Some(points) = self.receiver.recv() => {
                    buffer.extend(points);

                    // Flush if batch size reached or timeout expired
                    if buffer.len() >= self.batch_size || last_flush.elapsed() >= self.batch_timeout {
                        self.flush(&mut buffer).await?;
                        last_flush = Instant::now();
                    }
                }
                _ = tokio::time::sleep(self.batch_timeout) => {
                    // Timeout - flush pending data
                    if !buffer.is_empty() {
                        self.flush(&mut buffer).await?;
                        last_flush = Instant::now();
                    }
                }
            }
        }
    }

    async fn flush(&self, buffer: &mut Vec<TimeSeriesPoint>) -> Result<()> {
        if buffer.is_empty() {
            return Ok(());
        }

        info!("Flushing {} points to storage", buffer.len());

        self.store.write_batch(buffer.drain(..).collect()).await?;

        Ok(())
    }
}
```

**Key Points:**
- Batch writes for efficiency (default: 100 points or 5 seconds)
- Use existing `ParquetStore` with WAL
- Graceful handling of backpressure
- Metrics for write throughput

#### Acceptance Criteria
- [ ] Receives TimeSeriesPoints from channel
- [ ] Batches writes efficiently
- [ ] Flushes on timeout to prevent data loss
- [ ] WAL ensures durability
- [ ] Handles write errors gracefully
- [ ] Logs write statistics

#### Manual Verification
```bash
# After publishing MQTT message, check Parquet files
ls -lh /data/parquet/data/test123/year=*/month=*/day=*/

# Verify WAL exists
ls -lh /data/parquet/wal.log
```

---

### Task 4: Main.rs Integration
**ID:** AIR-002-T4
**Priority:** CRITICAL
**Estimated Hours:** 4-5
**Dependencies:** T2 (MQTT), T3 (storage)
**Status:** NOT STARTED

#### Description
Wire together all components in main.rs, replacing mock implementations with real ones.

#### Files to Modify
- **MODIFY:** `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

#### Implementation Details

**Key Changes:**
```rust
use air_quality_app::{
    api::create_router,
    config::AppConfig,
    ingestion::MqttHandler,
    pipeline::StorageWriter,
};
use platform_core::sources::mqtt::MqttConfig;
use platform_core::storage::parquet::ParquetStore;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (existing)

    // Load configuration
    let config = AppConfig::from_yaml("config.yaml")?;

    // Initialize storage
    let storage = Arc::new(ParquetStore::new(&config.storage.base_path)?);

    // Replay WAL on startup
    storage.replay_wal().await?;
    tracing::info!("WAL replay complete");

    // Create channel for MQTT -> Storage pipeline
    let (tx, rx) = mpsc::channel(1000);

    // Initialize MQTT handler
    let mqtt_config = MqttConfig {
        broker_url: config.mqtt.broker_url,
        port: config.mqtt.port,
        client_id: config.mqtt.client_id,
        topic_pattern: config.mqtt.topic_pattern,
        qos: rumqttc::QoS::AtLeastOnce,
        reconnect_delay: Duration::from_secs(config.mqtt.reconnect_delay_secs),
        max_reconnect_delay: Duration::from_secs(config.mqtt.max_reconnect_delay_secs),
        buffer_capacity: config.mqtt.buffer_capacity,
    };

    let mut mqtt_handler = MqttHandler::new(mqtt_config, tx).await?;

    // Initialize storage writer
    let mut storage_writer = StorageWriter::new(storage.clone(), rx);

    // Spawn background tasks
    let mqtt_handle = tokio::spawn(async move {
        if let Err(e) = mqtt_handler.run().await {
            tracing::error!("MQTT handler error: {}", e);
        }
    });

    let storage_handle = tokio::spawn(async move {
        if let Err(e) = storage_writer.run().await {
            tracing::error!("Storage writer error: {}", e);
        }
    });

    // Create router with real storage (remove mocks)
    let app = create_router(storage);

    // Start HTTP server
    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on {}", addr);

    // Graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    // Wait for background tasks
    mqtt_handle.abort();
    storage_handle.abort();

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");

    tracing::info!("Shutdown signal received");
}
```

#### Acceptance Criteria
- [ ] All components initialize on startup
- [ ] Background tasks run concurrently
- [ ] HTTP server starts successfully
- [ ] Graceful shutdown commits WAL
- [ ] No mock implementations remain

#### Manual Verification
```bash
# Start application
cargo run --bin air-quality-app

# Check logs show:
# - "WAL replay complete"
# - "MQTT handler started"
# - "Storage writer started"
# - "Server listening on 0.0.0.0:8080"

# Publish test data
mosquitto_pub -t "airgradient/readings/test123" -m '{"serialno":"test123","pm02":12.5,"rco2":450}'

# Query API
curl http://localhost:8080/api/v1/readings/latest?location_id=test123

# Should return actual data, not mock
```

---

### Task 5: Health Endpoint Integration
**ID:** AIR-002-T5
**Priority:** HIGH
**Estimated Hours:** 2-3
**Dependencies:** T4 (main integration)
**Status:** NOT STARTED

#### Description
Update health endpoint to report actual MQTT and storage status.

#### Files to Modify
- **MODIFY:** `/workspaces/neural-data-platform/apps/air-quality-app/src/api/handlers/health.rs`

#### Implementation Details

**Update health handler:**
```rust
pub async fn handle_health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, ApiError> {
    // Check MQTT health
    let mqtt_health = state.mqtt_handler.health_check().await?;

    // Check storage health
    let storage_health = state.storage.health_check().await?;

    let overall_healthy = mqtt_health.healthy && storage_health.healthy;
    let status = if overall_healthy {
        "healthy"
    } else if mqtt_health.healthy || storage_health.healthy {
        "degraded"
    } else {
        "unhealthy"
    };

    Ok(Json(HealthResponse {
        status: status.to_string(),
        timestamp: Utc::now(),
        components: HashMap::from([
            ("mqtt".to_string(), ComponentHealth {
                healthy: mqtt_health.healthy,
                message: mqtt_health.message,
            }),
            ("storage".to_string(), ComponentHealth {
                healthy: storage_health.healthy,
                message: storage_health.message,
            }),
        ]),
    }))
}
```

#### Acceptance Criteria
- [ ] Health endpoint returns actual component status
- [ ] Shows "healthy" when all components operational
- [ ] Shows "degraded" when one component failing
- [ ] Shows "unhealthy" when all components failing
- [ ] Includes detailed component messages

#### Manual Verification
```bash
# With everything running
curl http://localhost:8080/health
# Should show: {"status": "healthy", "components": {...}}

# Stop MQTT broker
# Should show: {"status": "degraded", ...}
```

---

### Task 6: Integration Tests
**ID:** AIR-002-T6
**Priority:** HIGH
**Estimated Hours:** 4-5
**Dependencies:** T5 (health endpoint)
**Status:** NOT STARTED

#### Description
Create integration tests that validate the complete ingestion pipeline.

#### Files to Create
- **CREATE:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration_test.rs`

#### Implementation Details

**Test scenarios:**
```rust
#[tokio::test]
async fn test_mqtt_to_parquet_flow() {
    // Setup test MQTT broker
    // Publish test message
    // Wait for ingestion
    // Query storage
    // Assert data present
}

#[tokio::test]
async fn test_data_persistence_after_restart() {
    // Ingest data
    // Stop application
    // Restart application
    // Query storage
    // Assert data still present
}

#[tokio::test]
async fn test_health_endpoint_accuracy() {
    // Check health with MQTT connected
    // Stop MQTT broker
    // Check health shows degraded
    // Restart MQTT broker
    // Check health recovers
}

#[tokio::test]
async fn test_invalid_message_handling() {
    // Publish invalid JSON
    // Assert logged but not stored
    // Publish valid message
    // Assert valid message stored
}

#[tokio::test]
async fn test_wal_recovery() {
    // Ingest data
    // Kill application mid-write
    // Restart application
    // Assert WAL replayed
    // Assert data integrity
}
```

#### Acceptance Criteria
- [ ] All integration tests pass
- [ ] Tests use real MQTT broker (testcontainers)
- [ ] Tests validate data persistence
- [ ] Tests verify error handling
- [ ] Tests confirm WAL recovery

#### Manual Verification
```bash
# Run integration tests
cargo test --test integration_test -- --nocapture

# Should see:
# - test_mqtt_to_parquet_flow ... ok
# - test_data_persistence_after_restart ... ok
# - test_health_endpoint_accuracy ... ok
# - test_invalid_message_handling ... ok
# - test_wal_recovery ... ok
```

---

## Milestone Definitions

### M1: Configuration and MQTT Connection
**Tasks:** T1, T2
**Duration:** ~9-10 hours (REVISED: T1 reduced from 4h to 2h)
**Verification:**
```bash
cargo run --bin air-quality-server
# Logs show: "MQTT handler started"
# Logs show: "Connected to MQTT broker"
```

### M2: Data Storage
**Tasks:** T3
**Duration:** ~5-6 hours
**Verification:**
```bash
# Publish message, check Parquet files exist
ls /data/parquet/data/*/year=*/month=*/day=*/
```

### M3: End-to-End Integration
**Tasks:** T4, T5
**Duration:** ~6-8 hours
**Verification:**
```bash
# Publish MQTT message
# Query REST API
curl http://localhost:8080/api/v1/readings/latest?location_id=test123
# Returns actual data
```

### M4: Validation and Testing
**Tasks:** T6
**Duration:** ~4-5 hours
**Verification:**
```bash
cargo test --test integration_test
# All tests pass
```

---

## Dependency Graph

```
T1 (Config)
│
├──> T2 (MQTT Handler)
│    │
│    └──> T4 (Main Integration)
│         │
│         └──> T5 (Health Endpoint)
│              │
│              └──> T6 (Integration Tests)
│
└──> T3 (Storage Writer)
     │
     └──> T4 (Main Integration)
```

**Critical Path:** T1 → T2 → T4 → T5 → T6
**Parallel Opportunity:** T2 and T3 can be developed simultaneously after T1

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **MQTT broker unavailable during development** | Medium | Medium | Use local Mosquitto instance, document setup in README |
| **Parquet write performance issues** | Low | High | Implement batching (100 points/5s), monitor write latency |
| **Memory leaks in background tasks** | Medium | High | Add metrics for channel depth, set bounded channels |
| **WAL corruption on crash** | Low | Critical | Add WAL integrity checks, implement recovery mode |
| **Configuration parsing errors** | High | Low | Comprehensive error messages, validate on startup |
| **Test environment flakiness** | Medium | Medium | Use testcontainers for isolation, add retries |

### Risk Mitigation Strategies

**MQTT Broker Availability:**
- Document Mosquitto installation in README
- Provide docker-compose.yml for local testing
- Implement robust reconnection logic

**Performance Issues:**
- Start with conservative batch sizes
- Add metrics to identify bottlenecks
- Profile write operations if needed

**Memory Management:**
- Set bounded channels (1000 items)
- Monitor channel depth via metrics
- Implement backpressure handling

**Data Integrity:**
- WAL enabled by default
- Graceful shutdown commits pending writes
- Recovery mode on startup

---

## Verification Checkpoints

### Checkpoint 1: MQTT Connection (After T2)
```bash
# Start app
cargo run --bin air-quality-app

# Expected logs:
# INFO air_quality_app: MQTT handler started
# INFO rumqttc: Connection established
# INFO rumqttc: Subscribed to airgradient/readings/+

# Test message
mosquitto_pub -t "airgradient/readings/test" -m '{"serialno":"test","pm02":10,"rco2":400}'

# Expected logs:
# INFO air_quality_app::ingestion: Received 2 time series points from MQTT
```

### Checkpoint 2: Data Persistence (After T3)
```bash
# Publish test data
mosquitto_pub -t "airgradient/readings/sensor1" -m '{"serialno":"sensor1","pm02":12.5,"rco2":450}'

# Wait 5 seconds (flush timeout)

# Check Parquet files
find /data/parquet -name "*.parquet" -exec ls -lh {} \;

# Should see:
# /data/parquet/data/sensor1/year=2025/month=12/day=14/readings.parquet (4.2K)
```

### Checkpoint 3: REST API (After T4)
```bash
# Query latest reading
curl -s http://localhost:8080/api/v1/readings/latest?location_id=sensor1 | jq

# Expected response:
# {
#   "location_id": "sensor1",
#   "readings": [
#     {
#       "metric": "pm25",
#       "value": 12.5,
#       "timestamp": "2025-12-14T10:30:00Z"
#     },
#     {
#       "metric": "co2",
#       "value": 450.0,
#       "timestamp": "2025-12-14T10:30:00Z"
#     }
#   ]
# }
```

### Checkpoint 4: Health Status (After T5)
```bash
# Check health
curl -s http://localhost:8080/health | jq

# Expected (healthy):
# {
#   "status": "healthy",
#   "timestamp": "2025-12-14T10:30:00Z",
#   "components": {
#     "mqtt": {
#       "healthy": true,
#       "message": "MQTT connection healthy"
#     },
#     "storage": {
#       "healthy": true,
#       "message": "Storage is healthy"
#     }
#   }
# }

# Stop MQTT broker
docker stop mosquitto

# Check health again
curl -s http://localhost:8080/health | jq

# Expected (degraded):
# {
#   "status": "degraded",
#   "components": {
#     "mqtt": {
#       "healthy": false,
#       "message": "MQTT connection unhealthy"
#     },
#     ...
#   }
# }
```

### Checkpoint 5: Integration Tests (After T6)
```bash
# Run all integration tests
cargo test --test integration_test -- --test-threads=1 --nocapture

# Expected output:
# running 5 tests
# test test_mqtt_to_parquet_flow ... ok (2.3s)
# test test_data_persistence_after_restart ... ok (4.1s)
# test test_health_endpoint_accuracy ... ok (1.8s)
# test test_invalid_message_handling ... ok (1.2s)
# test test_wal_recovery ... ok (3.5s)
#
# test result: ok. 5 passed; 0 failed
```

---

## Resource Requirements

### Single Developer Timeline
**Total Duration:** 22-30 hours (3-4 working days) (REVISED: T1 reduced 2h)

```
Day 1 (8h):
  - T1: Configuration (2h) [REVISED from 4h]
  - T2: MQTT Handler (6h)

Day 2 (8h):
  - T2: MQTT Handler completion (2h)
  - T3: Storage Writer (6h)

Day 3 (8h):
  - T4: Main Integration (5h)
  - T5: Health Endpoint (3h)

Day 4 (6h):
  - T6: Integration Tests (5h)
  - Buffer for issues (1h) [Reduced due to simpler config]
```

### Two Developer Timeline (Recommended)
**Total Duration:** 14-18 hours (2 working days) (REVISED: T1 reduced)

```
Developer 1 (Backend/MQTT):
  Day 1: T1 (2h) + T2 (6h) [REVISED: T1 faster]
  Day 2: T4 (4h) + T6 (4h)

Developer 2 (Storage/API):
  Day 1: T1 review (30min) + T3 (6h)
  Day 2: T5 (3h) + T6 support (3h)
```

### Required Resources
- **Local Development:**
  - Mosquitto MQTT broker (Docker or native)
  - 500MB disk space for Parquet storage
  - Rust toolchain 1.75+

- **CI/CD:**
  - GitHub Actions runner
  - Docker support for testcontainers
  - 2GB RAM for tests

---

## Success Metrics

### Functional Requirements
- [ ] MQTT messages ingested within 1 second
- [ ] Parquet writes complete within 5 seconds
- [ ] REST API returns real data (no mocks)
- [ ] Health endpoint accurate (±1 second latency)
- [ ] WAL recovery completes in <10 seconds

### Performance Requirements
- [ ] Handle 10 messages/second throughput
- [ ] <100MB memory footprint
- [ ] <5% CPU usage during idle
- [ ] <50% CPU usage during peak load

### Reliability Requirements
- [ ] Auto-reconnect to MQTT within 30 seconds
- [ ] No data loss on graceful shutdown
- [ ] WAL recovery on crash
- [ ] Zero data corruption in storage

---

## Post-Implementation Checklist

After completing all tasks, verify:

- [ ] All integration tests passing
- [ ] Documentation updated (README, architecture diagrams)
- [ ] Configuration documented (config.yaml.example)
- [ ] Error messages are clear and actionable
- [ ] Logs include appropriate context
- [ ] Metrics available for monitoring
- [ ] Health endpoint accurate
- [ ] No mock implementations remain
- [ ] Code reviewed and approved
- [ ] Performance benchmarks meet targets

---

## Next Steps (Out of Scope for AIR-002)

These items are deferred to future features:

1. **Configuration Standardization** (AIR-003) [NEW]
   - Migrate to config-store library
   - Add MqttConfig to config-store/configs/sources.rs
   - Add StorageConfig to config-store/configs/storage.rs
   - TOML format standardization
   - Advanced validation schemas
   - Estimated: 3-5 hours

2. **Alerting System** (AIR-004)
   - Threshold monitoring
   - Alert generation and deduplication
   - Alert persistence

3. **Forecasting Integration** (AIR-005)
   - ruv-FANN model loading
   - Feature engineering pipeline
   - Forecast API endpoint

4. **MCP Tools** (AIR-006)
   - Claude integration
   - Analysis workflows
   - Reporting tools

5. **Advanced Features** (AIR-007+)
   - Multi-location support
   - Data aggregation endpoints
   - Custom dashboards

---

## Appendix: File Structure

After implementation, the structure will be:

```
apps/air-quality-app/
├── src/
│   ├── main.rs              (MODIFIED - T4)
│   ├── config.rs            (MODIFIED - T1)
│   ├── ingestion/
│   │   ├── mod.rs           (NEW - T2)
│   │   └── mqtt_handler.rs  (NEW - T2)
│   ├── pipeline/
│   │   ├── mod.rs           (NEW - T3)
│   │   └── storage_writer.rs(NEW - T3)
│   └── api/
│       └── handlers/
│           └── health.rs    (MODIFIED - T5)
├── tests/
│   └── integration_test.rs  (NEW - T6)
├── config.yaml              (NEW - T1)
└── Cargo.toml               (MODIFIED - dependencies)
```

---

**End of Roadmap**
