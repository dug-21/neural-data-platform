# DP-012 SPARC Architecture (SPARC-A)

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Phase**: Architecture
**Created**: 2026-01-18
**Status**: Complete

---

## 1. Executive Summary

This document defines the architectural decisions, component relationships, and system design for DP-012. It includes Architecture Decision Records (ADRs), component diagrams, and integration specifications.

---

## 2. Architecture Decision Records

### ADR-012-001: Event Bus Implementation with tokio::broadcast

**Status**: Accepted

**Context**:
The system needs to replace the current single-consumer mpsc channel with a multi-consumer broadcast mechanism. Options considered:
1. External message broker (Redis Streams, NATS, Kafka)
2. tokio::broadcast channel (in-process)
3. Custom ring buffer with multiple readers

**Decision**:
Use `tokio::broadcast` channel for the event bus.

**Rationale**:
- **Simplicity**: No external dependencies, no network overhead
- **Performance**: Zero-copy with Arc<RawDataPoint>, < 1ms broadcast latency
- **Sufficient for scale**: 1000+ events/sec is well within broadcast capacity
- **Pi-appropriate**: No additional memory/CPU for external broker
- **Future-proof**: Can add external broker later if distributed deployment needed

**Consequences**:
- (+) Simple deployment - no additional services
- (+) Excellent performance for single-node deployment
- (+) Easy to test and reason about
- (-) Not suitable for distributed deployment without redesign
- (-) Message history not preserved (subscribers must catch up from Bronze)

**Alternatives Rejected**:
- External broker: Overkill for single Pi deployment, adds operational complexity
- Custom ring buffer: Re-inventing the wheel when tokio::broadcast exists

---

### ADR-012-002: Subscriber Isolation via Independent Tokio Tasks

**Status**: Accepted

**Context**:
Subscribers need to be isolated so one failure doesn't affect others. Options:
1. Sequential processing in single task
2. Independent tokio tasks per subscriber
3. Separate processes/containers per subscriber

**Decision**:
Each subscriber runs in its own tokio task, spawned by SubscriberCoordinator.

**Rationale**:
- **Isolation**: Task panic doesn't crash the application
- **Independence**: Each subscriber has its own receive loop and buffering
- **Simple**: No IPC overhead of separate processes
- **Monitorable**: JoinHandle allows health monitoring

**Consequences**:
- (+) Subscriber failure isolated
- (+) Independent backpressure handling
- (+) Easy to add/remove subscribers dynamically
- (-) All share same process memory limits
- (-) CPU-intensive subscriber could starve others (mitigated by async nature)

---

### ADR-012-003: Streaming Silver with Bronze Catch-up

**Status**: Accepted

**Context**:
Silver needs < 5s latency but must handle restarts, lag, and DB outages without data loss. Options:
1. Pure streaming (accept data loss on lag/crash)
2. Streaming with WAL
3. Streaming with Bronze catch-up on startup

**Decision**:
Silver subscriber catches up from Bronze Parquet on startup, then streams.

**Rationale**:
- **Bronze is source of truth**: Already durable via WAL
- **No duplicate storage**: Don't need separate WAL for Silver
- **Simple recovery**: Query MAX(observation_time), read Parquet since then
- **UPSERT idempotency**: Duplicates from catch-up are handled gracefully

**Consequences**:
- (+) Zero data loss on restart
- (+) Handles lag gracefully (catch up on next restart)
- (+) Simple implementation
- (-) Startup time proportional to catch-up window
- (-) Requires Bronze to be available for catch-up

**Catch-up Algorithm**:
```
1. Query Silver: SELECT MAX(observation_time) FROM table
2. List Bronze files modified after watermark
3. Read and transform each file
4. UPSERT to Silver (handles duplicates)
5. Switch to streaming mode
```

---

### ADR-012-004: Config Reuse - SilverEtlConfig for Streaming

**Status**: Accepted

**Context**:
Streaming Silver needs transform configuration. Options:
1. Create new config format for streaming
2. Reuse existing SilverEtlConfig from stream YAMLs
3. Share code with silver-etl

**Decision**:
Reuse existing `SilverEtlConfig` structure. Port transform concepts from silver-etl SQL generation to Rust functions.

