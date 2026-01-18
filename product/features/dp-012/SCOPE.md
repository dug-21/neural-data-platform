# DP-012: Unified Event Bus Architecture with Streaming Subscribers

**Feature ID**: dp-012
**Title**: Unified Event Bus with Config-Driven Subscribers
**Status**: Scope Definition
**Created**: 2026-01-17
**Depends On**: dp-004 (Bronze Layer), dp-006 (Silver Layer), dp-010 (MCP Silver Tools)

---

## Executive Summary

Replace the current point-to-point ingestion pipeline with a **unified event bus architecture** where all data flows through a broadcast channel and all consumers (Bronze storage, Silver ETL, alerts, event notifications) are config-driven **subscribers**.

**Key Outcomes**:
- Data available in Silver within **1-5 seconds** of receipt (vs 5+ minutes today)
- Config-driven subscriber enable/disable - no code changes
- Event notifications via MQTT for external consumers (ML, automation)
- Plugin architecture for parsers/transforms
- Foundation for Gold layer, ML layer, and future consumers

**Architectural Principle**: The ingestion layer (air-quality-app) must remain predictable and never block. Complex workloads like ML/Features belong in separate containers that subscribe to event notifications.

---

## Current State

### Architecture Today

```
Sources (MQTT, HTTP)
        │
        ▼
    mpsc channel (1000 capacity)
        │
        ▼
  RawStorageWriter
  - Batches (50 points / 30s timeout)
  - Writes raw JSON to Parquet
        │
        ▼
    BRONZE LAYER
        │
        │ (5-minute batch ETL daemon)
        ▼
    SILVER LAYER (TimescaleDB)
```

### Current Components

| Component | Location | Purpose |
|-----------|----------|---------|
| `MqttSource` | `core/src/sources/mqtt/` | MQTT subscription, raw JSON capture |
| `HttpPollingSource` | `core/src/sources/http_poll.rs` | HTTP polling, raw JSON capture |
| `RawStorageWriter` | `apps/air-quality-app/src/pipeline/` | Batches and writes to Bronze |
| `ParquetStore` | `core/src/storage/parquet.rs` | Bronze Parquet files |
| `silver-etl daemon` | `apps/silver-etl/` | Batch Bronze→Silver ETL |
| Parsers | `core/src/parsers/` | **NOT USED** - exist but not in Bronze path |

### Current Limitations

| Limitation | Impact |
|------------|--------|
| Silver latency: 5+ minutes | Cannot do real-time ML/alerts |
| Single consumer (Bronze) | Adding consumers requires code changes |
| Parsers unused | Transform logic duplicated in Silver ETL SQL |
| No real-time processing path | ML must query stale data |

---

## Target Architecture

### Unified Event Bus

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              SOURCES                                         │
│                                                                              │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐                     │
│   │ MQTT Source │    │ HTTP Source │    │ Future...   │                     │
│   └──────┬──────┘    └──────┬──────┘    └──────┬──────┘                     │
│          └──────────────────┼──────────────────┘                             │
│                             │                                                │
│                             ▼                                                │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │                         EVENT BUS                                    │   │
│   │              (tokio::broadcast::channel<Arc<RawDataPoint>>)          │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                             │                                                │
└─────────────────────────────┼────────────────────────────────────────────────┘
                              │
    ┌─────────────────────────┼────────────────────┬────────────────────┐
    │                         │                    │                    │
    ▼                         ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   BRONZE     │    │   SILVER     │    │  THRESHOLD   │    │   EVENT      │
│  Subscriber  │    │  Subscriber  │    │  Processor   │    │  NOTIFIER    │
│              │    │              │    │              │    │              │
│ - Raw JSON   │    │ - Transform  │    │ - Field      │    │ - MQTT pub   │
│ - Batching   │    │ - DQ Rules   │    │   checks     │    │ - Fire &     │
│ - Parquet    │    │ - TimescaleDB│    │ - Alerts     │    │   forget     │
│              │    │              │    │ - Webhooks   │    │ - QoS 0      │
│(1-2s latency)│    │(1-5s latency)│    │ (<100ms)     │    │ (< 1ms)      │
└──────────────┘    └──────────────┘    └──────────────┘    └──────────────┘
                                                                   │
                                                                   │ MQTT
                                                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    EXTERNAL CONSUMERS (FUTURE SCOPE)                         │
