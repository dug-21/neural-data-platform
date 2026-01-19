# Deployment Architecture: Post DP-012

## Key Clarification

The **Event Bus is in-process** - it's a `tokio::broadcast` channel inside the `air-quality-app` binary, NOT a separate message broker.

---

## Container/Service Layout

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           RASPBERRY PI 5                                     │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                         Docker Compose                               │    │
│  │                                                                      │    │
│  │  ┌──────────────┐     ┌──────────────────────────────────────────┐  │    │
│  │  │  mosquitto   │     │          air-quality-app                  │  │    │
│  │  │  (MQTT)      │────▶│                                           │  │    │
│  │  │  :1883       │     │  ┌─────────────────────────────────────┐  │  │    │
│  │  └──────────────┘     │  │     SOURCES (tokio tasks)           │  │  │    │
│  │                       │  │  ┌─────────┐  ┌─────────────────┐   │  │  │    │
│  │                       │  │  │  MQTT   │  │  HTTP Polling   │   │  │  │    │
│  │  ┌──────────────┐     │  │  │ Client  │  │  (OWM, NWS)     │   │  │  │    │
│  │  │    etcd      │     │  │  └────┬────┘  └────────┬────────┘   │  │  │    │
│  │  │  (config)    │────▶│  │       │                │            │  │  │    │
│  │  │  :2379       │     │  │       └────────┬───────┘            │  │  │    │
│  │  └──────────────┘     │  │                │                    │  │  │    │
│  │                       │  │                ▼                    │  │  │    │
│  │                       │  │  ┌─────────────────────────────┐    │  │  │    │
│  │                       │  │  │       EVENT BUS             │    │  │  │    │
│  │                       │  │  │  (tokio::broadcast channel) │    │  │  │    │
│  │                       │  │  │      in-process, ~0 latency │    │  │  │    │
│  │                       │  │  └──────────────┬──────────────┘    │  │  │    │
│  │                       │  │                 │                   │  │  │    │
│  │                       │  │    ┌────────────┼────────────┐      │  │  │    │
│  │                       │  │    │            │            │      │  │  │    │
│  │                       │  │    ▼            ▼            ▼      │  │  │    │
│  │                       │  │ ┌──────┐  ┌──────────┐  ┌────────┐  │  │  │    │
│  │                       │  │ │Bronze│  │  Silver  │  │Process-│  │  │  │    │
│  │                       │  │ │ Sub  │  │   Sub    │  │  ors   │  │  │  │    │
│  │                       │  │ └──┬───┘  └────┬─────┘  └───┬────┘  │  │  │    │
│  │                       │  │    │           │            │       │  │  │    │
│  │                       │  └────┼───────────┼────────────┼───────┘  │  │    │
│  │                       │       │           │            │          │  │    │
│  │                       └───────┼───────────┼────────────┼──────────┘  │    │
│  │                               │           │            │             │    │
│  │                               ▼           │            │             │    │
│  │                        ┌──────────┐       │            │             │    │
│  │                        │  /data/  │       │            │             │    │
│  │                        │  raw/    │       │            │             │    │
│  │                        │ (NVMe)   │       │            │             │    │
│  │                        │ Parquet  │       │            │             │    │
│  │                        └──────────┘       │            │             │    │
│  │                                           │            │             │    │
│  │  ┌──────────────┐                         │            │             │    │
│  │  │ timescaledb  │◀────────────────────────┘            │             │    │
│  │  │  (Silver)    │                                      │             │    │
│  │  │  :5432       │◀─────────────────────────────────────┘             │    │
│  │  │              │   (ML queries Silver for context)                  │    │
│  │  └──────────────┘                                                    │    │
│  │                                                                      │    │
│  │  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐         │    │
│  │  │   grafana    │     │ ndp-mcp-svr  │     │ silver-etl   │         │    │
│  │  │  (dashboards)│     │  (AI tools)  │     │ (backfill)   │         │    │
│  │  │  :3000       │     │  :9100       │     │  (on-demand) │         │    │
│  │  └──────────────┘     └──────────────┘     └──────────────┘         │    │
│  │                                                                      │    │
│  └──────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

