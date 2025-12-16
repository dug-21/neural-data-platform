# Component Dependency Map

**Version**: 1.2.0
**Last Updated**: 2025-12-16
**Status**: Current (Updated for AIR-005 HTTP Polling + Coordinator Integration)

---

## Overview

This document provides a visual map of all components in the Neural Data Platform, their dependencies, and interaction patterns. Use this as a reference when:

- Understanding the system architecture
- Planning new features
- Debugging issues
- Onboarding new team members

---

## Crate Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           APPLICATION LAYER                                  │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    air-quality-app                                   │   │
│  │                  (apps/air-quality-app/)                            │   │
│  │                                                                      │   │
│  │  Responsibilities:                                                   │   │
│  │  - HTTP API server (axum)                                           │   │
│  │  - MQTT ingestion pipeline                                          │   │
│  │  - Configuration loading                                            │   │
│  │  - Service orchestration                                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│            │                    │                    │                       │
│            │ depends on         │ depends on         │ depends on            │
│            ▼                    ▼                    ▼                       │
└────────────┼────────────────────┼────────────────────┼───────────────────────┘
             │                    │                    │
┌────────────┼────────────────────┼────────────────────┼───────────────────────┐
│            │        LIBRARY LAYER                    │                       │
│            │                    │                    │                       │
│  ┌─────────▼─────────┐  ┌──────▼──────────┐  ┌─────▼──────────────┐        │
│  │   neural-core     │  │  config-client  │  │   External Crates  │        │
│  │     (core/)       │  │(config-client/) │  │                    │        │
│  │                   │  │                 │  │  - axum            │        │
│  │ - TimeSeriesPoint │  │ - ConfigClient  │  │  - tokio           │        │
│  │ - StreamConfig    │  │ - StreamRegistry│  │  - serde           │        │
│  │ - MqttSource      │  │ - Watch API     │  │  - tracing         │        │
│  │ - ParquetStore    │  │                 │  │  - parquet         │        │
│  │ - Source trait    │  │                 │  │  - rumqttc         │        │
│  │ - Store trait     │  │                 │  │  - chrono          │        │
│  └─────────┬─────────┘  └────────┬────────┘  └────────────────────┘        │
│            │                     │                                          │
│            │                     │ depends on                               │
│            │                     ▼                                          │
│            │            ┌────────────────┐                                  │
│            │            │  etcd-client   │                                  │
│            │            │  (external)    │                                  │
│            │            └────────────────┘                                  │
│            │                                                                │
│            │ re-exports                                                     │
│            ▼                                                                │
│  ┌───────────────────────────────────────────────────────────────────┐     │
│  │                    neural-core (core/)                             │     │
│  │                                                                    │     │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐  │     │
│  │  │  types/    │  │  sources/  │  │  storage/  │  │  traits.rs │  │     │
│  │  │            │  │            │  │            │  │            │  │     │
│  │  │ TimeSeries │  │ MqttSource │  │ ParquetStor│  │ Source     │  │     │
│  │  │ Point      │  │ MqttConfig │  │ WalWriter  │  │ Store      │  │     │
│  │  │ StreamConf │  │            │  │            │  │ Forecast   │  │     │
│  │  │ SchemaField│  │            │  │            │  │ HealthStat │  │     │
│  │  │ SourceConf │  │            │  │            │  │            │  │     │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘  │     │
│  └───────────────────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Module Breakdown: air-quality-app