│                                                                              │
│   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐                  │
│   │ ML Processor │    │ Gold Layer   │    │ S3 Archive   │                  │
│   │ (dp-013+)    │    │ (dp-014+)    │    │ (dp-015+)    │                  │
│   └──────────────┘    └──────────────┘    └──────────────┘                  │
│                                                                              │
│   Subscribes to: ndp/events/{stream_id}                                      │
│   Queries: Silver for context                                                │
│   Runs in: Separate container (never blocks ingestion)                       │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Everything is a Subscriber** - Bronze, Silver, ML, Alerts are all equal consumers
2. **Config-Driven** - Enable/disable subscribers via YAML, not code
3. **Independent Lifecycles** - Subscribers can fail/restart without affecting others
4. **Backpressure Per-Subscriber** - Slow subscriber doesn't block the bus
5. **Reuse Existing Config** - `silver_etl` config in stream YAML is complete and working

---

## Components to Modify

### 1. Core Library (`core/`)

| File/Module | Change Type | Description |
|-------------|-------------|-------------|
| **NEW** `src/event_bus/mod.rs` | Create | Event bus wrapper around broadcast channel |
| **NEW** `src/subscribers/mod.rs` | Create | Subscriber trait and coordinator |
| **NEW** `src/subscribers/bronze.rs` | Create | Bronze subscriber (refactor from RawStorageWriter) |
| **NEW** `src/subscribers/silver.rs` | Create | Streaming Silver subscriber |
| **NEW** `src/subscribers/processor.rs` | Create | Generic processor subscriber wrapper |
| **NEW** `src/subscribers/event_notifier.rs` | Create | MQTT event notification subscriber |
| **NEW** `src/processors/mod.rs` | Create | Processor trait and registry |
| **NEW** `src/processors/threshold.rs` | Create | Threshold/alert processor |
| **NEW** `src/outputs/mod.rs` | Create | Output sink traits (TimescaleDB, MQTT, Webhook) |
| **NEW** `src/silver/mod.rs` | Create | Streaming Silver transform module |
| **NEW** `src/silver/transform.rs` | Create | Port transform logic from silver-etl sql_gen.rs |
| **NEW** `src/silver/dq_evaluator.rs` | Create | Port DQ evaluation from silver-etl dq.rs |
| `src/parsers/` | **DEPRECATE** | Mark as deprecated - not used in Bronze→Silver path |
| `src/traits.rs` | Modify | Add `Subscriber`, `Processor`, `OutputSink` traits |
| `src/config/mod.rs` | Modify | Add subscriber and processor config types |
| `src/config/silver_etl.rs` | No change | **Already complete** - reuse existing config |
| **NEW** `src/config/subscribers.rs` | Create | Subscriber configuration schema |
| **NEW** `src/config/processors.rs` | Create | Processor configuration schema |

### 2. Air Quality App (`apps/air-quality-app/`)

| File | Change Type | Description |
|------|-------------|-------------|
| `src/main.rs` | Modify | Replace mpsc with event bus, start subscriber coordinator |
| `src/pipeline/storage_writer.rs` | Deprecate | Functionality moves to Bronze subscriber |
| `src/coordinator/ingestion_coordinator.rs` | Modify | Wire sources to event bus instead of mpsc |
| `src/coordinator/source_manager.rs` | Modify | Sources publish to event bus |
| **NEW** `src/subscriber_coordinator.rs` | Create | Manages subscriber lifecycles |

### 3. Silver ETL (`apps/silver-etl/`)

| File | Change Type | Description |
|------|-------------|-------------|
| `src/daemon.rs` | Modify | Becomes backfill-only mode |
| `src/sql_gen.rs` | Reference | **Port logic to core** - transform concepts move to `core/src/silver/` |
| `src/dq.rs` | Reference | **Port logic to core** - DQ concepts move to `core/src/silver/` |
| `src/runner.rs` | No change | Continues to work for batch backfill |

**Note**: We're not modifying silver-etl to export functions. Instead, we're **porting the transform concepts** from its SQL generation to Rust functions in core. The silver-etl app continues to use DuckDB SQL for batch backfill, while streaming Silver uses the new Rust implementation in core.

### 4. Config Client (`config-client/`)

