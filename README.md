# Neural Data Platform

[![Rust](https://img.shields.io/badge/rust-1.70+-orange?style=flat-square)](https://rustlang.org)
[![Docker](https://img.shields.io/badge/docker-ready-2496ED?style=flat-square&logo=docker)](deploy/pi/docker-compose.yml)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)

> **Configuration-driven time-series data platform** for edge deployment on Raspberry Pi. Ingest data from multiple sources, store in columnar format, and visualize with real-time dashboards.

---

## Overview

The Neural Data Platform is a generic, extensible data ingestion and analytics system built in Rust. It uses a **Domain Adapter Pattern** (hexagonal architecture) to support pluggable data sources and storage backends, with configuration-driven stream management that requires no code changes to add new data streams.

### Key Features

| Feature | Description |
|---------|-------------|
| **Domain Adapter Pattern** | Pluggable sources (`Source` trait) and storage (`Store` trait) |
| **Configuration-Driven** | Add new streams via YAML config, no code changes required |
| **Multi-Source Ingestion** | MQTT broker/listener, HTTP polling, webhooks |
| **Bronze Data Layer** | Append-only Parquet storage with WAL for crash recovery |
| **Silver Query Layer** | DuckDB views over Parquet for analytics |
| **Real-Time Dashboards** | Grafana with 4 provisioned dashboards |
| **Edge-First Design** | Optimized for Raspberry Pi 5 (<2GB memory footprint) |
| **GitOps Configuration** | YAML configs synced to etcd on startup |

### Current Streams

| Stream | Source | Data |
|--------|--------|------|
| Indoor Air Quality | AirGradient sensor via MQTT | PM2.5, CO2, temperature, humidity |
| Outdoor Weather | OpenWeatherMap API (HTTP poll) | Temperature, humidity, pressure, wind |
| Outdoor Air Quality | OpenWeatherMap API (HTTP poll) | AQI, PM2.5, PM10, NO2, O3, CO |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         STREAM REGISTRY (etcd)                          │
│  /streams/air-quality/config                                            │
│  /streams/outdoor-weather/config                                        │
│  /streams/outdoor-air-quality/config                                    │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ watch + load
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          air-quality-app                                │
│                         (Rust, single binary)                           │
│                                                                         │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    │
│  │   MqttSource    │    │ HttpPollingSource│    │  WebhookHandler │    │
│  │  (Source Trait) │    │  (Source Trait) │    │  (Source Trait) │    │
│  └────────┬────────┘    └────────┬────────┘    └────────┬────────┘    │
│           └──────────────────────┼──────────────────────┘              │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │     IngestionRouter       │                       │
│                    │   - Schema validation     │                       │
│                    │   - Dead letter queue     │                       │
│                    └─────────────┬─────────────┘                       │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │      StorageWriter        │                       │
│                    │   - Batch accumulation    │                       │
│                    │   - Timeout-based flush   │                       │
│                    └─────────────┬─────────────┘                       │
│                                  ▼                                      │
│                    ┌───────────────────────────┐                       │
│                    │       ParquetStore        │                       │
│                    │   - Append-only writes    │                       │
│                    │   - WAL for recovery      │                       │
│                    └───────────────────────────┘                       │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ write Parquet
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        BRONZE LAYER (Parquet)                           │
│                                                                         │
│    /data/air-quality/YYYY-MM-DD_readings.parquet                       │
│    /data/outdoor-weather/YYYY-MM-DD_readings.parquet                   │
│    /data/outdoor-air-quality/YYYY-MM-DD_readings.parquet               │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ read_parquet()
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                   SILVER LAYER (DuckDB Embedded)                        │
│                                                                         │
│    Grafana DuckDB plugin queries Parquet files directly                │
│    - Direct Bronze queries via read_parquet()                          │
│    - Time bucket aggregations                                          │
│    - Cross-stream comparisons                                          │
└─────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ HTTP :3000
                                   ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                              Grafana                                    │
│                                                                         │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐     │
│  │  Indoor Air      │  │ Outdoor Weather  │  │  Outdoor AQI     │     │
│  │  Quality         │  │  Conditions      │  │  Dashboard       │     │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘     │
│  ┌──────────────────┐                                                  │
│  │  Indoor vs       │                                                  │
│  │  Outdoor Compare │                                                  │
│  └──────────────────┘                                                  │
└─────────────────────────────────────────────────────────────────────────┘
```

### Domain Adapter Pattern

The platform uses hexagonal architecture with trait-based abstractions:

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

| Port | Adapter | Status |
|------|---------|--------|
| Source | `MqttSource` | Production |
| Source | `HttpPollingSource` | Production |
| Store | `ParquetStore` | Production |
| Config | `ConfigClient` (etcd) | Production |
| Analytics | DuckDB (embedded in Grafana) | Production |

---

## Quick Start

### Prerequisites

- Docker 20.10+ and Docker Compose 2.0+
- Raspberry Pi 5 (or x86 for development)
- OpenWeatherMap API key (free tier)

### Deployment

```bash
# Clone repository
git clone https://github.com/your-org/neural-data-platform.git
cd neural-data-platform

# Configure environment
cp deploy/pi/.env.example deploy/pi/.env
# Edit .env with your API keys and coordinates

# Deploy to Pi
cd deploy/pi
./deploy.sh start

# View logs
./deploy.sh logs

# Check status
./deploy.sh status
```

### Access Dashboards

- **Grafana**: http://localhost:3000 (default: admin/admin)
- **Health API**: http://localhost:8080/health

---

## Configuration

### Adding a New Stream

Streams are defined via YAML configuration with automatic sync to etcd:

```yaml
# config/base/streams/my-stream/config.yaml
stream_id: my-stream
description: My new data stream
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
    range: [-50, 100]

sources:
  - type: mqtt
    enabled: true
    topic: sensors/my-stream/+
    qos: 1
```

Then sync to etcd:

```bash
./deploy.sh sync
```

See [How to Add a New Stream](docs/procedures/HOW_TO_ADD_NEW_STREAM.md) for detailed instructions.

### Stream Configuration Schema

| Field | Type | Description |
|-------|------|-------------|
| `stream_id` | string | Unique ID (kebab-case, 3-64 chars) |
| `fields` | array | Schema fields with types, units, ranges |
| `sources` | array | Data source configurations |
| `retention_days` | int | Data retention period |
| `partitioning_strategy` | string | `daily` or `hourly` |

### Source Types

| Type | Use Case | Configuration |
|------|----------|---------------|
| `mqtt` | IoT sensors, real-time push | topic, qos, broker |
| `http_poll` | External APIs | url, interval, auth, parser |
| `webhook` | Event-driven push | path, auth |

---

## Project Structure

```
neural-data-platform/
├── core/                       # neural-core library
│   └── src/
│       ├── types/              # TimeSeriesPoint, StreamConfig
│       ├── sources/            # MqttSource, HttpPollingSource
│       ├── storage/            # ParquetStore, WalWriter
│       └── traits.rs           # Source, Store, Forecast traits
├── apps/
│   └── air-quality-app/        # Main application binary
│       └── src/
│           ├── coordinator/    # IngestionCoordinator, SourceManager
│           ├── ingestion/      # MqttHandler
│           └── pipeline/       # StorageWriter
├── config/
│   ├── base/streams/           # Stream YAML configs (GitOps)
│   ├── grafana/                # Grafana provisioning & dashboards
│   └── duckdb/                 # DuckDB SQL views
├── config-client/              # etcd configuration client
├── deploy/pi/                  # Docker Compose deployment
└── docs/
    ├── architecture/           # Architecture documentation
    └── procedures/             # How-to guides
```

---

## Docker Services

| Service | Image | Memory | Purpose |
|---------|-------|--------|---------|
| mosquitto | eclipse-mosquitto:2.0 | 128MB | MQTT broker |
| etcd | quay.io/coreos/etcd:v3.5.11 | 256MB | Configuration store |
| air-quality-app | Custom Rust | 512MB | Data ingestion |
| duckdb | datacatering/duckdb:v1.1.3 | 512MB | Analytics (init only) |
| grafana | grafana/grafana:latest-ubuntu | 256MB | Dashboards |

**Total Memory**: ~1.6GB (suitable for Raspberry Pi 5)

---

## Development

### Build

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Clippy linting
cargo clippy

# Format check
cargo fmt --check
```

### Local Development

```bash
# Start infrastructure only
docker compose -f deploy/pi/docker-compose.yml up -d mosquitto etcd

# Run app locally
RUST_LOG=info cargo run -p air-quality-app

# Or use development overlay
docker compose -f deploy/pi/docker-compose.yml -f deploy/dev/docker-compose.override.yml up
```

---

## Dashboards

### Indoor Air Quality
Real-time sensor readings from AirGradient:
- PM2.5 levels with EPA thresholds
- CO2 levels with ventilation thresholds
- Temperature and humidity trends

### Outdoor Weather Conditions
OpenWeatherMap current weather:
- Temperature and feels-like
- Wind speed and pressure
- Cloud cover

### Outdoor Air Quality
OpenWeatherMap air pollution:
- AQI gauge (1-5 scale)
- PM2.5 and PM10 levels
- Pollutant breakdown (NO2, O3, CO, SO2)

### Indoor vs Outdoor Comparison
Cross-stream analysis:
- Temperature delta (indoor - outdoor)
- PM2.5 comparison
- Humidity comparison
- Indoor CO2 (outdoor reference not available)

---

## Roadmap

### Current (v1.x)
- [x] Multi-source ingestion (MQTT, HTTP)
- [x] Bronze layer (Parquet)
- [x] Silver layer (DuckDB views)
- [x] Grafana dashboards
- [x] GitOps configuration

### Future
- [ ] **Neural Predictions**: Time-series forecasting with ruv-FANN
- [ ] **Action Triggers**: Threshold-based alerts and automations
- [ ] **Additional Streams**: Energy monitoring, home automation events
- [ ] **Gold Layer**: ML-ready feature engineering
- [ ] **TimescaleDB**: Materialized Silver layer for complex queries

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture Overview](docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) | Full system architecture |
| [Component Map](docs/architecture/COMPONENT_DEPENDENCY_MAP.md) | Component dependencies |
| [Add New Stream](docs/procedures/HOW_TO_ADD_NEW_STREAM.md) | Step-by-step guide |
| [Add New Source](docs/procedures/HOW_TO_ADD_NEW_SOURCE.md) | Implement new source types |

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<div align="center">

**Built with Rust for edge deployment**

[Documentation](docs/) · [Issues](https://github.com/your-org/neural-data-platform/issues)

</div>