```
apps/air-quality-app/
├── src/
│   ├── main.rs                 # Entry point, orchestration
│   ├── lib.rs                  # Library exports
│   │
│   ├── config.rs               # AppConfig struct (YAML loading)
│   ├── config_etcd.rs          # EtcdAppConfig (etcd loading)
│   ├── stream_integration.rs   # StreamConfig → AppConfig conversion
│   ├── error.rs                # Error types
│   ├── response.rs             # API response types
│   │
│   ├── api/                    # HTTP API layer
│   │   ├── mod.rs              # Router setup
│   │   ├── routes.rs           # Route definitions, AppServices
│   │   └── handlers/           # Request handlers
│   │       ├── mod.rs
│   │       ├── health.rs       # GET /health
│   │       ├── readings.rs     # GET /api/v1/air-quality/*
│   │       ├── forecast.rs     # GET /api/v1/forecast/*
│   │       ├── alerts.rs       # Alerting endpoints
│   │       └── locations.rs    # Location management
│   │
│   ├── ingestion/              # Data ingestion layer
│   │   ├── mod.rs
│   │   └── mqtt_handler.rs     # MqttHandler (MQTT → Channel)
│   │
│   ├── pipeline/               # Data pipeline layer
│   │   ├── mod.rs
│   │   └── storage_writer.rs   # StorageWriter (Channel → Parquet)
│   │
│   ├── coordinator/            # Multi-stream coordination (AIR-004/005)
│   │   ├── mod.rs
│   │   ├── router.rs               # Stream routing with schema validation
│   │   ├── source_manager.rs       # Source lifecycle management (AIR-005)
│   │   └── ingestion_coordinator.rs # Main coordinator orchestration (AIR-005)
│   │
│   └── mcp/                    # MCP server (optional)
│       ├── mod.rs
│       ├── server.rs
│       └── tools.rs
│
├── Cargo.toml                  # Dependencies
└── config.yaml                 # Default configuration
```

---

## Module Breakdown: neural-core

```
core/
├── src/
│   ├── lib.rs                  # Public exports
│   │
│   ├── types/                  # Domain types
│   │   ├── mod.rs
│   │   ├── air_quality.rs      # AirQualityReading (legacy)
│   │   ├── stream_config.rs    # StreamConfig, SchemaField, SourceConfig
│   │   └── stream_record.rs    # StreamRecord (future)
│   │
│   ├── sources/                # Data source adapters
│   │   ├── mod.rs
│   │   ├── mqtt.rs             # MqttSource, MqttConfig
│   │   ├── http_poll.rs        # HttpPollingSource, ResponseParser trait ✅
│   │   └── parsers/            # HTTP response parsers ✅
│   │       ├── mod.rs
│   │       ├── weather.rs      # OpenWeatherMap current weather ✅
│   │       └── air_pollution.rs # OpenWeatherMap air pollution ✅
│   │
│   ├── storage/                # Storage adapters
│   │   ├── mod.rs
│   │   └── parquet.rs          # ParquetStore, WalWriter
│   │
│   ├── coordinator/            # Multi-stream coordination (AIR-005)
│   │   ├── mod.rs
│   │   ├── source_manager.rs   # Lifecycle management for all source types
│   │   └── ingestion_coordinator.rs # Main orchestration component
│   │
│   ├── traits.rs               # Port interfaces (Source, Store, Forecast)
│   └── error.rs                # CoreError enum
│
└── Cargo.toml
```

---

## Module Breakdown: config-client

```
config-client/
├── src/
│   ├── lib.rs                  # Public exports
│   ├── client.rs               # ConfigClient (etcd wrapper)
│   ├── watch.rs                # WatchHandle (hot-reload)
│   ├── error.rs                # ConfigError
│   │
│   └── stream/                 # Stream registry
│       ├── mod.rs
│       └── registry.rs         # StreamRegistry
│
└── Cargo.toml
```

---

## Data Flow Diagram