| File | Change Type | Description |
|------|-------------|-------------|
| `src/lib.rs` | Modify | Add subscriber config loading |
| **NEW** `src/subscriber_registry.rs` | Create | Load/watch subscriber configs from etcd |

### 5. Configuration (`config/`)

| Path | Change Type | Description |
|------|-------------|-------------|
| **NEW** `base/platform.yaml` | Create | Platform-level subscriber definitions |
| **NEW** `base/processors/*.yaml` | Create | Processor definitions (threshold, ML, etc.) |
| `base/streams/*/config.yaml` | Modify | Add `subscribers:` section for per-stream overrides |
| **NEW** `schemas/subscriber.schema.json` | Create | JSON Schema for subscriber config |
| **NEW** `schemas/processor.schema.json` | Create | JSON Schema for processor config |

### 6. Deployment (`deploy/`)

| File | Change Type | Description |
|------|-------------|-------------|
| `pi/docker-compose.yml` | Modify | silver-etl-daemon becomes optional/backfill |
| `pi/.env.example` | Modify | Add subscriber configuration env vars |

---

## Configuration Schema

### Platform-Level Subscribers

```yaml
# config/base/platform.yaml
event_bus:
  capacity: 10000
  lag_warning_threshold: 1000

subscribers:
  # Bronze Layer - raw archival (REQUIRED)
  - id: bronze
    type: storage
    enabled: true
    config:
      format: parquet
      path: /data/raw/{stream_id}
      partitioning: daily
      batch_size: 50
      batch_timeout_secs: 2
      wal_enabled: true

  # Silver Layer - streaming analytics (REQUIRED)
  - id: silver
    type: timescale
    enabled: true
    config:
      connection_string: ${TIMESCALE_URL}
      batch_size: 100
      batch_timeout_secs: 5
      use_stream_etl_config: true  # Reuse silver_etl from stream configs

  # Threshold Alerts (OPTIONAL)
  - id: threshold-alerts
    type: processor
    enabled: true
    processor_id: threshold-alerts  # References /processors/threshold-alerts.yaml

  # Event Notifier - MQTT notifications for external consumers
  # Enables ML, Gold layer, and other future consumers without code changes
  - id: event-notifier
    type: notifier
    enabled: ${EVENT_NOTIFIER_ENABLED:-false}  # Toggle via env var
    config:
      mqtt_broker: ${MQTT_BROKER:-mosquitto:1883}
      topic_pattern: "ndp/events/{stream_id}"
      qos: 0  # Fire-and-forget, never block
      payload_fields:
        - stream_id
        - ndp_id
        - timestamp
      # NEVER include raw_payload - consumers query Silver for data

  # Future: S3 Archive (not in this scope)
  - id: s3-archive
    type: storage
    enabled: false
    config:
      format: parquet
      path: s3://${S3_BUCKET}/archive/{stream_id}
```

### Processor Definition

```yaml
# config/base/processors/threshold-alerts.yaml
processor_id: threshold-alerts
type: threshold
version: "1.0.0"
description: "Air quality threshold alerts"

config:
  rules:
    - name: pm25_unhealthy
      stream_filter: ["air-quality"]  # Only these streams
      field: raw_payload.pm02Compensated
      condition: "> 35.4"
      severity: warning
      message: "PM2.5 exceeds EPA 'Unhealthy for Sensitive Groups'"
      cooldown_secs: 300  # Don't re-alert for 5 min

    - name: co2_high
      stream_filter: ["air-quality"]
      field: raw_payload.rco2
      condition: "> 1000"
      severity: warning
      message: "CO2 elevated - consider ventilation"

outputs:
  - type: mqtt
    topic: "ndp/alerts/{stream_id}/{severity}"

  - type: timescale
    table: silver.alerts

  - type: webhook
    url: ${ALERT_WEBHOOK_URL}
    method: POST
```

### Event Notifier Design

The Event Notifier is intentionally minimal - it publishes lightweight notifications to enable external consumers without any risk of blocking ingestion.