**Rationale**:
- **Config is complete**: ~2000 lines of working configuration
- **No migration needed**: Same YAML, same behavior
- **Consistency**: Streaming and batch produce identical output
- **Maintainability**: One source of truth for transforms

**Consequences**:
- (+) Zero config migration
- (+) Same behavior guaranteed
- (+) Operators already familiar with config
- (-) Must port SQL concepts to Rust (one-time effort)
- (-) Two implementations to maintain (Rust streaming, DuckDB batch)

---

### ADR-012-005: Parser Deprecation

**Status**: Accepted

**Context**:
Parsers in `core/src/parsers/` produce `TimeSeriesPoint` (one row per metric). Silver needs one row per reading with all metrics as columns.

**Decision**:
Deprecate parsers. Silver transforms use `SilverEtlConfig.field_mappings` which produce the correct columnar format.

**Rationale**:
- **Wrong output format**: Parsers produce metric-oriented data
- **Duplicate logic**: Transform logic already in silver-etl config
- **Unused in Bronze**: Bronze stores raw JSON, doesn't use parsers
- **Cleaner architecture**: One transform path, not two

**Consequences**:
- (+) Simpler architecture
- (+) No confusion about which transform to use
- (-) May need to resurrect for future metric-oriented use cases

**Migration**:
- Add deprecation warnings to parser modules
- Do not delete - keep for potential future use
- Document in CLAUDE.md that parsers are deprecated

---

### ADR-012-006: Event Notifier for ML Integration

**Status**: Accepted

**Context**:
ML processing needs real-time triggers but must not run in the ingestion process. Options:
1. ML as in-process subscriber (rejected: unpredictable workload)
2. Polling Silver from ML container
3. MQTT notifications from Event Notifier

**Decision**:
Implement EventNotifier subscriber that publishes lightweight MQTT notifications. ML (dp-013+) will run in separate container and subscribe to these notifications.

**Rationale**:
- **Decoupling**: ML container can be developed/deployed independently
- **Zero code change for ML**: When ML is ready, just enable notifier
- **Fire-and-forget**: QoS 0 means never blocks ingestion
- **Minimal payload**: Only IDs + timestamp, ML queries Silver for data

**Consequences**:
- (+) Perfect isolation between ingestion and ML
- (+) ML can crash/restart without affecting data pipeline
- (+) Simple integration point for future consumers
- (-) Slight delay for ML to query Silver after notification
- (-) MQTT broker becomes a dependency (already have Mosquitto)

---

### ADR-012-007: Threshold Processor Design

**Status**: Accepted

**Context**:
Real-time alerting needs to evaluate conditions on incoming data. Options:
1. Inline evaluation in Silver subscriber
2. Dedicated Processor trait with ProcessorSubscriber wrapper
3. External alerting service

**Decision**:
Create Processor trait. ThresholdProcessor implements it. ProcessorSubscriber wraps processors as Subscriber.

**Rationale**:
- **Separation of concerns**: Processors focus on logic, not event bus plumbing
- **Reusability**: Processor pattern works for thresholds, anomaly detection, etc.
- **Testability**: Processors testable without event bus
- **Configurability**: Processor configs separate from subscriber configs

**Consequences**:
- (+) Clean separation of concerns
- (+) Easy to add new processor types
- (+) Processor testable in isolation
- (-) Additional abstraction layer
- (-) ProcessorSubscriber wrapper adds small overhead

---

## 3. Component Architecture

### 3.1 Module Structure

```
core/src/
├── event_bus/
│   ├── mod.rs           # EventBus struct, EventBusConfig, EventBusError
│   └── metrics.rs       # EventBusMetrics
│
├── subscribers/
│   ├── mod.rs           # Subscriber trait, HealthStatus, SubscriberError
│   ├── coordinator.rs   # SubscriberCoordinator
│   ├── bronze.rs        # BronzeSubscriber
│   ├── silver.rs        # SilverSubscriber
│   ├── processor.rs     # ProcessorSubscriber<P: Processor>
│   └── event_notifier.rs # EventNotifier
│
├── silver/
│   ├── mod.rs           # Module exports
│   ├── transform.rs     # transform_to_silver(), field extraction
│   ├── dq_evaluator.rs  # evaluate_dq_rules()
│   └── types.rs         # SilverRow, SqlValue
│
├── processors/
│   ├── mod.rs           # Processor trait, ProcessorOutput, ProcessorError
│   ├── threshold.rs     # ThresholdProcessor
│   └── registry.rs      # ProcessorRegistry (optional)
│
├── outputs/
│   ├── mod.rs           # OutputSink trait, OutputError
│   ├── mqtt.rs          # MqttOutputSink
│   ├── timescale.rs     # TimescaleOutputSink
│   └── webhook.rs       # WebhookOutputSink (future)
│
├── config/
│   ├── ...              # Existing config modules
│   ├── subscribers.rs   # SubscriberConfig, BronzeSubscriberConfig, etc.
│   └── processors.rs    # ProcessorConfig, ThresholdRule
│
└── parsers/             # DEPRECATED - add warnings
    └── ...
```

