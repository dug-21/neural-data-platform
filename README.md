# Neural Data Platform

[![Rust](https://img.shields.io/badge/rust-1.70+-orange?style=flat-square)](https://rustlang.org)
[![Docker](https://img.shields.io/badge/docker-ready-2496ED?style=flat-square&logo=docker)](deploy/pi/docker-compose.yml)
[![TimescaleDB](https://img.shields.io/badge/timescaledb-pg15-blue?style=flat-square)](https://timescale.com)
[![License](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)

> **Configuration-driven time-series data platform** for edge deployment on Raspberry Pi. Ingest data from multiple sources, transform through a multi-layer data lake, and visualize with real-time dashboards.

---

## Overview

The Neural Data Platform (NDP) is a generic, extensible data ingestion and analytics system built in Rust. It implements a **Bronze → Silver → Gold** data lake architecture with configuration-driven ETL, requiring no code changes to add new data streams or transformations.

### Key Capabilities

| Capability | Description |
|------------|-------------|
| **Multi-Source Ingestion** | MQTT streaming, HTTP polling, webhooks (planned) |
| **Bronze Layer** | Append-only Parquet storage with WAL crash recovery |
| **Silver Layer** | TimescaleDB hypertables with config-driven ETL |
| **Data Quality** | Configurable DQ rules (range, null, freshness, cross-field) |
| **Data Dictionary** | Full schema lineage from Bronze source to Silver target |
| **MCP Integration** | 15 AI-agent tools for data exploration and monitoring |
| **Edge-First Design** | Optimized for Raspberry Pi 5 (<2GB memory footprint) |
| **GitOps Configuration** | YAML configs synced to etcd with hot-reload |

---

## Current Data Streams

| Stream | Source | Type | Description |
|--------|--------|------|-------------|
| `air-quality` | AirGradient MQTT | Real-time | PM2.5, CO2, temperature, humidity, VOC, NOx |
| `outdoor-weather` | OpenWeatherMap | 10-min poll | Temperature, humidity, pressure, wind |
| `outdoor-air-quality` | OpenWeatherMap | 5-min poll | AQI, PM2.5, PM10, NO2, O3, CO |
| `nws-observations` | NWS API | 5-min poll | Station weather observations (KSGJ) |
| `nws-station-observations` | NWS API | 5-min poll | Current conditions |
| `nws-forecast-hourly` | NWS API | 1-hour poll | Hourly weather forecast |
| `nws-gridpoints-forecast` | NWS API | 1-hour poll | 40+ weather metrics forecast |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CONFIGURATION (etcd + GitOps)                        │
│  /streams/{id}/config → Schema, sources, field mappings, DQ rules, ETL      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ watch + hot-reload
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           INGESTION (air-quality-app)                        │
│                                                                              │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐     │
│  │ MqttSource  │   │HttpPolling  │   │ NWS Parser  │   │ OWM Parser  │     │
│  │ (real-time) │   │  Source     │   │             │   │             │     │
│  └──────┬──────┘   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘     │
│         └─────────────────┴─────────────────┴─────────────────┘             │
│                                    │                                         │
│                                    ▼                                         │
│                    ┌───────────────────────────┐                            │
│                    │   tokio mpsc channel      │                            │
│                    │   (backpressure: 1000)    │                            │
│                    └─────────────┬─────────────┘                            │
│                                  ▼                                           │
│                    ┌───────────────────────────┐                            │
│                    │   StorageWriter + WAL     │                            │
│                    │   (batch: 100, timeout: 5s)│                            │
│                    └─────────────┬─────────────┘                            │
└──────────────────────────────────┼──────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BRONZE LAYER (Parquet)                               │
│                                                                              │
│   Schema: timestamp | source_id | ndp_id | context | raw_payload            │
│   Storage: /data/raw/{stream}/year=YYYY/month=MM/day=DD/data.parquet        │
│   Features: Append-only, WAL crash recovery, Snappy compression              │
└─────────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ silver-etl (config-driven)
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SILVER LAYER (TimescaleDB)                           │
│                                                                              │
│   Tables: silver.air_quality_observations, silver.weather_observations,     │
│           silver.weather_forecasts, silver.outdoor_air_quality              │
│   Features: Hypertables, continuous aggregates, DQ flags, data dictionary   │
│   ETL: Incremental watermark, upsert dedup, unit conversions, DQ rules      │
└─────────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ SQL queries
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              GRAFANA                                         │
│                                                                              │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐          │
│  │  Pipeline Health │  │ Indoor Environ-  │  │ Forecast Accuracy│          │
│  │  Dashboard       │  │ ment Dashboard   │  │ Dashboard        │          │
│  └──────────────────┘  └──────────────────┘  └──────────────────┘          │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Lake Layers

### Bronze Layer (Raw Archive)

- **Purpose**: Immutable raw data archive for audit, replay, and reprocessing
- **Storage**: Apache Parquet with daily partitioning
- **Schema**: Wide format preserving exact source payloads
- **Crash Recovery**: Write-Ahead Log (WAL) ensures durability

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | i64 | Microseconds since epoch |
| `source_id` | String | `{stream_id}-{SourceType}` |
| `ndp_id` | String? | Stable platform identifier |
| `context` | JSON? | Config-derived metadata |
| `raw_payload` | JSON | Exact payload from source |

### Silver Layer (Analytics-Ready)

- **Purpose**: Clean, typed data optimized for queries and dashboards
- **Storage**: TimescaleDB hypertables with time-based chunking
- **ETL**: Config-driven Bronze→Silver transformation
- **Features**: Data quality flags, incremental loading, deduplication

### Gold Layer (ML Features) - Planned

- **Purpose**: Feature engineering for predictions
- **Features**: Time-windowed aggregations, cross-stream joins

---

## Configuration-Driven ETL

All stream behavior is defined in YAML, requiring no Rust code changes:

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
description: "AirGradient indoor air quality sensors"

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      unit: "ug/m3"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    - source_path: raw_payload.atmpCompensated
      target_column: temperature_c
      type: double_precision
      transform:
        type: unit_conversion
        from: celsius
        to: celsius
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert
```

### Supported Transforms

| Transform | Example |
|-----------|---------|
| Unit Conversion | Kelvin → Celsius, hPa → Pa, m/s → km/h |
| Timestamp | Microseconds, Unix seconds, ISO8601 |
| Expression | SQL CASE statements |
| Array Explosion | Flatten nested forecast arrays |

### Data Quality Rules

| Rule | Actions | Description |
|------|---------|-------------|
| `range_check` | flag, reject, clamp | Numeric bounds validation |
| `null_check` | flag, reject | Required field validation |
| `freshness_check` | flag | Timestamp recency |
| `cross_field_check` | flag | Physical constraints (PM10 >= PM2.5) |

---

## MCP Tools for AI Agents

The platform exposes **15 MCP tools** for AI-powered data exploration:

### Bronze Layer Tools

| Tool | Description |
|------|-------------|
| `list_streams` | List all streams with metadata and storage info |
| `describe_schema` | Get source/target schema with gap analysis |
| `validate_config` | Compare etcd config vs actual Parquet |
| `sample_data` | Preview raw Bronze records |

### Silver Layer Tools

| Tool | Description |
|------|-------------|
| `list_silver_tables` | List hypertables with row counts |
| `describe_silver_table` | Get Silver schema with columns and units |
| `sample_silver_data` | Sample rows with time filtering |
| `silver_stats` | Statistics: row counts, time ranges, DQ summary |

### Data Dictionary Tools

| Tool | Description |
|------|-------------|
| `query_dictionary` | Search columns across layers |
| `describe_column` | Full metadata with lineage and DQ rules |
| `trace_lineage` | Trace Silver column to Bronze source |
| `list_dq_rules` | List DQ rules per table/column |

### ETL Observability Tools

| Tool | Description |
|------|-------------|
| `etl_status` | Current ETL status for all streams |
| `etl_history` | Historical ETL runs with success/failure |
| `data_freshness` | Staleness report across layers |

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

# Deploy
cd deploy/pi
./deploy.sh start

# Sync configuration
./deploy.sh sync

# Start Silver ETL daemon
./deploy.sh silver-daemon

# Check status
./deploy.sh status
```

### Access Points

| Service | URL | Credentials |
|---------|-----|-------------|
| Grafana | http://localhost:3000 | admin/admin |
| Health API | http://localhost:8080/health | - |
| MCP Server | http://localhost:9100 | - |

---

## Docker Services

| Service | Image | Memory | Purpose |
|---------|-------|--------|---------|
| mosquitto | eclipse-mosquitto:2.0 | 128MB | MQTT broker |
| etcd | quay.io/coreos/etcd:v3.5.11 | 256MB | Configuration store |
| air-quality-app | neural-data-platform/air-quality-app | 512MB | Multi-stream ingestion |
| timescaledb | timescale/timescaledb:latest-pg15 | 256MB | Silver layer storage |
| ndp-mcp-server | neural-data-platform/ndp-mcp-server | 64MB | MCP protocol server |
| grafana | grafana/grafana:latest-ubuntu | 256MB | Dashboards |
| silver-etl-daemon | neural-data-platform/silver-etl | 256MB | Continuous ETL |

**Total Memory**: ~1.7GB (suitable for Raspberry Pi 5 with 8GB)

---

## Deployment Commands

```bash
# Core operations
./deploy.sh start          # Start all services
./deploy.sh stop           # Stop all services
./deploy.sh status         # Check health and URLs
./deploy.sh logs           # Follow all logs

# Configuration
./deploy.sh sync           # Sync YAML to etcd
./deploy.sh list-streams   # List configured streams

# Silver ETL
./deploy.sh silver-migrate       # Run schema migrations
./deploy.sh silver-daemon        # Start ETL daemon
./deploy.sh silver-daemon-stop   # Stop ETL daemon

# Updates
./deploy.sh update               # Pull + rebuild all
./deploy.sh update app           # Rebuild specific service
```

---

## Project Structure

```
neural-data-platform/
├── core/                       # neural-core library
│   └── src/
│       ├── types/              # TimeSeriesPoint, RawDataPoint, StreamConfig
│       ├── sources/            # MqttSource, HttpPollingSource, parsers
│       ├── storage/            # ParquetStore, WalWriter
│       └── traits.rs           # Source, RawSource, Store, RawStore
├── apps/
│   ├── air-quality-app/        # Main ingestion binary
│   ├── silver-etl/             # Bronze→Silver ETL binary
│   └── ndp-mcp-server/         # MCP protocol server
├── config/
│   ├── base/streams/           # Stream YAML configs (GitOps)
│   └── grafana/                # Dashboard provisioning
├── config-client/              # etcd configuration client
├── deploy/pi/                  # Docker Compose deployment
└── docs/                       # Architecture documentation
```

---

## Domain Adapter Pattern

The platform uses hexagonal architecture with trait-based abstractions:

```rust
// Source Port - All data sources implement this
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> Result<Vec<TimeSeriesPoint>, CoreError>;
    async fn health_check(&self) -> Result<HealthStatus, CoreError>;
}

// RawSource Port - For Bronze layer (preserves raw payload)
#[async_trait]
pub trait RawSource: Send + Sync {
    async fn fetch_raw(&self) -> Result<Vec<RawDataPoint>, CoreError>;
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
| Parser | `WeatherParser`, `AirPollutionParser`, `NWSParser` | Production |
| Store | `ParquetStore` | Production |
| Analytics | `TimescaleDB` | Production |
| Config | `ConfigClient` (etcd) | Production |

---

## Grafana Dashboards

| Dashboard | Purpose |
|-----------|---------|
| Pipeline Health | Operational monitoring - data freshness, ETL status |
| Indoor Environment | "Should I open windows?" - ventilation recommendations |
| Forecast Accuracy | NWS forecast reliability by lead time |
| Personal Weather Forecast | Unified forecast view |

---

## Development

### Build

```bash
cargo build                  # Build all crates
cargo test                   # Run tests
cargo clippy                 # Lint
cargo fmt --check            # Format check
```

### Local Development

```bash
# Start infrastructure
docker compose -f deploy/pi/docker-compose.yml up -d mosquitto etcd timescaledb

# Run app locally
RUST_LOG=info cargo run -p air-quality-app
```

---

## Roadmap

### Current (v2.x) - Complete

- [x] Multi-source ingestion (MQTT, HTTP polling)
- [x] Bronze layer (Parquet with WAL)
- [x] Silver layer (TimescaleDB with config-driven ETL)
- [x] Data Quality rules and transparency
- [x] MCP server with 15 tools
- [x] Grafana dashboards
- [x] GitOps configuration

### Future

- [ ] **Gold Layer**: ML-ready feature engineering
- [ ] **Neural Predictions**: Time-series forecasting with ruv-FANN
- [ ] **Action Triggers**: Threshold-based alerts and automations
- [ ] **Additional Streams**: Energy monitoring, home automation
- [ ] **S3/Cloud Storage**: Bronze layer cloud backend

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture Overview](docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) | Full system architecture |
| [Config-Driven ETL](docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md) | Silver layer ETL design |
| [Add New Stream](docs/procedures/HOW_TO_ADD_NEW_STREAM.md) | Step-by-step guide |
| [Add New Source](docs/procedures/HOW_TO_ADD_NEW_SOURCE.md) | Implement new source types |

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

<div align="center">

**Built with Rust for edge deployment**

[Documentation](docs/) | [Issues](https://github.com/your-org/neural-data-platform/issues)

</div>