```rust
// core/src/subscribers/event_notifier.rs
// Extremely simple - no ML code, no complex logic

pub struct EventNotifier {
    mqtt_client: AsyncClient,
    enabled: bool,           // From config/env var
    topic_pattern: String,   // e.g., "ndp/events/{stream_id}"
}

#[async_trait]
impl Subscriber for EventNotifier {
    async fn handle(&self, event: Arc<RawDataPoint>) -> Result<(), SubscriberError> {
        if !self.enabled {
            return Ok(());  // No-op when disabled
        }

        let topic = self.topic_pattern.replace("{stream_id}", &event.stream_id());
        let payload = json!({
            "stream_id": event.stream_id(),
            "ndp_id": event.ndp_id,
            "timestamp": event.timestamp,
        });

        // QoS 0 = Fire-and-forget. Never blocks, never waits for ACK.
        // If MQTT broker is down, we just drop the notification.
        let _ = self.mqtt_client.try_publish(&topic, QoS::AtMostOnce, false, payload);

        Ok(())
    }
}
```

**Design Principles**:
- **Never block**: QoS 0 (at-most-once), non-blocking publish
- **Lightweight payload**: Only IDs and timestamp - consumers query Silver for data
- **Configurable**: Enable/disable via `EVENT_NOTIFIER_ENABLED` env var
- **Future-proof**: When ML layer is ready, just enable the notifier - no code changes

### ML Processor (Future Reference - dp-013+)

When ML is implemented as a **separate container**, it will:

1. **Subscribe to MQTT** topic `ndp/events/{stream_id}`
2. **Query Silver** for context (recent readings, aggregates)
3. **Run inference** without affecting ingestion
4. **Write predictions** back to Silver

```yaml
# FUTURE: config/base/processors/ml-air-quality.yaml (dp-013+)
# This will run in ndp-ml-processor container, NOT air-quality-app

processor_id: ml-air-quality
type: ml_inference
version: "1.0.0"
description: "Air quality predictions using Silver context"

config:
  trigger:
    type: mqtt
    topic: "ndp/events/air-quality"

  context:
    source: silver.air_quality_observations
    window: 30 minutes
    connection_string: ${TIMESCALE_URL}

  model:
    type: onnx
    path: /models/air-quality-forecast.onnx

outputs:
  - type: timescale
    table: silver.predictions
```

This is **out of scope for dp-012** - listed here as design reference for future implementation.

### Stream-Level Overrides

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
# ... existing config ...

# Override which subscribers process this stream
subscribers:
  bronze: true       # Default
  silver: true       # Default
  threshold-alerts: true
  ml-predictions: true
  s3-archive: false  # Not for this stream
```

---

## Silver Transform Logic (from silver-etl)

### Existing Assets

The transform logic already exists and is working in `apps/silver-etl/`:

| File | Purpose | Reuse Strategy |
|------|---------|----------------|
| `sql_gen.rs` | Generates DuckDB SQL for transforms | Port concepts to Rust functions |
| `dq.rs` | Generates DQ flag SQL expressions | Port to Rust evaluation |
| `pre_transform.rs` | Array explosion (PIVOT) | Port if needed for NWS |
| `config.rs` | Loads `SilverEtlConfig` from etcd | Reuse directly |

### Config is Complete

The `silver_etl` section in stream configs is **already complete and working**:

```yaml
# Already in config/base/streams/air-quality/config.yaml
silver_etl:
  target_table: silver.air_quality_observations
  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
```

The config types are already in `core/src/config/silver_etl.rs` (~2000 lines):
- `SilverEtlConfig` - Main config struct
- `SilverFieldMapping` - Field mapping with transforms
- `DqRule` - All 11 DQ rule types
- `TransformConfig` - Unit conversion, expression, lookup

### What We're Building

Port the **transform concepts** from SQL generation to Rust functions:

```rust
// NEW: core/src/silver/transform.rs

use crate::config::SilverEtlConfig;
use crate::types::RawDataPoint;