---

## Message Flow Detail

### 1. External → Sources (Network I/O)

```
┌─────────────┐         ┌─────────────────────────────────────┐
│  mosquitto  │   TCP   │  air-quality-app                    │
│   :1883     │────────▶│  MqttSource (rumqttc async client)  │
└─────────────┘         └─────────────────────────────────────┘

┌─────────────┐         ┌─────────────────────────────────────┐
│ OpenWeather │  HTTPS  │  air-quality-app                    │
│   API       │────────▶│  HttpPollingSource (reqwest)        │
└─────────────┘         └─────────────────────────────────────┘

┌─────────────┐         ┌─────────────────────────────────────┐
│  NWS API    │  HTTPS  │  air-quality-app                    │
│             │────────▶│  HttpPollingSource (reqwest)        │
└─────────────┘         └─────────────────────────────────────┘
```

**Protocol**: MQTT (TCP), HTTPS
**Library**: `rumqttc`, `reqwest`

### 2. Sources → Event Bus (In-Process)

```rust
// Inside air-quality-app process
// Sources call:
event_bus.publish(RawDataPoint { ... });

// This is a tokio::broadcast::send() - microseconds latency
// All subscribers receive Arc<RawDataPoint> (zero-copy)
```

**Protocol**: None (in-process memory)
**Latency**: ~microseconds

### 3. Event Bus → Subscribers (In-Process)

```rust
// Each subscriber is a tokio task within air-quality-app
// They receive from broadcast channel:

loop {
    let point = receiver.recv().await;  // Arc<RawDataPoint>
    // Process...
}
```

**Protocol**: None (in-process memory)
**Latency**: ~microseconds

### 4. Bronze Subscriber → Storage (File I/O)

```
┌──────────────────────┐         ┌─────────────────────┐
│  Bronze Subscriber   │  write  │  /data/raw/         │
│  (in air-quality-app)│────────▶│  (NVMe SSD)         │
│                      │         │  Parquet files      │
└──────────────────────┘         └─────────────────────┘
```

**Protocol**: File I/O (Parquet via `polars`)
**Latency**: 1-10ms per batch (NVMe)

### 5. Silver Subscriber → TimescaleDB (Network I/O)

```
┌──────────────────────┐         ┌─────────────────────┐
│  Silver Subscriber   │   SQL   │  timescaledb        │
│  (in air-quality-app)│────────▶│  :5432              │
│                      │  TCP    │  silver.* tables    │
└──────────────────────┘         └─────────────────────┘
```

**Protocol**: PostgreSQL wire protocol (TCP)
**Library**: `tokio-postgres` or `sqlx`
**Latency**: 1-5ms per batch insert

### 6. ML Processor → TimescaleDB (Network I/O)

```
┌──────────────────────┐         ┌─────────────────────┐
│  ML Processor        │  query  │  timescaledb        │
│  (in air-quality-app)│◀───────▶│  :5432              │
│                      │   SQL   │  (get context)      │
└──────────────────────┘         └─────────────────────┘
```

**Protocol**: PostgreSQL wire protocol (TCP)
**Latency**: 5-20ms for context query

### 7. Processors → Outputs (Network I/O)

```
┌──────────────────────┐         ┌─────────────────────┐
│  Threshold Processor │  MQTT   │  mosquitto          │
│  (in air-quality-app)│────────▶│  :1883              │
│                      │  publish│  ndp/alerts/#       │
└──────────────────────┘         └─────────────────────┘

┌──────────────────────┐         ┌─────────────────────┐
│  Threshold Processor │  HTTPS  │  External Webhook   │
│  (in air-quality-app)│────────▶│  (Slack, etc.)      │
└──────────────────────┘         └─────────────────────┘
```

**Protocol**: MQTT (TCP), HTTPS
**Library**: `rumqttc`, `reqwest`

---

## Docker Services (Post DP-012)