### 3.2 Dependency Graph

```
                    ┌─────────────────────┐
                    │     event_bus       │
                    │   (EventBus)        │
                    └─────────┬───────────┘
                              │
              ┌───────────────┼───────────────┐
              │               │               │
              ▼               ▼               ▼
    ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
    │   subscribers   │ │    silver       │ │   processors    │
    │                 │ │                 │ │                 │
    │ - Subscriber    │ │ - transform     │ │ - Processor     │
    │ - Coordinator   │ │ - dq_evaluator  │ │ - Threshold     │
    │ - Bronze        │ │ - SilverRow     │ │                 │
    │ - Silver ───────┼─┤                 │ └────────┬────────┘
    │ - Processor ────┼─┼─────────────────┼──────────┘
    │ - EventNotifier │ │                 │
    └────────┬────────┘ └────────┬────────┘
             │                   │
             │                   │
             ▼                   ▼
    ┌─────────────────┐ ┌─────────────────┐
    │    outputs      │ │     config      │
    │                 │ │                 │
    │ - OutputSink    │ │ - SilverEtl     │
    │ - MQTT          │ │ - Subscribers   │
    │ - Timescale     │ │ - Processors    │
    └─────────────────┘ └─────────────────┘
             │
             ▼
    ┌─────────────────┐
    │    storage      │  (existing)
    │                 │
    │ - RawStore      │
    │ - ParquetStore  │
    └─────────────────┘
```

### 3.3 Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                           AIR-QUALITY-APP                               │
│                                                                         │
│  ┌───────────────┐    ┌───────────────┐    ┌───────────────────────┐   │
│  │  MQTT Source  │    │  HTTP Source  │    │  Future Sources...    │   │
│  └───────┬───────┘    └───────┬───────┘    └───────────┬───────────┘   │
│          │                    │                        │               │
│          └────────────────────┴────────────────────────┘               │
│                               │                                        │
│                               ▼                                        │
│          ┌────────────────────────────────────────────┐                │
│          │              EVENT BUS                      │                │
│          │    tokio::broadcast<Arc<RawDataPoint>>     │                │
│          │                                            │                │
│          │  capacity: 10,000                          │                │
│          │  lag_warning_threshold: 1,000              │                │
│          └─────────────┬──────────────────────────────┘                │
│                        │                                               │
│       ┌────────────────┼────────────────┬─────────────────┐            │
│       │                │                │                 │            │
│       ▼                ▼                ▼                 ▼            │
│  ┌─────────┐     ┌─────────┐     ┌───────────┐     ┌───────────┐      │
│  │ BRONZE  │     │ SILVER  │     │ PROCESSOR │     │  EVENT    │      │
│  │Subscriber│    │Subscriber│    │ Subscriber│     │ NOTIFIER  │      │
│  │         │     │         │     │           │     │           │      │
│  │ batch=50│     │ batch=100│    │ Threshold │     │ QoS=0     │      │
│  │ 2s flush│     │ 5s flush│    │ Processor │     │fire+forget│      │
│  └────┬────┘     └────┬────┘    └─────┬─────┘     └─────┬─────┘      │
│       │               │               │                 │            │
└───────┼───────────────┼───────────────┼─────────────────┼────────────┘
        │               │               │                 │
        ▼               ▼               ▼                 ▼
  ┌───────────┐   ┌───────────┐   ┌───────────┐   ┌───────────┐
  │  PARQUET  │   │TIMESCALEDB│   │   MQTT    │   │   MQTT    │
  │  (Bronze) │   │  (Silver) │   │  (Alerts) │   │ (Events)  │
  │           │   │           │   │           │   │           │
  │/data/raw/ │   │silver.*   │   │ndp/alerts/│   │ndp/events/│
  └───────────┘   └───────────┘   └───────────┘   └───────────┘
