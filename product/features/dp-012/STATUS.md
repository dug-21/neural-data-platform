# DP-012: Status Tracker

**Feature**: Unified Event Bus Architecture with Streaming Subscribers
**Current Phase**: Implementation (SPARC-C) - Phase 1 Event Bus Foundation
**Last Updated**: 2026-01-18 14:30

---

## Phase Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| Scope Definition | ✅ Complete | 2026-01-17 | 2026-01-17 |
| Specification (SPARC-S) | ✅ Complete | 2026-01-18 | 2026-01-18 |
| Pseudocode (SPARC-P) | ✅ Complete | 2026-01-18 | 2026-01-18 |
| Architecture (SPARC-A) | ✅ Complete | 2026-01-18 | 2026-01-18 |
| Refinement (SPARC-R) | ✅ Complete | 2026-01-18 | 2026-01-18 |
| Completion (SPARC-C) | 🚧 In Progress | 2026-01-18 | - |

---

## Implementation Milestones

| Milestone | Status | Target | Notes |
|-----------|--------|--------|-------|
| Phase 1: Event Bus Foundation | 🚧 In Progress | Week 1 | Bronze via event bus |
| Phase 2: Streaming Silver | ⏳ Pending | Week 2 | Port silver-etl transforms to core, Silver < 5s |
| Phase 3: Processor Framework & Event Notifier | ⏳ Pending | Week 3 | Threshold alerts + MQTT notifications |
| Phase 4: Polish | ⏳ Pending | Week 4 | Docs, dashboards, silver-etl backfill mode |

---

## Phase 1 Implementation Checklist

### Files to Create (in order)

| # | File | Responsibility | Dependencies | Status |
|---|------|----------------|--------------|--------|
| 1 | `core/src/event_bus/mod.rs` | EventBus struct, EventBusConfig, EventBusError, EventBusMetrics | None | ⏳ Pending |
| 2 | `core/src/subscribers/mod.rs` | Subscriber trait, HealthStatus, SubscriberError | event_bus | ⏳ Pending |
| 3 | `core/src/subscribers/coordinator.rs` | SubscriberCoordinator for lifecycle management | Subscriber trait | ⏳ Pending |
| 4 | `core/src/subscribers/bronze.rs` | BronzeSubscriber (refactor RawStorageWriter) | Subscriber, ParquetStore | ⏳ Pending |
| 5 | `apps/air-quality-app/src/main.rs` | Wire EventBus + SubscriberCoordinator | All above | ⏳ Pending |

### Phase 1 Detailed Tasks

**1. EventBus Module (`core/src/event_bus/mod.rs`)**
- [ ] Create EventBusConfig struct (capacity, lag_warning_threshold)
- [ ] Create EventBusMetrics struct (published, lagged_total, subscriber_count)
- [ ] Create EventBusError enum (ChannelClosed)
- [ ] Implement EventBus::new(config)
- [ ] Implement EventBus::publish(point) - wraps in Arc, broadcasts
- [ ] Implement EventBus::subscribe() - returns broadcast::Receiver
- [ ] Implement EventBus::subscriber_count()
- [ ] Unit tests for broadcast behavior, lag handling

**2. Subscriber Trait (`core/src/subscribers/mod.rs`)**
- [ ] Define Subscriber trait with async_trait
- [ ] Define methods: id(), start(), stop(), accepts_stream(), health_check()
- [ ] Create SubscriberError enum
- [ ] Create HealthStatus enum (Healthy, Degraded, Unhealthy)
- [ ] Export from module

**3. SubscriberCoordinator (`core/src/subscribers/coordinator.rs`)**
- [ ] Create SubscriberCoordinator struct (event_bus, subscribers, handles)
- [ ] Implement add_subscriber(Box<dyn Subscriber>)
- [ ] Implement start_all() - spawns tokio tasks for each subscriber
- [ ] Implement stop_all() - graceful shutdown with timeout
- [ ] Implement health_status() - aggregates subscriber health
- [ ] Unit tests with mock subscribers

**4. BronzeSubscriber (`core/src/subscribers/bronze.rs`)**
- [ ] Create BronzeSubscriberConfig (batch_size, batch_timeout_secs, stream_filter)
- [ ] Create BronzeSubscriber struct (store, buffer, config)
- [ ] Implement Subscriber trait
- [ ] Implement buffer_point() - adds to buffer, checks flush threshold
- [ ] Implement flush() - writes batch to ParquetStore with retry
- [ ] Receive loop with tokio::select! for timeout-based flushing
- [ ] Unit tests with MockRawStore

**5. Application Wiring**
- [ ] Update air-quality-app to create EventBus
- [ ] Create SubscriberCoordinator and add BronzeSubscriber
- [ ] Modify sources to publish to EventBus (not mpsc)
- [ ] Verify Bronze output unchanged (compare Parquet files)

### Acceptance Criteria for Phase 1

| Criterion | Validation Method |
|-----------|------------------|
| EventBus broadcasts to multiple subscribers | Unit test: 2 receivers get same event |
| Subscriber isolation | Unit test: one subscriber fails, others continue |
| Bronze writes unchanged | Integration test: compare Parquet files before/after |
| Lag handled gracefully | Unit test: slow subscriber gets Lagged error, continues |
| Graceful shutdown | Integration test: stop_all flushes buffers |

