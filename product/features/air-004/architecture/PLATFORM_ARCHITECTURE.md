# AIR-004: Generic Multi-Stream Data Platform Architecture

## Overview

This document captures the architectural decisions for evolving the neural-data-platform from a single-stream air quality system to a generic multi-stream data platform capable of ingesting, storing, and analyzing data from heterogeneous sources.

## Problem Statement

The platform needs to:
1. Ingest data from multiple sources (MQTT, HTTP polling, webhooks, etc.)
2. Support multiple independent data streams (air quality, home events, weather, etc.)
3. Enable predictive analytics across streams using time-based correlation
4. Provide real-time dashboards and alerting
5. Export aggregates to home automation systems (Homebridge)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         STREAM REGISTRY (etcd)                          │
│                                                                         │
│  streams/air-quality:                                                   │
│    sources: [{type: mqtt, topic: "airgradient/#"}]                     │
│    schema: {pm25: float, co2: int, voc: int, temp: float, ...}         │
│                                                                         │
│  streams/home-events:                                                   │
│    sources: [{type: mqtt, topic: "home/events/#"},                     │
│              {type: webhook, path: "/api/events"}]                     │
│    schema: {event_type: string, target: string, state: string}         │
│                                                                         │
│  streams/weather:                                                       │
│    sources: [{type: http_poll, url: "...", interval: "5m"}]            │
│    schema: {temp: float, humidity: float, pressure: float, ...}        │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ watch
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                        INGESTION COORDINATOR                            │
│                         (single Rust binary)                            │
│                                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │ MqttSource  │  │ HttpPoller  │  │  Webhook    │  │ FileWatch   │   │
│  │ (spawned    │  │ (spawned    │  │  Handler    │  │ (future)    │   │
│  │  per topic) │  │  per url)   │  │             │  │             │   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘   │
│         └────────────────┴────────────────┴────────────────┘           │
│                                    │                                    │
│                                    ▼                                    │
│                    ┌───────────────────────────────────────┐           │
│                    │         Ingestion Router              │           │
│                    │  - Schema validation                  │           │
│                    │  - Route by stream_id                 │           │
│                    └───────────────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                 ┌──────────────────┼──────────────────┐
                 ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         BRONZE (Parquet)                                │