```

---

## 4. Integration Architecture

### 4.1 Current Architecture (Before DP-012)

```
┌──────────────────────────────────────────────────────────────────┐
│                        AIR-QUALITY-APP                            │
│                                                                   │
│  Sources ──► mpsc(1000) ──► RawStorageWriter ──► Parquet (Bronze)│
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
                                │
                          (5-min cron)
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│                         SILVER-ETL                                │
│                                                                   │
│  Read Parquet ──► DuckDB Transform ──► Write TimescaleDB         │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 4.2 Target Architecture (After DP-012)

```
┌──────────────────────────────────────────────────────────────────┐
│                        AIR-QUALITY-APP                            │
│                                                                   │
│  Sources ──► EventBus ──┬──► BronzeSubscriber ──► Parquet        │
│                         ├──► SilverSubscriber ──► TimescaleDB    │
│                         ├──► ProcessorSubscriber ──► MQTT/DB     │
│                         └──► EventNotifier ──► MQTT              │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
                                │
                          (backfill only)
                                │
                                ▼
┌──────────────────────────────────────────────────────────────────┐
│                   SILVER-ETL (Backfill Mode)                      │
│                                                                   │
│  Manual backfill for extended outages or config changes          │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 4.3 Future Architecture (With ML - dp-013+)

```
┌──────────────────────────────────────────────────────────────────┐
│                        AIR-QUALITY-APP                            │
│                                                                   │
│  Sources ──► EventBus ──┬──► Bronze, Silver, Threshold           │
│                         └──► EventNotifier ──► MQTT ─────────────┼───┐
│                                                                   │   │
└──────────────────────────────────────────────────────────────────┘   │
                                                                       │
                        ┌──────────────────────────────────────────────┘
                        │
                        ▼
┌──────────────────────────────────────────────────────────────────┐
│                    NDP-ML-PROCESSOR                               │
│                    (Separate Container)                           │
│                                                                   │
│  Subscribe MQTT ──► Query Silver ──► Inference ──► Write Silver  │
│                                                                   │
│  - Never blocks ingestion                                        │
│  - Can crash/restart independently                               │
│  - Scales separately                                             │
└──────────────────────────────────────────────────────────────────┘
```

---

## 5. Configuration Architecture

### 5.1 Configuration Hierarchy

```
config/
├── base/
│   ├── platform.yaml           # Platform-level settings
│   │   ├── event_bus:          # EventBus config
│   │   └── subscribers:        # Subscriber definitions
│   │
│   ├── processors/             # Processor definitions
│   │   └── threshold-alerts.yaml
│   │
│   └── streams/                # Per-stream configs (existing)
│       ├── air-quality/
│       │   └── config.yaml     # Includes silver_etl section
│       └── outdoor-weather/
│           └── config.yaml
│
└── schemas/                    # JSON Schemas for validation
    ├── platform.schema.json
    ├── subscriber.schema.json
    └── processor.schema.json
