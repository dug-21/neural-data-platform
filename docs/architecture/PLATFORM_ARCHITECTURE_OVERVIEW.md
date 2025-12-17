# Neural Data Platform - Architecture Overview

**Version**: 1.2.0
**Last Updated**: 2025-12-16
**Status**: Production (Raspberry Pi 5) + External Data Integration (AIR-005 Complete)

---

## Executive Summary

The Neural Data Platform is a multi-stream data ingestion and storage system designed for edge deployment on Raspberry Pi. It evolved through four development phases (AIR-001 through AIR-004) from a single-purpose air quality monitor to a generic, extensible platform supporting multiple data streams from heterogeneous sources.

### Key Characteristics

- **Edge-First**: Designed for resource-constrained deployment (Raspberry Pi 5, <1GB memory)
- **Domain Adapter Pattern**: Pluggable sources and storage backends via trait abstractions
- **Configuration-Driven**: etcd-based configuration with hot-reload capability
- **Stream Registry**: Dynamic stream management without code changes
- **Dual-Layer Storage**: Bronze (Parquet) for raw data, Silver (TimescaleDB) for queries

---

## Architecture Evolution

### AIR-001: Foundation
- Basic Rust application structure
- Hexagonal architecture with ports and adapters
- Core traits: `Source`, `Store`, `Forecast`
- Initial MQTT ingestion from AirGradient sensors

### AIR-002: Pipeline Maturation
- Channel-based async pipeline (tokio mpsc)
- `MqttHandler` → Channel → `StorageWriter` pattern
- Parquet storage with WAL for crash recovery
- Batch processing with configurable size/timeout

### AIR-003: Configuration Management
- etcd-based distributed configuration
- Thin Rust wrapper (`config-client`, ~260 LOC)
- Watch API for hot-reload
- GitOps sync pattern (YAML → etcd via ConfigSyncService)
- Environment variable overrides

### AIR-004: Multi-Stream Platform
- Stream Registry in etcd (`/streams/{id}/config`)
- Generic `StreamConfig` type with schema validation
- Multiple source types: MQTT, HTTP polling, Webhooks
- `IngestionCoordinator` for multi-stream orchestration
- Fallback chain: StreamRegistry → Legacy etcd → config.yaml → defaults

### AIR-005: External Data Integration ✅ COMPLETE
- Generic HTTP polling source with pluggable parsers
- ResponseParser trait for extensible data sources
- Flexible authentication (API keys, headers, bearer tokens)
- Retry logic with exponential backoff and jitter
- OpenWeatherMap integration:
  - Current Weather API (temperature, wind, precipitation)
  - Air Pollution API (AQI, PM2.5, PM10, gases)
- Stream configurations for outdoor weather and air quality data
- IngestionCoordinator for multi-stream orchestration
- SourceManager for source lifecycle management
- IngestionRouter for schema validation and dead-letter queue