│                                                                         │
│    data/bronze/              data/bronze/           data/bronze/        │
│    air-quality/              home-events/           weather/            │
│    └─2025/12/15/*.parquet    └─2025/12/15/...      └─2025/12/15/...   │
│                                                                         │
│    (append-only, full history, schema per stream)                       │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ sync (dual-write or batch ETL)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      SILVER/GOLD (TimescaleDB)                          │
│                                                                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐        │
│  │ air_quality     │  │ home_events     │  │ weather         │        │
│  │ (hypertable)    │  │ (hypertable)    │  │ (hypertable)    │        │
│  │                 │  │                 │  │                 │        │
│  │ pm25 FLOAT      │  │ event_type TEXT │  │ temp FLOAT      │        │
│  │ co2 INT         │  │ target TEXT     │  │ humidity FLOAT  │        │
│  │ voc INT         │  │ state TEXT      │  │ pressure FLOAT  │        │
│  │ ...             │  │ ...             │  │ ...             │        │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘        │
│                                                                         │
│  Continuous Aggregates: mv_air_quality_5min, mv_air_quality_1hr, ...   │
│  Compression: after 7 days                                              │
│  Retention: configurable per stream                                     │
└─────────────────────────────────────────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         ▼                          ▼                          ▼
┌─────────────────┐    ┌─────────────────────┐    ┌─────────────────────┐
│    Grafana      │    │  Predictive Model   │    │  Triggers/Alerts    │
│   Dashboards    │    │                     │    │                     │
│                 │    │  Training: Bronze   │    │  Query: TimescaleDB │
│  Real-time      │    │  (Polars + Parquet) │    │  Threshold checks   │
│  visualizations │    │                     │    │  → Homebridge       │
│                 │    │  Inference: Silver  │    │  → Notifications    │
│                 │    │  (TimescaleDB)      │    │                     │
└─────────────────┘    └─────────────────────┘    └─────────────────────┘
```

## Key Design Decisions

### ADR-001: Coordinator Model

**Decision**: Single process, multi-source coordinator

**Context**: Need to orchestrate multiple data sources feeding multiple streams.

**Options Considered**:
- A) Single process with Tokio tasks per source
- B) Source-per-process with message bus (EventBus/Redis)

**Choice**: Option A

**Rationale**:
- Home-scale deployment, not enterprise
- Simpler deployment and operations
- Shared memory for efficiency
- Single binary to manage
- Can evolve to B if scale demands

---

### ADR-002: Stream Registry Location

**Decision**: etcd-based stream registry with watch API

**Context**: Need dynamic configuration of streams and sources.

**Choice**: Store stream definitions in etcd under `streams/{stream-id}/`

**Rationale**:
- Already using etcd for configuration (AIR-003)
- Watch API enables hot-reload without restart
- GitOps sync pattern already established
- Consistent with existing infrastructure

---

### ADR-003: Post-Ingestion Storage Model

**Decision**: Independent typed tables per stream (Hybrid approach)

**Context**: Choose between unified table (all streams) vs independent tables.

**Options Considered**:
- Unified: Single table with `stream_id` column and JSONB data
- Independent: Typed table per stream
- Hybrid: Independent typed tables, registry-driven schema

**Choice**: Hybrid - Independent streams with common envelope

**Rationale**:
- Query performance: typed columns beat JSONB for aggregations
- Compression: Parquet and TimescaleDB optimize typed columns better
- Schema validation: can enforce constraints per stream
- Stream count is manageable (3-5 streams)
- Registry-driven: schema in etcd, not hardcoded

**Trade-off Accepted**: Adding new stream requires table migration (mitigated by automation)

---

### ADR-004: Bronze Layer Storage

**Decision**: Parquet files partitioned by stream and date

**Structure**:
```
data/bronze/
├── air-quality/
│   └── 2025/12/15/*.parquet
├── home-events/
│   └── 2025/12/15/*.parquet
└── weather/
    └── 2025/12/15/*.parquet
```

**Rationale**:
- Append-only, immutable history
- Efficient for batch analytics and model training
- Polars/DuckDB query directly
- Schema per stream in Parquet metadata
- Cost-effective storage

---

### ADR-005: Silver/Gold Layer Storage

**Decision**: TimescaleDB with hypertables per stream

**Context**: Need real-time queryable storage for dashboards, alerts, and cross-stream analytics.

**Choice**: TimescaleDB (PostgreSQL extension)

**Rationale**:
- Native time-series optimization (hypertables, chunking)
- Continuous aggregates for automatic rollups
- SQL interface for Grafana integration
- ASOF JOIN for cross-stream time correlation
- Compression policies for storage efficiency
- Already have Docker config and schema patterns

**Alternative Considered**: QuestDB
- Faster for pure time-series
- Less mature ecosystem
- TimescaleDB provides Postgres compatibility (richer SQL, extensions)

---

### ADR-006: Bronze to Silver Sync

**Decision**: Dual-write initially, evolve to ETL if needed

**Options**:
- Dual-write: Ingestion writes to both Bronze and Silver simultaneously
- ETL: Periodic batch job syncs Bronze → Silver

**Choice**: Dual-write for simplicity

**Rationale**:
- Simple implementation
- No sync lag
- Can evolve to ETL for backfill or reprocessing
- Home scale doesn't require complex orchestration

---

### ADR-007: Source Abstraction

**Decision**: Unified Source trait supporting both push and poll patterns

**Interface**:
```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn stream_id(&self) -> &str;
    fn source_type(&self) -> SourceType;

    // For poll-based sources (HTTP, file)
    async fn fetch(&self) -> Result<Vec<StreamRecord>>;

    // For push-based sources (MQTT, WebSocket)
    async fn subscribe(&self) -> Result<Receiver<StreamRecord>>;

    async fn health_check(&self) -> Result<HealthStatus>;
}
```

**Supported Source Types**:
| Type | Pattern | Example |
|------|---------|---------|
| mqtt | Push | AirGradient sensor |
| http_poll | Poll | Weather API |
| webhook | Push | Manual event triggers |
| websocket | Push | Real-time feeds |
| file_watch | Trigger | CSV imports |

---

### ADR-008: Event Duration Modeling

**Decision**: Events are point-in-time; duration is derived in analytics

**Context**: Home events (window open, cooking) have duration, but how to model?

**Choice**: Store discrete events, compute duration at query time

**Example**:
```
{timestamp: "10:00", event_type: "window_state", state: "open", target: "front"}
{timestamp: "10:45", event_type: "window_state", state: "closed", target: "front"}
```

Duration = 45 minutes, computed via:
```sql
SELECT
    open_event.timestamp,
    close_event.timestamp - open_event.timestamp as duration
FROM home_events open_event
JOIN home_events close_event
  ON open_event.target = close_event.target
 AND open_event.state = 'open'
 AND close_event.state = 'closed'
 AND close_event.timestamp > open_event.timestamp;
```

**Rationale**:
- Events are atomic, simple to record
- Duration is analytics concern, not data concern
- Flexible for different analysis needs
- No schema coupling between event types

---

### ADR-009: Predictive Model Data Access

**Decision**: Training from Bronze, inference from Silver

**Training Pipeline**:
```
Bronze (Parquet) → Polars → Join streams by timestamp → Train model
```

**Inference Pipeline**:
```
Silver (TimescaleDB) → Query recent window → Run prediction → Output
```

**Rationale**:
- Training needs full history (Bronze)
- Inference needs recent, fast access (Silver)
- Clear separation of batch vs real-time paths

---

## Stream Registry Schema

### etcd Key Structure

```
streams/{stream-id}/config     → stream configuration
streams/{stream-id}/schema     → field definitions
streams/{stream-id}/sources    → source configurations
```

### Example Configuration

```yaml
# streams/air-quality/config
stream_id: air-quality
description: Indoor air quality measurements
retention_days: 365
compression_after_days: 7

# streams/air-quality/schema
fields:
  - name: pm25
    type: float
    unit: µg/m³
    nullable: false
  - name: pm10
    type: float
    unit: µg/m³
    nullable: true
  - name: co2
    type: int
    unit: ppm
    nullable: false
  - name: voc
    type: int
    unit: index
    nullable: true
  - name: temperature
    type: float
    unit: celsius
    nullable: true
  - name: humidity
    type: float
    unit: percent
    nullable: true

# streams/air-quality/sources
sources:
  - type: mqtt
    topic: airgradient/readings/#
    qos: 1
```

```yaml
# streams/home-events/config
stream_id: home-events
description: Discrete home activity events
retention_days: 730

# streams/home-events/schema
fields:
  - name: event_type
    type: string
    nullable: false
  - name: target
    type: string
    nullable: false
  - name: state
    type: string
    nullable: true
  - name: metadata
    type: json
    nullable: true

# streams/home-events/sources
sources:
  - type: mqtt
    topic: home/events/#
    qos: 1
  - type: webhook
    path: /api/events
    auth: bearer
```

```yaml
# streams/weather/config
stream_id: weather
description: Outdoor weather conditions
retention_days: 365

# streams/weather/schema
fields:
  - name: temperature
    type: float
    unit: celsius
    nullable: false
  - name: humidity
    type: float
    unit: percent
    nullable: false
  - name: pressure
    type: float
    unit: hPa
    nullable: true
  - name: wind_speed
    type: float
    unit: m/s
    nullable: true
  - name: conditions
    type: string
    nullable: true

# streams/weather/sources
sources:
  - type: http_poll
    url: https://api.openweathermap.org/data/2.5/weather
    interval: 5m
    auth:
      type: api_key
      key_param: appid
      key_env: OPENWEATHERMAP_API_KEY
```

---

## Implementation Components

### Existing (Reuse/Extend)

| Component | Location | Status |
|-----------|----------|--------|
| ParquetStore | `core/src/storage/parquet.rs` | Extend for multi-stream |
| MqttSource | `core/src/sources/mqtt.rs` | Adapt to new trait |
| HttpPollingSource | `core/src/sources/http.rs` | Adapt to new trait |
| etcd config client | `apps/air-quality-app/` | Reuse (AIR-003) |
| EventBus | `neural-core/src/eventbus/` | Available if needed |
| Grafana configs | `docker/production/configs/grafana/` | Extend |

### New Components

| Component | Description | Priority |
|-----------|-------------|----------|
| Stream Registry | etcd schema + watch integration | High |
| Generic Source trait | Unified push/poll interface | High |
| Ingestion Router | Schema validation, stream routing | High |
| TimescaleDB Adapter | Rust sqlx-based adapter | High |
| Auto DDL generation | Registry schema → CREATE TABLE | Medium |
| Webhook handler | Axum endpoint for manual events | Medium |
| Grafana dashboards | Per-stream dashboard templates | Medium |

---

## Migration Path from AIR-003

1. **Phase 1**: Refactor air-quality-app to use new Source trait
2. **Phase 2**: Add Stream Registry to etcd, migrate air-quality config
3. **Phase 3**: Implement Ingestion Coordinator with router
4. **Phase 4**: Add TimescaleDB adapter and dual-write
5. **Phase 5**: Add Grafana dashboards
6. **Phase 6**: Add additional streams (home-events, weather)

---

## Related Documents

- AIR-001: Core platform foundation
- AIR-002: Configuration management
- AIR-003: etcd-based configuration with hot-reload
- `neural-core/src/eventbus/`: EventBus implementation (if needed later)
- `docker/timescaledb/`: TimescaleDB Docker configuration

---

## Open Questions

1. **Webhook authentication**: Bearer token, API key, or both?
2. **Schema evolution**: How to handle adding fields to existing streams?
3. **Backfill**: Process for replaying Bronze → Silver after schema change?
4. **Alerting**: Threshold-based triggers - in-app or external (Grafana alerts)?

---

*Last Updated: 2025-12-15*
*Status: Design Complete, Pending Implementation*