```

### 5.2 Configuration Loading Flow

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  config/base/   │────►│   etcd sync     │────►│   etcd store    │
│  (YAML files)   │     │  (deploy.sh)    │     │                 │
└─────────────────┘     └─────────────────┘     └────────┬────────┘
                                                          │
                                                          ▼
┌─────────────────────────────────────────────────────────────────┐
│                       CONFIG-CLIENT                              │
│                                                                  │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐       │
│  │ PlatformConfig│  │ StreamConfigs │  │ProcessorConfigs│       │
│  │               │  │               │  │               │       │
│  │ - event_bus   │  │ - silver_etl  │  │ - rules       │       │
│  │ - subscribers │  │ - sources     │  │ - outputs     │       │
│  └───────────────┘  └───────────────┘  └───────────────┘       │
│                                                                  │
└──────────────────────────────────────┬──────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                      AIR-QUALITY-APP                             │
│                                                                  │
│  Loads configs at startup, creates EventBus and Subscribers     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.3 Hot Reload Considerations

**Current Scope**: No hot reload. Config changes require restart.

**Future Consideration**: If hot reload needed:
1. SubscriberCoordinator watches etcd for changes
2. Stop affected subscriber
3. Create new subscriber with new config
4. Start new subscriber

---

## 6. Error Handling Architecture

### 6.1 Error Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                        Error Types                               │
│                                                                  │
│  EventBusError                                                   │
│  ├── NoReceivers        (info, continue)                        │
│  └── ChannelClosed      (fatal, shutdown)                       │
│                                                                  │
│  SubscriberError                                                 │
│  ├── StartupFailed      (fatal for subscriber)                  │
│  ├── ShutdownFailed     (warning, continue)                     │
│  ├── ProcessingError    (warning, continue)                     │
│  ├── StorageError       (retry, then warning)                   │
│  └── ConfigError        (fatal, don't start)                    │
│                                                                  │
│  TransformError                                                  │
│  ├── FieldExtraction    (skip row, continue)                    │
│  ├── TypeConversion     (skip row, continue)                    │
│  ├── RequiredFieldMissing (skip row, continue)                  │
│  └── DqEvaluation       (flag row, continue)                    │
│                                                                  │
│  ProcessorError                                                  │
│  ├── EvaluationFailed   (skip rule, continue)                   │
│  └── OutputFailed       (warning, continue)                     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Error Recovery Matrix

| Error | Scope | Recovery | Action |
|-------|-------|----------|--------|
| EventBus full | System | Automatic | Oldest events dropped |
| Subscriber lagged | Subscriber | Automatic | Log warning, continue |
| Bronze write fail | Subscriber | Retry 3x | Log error, continue |
| Silver DB down | Subscriber | Retry + backoff | Catch up on restart |
| Transform error | Row | Skip | Log warning, continue |
| DQ rule fail | Row | Flag | Set dq_flags, continue |
| MQTT down | EventNotifier | Ignore | Fire-and-forget |
| Processor error | Row | Skip | Log warning, continue |

### 6.3 Graceful Degradation

```
PRIORITY ORDER (most critical first):
1. Bronze write - MUST succeed (data durability)
2. Silver write - Should succeed (retry, catch up later)
3. Threshold alerts - Nice to have (skip if issues)
4. Event notifications - Optional (fire-and-forget)

If Bronze fails: Retry with backoff, eventually log critical error
If Silver fails: Log error, will catch up on restart
If Processor fails: Log warning, continue processing
If EventNotifier fails: Ignore (QoS 0)
```

---

## 7. Testing Architecture

### 7.1 Test Pyramid

```
                    ┌───────────────┐
                    │  Integration  │  (10% - Real DB, Real MQTT)
                    │    Tests      │
                    └───────┬───────┘
                            │
              ┌─────────────┴─────────────┐
              │      Component Tests       │  (30% - In-memory stores)
              │  (BronzeSubscriber, etc.)  │
              └─────────────┬─────────────┘
                            │
        ┌───────────────────┴───────────────────┐
        │            Unit Tests                  │  (60% - Mocks)
        │  (transform, dq_evaluator, condition) │
        └────────────────────────────────────────┘
```

### 7.2 Mock Strategy (London TDD)

```rust
// Every trait has a mock
mock! {
    pub RawStore {}
    impl RawStore for RawStore {
        fn write_raw_batch(&self, batch: Vec<RawDataPoint>) -> Result<(), StorageError>;
    }
}

mock! {
    pub BronzeReader {}
    impl BronzeReader for BronzeReader {
        fn list_files_since(&self, stream: &str, since: Option<DateTime<Utc>>)
            -> Result<Vec<PathBuf>, StorageError>;
        fn read_parquet(&self, path: &Path) -> Result<Vec<RawDataPoint>, StorageError>;
    }
}

mock! {
    pub MqttClient {}
    impl MqttClient for MqttClient {
        fn try_publish(&self, topic: &str, qos: QoS, retain: bool, payload: Vec<u8>)
            -> Result<(), MqttError>;
    }
}
```

### 7.3 In-Memory Test Infrastructure

```rust
// For component tests
pub struct InMemoryRawStore {
    data: Arc<Mutex<Vec<RawDataPoint>>>,
}