---

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         STREAM REGISTRY (etcd)                          │
│                                                                         │
│  /streams/air-quality/config    → Stream metadata, retention, schema    │
│  /streams/air-quality/sources   → Source configurations (MQTT, HTTP)    │
│  /config/air-quality/*          → Legacy app configuration              │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ watch + load
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          air-quality-app                                │
│                         (Rust, single binary)                           │
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    │
│  │  MqttHandler    │    │  HttpPoller     │    │  WebhookHandler │    │
│  │  (Source Trait) │    │  (Source Trait) │    │  (Source Trait) │    │
│  └────────┬────────┘    └────────┬────────┘    └────────┬────────┘    │
│           └──────────────────────┼──────────────────────┘              │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │   tokio mpsc channel      │                       │
│                    │   (TimeSeriesPoint flow)  │                       │
│                    └─────────────┬─────────────┘                       │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │     StorageWriter         │                       │
│                    │   - Batch accumulation    │                       │
│                    │   - Timeout-based flush   │                       │
│                    └─────────────┬─────────────┘                       │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │      ParquetStore         │                       │
│                    │   - Append-only writes    │                       │
│                    │   - WAL for recovery      │                       │
│                    └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        BRONZE LAYER (Parquet)                           │
│                                                                         │
│    /data/air-quality/2025-12-16_00.parquet                             │
│    /data/air-quality/2025-12-16_01.parquet                             │
│    ...                                                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Domain Adapter Pattern

The platform uses a **Domain Adapter Pattern** (variant of Hexagonal Architecture) where:

1. **Core Domain**: `TimeSeriesPoint`, `StreamConfig`, business logic
2. **Ports**: Trait definitions (`Source`, `Store`, `Forecast`)
3. **Adapters**: Concrete implementations (`MqttSource`, `ParquetStore`, `ConfigClient`)

### Core Traits

```rust
// Source Port - All data sources implement this
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}

// Store Port - All storage backends implement this
#[async_trait]
pub trait Store: Send + Sync {
    async fn write(&self, points: &[TimeSeriesPoint]) -> Result<(), CoreError>;
    async fn query(&self, filter: QueryFilter) -> Result<Vec<TimeSeriesPoint>, CoreError>;
}
```

### Current Adapters

| Port | Adapter | Location | Status |
|------|---------|----------|--------|
| Source | `MqttSource` | `neural-core/src/sources/mqtt.rs` | ✅ Production |
| Source | `HttpPollingSource` | `neural-core/src/sources/http_poll.rs` | ✅ Implemented |
| Parser | `WeatherParser` | `neural-core/src/sources/parsers/weather.rs` | ✅ Implemented |
| Parser | `AirPollutionParser` | `neural-core/src/sources/parsers/air_pollution.rs` | ✅ Implemented |
| Store | `ParquetStore` | `neural-core/src/storage/parquet.rs` | ✅ Production |
| Config | `ConfigClient` | `config-client/src/client.rs` | ✅ Production |
| Config | `StreamRegistry` | `config-client/src/stream/registry.rs` | ✅ Production |
| Config | `ConfigSyncService` | `apps/air-quality-app/src/config_sync/service.rs` | ✅ Production |

---

## Component Dependency Map

```
┌──────────────────────────────────────────────────────────────────┐
│                        air-quality-app                           │
│                      (apps/air-quality-app/)                     │
└──────────────────────────────────────────────────────────────────┘
            │                    │                    │
            ▼                    ▼                    ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│   neural-core    │  │  config-client   │  │      axum        │
│   (core/)        │  │ (config-client/) │  │   (HTTP API)     │
└──────────────────┘  └──────────────────┘  └──────────────────┘
         │                    │
         │                    ▼
         │            ┌──────────────────┐
         │            │   etcd-client    │
         │            │   (external)     │
         │            └──────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────┐
│                         neural-core                               │
├──────────────────┬──────────────────┬──────────────────┬─────────┤
│ types/           │ sources/         │ storage/         │ traits/ │
│ - TimeSeriesPoint│ - MqttSource     │ - ParquetStore   │ - Source│
│ - StreamConfig   │ - MqttConfig     │ - WalWriter      │ - Store │
│ - SchemaField    │                  │                  │ - Fore- │
│ - SourceConfig   │                  │                  │   cast  │
└──────────────────┴──────────────────┴──────────────────┴─────────┘
```

---

## Configuration Hierarchy

The application loads configuration with this priority (highest to lowest):

1. **Stream Registry** (`/streams/{id}/config` in etcd)
2. **Legacy etcd** (`/air-quality/*` paths)
3. **config.yaml** (file-based fallback)
4. **Defaults** (hardcoded in code)

### GitOps Configuration Sync (AIR-005)

Stream configurations are managed via GitOps YAML files and automatically synced to etcd on application startup:

```
config/base/streams/
├── air-quality/
│   └── config.yaml        → Indoor air quality stream
├── outdoor-weather/
│   └── config.yaml        → OpenWeatherMap weather data
└── outdoor-air-quality/
    └── config.yaml        → OpenWeatherMap air pollution data
```

**ConfigSyncService** handles the synchronization:
1. Discovers `config.yaml` files in `config/base/streams/` subdirectories
2. Parses YAML to `StreamConfig` structs with validation
3. Saves each config to etcd via `StreamRegistry::save_stream()`
4. Runs automatically at application startup

**Sync Methods**:
- **Automatic**: Application startup triggers `ConfigSyncService.sync_all()`
- **Manual**: `ETCD_CONTAINER=etcd ./scripts/sync-config-to-etcd.sh production`
- **Deployment**: `./deploy/pi/deploy.sh sync` or `./deploy/pi/deploy.sh init-streams`

**Environment Variable Expansion**:
Source configurations support environment variable references:
```yaml
auth_value: "${OPENWEATHERMAP_API_KEY}"  # Expanded at runtime
```

### Configuration Flow

```rust
// main.rs configuration loading
let config = match load_from_stream_config(&[&etcd_endpoint], "air-quality").await {
    Ok(stream_config) => stream_config,  // Priority 1: Stream Registry
    Err(_) => match load_from_etcd().await {
        Ok(etcd_config) => etcd_config,  // Priority 2: Legacy etcd
        Err(_) => match AppConfig::from_yaml("config.yaml") {
            Ok(yaml_config) => yaml_config,  // Priority 3: YAML file
            Err(_) => AppConfig::default_config(),  // Priority 4: Defaults
        }
    }
};
```

---

## Key Data Structures

### StreamConfig

Defines a data stream's schema, sources, and storage settings:

```rust
pub struct StreamConfig {
    pub stream_id: String,           // e.g., "air-quality"
    pub description: String,
    pub version: String,             // semver
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    pub partitioning_strategy: String,
    pub fields: Vec<SchemaField>,    // Schema definition
    pub sources: Vec<SourceConfig>,  // Data sources
    pub storage: Option<StorageConfig>,
}
```

### SchemaField

Defines a single field in a stream schema:

```rust
pub struct SchemaField {
    pub name: String,                // snake_case, e.g., "pm25"
    pub field_type: FieldType,       // Float, Int, String, Bool, Json
    pub unit: Option<String>,        // e.g., "µg/m³"
    pub nullable: bool,
    pub range: Option<Vec<f64>>,     // [min, max]
    pub display_precision: Option<u32>,
}
```

### SourceConfig

Defines a data source within a stream:

```rust
pub struct SourceConfig {
    pub source_type: SourceType,     // Mqtt, HttpPoll, Webhook, FileWatch
    pub enabled: bool,
    pub params: HashMap<String, Value>, // Source-specific parameters
}
```

---

## Deployment Architecture

### Pi Production Stack

```yaml
# deploy/pi/docker-compose.yml
services:
  mosquitto:      # MQTT Broker (128MB limit)
    ports: ["1883:1883"]

  etcd:           # Config Store (256MB limit)
    ports: ["2379:2379"]

  air-quality-app: # Main Application (512MB limit)
    ports: ["8080:8080"]
    depends_on: [mosquitto, etcd]
```

### Resource Constraints

| Service | Memory Limit | Actual Usage |
|---------|-------------|--------------|
| mosquitto | 128MB | ~50MB |
| etcd | 256MB | ~100MB |
| air-quality-app | 512MB | ~200MB |
| **Total** | **896MB** | **~350MB** |

---

## Data Flow

### Ingestion Pipeline

```
AirGradient Sensor
        │
        │ MQTT publish (airgradient/readings/+)
        ▼
┌──────────────────┐
│    mosquitto     │
│  (MQTT broker)   │
└────────┬─────────┘
         │ subscribe
         ▼
┌──────────────────┐
│   MqttHandler    │
│  - Parse JSON    │
│  - Create Point  │
└────────┬─────────┘
         │ send(TimeSeriesPoint)
         ▼
┌──────────────────┐
│  mpsc channel    │
│ (buffer: 1000)   │
└────────┬─────────┘
         │ receive
         ▼
┌──────────────────┐
│  StorageWriter   │
│  - Batch: 100    │
│  - Timeout: 5s   │
└────────┬─────────┘
         │ write_batch
         ▼
┌──────────────────┐
│  ParquetStore    │
│  - WAL append    │
│  - Parquet write │
└──────────────────┘
```

### Query Path

```
HTTP Request (GET /api/v1/air-quality/latest)
        │
        ▼
┌──────────────────┐
│   axum router    │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  readings.rs     │
│  (handler)       │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  ParquetStore    │
│  (query)         │
└────────┬─────────┘
         │
         ▼
    JSON Response
```

---

## External Data Sources

### OpenWeatherMap Integration ✅ IMPLEMENTED

The platform now integrates external weather and air quality data from OpenWeatherMap APIs:

**Current Weather API**:
- Endpoint: `https://api.openweathermap.org/data/2.5/weather`
- Metrics: temperature, feels_like, pressure, humidity, wind_speed, wind_deg, wind_gust, clouds, visibility, rain_1h, snow_1h
- Parser: `WeatherParser` (`core/src/sources/parsers/weather.rs`)
- Stream: `outdoor-weather` (`config/streams/outdoor-weather.yaml`)

**Air Pollution API**:
- Endpoint: `https://api.openweathermap.org/data/2.5/air_pollution`
- Metrics: aqi (1-5 scale), co, no, no2, o3, so2, pm2_5, pm10, nh3
- Parser: `AirPollutionParser` (`core/src/sources/parsers/air_pollution.rs`)
- Stream: `outdoor-air-quality` (`config/streams/outdoor-air-quality.yaml`)

**Configuration**:
- Authentication: Query parameter with `OPENWEATHERMAP_API_KEY` environment variable
- Poll Interval: 600 seconds (10 minutes, respects free tier limit of 60 calls/minute)
- Coordinates: Configured via `LAT` and `LON` environment variables
- Storage: Parquet format with daily partitioning
- Retention: 90 days, compression after 7 days

**Architecture Pattern**:
The generic HTTP polling source uses a plugin-based parser registry:
1. `HttpPollingSource` handles HTTP requests, authentication, retries
2. `ResponseParser` trait allows pluggable parsers for different APIs
3. `ParserRegistry` manages parser instances by name
4. Stream configurations reference parsers by name

This pattern makes it easy to add new external data sources (AccuWeather, PurpleAir, etc.) by implementing the `ResponseParser` trait.

## Extension Points

### Adding a New Source Type

1. Implement `Source` trait in `neural-core/src/sources/`
2. Add `SourceType` variant to `core/src/types/stream_config.rs`
3. Update `SourceManager` to spawn the new source type
4. Create source-specific configuration parsing

See: [HOW_TO_ADD_NEW_SOURCE.md](../procedures/HOW_TO_ADD_NEW_SOURCE.md)

### Adding a New HTTP Parser

1. Implement `ResponseParser` trait in `neural-core/src/sources/parsers/`
2. Register parser in `ParserRegistry` at startup
3. Create stream configuration YAML referencing parser by name
4. Add unit tests for parser logic

See: `WeatherParser` and `AirPollutionParser` as reference implementations

### Adding a New Stream

1. Create stream configuration directory: `config/base/streams/{stream-id}/`
2. Create `config.yaml` with stream schema, sources, and storage settings
3. Sync to etcd via `./deploy/pi/deploy.sh sync` or restart application
4. Verify registration: `docker exec etcd etcdctl get --prefix /streams/{stream-id}/`

See: [HOW_TO_ADD_NEW_STREAM.md](../procedures/HOW_TO_ADD_NEW_STREAM.md)

### Adding a New Storage Backend

1. Implement `Store` trait
2. Create adapter in `neural-core/src/storage/`
3. Update `StorageWriter` or create storage router
4. Add configuration options

---

## References

- [AIR-003 Architecture Summary](../../product/features/air-003/architecture/AIR-003-ARCHITECTURE-SUMMARY.md)
- [AIR-004 Platform Architecture](../../product/features/air-004/architecture/PLATFORM_ARCHITECTURE.md)
- [AIR-004 Coordinator Interfaces](../../product/features/air-004/architecture/COORDINATOR_INTERFACES.md)
- [Completion Guide (Pi)](../../product/features/air-004/completion/COMPLETION-PI-CORRECTED.md)

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.3.0 | 2025-12-17 | Added GitOps ConfigSyncService documentation |
| 1.2.0 | 2025-12-16 | AIR-005 complete with coordinator integration |
| 1.1.0 | 2025-12-16 | Added HTTP polling sources and parsers |
| 1.0.0 | 2025-12-16 | Initial comprehensive documentation |