---

## Components Modified

| Component | Status | Notes |
|-----------|--------|-------|
| `core/src/event_bus/` | ⏳ Not Started | New module |
| `core/src/subscribers/` | ⏳ Not Started | New module (includes event_notifier.rs) |
| `core/src/silver/` | ⏳ Not Started | New module - port transform logic from silver-etl |
| `core/src/processors/` | ⏳ Not Started | New module (threshold only, no ML) |
| `core/src/outputs/` | ⏳ Not Started | New module |
| `core/src/parsers/` | ⏳ Not Started | **DEPRECATE** - add deprecation warnings |
| `apps/air-quality-app/src/main.rs` | ⏳ Not Started | Event bus wiring |
| `apps/silver-etl/` | ⏳ Not Started | Backfill mode only |
| `config/base/platform.yaml` | ⏳ Not Started | Subscriber config + EVENT_NOTIFIER_ENABLED |
| `config/base/processors/` | ⏳ Not Started | Processor configs (threshold only) |

---

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Broadcast vs mpsc | `tokio::broadcast` | Multiple subscribers need same data |
| Subscriber isolation | Independent tasks | Failure in one doesn't affect others |
| Silver approach | Streaming subscriber | < 5s latency vs 5 min batch |
| Silver config | Reuse existing `silver_etl` | Config is complete and working in stream YAMLs |
| Transform logic | Port from silver-etl to core | Rust functions vs DuckDB SQL for streaming |
| Silver recovery | Catch-up from Bronze on startup | Bronze is source of truth, UPSERT handles duplicates |
| Parsers | **DEPRECATED** | Not used in Bronze→Silver path, silver_etl handles transforms |
| ML isolation | Separate container (dp-013) | ML has unpredictable workloads, must never block ingestion |
| ML integration | MQTT Event Notifier | Fire-and-forget notifications, zero code changes when ML added |
| Event Notifier toggle | Env var | `EVENT_NOTIFIER_ENABLED=true/false` for easy enable/disable |

---

## Blockers

| Blocker | Impact | Resolution |
|---------|--------|------------|
| None currently | - | - |

---

## Research Artifacts

| Document | Location |
|----------|----------|
| Platform Optimization Synthesis | `research/platform-minimize/00-synthesis-report.md` |
| Real-Time Processing Design | `research/platform-minimize/06-realtime-processing-layer-design.md` |
| Unified Event Bus Architecture | `research/platform-minimize/07-unified-event-bus-architecture.md` |
| Deployment Architecture | `research/platform-minimize/08-deployment-architecture.md` |

---

## SPARC Documentation Artifacts

### SPARC-S: Specification Phase

| Document | Location | Description |
|----------|----------|-------------|
| Main Specification | `specification/SPARC-S-SPECIFICATION.md` | Functional/non-functional requirements, test scenarios |
| Interface Contracts | `specification/INTERFACE-CONTRACTS.md` | Trait definitions, API specifications |
| London TDD Strategy | `specification/LONDON-TDD-STRATEGY.md` | Mock definitions, TDD cycles, test organization |

### SPARC-P: Pseudocode Phase

| Document | Location | Description |
|----------|----------|-------------|
| Pseudocode Design | `specification/SPARC-P-PSEUDOCODE.md` | Algorithm design, function signatures, data flow |

### SPARC-A: Architecture Phase

| Document | Location | Description |
|----------|----------|-------------|
| Architecture Design | `specification/SPARC-A-ARCHITECTURE.md` | ADRs, component diagrams, integration architecture |

### SPARC-R: Refinement Phase

| Document | Location | Description |
|----------|----------|-------------|
| Refinement | `specification/SPARC-R-REFINEMENT.md` | Edge cases, error handling, performance optimization |

---

---

## Dependencies and Blockers

### Dependencies (Resolved)

| Dependency | Status | Notes |
|------------|--------|-------|
| RawDataPoint type exists | ✅ Ready | `core/src/types/raw_data_point.rs` |
| ParquetStore exists | ✅ Ready | `core/src/storage/parquet.rs` |
| SilverEtlConfig exists | ✅ Ready | `core/src/config/silver_etl.rs` |
| tokio runtime | ✅ Ready | Already in dependencies |
| async_trait crate | ✅ Ready | Already in dependencies |

### Dependencies (To Verify)

| Dependency | Required For | Action |
|------------|--------------|--------|
| tokio::sync::broadcast | EventBus | Verify tokio features include "sync" |
| mockall crate | Unit tests | Add to dev-dependencies if missing |
| CancellationToken | Graceful shutdown | Add tokio_util if not present |

### Current Blockers

| Blocker | Impact | Resolution | Status |
|---------|--------|------------|--------|
| None identified | - | - | - |

---

## AgentDB Patterns Retrieved

The following patterns from AgentDB are relevant to this implementation:

| Pattern | Use |
|---------|-----|
| `architecture:event-bus-broadcast` | ADR-012-001 decisions for tokio::broadcast |
| `architecture:subscriber-trait` | Subscriber trait interface definition |
| `architecture:subscriber-isolation` | ADR-012-002 for independent tokio tasks |
| `arch-domain-adapter-pattern` | Core hexagonal architecture to follow |

---

*Status last updated: 2026-01-18 14:30 by ndp-scrum-master*