```yaml
# docker-compose.yml
services:
  mosquitto:
    image: eclipse-mosquitto:2.0
    ports: ["1883:1883"]
    # MQTT broker - external message bus for sensors

  etcd:
    image: quay.io/coreos/etcd:v3.5.11
    ports: ["2379:2379"]
    # Configuration store - stream configs, subscriber configs

  air-quality-app:
    build: ./apps/air-quality-app
    depends_on: [mosquitto, etcd, timescaledb]
    volumes:
      - /data/raw:/data/raw  # Bronze Parquet on NVMe
      - /models:/models       # ML models
    environment:
      - ETCD_ENDPOINT=http://etcd:2379
      - TIMESCALE_URL=postgresql://...
      - MQTT_BROKER=mosquitto:1883
    # Contains:
    #   - MQTT source (subscribes to mosquitto)
    #   - HTTP polling sources
    #   - Event bus (in-process)
    #   - Bronze subscriber (writes Parquet)
    #   - Silver subscriber (writes TimescaleDB)
    #   - Processor subscribers (threshold, ML)

  timescaledb:
    image: timescale/timescaledb:latest-pg15
    ports: ["5432:5432"]
    volumes:
      - timescale_data:/var/lib/postgresql/data
    # Silver layer storage

  grafana:
    image: grafana/grafana:latest
    ports: ["3000:3000"]
    # Dashboards - queries TimescaleDB

  ndp-mcp-server:
    build: ./apps/ndp-mcp-server
    ports: ["9100:9100"]
    # MCP tools for AI agents

  silver-etl:
    build: ./apps/silver-etl
    profiles: ["backfill"]  # Only run on-demand
    # Backfill/reprocessing tool - not primary ingestion path
```

---

## What Changed from Current?

| Aspect | Before (Current) | After (DP-012) |
|--------|------------------|----------------|
| Bronze ingestion | mpsc channel → RawStorageWriter | Event bus → Bronze subscriber |
| Silver ingestion | silver-etl daemon (5 min batch) | Silver subscriber (streaming) |
| silver-etl daemon | Always running | On-demand backfill only |
| Processors | None | Threshold, ML in air-quality-app |
| Inter-process comms | MQTT only (sensors) | MQTT (sensors + alerts) |
| In-process comms | mpsc (single consumer) | broadcast (multi consumer) |

---

## Message Protocols Summary

| Path | Protocol | Notes |
|------|----------|-------|
| Sensors → mosquitto | MQTT (external) | AirGradient devices |
| mosquitto → air-quality-app | MQTT (TCP) | rumqttc client |
| APIs → air-quality-app | HTTPS | reqwest polling |
| Sources → Event Bus | In-process (broadcast) | Zero-copy Arc |
| Event Bus → Subscribers | In-process (broadcast) | Zero-copy Arc |
| Bronze → NVMe | File I/O | Parquet via polars |
| Silver → TimescaleDB | PostgreSQL (TCP) | tokio-postgres |
| ML → TimescaleDB | PostgreSQL (TCP) | Context queries |
| Alerts → mosquitto | MQTT (TCP) | Publish alerts |
| Alerts → Webhook | HTTPS | External integrations |

---

## Why In-Process Event Bus (Not Kafka/Redis)?

| Consideration | In-Process Broadcast | External Broker (Kafka/Redis) |
|---------------|---------------------|-------------------------------|
| Latency | ~microseconds | ~milliseconds |
| Complexity | Single binary | Multiple services to manage |
| Memory | Shared (Arc) | Serialization overhead |
| Pi 5 resources | Minimal | Significant RAM/CPU |
| Failure domain | Single process | Distributed failure modes |
| Persistence | Subscribers handle | Broker handles |
| Scaling | Single node | Multi-node |

**Decision**: In-process is appropriate for single-node Pi deployment. If we needed multi-node, we'd add Kafka/NATS as an external bus.

---

## Scaling Path (Future)

If we outgrow single-node:

```
                    ┌─────────────────┐
                    │   NATS/Kafka    │
                    │  (external bus) │
                    └────────┬────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  Ingestion    │    │   Silver      │    │  Processors   │
│   Node        │    │   Node        │    │   Node        │
│ (sources)     │    │ (transforms)  │    │ (ML, alerts)  │
└───────────────┘    └───────────────┘    └───────────────┘
```

But for Pi 5 with NVMe, single-process is optimal.
