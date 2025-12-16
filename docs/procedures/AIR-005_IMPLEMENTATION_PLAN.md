# AIR-005 IngestionCoordinator Implementation Plan

**Version**: 1.0.0
**Date**: 2025-12-16
**Status**: Ready for Implementation
**Phase**: Refinement (TDD Implementation)

---

## Executive Summary

This implementation plan provides a detailed breakdown for building the AIR-005 IngestionCoordinator feature, which adds multi-stream, multi-source data ingestion capabilities to the Neural Data Platform. The plan follows London School TDD principles with mocked dependencies and test-first development.

**Key Deliverables:**
1. IngestionCoordinator - Central orchestration of sources and routing
2. SourceManager - Dynamic source lifecycle management
3. IngestionRouter - Schema validation and routing (already exists, needs integration)
4. Generic HTTP polling with ResponseParser trait
5. Stream Registry integration for dynamic configuration

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Phase 1: Core Module Additions](#phase-1-core-module-additions)
3. [Phase 2: App Module Additions](#phase-2-app-module-additions)
4. [Phase 3: Configuration Integration](#phase-3-configuration-integration)
5. [Phase 4: Integration Testing](#phase-4-integration-testing)
6. [Dependencies and Integration Points](#dependencies-and-integration-points)
7. [Test Strategy](#test-strategy)
8. [Implementation Sequence](#implementation-sequence)

---

## Architecture Overview

### Existing Components (Reusable)

| Component | Location | Status | Notes |
|-----------|----------|--------|-------|
| Source trait | `core/src/traits.rs` | ✅ Exists | No changes needed |
| Store trait | `core/src/traits.rs` | ✅ Exists | No changes needed |
| TimeSeriesPoint | `core/src/traits.rs` | ✅ Exists | Core data type |
| StreamConfig | `core/src/types/stream_config.rs` | ✅ Exists | Complete with validation |
| StreamRecord | `core/src/types/stream_record.rs` | ✅ Exists | Wraps TimeSeriesPoint |
| MqttSource | `core/src/sources/mqtt.rs` | ✅ Exists | Implements Source |
| HttpPollingSource | `core/src/sources/http_poll.rs` | ⚠️ Needs refactoring | Make generic |
| ParquetStore | `core/src/storage/parquet.rs` | ✅ Exists | Multi-stream capable |
| IngestionRouter | `apps/air-quality-app/src/coordinator/router.rs` | ✅ Exists | Schema validation |

### New Components (To Build)

| Component | Location | Purpose |
|-----------|----------|---------|
| SourceManager | `core/src/coordinator/source_manager.rs` | Spawn/stop sources based on config |
| IngestionCoordinator | `core/src/coordinator/mod.rs` | Orchestrate sources + router + storage |
| ResponseParser trait | `core/src/sources/parsers/mod.rs` | Pluggable HTTP response parsing |
| WeatherParser | `core/src/sources/parsers/weather.rs` | OpenWeatherMap current weather |
| AirPollutionParser | `core/src/sources/parsers/air_pollution.rs` | OpenWeatherMap air quality |
| AuthMethod enum | `core/src/sources/http_poll.rs` | Flexible authentication |
| RetryHandler | `core/src/sources/http_poll.rs` | Exponential backoff logic |

### Data Flow

```
StreamRegistry (etcd)
    │
    ├─ Load stream configs
    │
    ▼
IngestionCoordinator
    │
    ├─ SourceManager.spawn(MqttSource)
    ├─ SourceManager.spawn(HttpPollingSource[weather])
    ├─ SourceManager.spawn(HttpPollingSource[air_quality])
    │
    ▼
TimeSeriesPoints → mpsc::channel
    │
    ▼
IngestionRouter
    ├─ Validate against schema
    ├─ Route to correct storage
    │
    ▼
StorageWriter → ParquetStore
    ├─ /data/air-quality/
    ├─ /data/outdoor-weather/
    └─ /data/outdoor-air-quality/
```

---

## Phase 1: Core Module Additions

### 1.1 Generic HTTP Polling Refactor

**Files to Modify:**

#### `core/src/sources/http_poll.rs`

**Changes Required:**
1. Add `ResponseParser` trait
2. Add `AuthMethod` enum
3. Add `RetryConfig` struct
4. Add `EndpointConfig` struct (replace `SensorConfig`)
5. Add `ParserRegistry` for parser lookup
6. Refactor `poll_sensor()` → `poll_endpoint()`
7. Add `poll_with_retry()` with exponential backoff
8. Add error classification (`ErrorType` enum)

**Estimated Lines:** +250, -55

**Test Files:**
- `core/src/sources/http_poll.rs` (inline tests, London TDD)

**Test Count:** 15-20 tests covering:
- Parser registration and lookup
- Authentication application
- Retry logic (transient, rate-limited, permanent errors)
- Exponential backoff calculation
- Endpoint configuration validation
- Health check with per-endpoint tracking

**Dependencies:**
```toml
# core/Cargo.toml (existing)
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
chrono = "0.4"
```

---

#### `core/src/sources/parsers/mod.rs` (NEW)

**Purpose:** Parser module for HTTP response parsing

**Contents:**
```rust
pub mod weather;
pub mod air_pollution;

use crate::traits::TimeSeriesPoint;
use crate::error::CoreResult;
use chrono::{DateTime, Utc};

/// Trait for parsing HTTP API responses into TimeSeriesPoints
pub trait ResponseParser: Send + Sync + 'static {
    /// Parse raw JSON response into time series points
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Parser identifier for logging and config
    fn name(&self) -> &'static str;
}

/// Registry of available response parsers
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn ResponseParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self;
    pub fn get(&self, parser_type: &str) -> Option<Arc<dyn ResponseParser>>;
    pub fn register(&mut self, name: String, parser: Arc<dyn ResponseParser>);
}
```

**Estimated Lines:** 80

**Test Count:** 5 tests
- Parser registry initialization
- Parser registration
- Parser lookup (found/not found)
- Multiple parser registration
- Clone/thread safety

---

#### `core/src/sources/parsers/weather.rs` (NEW)

**Purpose:** OpenWeatherMap current weather API parser

**Contents:**
```rust
use super::ResponseParser;
use crate::traits::TimeSeriesPoint;
use crate::error::CoreResult;
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OpenWeatherResponse {
    main: MainData,
    wind: WindData,
    clouds: CloudData,
    rain: Option<PrecipData>,
    snow: Option<PrecipData>,
    visibility: Option<i32>,
    dt: i64,
    timezone: i32,
}

pub struct WeatherParser;

impl ResponseParser for WeatherParser {
    fn name(&self) -> &'static str { "openweather_current" }

    fn parse(&self, response_body: &str, location_id: &str, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

**Estimated Lines:** 150

**Test Count:** 8 tests
- Parse valid weather response
- Parse response with missing optional fields
- Parse response with all fields present
- Handle malformed JSON
- Handle missing required fields
- Verify correct metric names
- Verify timestamp normalization
- Verify tag enrichment

---

#### `core/src/sources/parsers/air_pollution.rs` (NEW)

**Purpose:** OpenWeatherMap air pollution API parser

**Contents:**
```rust
use super::ResponseParser;
use crate::traits::TimeSeriesPoint;
use crate::error::CoreResult;
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AirPollutionResponse {
    list: Vec<AirPollutionReading>,
}

pub struct AirPollutionParser;

impl ResponseParser for AirPollutionParser {
    fn name(&self) -> &'static str { "openweather_air_pollution" }

    fn parse(&self, response_body: &str, location_id: &str, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

**Estimated Lines:** 120

**Test Count:** 6 tests
- Parse valid air pollution response
- Parse empty list
- Handle malformed JSON
- Verify all pollutants present
- Verify AQI value
- Verify tag enrichment

---

### 1.2 Source Manager

#### `core/src/coordinator/source_manager.rs` (NEW)

**Purpose:** Manage lifecycle of data sources

**Key Responsibilities:**
1. Spawn sources based on StreamConfig
2. Track active sources
3. Stop sources when stream disabled
4. Handle source failures and restarts

**Public API:**
```rust
pub struct SourceManager {
    sources: Arc<RwLock<HashMap<String, SourceHandle>>>,
    registry: Arc<StreamRegistry>,
}

impl SourceManager {
    pub fn new(registry: Arc<StreamRegistry>) -> Self;

    pub async fn spawn_source(
        &self,
        stream_config: StreamConfig,
        tx: Sender<TimeSeriesPoint>,
    ) -> CoreResult<()>;

    pub async fn stop_source(&self, stream_id: &str) -> CoreResult<()>;

    pub async fn list_active_sources(&self) -> Vec<String>;

    pub async fn health_check(&self, stream_id: &str) -> CoreResult<HealthStatus>;
}

struct SourceHandle {
    source_type: SourceType,
    handle: JoinHandle<()>,
    cancel_token: CancellationToken,
}
```

**Estimated Lines:** 250

**Test Count:** 12 tests (London TDD)
- Spawn MQTT source from config
- Spawn HTTP polling source from config
- Stop running source
- List active sources
- Health check for active source
- Health check for stopped source
- Handle source spawn failure
- Handle duplicate source spawn
- Stop non-existent source
- Restart failed source
- Concurrent spawn/stop operations
- Source isolation (one failure doesn't affect others)

**Dependencies:**
```toml
# core/Cargo.toml additions
tokio = { version = "1.0", features = ["full"] }
tokio-util = "0.7"  # For CancellationToken
```

---

### 1.3 Ingestion Coordinator

#### `core/src/coordinator/mod.rs` (NEW)

**Purpose:** Orchestrate entire ingestion pipeline

**Contents:**
```rust
pub mod source_manager;
pub mod router;  // Re-export from app for now

use source_manager::SourceManager;
use crate::traits::{Store, TimeSeriesPoint};
use config_client::StreamRegistry;

pub struct IngestionCoordinator {
    source_manager: Arc<SourceManager>,
    router: Arc<IngestionRouter>,
    storage: Arc<dyn Store>,
    tx: Sender<TimeSeriesPoint>,
    rx: Arc<Mutex<Receiver<TimeSeriesPoint>>>,
}

impl IngestionCoordinator {
    pub fn new(
        registry: Arc<StreamRegistry>,
        storage: Arc<dyn Store>,
    ) -> Self;

    pub async fn start(&self) -> CoreResult<()>;

    pub async fn stop(&self) -> CoreResult<()>;

    pub async fn reload_config(&self) -> CoreResult<()>;

    pub async fn health_check(&self) -> CoreResult<HealthStatus>;

    async fn routing_loop(&self);
}
```

**Estimated Lines:** 300

**Test Count:** 15 tests (London TDD)
- Initialize coordinator
- Start coordinator (spawns sources)
- Stop coordinator (stops all sources)
- Route point to correct storage
- Reload configuration
- Add new stream dynamically
- Remove stream dynamically
- Health check (all sources)
- Handle source failure (continues routing others)
- Handle router failure (dead letter queue)
- Handle storage failure (retry logic)
- Concurrent point routing
- Backpressure handling (channel full)
- Graceful shutdown (flush pending data)
- Integration with stream registry watch

**Mocks Required:**
- `MockStore` (already exists in traits.rs)
- `MockStreamRegistry`
- `MockSource`

---

## Phase 2: App Module Additions

### 2.1 Configuration Loading

#### `apps/air-quality-app/src/config_etcd.rs` (MODIFY)

**Changes Required:**
1. Add function to load weather stream configs
2. Add function to build `HttpPollingConfig` from multiple streams
3. Add environment variable expansion for query params

**New Functions:**
```rust
pub async fn load_weather_streams(
    etcd_endpoints: &[&str],
) -> Result<Vec<StreamConfig>, ConfigError>;

pub fn build_polling_config(
    streams: Vec<StreamConfig>,
) -> Result<HttpPollingConfig, ConfigError>;
```

**Estimated Lines:** +100

**Test Count:** 5 tests
- Load weather stream configs from etcd
- Build polling config from multiple streams
- Handle missing weather streams
- Expand environment variables in query params
- Validate weather stream config

---

### 2.2 Application Integration

#### `apps/air-quality-app/src/main.rs` (MODIFY)

**Changes Required:**
1. Initialize `IngestionCoordinator` instead of individual sources
2. Replace direct source spawning with coordinator
3. Add graceful shutdown for coordinator

**Before:**
```rust
// Old approach
let mqtt_source = MqttSource::new(mqtt_config)?;
mqtt_source.start(tx.clone()).await?;

let storage_writer = StorageWriter::new(rx, parquet_store);
storage_writer.start().await?;
```

**After:**
```rust
// New approach
let coordinator = IngestionCoordinator::new(stream_registry, parquet_store)?;
coordinator.start().await?;

// Coordinator handles:
// - Spawning all sources (MQTT, HTTP polling)
// - Routing to storage
// - Health checks
// - Config reload
```

**Estimated Lines:** +50, -30

**Test Count:** Integration tests only (Phase 4)

---

#### `apps/air-quality-app/src/coordinator/mod.rs` (MODIFY)

**Changes Required:**
1. Update exports to include new components
2. Add re-exports for core coordinator components

**New Exports:**
```rust
pub use neural_core::coordinator::{
    IngestionCoordinator,
    SourceManager,
};

pub use router::IngestionRouter;  // Keep app-specific router for now
```

**Estimated Lines:** +10

---

## Phase 3: Configuration Integration

### 3.1 Stream Registry Integration

#### Stream Configs in etcd

**Path:** `/streams/outdoor-weather/config`

**Contents:** (See architecture doc for full YAML)
```yaml
stream_id: "outdoor-weather"
sources:
  - type: "http_poll"
    enabled: true
    endpoint:
      id: "openweather-current"
      url: "https://api.openweathermap.org/data/2.5/weather"
      parser_type: "openweather_current"
      auth:
        type: "query_param"
        param_name: "appid"
        value_env: "OPENWEATHERMAP_API_KEY"
      query_params:
        lat: "${WEATHER_LATITUDE}"
        lon: "${WEATHER_LONGITUDE}"
        units: "metric"
```

**Path:** `/streams/outdoor-air-quality/config`

Similar structure with `parser_type: "openweather_air_pollution"`

---

### 3.2 Environment Variables

**File:** `deploy/pi/.env.example`

**New Variables:**
```bash
# OpenWeatherMap Configuration
OPENWEATHERMAP_API_KEY=your_api_key_here
WEATHER_LATITUDE=40.7128
WEATHER_LONGITUDE=-74.0060
```

---

## Phase 4: Integration Testing

### 4.1 End-to-End Test

**File:** `apps/air-quality-app/tests/integration_ingestion_coordinator.rs` (NEW)

**Test Scenarios:**
1. Start coordinator with multiple streams
2. Verify data flows from sources to storage
3. Test dynamic config reload (add/remove streams)
4. Test source failure recovery
5. Test graceful shutdown
6. Test health check aggregation

**Estimated Lines:** 400

**Test Count:** 8 integration tests

**Test Environment:**
- Mock etcd server (testcontainers)
- Mock HTTP server for OpenWeatherMap (wiremock)
- Temporary file system for Parquet storage
- Test MQTT broker (rumqttd)

---

### 4.2 Performance Test

**File:** `apps/air-quality-app/tests/performance_ingestion.rs` (NEW)

**Metrics to Measure:**
1. Throughput (points/second)
2. Latency (source → storage)
3. Memory usage under load
4. Channel backpressure behavior
5. CPU utilization

**Test Load:**
- 1000 points/second for 60 seconds
- 10 concurrent sources
- Verify <100ms p95 latency

**Estimated Lines:** 200

---

## Dependencies and Integration Points

### Dependency Graph

```
┌─────────────────────────────────────────┐
│         IngestionCoordinator            │
│  (apps/air-quality-app/src/coordinator) │
└──────────────┬──────────────────────────┘
               │
               ├─ Depends on ─┐
               │               │
               ▼               ▼
┌────────────────────┐  ┌────────────────────┐
│   SourceManager    │  │  IngestionRouter   │
│   (core/coord)     │  │  (app/coord)       │
└─────────┬──────────┘  └──────────┬─────────┘
          │                        │
          ├─ Spawns ─┐             │
          │          │             │
          ▼          ▼             ▼
    ┌─────────┐  ┌──────────┐  ┌─────────┐
    │ Mqtt    │  │  Http    │  │ Parquet │
    │ Source  │  │ Polling  │  │ Store   │
    └─────────┘  └──────────┘  └─────────┘
```

### Critical Interfaces

| Interface | Provider | Consumer |
|-----------|----------|----------|
| `Source` trait | MqttSource, HttpPollingSource | SourceManager |
| `Store` trait | ParquetStore | IngestionCoordinator |
| `StreamConfig` | config-client | SourceManager |
| `TimeSeriesPoint` | All sources | Router, Storage |
| `ResponseParser` | WeatherParser, AirPollutionParser | HttpPollingSource |

---

## Test Strategy

### London School TDD Approach

**Principles:**
1. Test behavior, not implementation
2. Mock all dependencies
3. Verify interactions (method calls, order, arguments)
4. Test one component at a time
5. Build outside-in (high-level first)

### Test Pyramid

```
         ╱╲
        ╱  ╲
       ╱ E2E╲          8 tests (integration)
      ╱──────╲
     ╱        ╲
    ╱  Unit    ╲       60+ tests (London TDD)
   ╱────────────╲
  ╱              ╲
 ╱   Component    ╲    40+ tests (contract tests)
╱──────────────────╲
```

**Test Counts:**
- Unit tests (mocked): 60+ tests
- Component tests (real dependencies): 40+ tests
- Integration tests (full system): 8 tests

**Total Estimated Test Count:** 108+ tests

---

### Test File Structure

```
core/
├── src/
│   ├── coordinator/
│   │   ├── mod.rs                  # 15 tests (IngestionCoordinator)
│   │   └── source_manager.rs      # 12 tests (SourceManager)
│   ├── sources/
│   │   ├── http_poll.rs            # 20 tests (generic polling + retry)
│   │   └── parsers/
│   │       ├── mod.rs              # 5 tests (ParserRegistry)
│   │       ├── weather.rs          # 8 tests (WeatherParser)
│   │       └── air_pollution.rs   # 6 tests (AirPollutionParser)

apps/air-quality-app/
├── src/
│   ├── config_etcd.rs              # 5 tests (config loading)
│   └── coordinator/
│       └── router.rs               # Existing tests
├── tests/
│   ├── integration_ingestion_coordinator.rs  # 8 tests
│   └── performance_ingestion.rs              # 2 tests
```

---

### Mock Strategy

**Mocks Required:**

1. **MockStore** (already exists in `core/src/traits.rs`)
   - Mock `write()`, `write_batch()`
   - Verify storage calls

2. **MockStreamRegistry** (to create)
   - Mock `load_stream()`, `list_streams()`
   - Simulate config changes

3. **MockSource** (already exists in `core/src/traits.rs`)
   - Mock `fetch()`, `health_check()`
   - Simulate source failures

4. **MockHttpClient** (wiremock)
   - Mock API responses
   - Simulate network errors, rate limits

5. **MockMqttBroker** (rumqttd or testcontainers)
   - Simulate MQTT messages
   - Test reconnection logic

---

## Implementation Sequence

### Week 1: Core Foundations

**Day 1-2: Generic HTTP Polling**
- [ ] Write tests for `ResponseParser` trait
- [ ] Implement `ResponseParser` trait
- [ ] Write tests for `ParserRegistry`
- [ ] Implement `ParserRegistry`
- [ ] Write tests for `AuthMethod` enum
- [ ] Implement `AuthMethod` enum
- [ ] Write tests for `RetryConfig` and retry logic
- [ ] Implement retry logic with exponential backoff

**Day 3-4: Parser Implementations**
- [ ] Write tests for `WeatherParser`
- [ ] Implement `WeatherParser`
- [ ] Write tests for `AirPollutionParser`
- [ ] Implement `AirPollutionParser`
- [ ] Refactor `HttpPollingSource` to use parsers
- [ ] Write tests for refactored `HttpPollingSource`

**Day 5: SourceManager**
- [ ] Write tests for `SourceManager`
- [ ] Implement `SourceManager.spawn_source()`
- [ ] Implement `SourceManager.stop_source()`
- [ ] Implement `SourceManager.list_active_sources()`
- [ ] Implement `SourceManager.health_check()`

---

### Week 2: Coordinator and Integration

**Day 1-2: IngestionCoordinator**
- [ ] Write tests for `IngestionCoordinator`
- [ ] Implement `IngestionCoordinator.new()`
- [ ] Implement `IngestionCoordinator.start()`
- [ ] Implement `IngestionCoordinator.routing_loop()`
- [ ] Implement `IngestionCoordinator.stop()`
- [ ] Implement `IngestionCoordinator.reload_config()`
- [ ] Implement `IngestionCoordinator.health_check()`

**Day 3: Configuration Integration**
- [ ] Write tests for weather stream config loading
- [ ] Implement `load_weather_streams()` in `config_etcd.rs`
- [ ] Implement `build_polling_config()`
- [ ] Add environment variable expansion
- [ ] Create stream config YAML files

**Day 4: Application Integration**
- [ ] Update `main.rs` to use `IngestionCoordinator`
- [ ] Update `coordinator/mod.rs` exports
- [ ] Add graceful shutdown logic
- [ ] Test manual startup/shutdown

**Day 5: Integration Testing**
- [ ] Write integration tests
- [ ] Set up test environment (etcd, wiremock, MQTT)
- [ ] Run end-to-end tests
- [ ] Fix integration issues

---

### Week 3: Testing and Refinement

**Day 1-2: Full Test Pass**
- [ ] Run all unit tests (target: 100% pass rate)
- [ ] Run all integration tests
- [ ] Measure test coverage (target: >80%)
- [ ] Fix failing tests

**Day 3: Performance Testing**
- [ ] Write performance tests
- [ ] Run load tests
- [ ] Measure throughput and latency
- [ ] Optimize bottlenecks

**Day 4: Documentation**
- [ ] Update API documentation (rustdoc)
- [ ] Write deployment guide
- [ ] Write operator runbook
- [ ] Update ARCHITECTURE.md

**Day 5: Code Review and Cleanup**
- [ ] Run clippy (fix all warnings)
- [ ] Run rustfmt
- [ ] Code review
- [ ] Final testing

---

## Key Decisions and Memory Coordination

### Memory Coordination via Hooks

After each major component is implemented, store decisions in memory:

```bash
# After implementing ResponseParser trait
npx claude-flow@alpha hooks post-edit \
  --file "core/src/sources/parsers/mod.rs" \
  --memory-key "air005/plan/responseparser" \
  --value "Implemented ResponseParser trait with ParserRegistry for pluggable HTTP API parsing"

# After implementing SourceManager
npx claude-flow@alpha hooks post-edit \
  --file "core/src/coordinator/source_manager.rs" \
  --memory-key "air005/plan/sourcemanager" \
  --value "Implemented SourceManager with spawn/stop lifecycle, health tracking, and failure recovery"

# After implementing IngestionCoordinator
npx claude-flow@alpha hooks post-edit \
  --file "core/src/coordinator/mod.rs" \
  --memory-key "air005/plan/coordinator" \
  --value "Implemented IngestionCoordinator orchestrating SourceManager + IngestionRouter + Storage with config reload"
```

---

## Risk Mitigation

### High-Risk Areas

1. **Channel Backpressure**
   - Risk: Channel fills up, sources block or drop data
   - Mitigation: Monitor channel occupancy, configurable buffer size, dead letter queue

2. **Source Failure Cascade**
   - Risk: One source failure affects others
   - Mitigation: Isolate sources in separate tasks, use `CancellationToken`

3. **Config Reload Race Conditions**
   - Risk: Config reloads while sources are starting/stopping
   - Mitigation: Use RwLock, coordinate state transitions

4. **API Rate Limiting**
   - Risk: Exceed OpenWeatherMap rate limits
   - Mitigation: Respect Retry-After header, implement exponential backoff

5. **Memory Leaks**
   - Risk: Sources don't clean up on shutdown
   - Mitigation: Use RAII patterns, drop guards, thorough testing

---

## Success Criteria

**Unit Tests:**
- [ ] All tests pass (108+ tests)
- [ ] Code coverage >80%
- [ ] No clippy warnings

**Integration Tests:**
- [ ] End-to-end data flow works
- [ ] Config reload doesn't drop data
- [ ] Graceful shutdown flushes all data

**Performance:**
- [ ] Handle 1000 points/second
- [ ] p95 latency <100ms
- [ ] Memory usage <250MB

**Operational:**
- [ ] Health check returns accurate status
- [ ] All errors logged with context
- [ ] Metrics exported for all components

---

## File Checklist

### Files to Create (NEW)

- [ ] `core/src/coordinator/mod.rs`
- [ ] `core/src/coordinator/source_manager.rs`
- [ ] `core/src/sources/parsers/mod.rs`
- [ ] `core/src/sources/parsers/weather.rs`
- [ ] `core/src/sources/parsers/air_pollution.rs`
- [ ] `apps/air-quality-app/tests/integration_ingestion_coordinator.rs`
- [ ] `apps/air-quality-app/tests/performance_ingestion.rs`
- [ ] `docs/procedures/AIR-005_DEPLOYMENT_GUIDE.md`
- [ ] `docs/procedures/AIR-005_OPERATOR_RUNBOOK.md`

### Files to Modify (MODIFY)

- [ ] `core/src/sources/http_poll.rs` (+250, -55 lines)
- [ ] `core/src/sources/mod.rs` (+3 lines, add parsers module)
- [ ] `core/src/lib.rs` (+2 lines, export coordinator)
- [ ] `apps/air-quality-app/src/config_etcd.rs` (+100 lines)
- [ ] `apps/air-quality-app/src/main.rs` (+50, -30 lines)
- [ ] `apps/air-quality-app/src/coordinator/mod.rs` (+10 lines)

### Configuration Files to Create

- [ ] `config/streams/outdoor-weather.yaml`
- [ ] `config/streams/outdoor-air-quality.yaml`
- [ ] `deploy/pi/.env.example` (add weather vars)

---

## Appendix: Code Structure Summary

### Core Module Structure

```
core/src/
├── coordinator/
│   ├── mod.rs                      # IngestionCoordinator (300 lines)
│   └── source_manager.rs           # SourceManager (250 lines)
├── sources/
│   ├── mod.rs                      # Module exports
│   ├── http_poll.rs                # Generic HTTP polling (450 lines, refactored)
│   ├── mqtt.rs                     # Existing MQTT source
│   └── parsers/
│       ├── mod.rs                  # ResponseParser trait + ParserRegistry (80 lines)
│       ├── weather.rs              # WeatherParser (150 lines)
│       └── air_pollution.rs        # AirPollutionParser (120 lines)
├── storage/
│   ├── mod.rs
│   ├── parquet.rs                  # Existing ParquetStore
│   └── wal.rs                      # Existing WAL
├── types/
│   ├── mod.rs
│   ├── stream_config.rs            # Existing StreamConfig
│   └── stream_record.rs            # Existing StreamRecord
└── traits.rs                       # Existing Source, Store traits
```

### App Module Structure

```
apps/air-quality-app/src/
├── main.rs                         # Application entry (modify)
├── config_etcd.rs                  # Config loading (modify)
├── coordinator/
│   ├── mod.rs                      # Coordinator module (modify)
│   └── router.rs                   # Existing IngestionRouter
├── api/
│   └── ...                         # Existing API routes
└── ...
```

---

## Conclusion

This implementation plan provides a comprehensive roadmap for building the AIR-005 IngestionCoordinator feature. The plan follows London School TDD principles, ensures proper test coverage, and maintains backward compatibility with existing code.

**Key Takeaways:**
1. Refactor `HttpPollingSource` to be generic and configuration-driven
2. Build `SourceManager` to handle dynamic source lifecycle
3. Build `IngestionCoordinator` to orchestrate the entire pipeline
4. Integrate with existing `IngestionRouter` and `ParquetStore`
5. Test thoroughly with 108+ tests covering unit, component, and integration levels

**Next Steps:**
1. Review this plan with team
2. Begin Week 1 implementation (Generic HTTP Polling)
3. Use hooks to coordinate progress and decisions
4. Proceed with TDD: write tests first, then implementation

---

**Document Version:** 1.0.0
**Last Updated:** 2025-12-16
**Author:** Implementation Planner Agent