pub struct InMemoryTimescaleDb {
    tables: Arc<Mutex<HashMap<String, Vec<SilverRow>>>>,
}

pub struct InMemoryMqtt {
    messages: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
}
```

---

## 8. Deployment Architecture

### 8.1 Container Changes

**Before DP-012**:
```yaml
services:
  air-quality-app:
    # Ingestion + Bronze only
  silver-etl-daemon:
    # 5-minute batch ETL
```

**After DP-012**:
```yaml
services:
  air-quality-app:
    # Ingestion + Bronze + Silver (streaming) + Processors + EventNotifier
    environment:
      - EVENT_NOTIFIER_ENABLED=${EVENT_NOTIFIER_ENABLED:-false}
  silver-etl:  # Renamed from daemon
    # Backfill mode only - run manually
    profiles:
      - backfill  # Only starts with --profile backfill
```

### 8.2 Resource Allocation

| Component | Memory | CPU | Notes |
|-----------|--------|-----|-------|
| EventBus | 50MB | < 5% | Arc overhead minimal |
| BronzeSubscriber | 20MB | < 5% | Same as current |
| SilverSubscriber | 50MB | < 10% | DB connection pool |
| ThresholdProcessor | 10MB | < 5% | Rule evaluation |
| EventNotifier | 5MB | < 1% | Fire-and-forget |
| **Total Additional** | ~100MB | ~20% | Within Pi4 capacity |

### 8.3 Health Monitoring

```yaml
# Prometheus metrics endpoint
/metrics:
  # Event bus
  ndp_event_bus_published_total
  ndp_event_bus_lagged_total
  ndp_event_bus_subscriber_count

  # Subscribers
  ndp_subscriber_processed_total{subscriber="bronze|silver|..."}
  ndp_subscriber_errors_total{subscriber="..."}
  ndp_subscriber_lag_events_total{subscriber="..."}
  ndp_subscriber_buffer_size{subscriber="..."}

  # Silver specific
  ndp_silver_catchup_files_total
  ndp_silver_transform_errors_total
  ndp_silver_upsert_rows_total

  # Processors
  ndp_processor_alerts_total{processor="threshold"}
```

---

## 9. Security Architecture

### 9.1 Data Flow Security

| Flow | Security | Notes |
|------|----------|-------|
| EventBus | In-process | No network exposure |
| Bronze write | File permissions | Existing security |
| Silver write | TLS to TimescaleDB | Connection string from env |
| MQTT alerts | TLS optional | Internal network |
| MQTT notifications | TLS optional | Internal network |

### 9.2 Configuration Security

- Database credentials: Environment variables only
- MQTT credentials: Environment variables only
- No secrets in YAML files
- etcd access: Internal network only

---

## 10. Migration Architecture

### 10.1 Migration Phases

```
PHASE 1: Event Bus + Bronze
─────────────────────────────
Week 1
├── Create EventBus module
├── Create Subscriber trait
├── Create BronzeSubscriber
├── Wire into air-quality-app
└── Validate Bronze unchanged

PHASE 2: Streaming Silver
─────────────────────────────
Week 2
├── Create silver/transform module
├── Create SilverSubscriber
├── Implement catch-up logic
├── Wire into air-quality-app
└── Validate Silver matches batch ETL

PHASE 3: Processors + Notifier
─────────────────────────────
Week 3
├── Create Processor trait
├── Create ThresholdProcessor
├── Create EventNotifier
├── Wire into air-quality-app
└── Validate alerts fire correctly

PHASE 4: Polish
─────────────────────────────
Week 4
├── silver-etl backfill mode
├── Grafana dashboards
├── MCP tools
├── Documentation
└── Performance validation
```

### 10.2 Rollback Strategy

| Phase | Rollback |
|-------|----------|
| Phase 1 | Revert to mpsc channel |
| Phase 2 | Disable SilverSubscriber, run batch ETL |
| Phase 3 | Disable ProcessorSubscriber and EventNotifier |
| Phase 4 | No rollback needed (additive) |

### 10.3 Parallel Running

During migration, both old and new can run:
- Event bus publishes to subscribers
- Batch ETL continues running
- Compare results for validation
- Switch off batch ETL when confident

---

*Architecture document created: 2026-01-18*
*Next phase: SPARC-R (Refinement)*