/// Transform a RawDataPoint to Silver row using existing config
pub fn transform_to_silver(
    point: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<SilverRow, TransformError> {
    let mut row = SilverRow::new();

    // Apply timestamp transform
    row.timestamp = apply_timestamp_transform(
        &point.timestamp,
        &point.raw_payload,
        &config.timestamp,
    )?;

    // Apply identity fields
    for identity in &config.identity_fields {
        row.set(&identity.target, extract_identity(point, identity)?);
    }

    // Apply field mappings (same logic as sql_gen.rs, but in Rust)
    for mapping in &config.field_mappings {
        let value = extract_json_path(&point.raw_payload, &mapping.source_path)?;
        let typed_value = apply_type_cast(value, &mapping.column_type)?;
        let final_value = apply_transform(typed_value, &mapping.transform)?;
        row.set(&mapping.target_column, final_value);
    }

    // Evaluate DQ rules
    row.dq_flags = evaluate_dq_rules(&row, &config.dq_rules)?;

    Ok(row)
}
```

### Parser Deprecation

The parsers in `core/src/parsers/` are **deprecated** as of dp-012:

| Parser | Status | Reason |
|--------|--------|--------|
| `FlatJsonParser` | Deprecated | Bronze stores raw JSON, not TimeSeriesPoint |
| `JsonPathParser` | Deprecated | Silver uses `silver_etl` config, not parser config |
| `ArrayIteratorParser` | Deprecated | NWS handled by silver_etl pre_transform |
| `ColumnOrientedParser` | Deprecated | Same - silver_etl handles column-oriented data |

These produced `TimeSeriesPoint` (one row per metric value), which is the wrong output format. Silver needs one row per reading with all metrics as columns - exactly what `silver_etl` config provides.

**Action**: Add deprecation warnings to parser modules. Do not delete - may be useful for future metric-oriented use cases.

---

## Data Consistency & Recovery

### The Challenge

Streaming introduces gap risk that batch ETL doesn't have:

| Scenario | Batch ETL | Streaming |
|----------|-----------|-----------|
| Silver DB temporarily down | Next run catches up | Events lost from channel |
| Subscriber crashes | N/A | Events during downtime lost |
| Broadcast lag (slow subscriber) | N/A | Lagged events dropped |
| Insert fails mid-batch | Next run retries all | Partial batch lost |

### Design Principle

**Bronze is the source of truth.** As long as data is in Bronze Parquet, we can always recover Silver. Streaming is an optimization for latency, not a replacement for the Bronze→Silver relationship.

### Hybrid Streaming + Catch-up Design

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SILVER SUBSCRIBER LIFECYCLE                          │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     ON STARTUP: CATCH-UP MODE                        │    │
│  │                                                                      │    │
│  │  1. Query Silver: SELECT MAX(observation_time) FROM silver.table    │    │
│  │  2. Read Bronze Parquet files since that watermark                  │    │
│  │  3. Transform + UPSERT (same logic as streaming)                    │    │
│  │  4. Switch to streaming mode                                        │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     STREAMING MODE (NORMAL)                          │    │
│  │                                                                      │    │
│  │  - Receive events from broadcast channel                            │    │
│  │  - Transform using silver_etl config                                │    │
│  │  - Batch + UPSERT to TimescaleDB                                    │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                    │                                         │
│                                    ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                     ON LAG/ERROR: CONTINUE                           │    │
│  │                                                                      │    │
│  │  - If broadcast::RecvError::Lagged(n): log warning, continue        │    │
│  │  - Lagged events will be caught up on next restart                  │    │
│  │  - If DB error: retry with backoff, then continue                   │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Startup Catch-up Implementation

```rust
impl SilverSubscriber {
    async fn start(&mut self) -> Result<(), SubscriberError> {
        // PHASE 1: Catch-up from Bronze
        let watermark = self.get_silver_watermark().await?;
        info!("Silver watermark: {:?}, catching up from Bronze", watermark);

        let bronze_files = self.list_bronze_files_since(watermark).await?;
        for file in bronze_files {
            let points = self.read_bronze_parquet(&file).await?;
            for point in points {
                let row = transform_to_silver(&point, &self.config)?;
                self.buffer.push(row);
            }
            self.flush_upsert().await?;  // UPSERT handles duplicates
        }

        info!("Catch-up complete, switching to streaming mode");

        // PHASE 2: Streaming mode
        self.run_streaming_loop().await
    }
}
```

### UPSERT Semantics

Same pattern as silver-etl uses today - duplicates are handled gracefully:

```sql
INSERT INTO silver.air_quality_observations (observation_time, ndp_id, pm25, ...)
VALUES ($1, $2, $3, ...)
ON CONFLICT (observation_time, ndp_id)
DO UPDATE SET pm25 = EXCLUDED.pm25, ...
```

- Streaming can re-process same data without issues
- Catch-up + streaming may overlap - no problem
- Idempotent by design

### Lag Handling

```rust
match receiver.recv().await {
    Ok(point) => { /* process */ }
    Err(broadcast::error::RecvError::Lagged(n)) => {
        warn!("Silver subscriber lagged, missed {} events", n);
        metrics.lagged_events.add(n);
        // Continue - will catch up on next restart
    }
    Err(broadcast::error::RecvError::Closed) => break,
}
```

### Manual Backfill (silver-etl)

For extended outages or config changes, operator can run:

```bash
# Backfill specific time range
silver-etl backfill --stream air-quality --since "2026-01-01" --until "2026-01-15"

# Reprocess after config change (e.g., new DQ rule)
silver-etl backfill --stream air-quality --reprocess --since "2026-01-01"
```

### Recovery Guarantees Summary

| Scenario | Recovery Mechanism |
|----------|-------------------|
| Normal restart | Automatic catch-up from Bronze on startup |
| Lag (slow subscriber) | Automatic catch-up on next restart |
| DB temporarily down | Retry with backoff + catch-up on restart |
| Extended outage | Operator runs `silver-etl backfill` |
| Config change (new fields/rules) | Operator runs `silver-etl backfill --reprocess` |

---

## Implementation Phases

### Phase 1: Event Bus Foundation (Week 1)

**Goal**: Replace mpsc with broadcast event bus, Bronze as first subscriber

| Task | Component | Estimate |
|------|-----------|----------|
| Create `EventBus` struct | `core/src/event_bus/` | 0.5 day |
| Create `Subscriber` trait | `core/src/subscribers/traits.rs` | 0.5 day |
| Create `SubscriberCoordinator` | `core/src/subscribers/coordinator.rs` | 1 day |
| Create `BronzeSubscriber` | `core/src/subscribers/bronze.rs` | 1 day |
| Wire into `air-quality-app` | `apps/air-quality-app/src/main.rs` | 1 day |
| Validate Bronze still works | Testing | 0.5 day |

**Deliverable**: Bronze ingestion works via event bus (no behavior change)

### Phase 2: Streaming Silver Subscriber (Week 2)

**Goal**: Silver as streaming subscriber, data in Silver within 5 seconds

| Task | Component | Estimate |
|------|-----------|----------|
| Create `core/src/silver/` module | Transform logic ported from silver-etl | 2 days |
| - `transform.rs` | Field mapping, JSON extraction, type casting | (included) |
| - `dq_evaluator.rs` | DQ rule evaluation in Rust | (included) |
| Create `SilverSubscriber` | `core/src/subscribers/silver.rs` | 1 day |
| - Startup catch-up from Bronze | Read Parquet since watermark | (included) |
| - Streaming loop with UPSERT | Batch + write to TimescaleDB | (included) |
| TimescaleDB output sink | `core/src/outputs/timescale.rs` | 1 day |
| Integration testing | Verify streaming + catch-up | 1 day |

**Key Insight**: We're porting transform concepts from `silver-etl/sql_gen.rs` to Rust, using the **existing** `SilverEtlConfig` from stream configs. No new config needed.

**Data Consistency**: Subscriber catches up from Bronze on startup, ensuring no data loss even if streaming lags or crashes.

**Deliverable**: Data flows to Silver within 1-5 seconds of receipt, with automatic recovery

### Phase 3: Processor Framework & Event Notifier (Week 3)

**Goal**: Config-driven processors for alerts, plus Event Notifier for future ML integration

| Task | Component | Estimate |
|------|-----------|----------|
| Create `Processor` trait | `core/src/processors/traits.rs` | 0.5 day |
| Create `ProcessorRegistry` | `core/src/processors/registry.rs` | 0.5 day |
| Create `ProcessorSubscriber` wrapper | `core/src/subscribers/processor.rs` | 1 day |
| Implement `ThresholdProcessor` | `core/src/processors/threshold.rs` | 1 day |
| Implement `EventNotifier` subscriber | `core/src/subscribers/event_notifier.rs` | 0.5 day |
| MQTT output sink | `core/src/outputs/mqtt.rs` | 0.5 day |
| Webhook output sink | `core/src/outputs/webhook.rs` | 0.5 day |
| Config schema and env var toggle | `config/` | 0.5 day |

**Deliverable**: Threshold alerts working, Event Notifier ready (disabled by default)

### Phase 4: Polish and Documentation (Week 4)

| Task | Description | Estimate |
|------|-------------|----------|
| silver-etl backfill mode | Modify daemon for backfill-only | 1 day |
| Grafana dashboard | Subscriber metrics visualization | 1 day |
| MCP tools for subscribers | List/status/metrics tools | 1 day |
| Documentation | Architecture, config reference | 1 day |
| Performance testing | Validate latency targets | 1 day |

---

## Success Criteria

| Criterion | Target | Validation |
|-----------|--------|------------|
| Bronze latency | < 2 seconds | Measure timestamp delta |
| Silver latency | < 5 seconds | Measure event→Silver query |
| Silver catch-up on restart | No data loss | Stop subscriber, ingest data, restart, verify all data in Silver |
| Silver UPSERT idempotency | No duplicates | Process same data twice, verify single row per key |
| Subscriber config changes | No code deploy | Change YAML, verify behavior |
| Event Notifier toggle | Enable via env var | Set `EVENT_NOTIFIER_ENABLED=true`, verify MQTT publish |
| Event bus throughput | > 1000 events/sec | Load test |
| Memory overhead | < 100MB above baseline | Monitor during load |
| Subscriber failure isolation | One fails, others continue | Kill subscriber, verify others |
| Event Notifier non-blocking | Never affects ingestion | MQTT broker down, verify Bronze/Silver unaffected |

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Broadcast lag on slow subscriber | Medium | Medium | Per-subscriber lag monitoring, drop policy config |
| Silver transform logic divergence | Medium | Medium | Test streaming vs batch ETL produce same results |
| Silver subscriber SQL errors | Medium | Medium | Retry logic, dead letter queue |
| Migration data loss | Low | High | Run old and new in parallel initially |
| Complexity increase | Low | Medium | Simplified scope - no parser plugins, reuse config |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-004 Bronze Layer | ✅ Complete | RawDataPoint schema in use |
| dp-006 Silver Layer | ✅ Complete | TimescaleDB tables exist |
| dp-010 MCP Silver Tools | ✅ Complete | Can query Silver for ML context |
| silver-etl transform logic | ✅ Complete | Port concepts from sql_gen.rs to Rust |
| SilverEtlConfig | ✅ Complete | Already in core/src/config/silver_etl.rs |
| tokio broadcast channel | ✅ Available | In tokio crate |
| tokio-postgres or sqlx | ✅ Available | For Silver subscriber DB writes |

---

## Out of Scope

| Item | Reason | Future |
|------|--------|--------|
| ML Processor | Separate container, different workload patterns | dp-013 |
| Gold layer subscriber | Separate feature | dp-014+ |
| S3 archive subscriber | Separate feature | dp-015+ |
| Parser plugin architecture | Parsers deprecated, silver_etl config is sufficient | Not planned |
| Distributed event bus | Single-node sufficient for Pi | Future if needed |
| Exactly-once semantics | At-least-once sufficient | Future if needed |

### ML Processor Exclusion Rationale

The ML Processor is **intentionally excluded** from dp-012 because:

1. **Workload Isolation**: ML inference has unpredictable latency and resource consumption. It must never block the ingestion path.
2. **Separate Container**: ML will run in its own container (`ndp-ml-processor`) with its own resource limits.
3. **Event-Driven Integration**: The Event Notifier (in-scope) provides the hook for ML integration via MQTT.
4. **Zero Code Changes When ML Added**: By implementing the Event Notifier now, ML can be added later by:
   - Creating `ndp-ml-processor` container
   - Subscribing to `ndp/events/{stream_id}` topics
   - Setting `EVENT_NOTIFIER_ENABLED=true`
   - No changes to `air-quality-app` required

---

## References

- [Research: Unified Event Bus Architecture](../../research/platform-minimize/07-unified-event-bus-architecture.md)
- [Research: Real-Time Processing Layer Design](../../research/platform-minimize/06-realtime-processing-layer-design.md)
- [Research: Deployment Architecture](../../research/platform-minimize/08-deployment-architecture.md)
- [dp-004 SCOPE](../dp-004/SCOPE.md) - Bronze Layer
- [dp-006 SCOPE](../dp-006/SCOPE.md) - Silver Layer

---

*Scope defined: 2026-01-17*
*Updated: 2026-01-18 - ML moved to future scope (dp-013), Event Notifier added*
*Updated: 2026-01-18 - Parsers deprecated, silver-etl transform logic to be ported to core*
*Updated: 2026-01-18 - Data consistency section: catch-up from Bronze on startup, UPSERT semantics*