```
                    ┌─────────────────────┐
                    │  AirGradient Sensor │
                    │    (External IoT)   │
                    └──────────┬──────────┘
                               │
                               │ MQTT publish
                               │ topic: airgradient/readings/+
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          INFRASTRUCTURE                                   │
│                                                                           │
│  ┌─────────────────────────┐      ┌─────────────────────────┐           │
│  │       mosquitto         │      │          etcd           │           │
│  │     (MQTT Broker)       │      │    (Config Store)       │           │
│  │                         │      │                         │           │
│  │  Port: 1883             │      │  Port: 2379             │           │
│  │  Memory: 128MB          │      │  Memory: 256MB          │           │
│  └───────────┬─────────────┘      └───────────┬─────────────┘           │
│              │                                │                          │
└──────────────┼────────────────────────────────┼──────────────────────────┘
               │                                │
               │ subscribe                      │ get/watch
               ▼                                ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        air-quality-app                                    │
│                                                                           │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                     CONFIGURATION LOADING                          │ │
│  │  main.rs → load_from_stream_config() → ConfigClient → etcd         │ │
│  │         ↓                                                          │ │
│  │  Fallback: load_from_etcd() → config.yaml → defaults               │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                               │                                          │
│                               │ AppConfig                                │
│                               ▼                                          │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                     INGESTION PIPELINE                              │ │
│  │                                                                     │ │
│  │  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐          │ │
│  │  │ MqttHandler │ ──► │mpsc channel │ ──► │StorageWriter│          │ │
│  │  │             │     │(buffer:1000)│     │             │          │ │
│  │  │  - fetch()  │     │             │     │ - batch:100 │          │ │
│  │  │  - parse    │     │TimeSeriesPt │     │ - timeout:5s│          │ │
│  │  └─────────────┘     └─────────────┘     └──────┬──────┘          │ │
│  │                                                  │                  │ │
│  │  ┌─────────────────────────────────────────────────────────────┐   │ │
│  │  │              HTTP POLLING PIPELINE ✅ IMPLEMENTED           │   │ │
│  │  │                                                             │   │ │
│  │  │  ┌─────────────────┐     ┌─────────────────┐              │   │ │
│  │  │  │HttpPollingSource│ ──► │ ParserRegistry  │              │   │ │
│  │  │  │                 │     │ - WeatherParser │              │   │ │
│  │  │  │ - poll interval │     │ - AirPollution  │              │   │ │
│  │  │  │ - retry logic   │     │   Parser        │              │   │ │
│  │  │  │ - auth methods  │     │                 │              │   │ │
│  │  │  └───────┬─────────┘     └─────────────────┘              │   │ │
│  │  │          │                                                 │   │ │
│  │  │          │ send(TimeSeriesPoint)                          │   │ │
│  │  │          └────────────────────────────────────────────────┼───┘ │
│  │  │                                                            │     │
│  │  └────────────────────────────────────────────────────────────┼─────┘
│  │                                                                │      │
│  └────────────────────────────────────────────────────────────────┼──────┘
│                                                     │                    │
│                                                     │ write_batch        │
│                                                     ▼                    │
│  ┌────────────────────────────────────────────────────────────────────┐ │
│  │                      STORAGE LAYER                                  │ │
│  │                                                                     │ │
│  │  ┌─────────────┐     ┌─────────────┐                               │ │
│  │  │ParquetStore │ ──► │  WalWriter  │ (crash recovery)              │ │
│  │  │             │     │             │                               │ │
│  │  │ - write()   │     │ - append()  │                               │ │
│  │  │ - query()   │     │ - replay()  │                               │ │
│  │  └──────┬──────┘     └─────────────┘                               │ │
│  │         │                                                           │ │
│  └─────────┼───────────────────────────────────────────────────────────┘ │
│            │                                                              │
│            │                    ┌───────────────────────────────────────┐│
│            │                    │          HTTP API (axum)              ││
│            │                    │                                       ││
│            │ query              │  GET /health                          ││
│            └───────────────────►│  GET /api/v1/air-quality/latest       ││
│                                 │  GET /api/v1/air-quality/history      ││
│                                 │  GET /api/v1/forecast/*               ││
│                                 │  Port: 8080                           ││
│                                 └───────────────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────────┘
                               │
                               │ Parquet files
                               ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                        BRONZE LAYER (Storage)                             │
│                                                                           │
│  Volume: air-quality-data                                                │
│  Path: /data/                                                            │
│                                                                           │
│  /data/2025-12-16_00.parquet                                             │
│  /data/2025-12-16_01.parquet                                             │
│  /data/wal/                    (write-ahead log)                         │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Interface Contracts

### Source → Channel (TimeSeriesPoint)

```rust
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub fields: HashMap<String, f64>,    // Measurements
    pub tags: HashMap<String, String>,   // Metadata
}
```

### Channel → StorageWriter (Batch)

```rust
// StorageWriter receives TimeSeriesPoint via mpsc
// Accumulates batch_size points OR timeout
// Calls store.write(&batch)
```

### Store Interface (ParquetStore)

```rust
#[async_trait]
pub trait Store: Send + Sync {
    async fn write(&self, points: &[TimeSeriesPoint]) -> Result<(), CoreError>;
    async fn query(&self, filter: QueryFilter) -> Result<Vec<TimeSeriesPoint>, CoreError>;
}
```

### ConfigClient Interface

```rust
impl ConfigClient {
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError>;
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError>;
    pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>;
}
```

---

## External Dependencies

### Runtime Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| tokio | 1.x | Async runtime |
| axum | 0.7 | HTTP framework |
| rumqttc | 0.23 | MQTT client |
| etcd-client | 0.13 | etcd client |
| parquet | 52 | Parquet I/O |
| serde | 1.0 | Serialization |
| tracing | 0.1 | Logging |
| chrono | 0.4 | Date/time |

### Infrastructure Dependencies

| Service | Version | Purpose |
|---------|---------|---------|
| mosquitto | 2.0 | MQTT broker |
| etcd | 3.5.11 | Config store |
| Docker | 24+ | Container runtime |

---

## Key Patterns

### 1. Channel-Based Pipeline

```
Producer → mpsc::channel → Consumer
```

Used for decoupling ingestion from storage, enabling backpressure.

### 2. Trait-Based Abstraction (Ports)

```rust
trait Source { ... }
trait Store { ... }
```

Enables swapping implementations without changing business logic.

### 3. Fallback Configuration

```
StreamRegistry → Legacy etcd → YAML → Defaults
```

Graceful degradation when configuration sources unavailable.

### 4. WAL for Crash Recovery

```
write() → WAL append → Parquet flush
startup → WAL replay → continue
```

Ensures no data loss on crash.

---

## Recent Additions (AIR-005)

### HTTP Polling Source ✅ IMPLEMENTED
**Location**: `core/src/sources/http_poll.rs`
**Features**:
- Generic HTTP polling with configurable endpoints
- ResponseParser trait for pluggable parsers
- Flexible authentication (None, QueryParam, Header, Bearer)
- Retry logic with exponential backoff and jitter
- ParserRegistry for parser management

### OpenWeatherMap Parsers ✅ IMPLEMENTED

**WeatherParser** (`core/src/sources/parsers/weather.rs`):
- Parses Current Weather API responses
- Extracts: temperature, feels_like, pressure, humidity, wind metrics, clouds, visibility, precipitation
- Comprehensive unit tests

**AirPollutionParser** (`core/src/sources/parsers/air_pollution.rs`):
- Parses Air Pollution API responses
- Extracts: AQI, CO, NO, NO2, O3, SO2, PM2.5, PM10, NH3
- Comprehensive unit tests

### Stream Configurations ✅ CREATED

**outdoor-weather.yaml** (`config/streams/outdoor-weather.yaml`):
- 11 weather metrics with ranges and units
- 10-minute poll interval
- Parquet storage with 90-day retention

**outdoor-air-quality.yaml** (`config/streams/outdoor-air-quality.yaml`):
- 9 air quality metrics
- 10-minute poll interval
- Parquet storage with 90-day retention

---

## References

- [PLATFORM_ARCHITECTURE_OVERVIEW.md](./PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [HOW_TO_ADD_NEW_SOURCE.md](../procedures/HOW_TO_ADD_NEW_SOURCE.md)
- [HOW_TO_ADD_NEW_STREAM.md](../procedures/HOW_TO_ADD_NEW_STREAM.md)
- [AIR-005_INGESTION_COORDINATOR_DESIGN.md](./AIR-005_INGESTION_COORDINATOR_DESIGN.md)
- [HTTP_POLLING_SOURCE_REFACTOR.md](./HTTP_POLLING_SOURCE_REFACTOR.md)
