# AIR-004: Architecture Context from ReasoningBank

**Generated**: 2025-12-15
**Source**: ReasoningBank Memory System Analysis
**Purpose**: Document architecture decisions and context to inform AIR-004 SPARC development

---

## Executive Summary

This report extracts and synthesizes architecture context from the ReasoningBank memory system and existing feature documentation (AIR-001, AIR-002, AIR-003) to inform the AIR-004 multi-stream data platform implementation.

**Key Finding**: ReasoningBank contains 65 memories with extensive architecture decisions from AIR-001 through AIR-003, providing a strong foundation for AIR-004's evolution to a generic multi-stream platform.

---

## ReasoningBank Status

### Database Information
- **Location**: `/workspaces/neural-data-platform/.swarm/memory.db`
- **Total Memories**: 65 entries
- **Categories**: 2 (architecture, features)
- **Average Confidence**: 80.0%
- **Embeddings**: 65 active
- **Database Size**: 0.38 MB
- **Storage Mode**: ReasoningBank (AI-powered, auto-selected)

### Memory Organization
ReasoningBank is initialized and operational with semantic search capabilities. The system uses hash-based embeddings in NPX mode with a fallback to transformer embeddings when globally installed.

---

## Architecture Decisions in Memory

### 1. AIR-001: Core Air Quality Platform

**Memory Keys**: `air001/arch/*`, `architecture/air001-*`

#### Hexagonal Architecture Pattern
```yaml
Pattern: hexagonal-ports-adapters
Description: Hexagonal architecture with ports (interfaces) and adapters
Layers: core-domains-apps
Status: Implemented (Phase 1)
```

**Key Decision**: Isolation of domain logic from infrastructure concerns
- Core domain types in `neural-core/src/prediction/`
- Adapters for MQTT, HTTP, Parquet storage
- Applications consume core via well-defined interfaces

#### Data Storage Strategy
```yaml
Pattern: parquet-columnar-storage
Format: Apache Parquet with Snappy compression
Partitioning: Daily by location
Location: /app/data/bronze/air-quality/YYYY/MM/DD/*.parquet
```

**Rationale**:
- Efficient columnar storage for time-series analytics
- Compression reduces storage costs
- Daily partitioning enables efficient queries and retention policies

#### MQTT Ingestion Pattern
```yaml
Pattern: mqtt-dual-source-ingestion
Sources:
  - MQTT stream (airgradient/readings/{SERIAL_NUMBER})
  - HTTP polling fallback (local API)
Topic Pattern: airgradient/readings/+
QoS: 1 (at-least-once delivery)
```

**Field Count**: 29 fields from both sources
**Data Types**: Float32 for PM counts/raw values, Int for indices

#### Validation Ranges
Stored in memory for consistency:
```yaml
co2: 380-10000 ppm
pm25: 0-500 µg/m³
tvoc: 1-500 index
nox: 1-500 index
humidity: 0-100 percent
temperature: -10 to 50 celsius
```

#### TDD Methodology
```yaml
Approach: London School (mock-driven)
Coverage Target: 90% minimum
Testing Strategy: Unit → Integration → E2E
```

---

### 2. AIR-002: MQTT to Parquet Pipeline

**Memory Keys**: `air002/*`

#### Implementation Status
```yaml
Tasks:
  T1: Configuration (COMPLETE)
  T2: MQTT Handler (COMPLETE)
  T3: Storage Writer (COMPLETE)
  T4: Main Integration (COMPLETE - with fixes)
Commit: 4d911b5 (39 files, 13,415 additions)
```

#### Pipeline Architecture
```yaml
Flow: MqttHandler → mpsc channel → StorageWriter → ParquetStore
Batch Size: 100 messages
Timeout: 5 seconds
Pattern: Asynchronous streaming with backpressure
```

**Key Components**:
- `apps/air-quality-app/src/ingestion/mqtt.rs` - MQTT handler
- `apps/air-quality-app/src/pipeline/storage.rs` - Storage writer
- `apps/air-quality-app/src/main.rs` - Integration and orchestration

#### Docker Configuration
```yaml
Services: mosquitto, air-quality-app
Volumes: mosquitto-data, air-quality-data
Health Checks: mosquitto_sub test, app readiness
```

---

### 3. AIR-003: etcd Configuration Management

**Memory Keys**: `air003/*`, `architecture/config-pattern`, `architecture/etcd-*`

#### Configuration Hierarchy Pattern
```yaml
Pattern: etcd-first-config-hierarchy
Precedence:
  1. Environment variables (highest)
  2. etcd values
  3. File-based fallback (lowest)
Implementation: etcd v3.5.11 with thin Rust wrapper (260 LOC)
```

**Key Decision**: Adopt battle-tested infrastructure instead of custom build
- **Development Time Saved**: 6-8 weeks
- **Code Maintainability**: 260 LOC vs 2000+ for custom solution
- **Performance**: Config retrieval < 10ms (p95)

#### etcd Architecture Components
```yaml
config-client (Rust crate):
  - client.rs: 122 LOC (CRUD operations, prefix management)
  - watch.rs: 81 LOC (real-time updates, callbacks)
  - error.rs: 32 LOC (error types)
  Total: 260 LOC
```

#### GitOps Configuration Sync
```yaml
Pattern: gitops-configuration-sync (Kustomize-style)
Structure:
  config/base/: Default configuration (all environments)
  config/overlays/: Environment-specific overrides
Sync: Bash script + Python on startup
Format: YAML → flattened etcd keys
```

**Example**:
```yaml
# YAML
mqtt:
  broker_url: "localhost"
  port: 1883

# etcd keys
/air-quality/mqtt/broker_url = "localhost"
/air-quality/mqtt/port = 1883
```

#### Watch/Subscribe Pattern
```yaml
Pattern: event-driven-config-updates
Implementation: etcd watch API with callbacks
Latency: < 100ms notification
Use Case: Hot-reload configuration without restart
```

#### Environment Variable Override
```yaml
Pattern: configuration-precedence-hierarchy
Convention: {APP}_{PATH_WITH_UNDERSCORES}
Example: /mqtt/broker_url → AIR_QUALITY_MQTT_BROKER_URL
```

#### Hierarchical Key Design
```yaml
Pattern: hierarchical-configuration-namespace
Structure: /service-name/category/key
Max Depth: 2-3 levels recommended
Example: /air-quality/mqtt/broker_url
```

---

## Platform Evolution Context

### Current State (AIR-001 → AIR-003)
```
Single Stream Architecture:
  Air Quality → MQTT → Parse → Parquet → Storage
  Configuration: etcd-based with hot-reload
  Deployment: Docker Compose, Raspberry Pi 5 ready
```

### AIR-004 Target State
```
Multi-Stream Platform:
  Stream Registry (etcd) → Multiple Sources → Router → Bronze/Silver layers
  Streams: air-quality, home-events, weather, ...
  Analytics: Cross-stream correlation, predictive models
```

---

## Architectural Patterns Established

### 1. Distributed Configuration (AIR-003)
**Pattern**: etcd-distributed-configuration
**Status**: Production-ready
**Reuse for AIR-004**: ✅ Stream registry can use same etcd instance and patterns

### 2. Hexagonal Ports-Adapters (AIR-001)
**Pattern**: hexagonal-architecture
**Status**: Core domain isolation established
**Reuse for AIR-004**: ✅ Generic Source trait follows same pattern

### 3. GitOps Configuration Sync (AIR-003)
**Pattern**: gitops-etcd-config-sync
**Status**: Operational
**Reuse for AIR-004**: ✅ Stream definitions can follow same YAML → etcd sync

### 4. Parquet Bronze Layer (AIR-001, AIR-002)
**Pattern**: parquet-columnar-storage
**Status**: Implemented for air-quality stream
**Reuse for AIR-004**: ✅ Extend to multi-stream with partitioning by stream_id

### 5. Event-Driven Updates (AIR-003)
**Pattern**: watch-notification-pattern
**Status**: Proven with config hot-reload
**Reuse for AIR-004**: ✅ Dynamic source registration using etcd watch

---

## Memory Entries Relevant to AIR-004

### Stream Registry Architecture
```yaml
architecture/air003-etcd-pattern:
  - etcd already supports hierarchical keys
  - Watch API proven for real-time updates
  - Schema validation patterns established

Recommendation: Store stream definitions in etcd under streams/{stream-id}/
```

### Multi-Stream Data Flow
```yaml
architecture/data-flow-patterns:
  Flow: Sensor → MQTT → MqttSource → Parser → Validator → Store

Extension for AIR-004:
  Flow: Source (generic) → Router (by stream_id) → Validator → Multi-Store (Bronze + Silver)
```

### Infrastructure Patterns
```yaml
architecture/infrastructure-patterns:
  Docker Compose: Multi-file for environment separation
  Volumes: Persistent storage for etcd-data, air-quality-data
  Health Checks: Service dependency management

Extension for AIR-004:
  Add: TimescaleDB container with hypertables per stream
  Add: Grafana with per-stream dashboard templates
```

### API Design Patterns
```yaml
architecture/air001-api-design:
  Pattern: Axum-based REST API with MCP integration
  Status: Designed for air-quality

Extension for AIR-004:
  Add: Generic query API for any stream
  Add: Cross-stream analytics endpoints
```

---

## Constraints and Requirements from Memory

### Performance Requirements
```yaml
From air001:
  - MQTT throughput: 1 message/sec sustained
  - Query latency: < 100ms p99 for 1-day range
  - Memory usage: < 1.5GB on Raspberry Pi 5

For AIR-004:
  - Must maintain performance with 3-5 concurrent streams
  - Config retrieval: < 10ms p95 (AIR-003 standard)
  - Watch notification: < 100ms (AIR-003 standard)
```

### Deployment Constraints
```yaml
Target Platform: Raspberry Pi 5 (Ubuntu 22.04)
Container Strategy: Docker Compose for orchestration
Network: Bridge network (neural-network)
Storage: Named volumes for persistence
```

### Configuration Standards
```yaml
From AIR-003:
  - All config in etcd (no hardcoded values)
  - Environment variable overrides supported
  - GitOps sync from YAML source of truth
  - Hot-reload via watch API
```

---

## Platform Architecture Summary (from Memory)

```yaml
architecture/platform-overview:
  Platform: Neural Data Platform
  Vision: Generic time-series platform for IoT, home automation, ML

Current Capabilities:
  - Air quality monitoring (AIR-001, AIR-002)
  - etcd configuration management (AIR-003)
  - Hexagonal architecture
  - Parquet storage
  - Docker deployment

AIR-004 Evolution:
  - Generic multi-stream ingestion
  - Stream registry (etcd-based)
  - TimescaleDB for queryable silver layer
  - Cross-stream analytics
  - Homebridge integration
```

---

## Design Decisions Summary (ADRs from Memory)

### ADR-001: Hexagonal Architecture (AIR-001)
**Decision**: Use hexagonal ports-adapters pattern
**Rationale**: Domain isolation, testability, adapter swapping
**Status**: Implemented
**Impact on AIR-004**: Generic Source trait follows same pattern

### ADR-002: etcd for Configuration (AIR-003)
**Decision**: Use etcd instead of custom configuration server
**Rationale**: 6-8 weeks saved, battle-tested, 260 LOC vs 2000+
**Status**: Implemented
**Impact on AIR-004**: Stream registry uses same etcd instance

### ADR-003: Parquet for Bronze Layer (AIR-001)
**Decision**: Apache Parquet with Snappy compression
**Rationale**: Columnar efficiency, compression, analytics-friendly
**Status**: Implemented
**Impact on AIR-004**: Extend to multi-stream partitioning

### ADR-004: GitOps Configuration Sync (AIR-003)
**Decision**: YAML source of truth with sync to etcd
**Rationale**: Version control, audit trail, familiar workflow
**Status**: Implemented
**Impact on AIR-004**: Stream definitions follow same pattern

### ADR-005: London School TDD (AIR-001)
**Decision**: Mock-driven TDD with 90% coverage target
**Rationale**: Faster feedback, design-first approach
**Status**: Adopted
**Impact on AIR-004**: Continue same testing methodology

---

## Rust Coding Patterns (from Memory)

```yaml
architecture/rust-coding-patterns:
  Module Organization: Hierarchical (lib.rs → modules → submodules)
  Error Handling: thiserror for domain errors, anyhow for apps
  Async Runtime: tokio 1.x with async/await
  Serialization: serde + serde_json
  Testing: Unit (cargo test), Integration (testcontainers), E2E (scripts)
  Logging: tracing with structured fields
  File Size Limit: < 500 lines per module (maintainability)
```

---

## Feature Summary (from Memory)

### AIR-001: Air Quality Module - Core Implementation
```yaml
Status: Phase 1 (Foundation) - In Progress
Components:
  - Core traits (TimeSeriesPoint, Store, Source, Forecast)
  - AirGradient domain types (29 fields)
  - MQTT ingestion
  - Parquet storage
Next Phase: Intelligence layer (AQI, alerts, forecasting)
```

### AIR-002: MQTT to Parquet Data Ingestion Pipeline
```yaml
Status: Complete (commit 4d911b5)
Components:
  - MqttHandler with dual-source support
  - StorageWriter with batching
  - ParquetStore integration
  - Docker Compose setup
Blocker Resolved: neural_core sources module integration
```

### AIR-003: Universal Configuration Management
```yaml
Status: Complete (SPARC suite delivered)
Documents: 7 documents, ~500KB total
Components:
  - etcd container (v3.5.11)
  - config-client Rust crate (260 LOC)
  - GitOps sync script
  - Watch/subscribe implementation
  - Environment override support
Key Decision: Pivot to etcd instead of custom build
```

---

## Gaps Identified

### 1. No Multi-Stream Infrastructure
**Current State**: Single air-quality stream hardcoded
**Required for AIR-004**: Generic stream registry, dynamic source spawning
**Memory Context**: None - AIR-004 is net-new architecture

### 2. No Silver/Gold Layer
**Current State**: Bronze (Parquet) only
**Required for AIR-004**: TimescaleDB for queryable time-series
**Memory Context**: No prior TimescaleDB integration

### 3. No Cross-Stream Analytics
**Current State**: Single-stream analytics only
**Required for AIR-004**: Join air-quality with home-events, weather
**Memory Context**: Predictive model design exists (ruv-FANN) but single-stream

### 4. No Generic Source Trait
**Current State**: MqttSource and HttpPollingSource are air-quality specific
**Required for AIR-004**: Unified Source trait for push/poll patterns
**Memory Context**: Hexagonal pattern established, trait needs genericization

---

## Recommendations for AIR-004 SPARC Documents

### 1. Specification (SPARC Phase 1)
**Reuse from Memory**:
- Configuration hierarchy pattern (AIR-003)
- Hexagonal architecture (AIR-001)
- GitOps sync pattern (AIR-003)
- Performance requirements (AIR-001)

**New Requirements**:
- Stream registry schema (etcd keys: streams/{stream-id}/config)
- Generic Source trait interface
- TimescaleDB schema per stream
- Cross-stream query API

### 2. Pseudocode (SPARC Phase 2)
**Reuse from Memory**:
- etcd client usage patterns (AIR-003)
- MQTT ingestion flow (AIR-002)
- Parquet storage flow (AIR-001)

**New Pseudocode**:
- Stream registry watch loop
- Generic source spawning
- Bronze → Silver sync (dual-write or ETL)
- Cross-stream join queries

### 3. Architecture (SPARC Phase 3)
**Reuse from Memory**:
- Hexagonal architecture diagram (AIR-001)
- etcd configuration architecture (AIR-003)
- Docker Compose patterns (AIR-002)

**New Architecture**:
- Stream registry C4 diagrams
- Multi-stream data flow
- TimescaleDB hypertable design
- Grafana dashboard architecture

### 4. Refinement (SPARC Phase 4)
**Reuse from Memory**:
- London School TDD methodology (AIR-001)
- 90% coverage target (AIR-001)
- Integration test patterns (AIR-003)

**New Testing**:
- Stream registry validation
- Multi-stream ingestion tests
- Cross-stream analytics tests
- TimescaleDB schema migrations

### 5. Completion (SPARC Phase 5)
**Reuse from Memory**:
- Docker deployment patterns (AIR-002)
- Raspberry Pi 5 target (AIR-001)
- Performance benchmarks (AIR-001)

**New Deployment**:
- TimescaleDB container
- Grafana dashboards
- Stream-specific alerting
- Homebridge plugin integration

---

## Technical Stack Consistency

### Already Established (Reuse)
```yaml
Language: Rust (edition 2021)
Async Runtime: tokio 1.x
Configuration: etcd v3.5.11
Serialization: serde + serde_json
Storage (Bronze): Apache Parquet
Container: Docker Compose
Logging: tracing
Error Handling: thiserror + anyhow
Testing: cargo test + testcontainers
```

### New for AIR-004
```yaml
Storage (Silver/Gold): TimescaleDB (PostgreSQL extension)
Database Client: sqlx (async Rust)
Dashboards: Grafana (already in docker configs)
API Framework: Axum (already chosen in AIR-001)
Analytics: Polars (already in dependencies)
```

---

## Open Questions from Memory Analysis

### 1. EventBus Usage
**Memory Reference**: `architecture/platform-overview` mentions EventBus available
**Status**: Implemented in `neural-core/src/eventbus/` but not used in AIR-001/002/003
**Question for AIR-004**: Use EventBus for inter-service messaging or stick with mpsc channels?
**Recommendation**: Evaluate if EventBus provides value for multi-stream coordination

### 2. MCP Integration Scope
**Memory Reference**: `architecture/air001-api-design` mentions MCP integration
**Status**: Designed but not implemented
**Question for AIR-004**: Integrate MCP server for stream management or defer to Phase 2?
**Recommendation**: Defer MCP to AIR-005, focus on core multi-stream first

### 3. ruv-FANN Predictive Model
**Memory Reference**: Multiple mentions of ruv-FANN for forecasting
**Status**: Dependency exists, not integrated
**Question for AIR-004**: Include cross-stream predictive models or defer?
**Recommendation**: Include in SPARC spec but implement in Refinement phase

### 4. Webhook Authentication
**Memory Reference**: AIR-004 PLATFORM_ARCHITECTURE.md mentions webhook source
**Question**: Bearer token, API key, or both?
**Recommendation**: Support both, configure per-source in stream registry

---

## Migration Path from AIR-003 to AIR-004

### Phase 1: Refactor Existing Components
1. **Generalize MqttSource** (AIR-002)
   - Extract stream-specific logic to configuration
   - Implement generic Source trait
   - Test with air-quality stream (backward compatibility)

2. **Extend ParquetStore** (AIR-001)
   - Add stream_id partitioning: `bronze/{stream-id}/YYYY/MM/DD/`
   - Test with air-quality stream

### Phase 2: Add Stream Registry
1. **etcd Schema** (build on AIR-003)
   - Define streams/{stream-id}/config keys
   - Store schema, sources, retention policies
   - Use existing config-client crate

2. **Watch Integration**
   - Use AIR-003 watch patterns
   - Hot-reload stream definitions
   - Dynamic source spawning

### Phase 3: Add Silver Layer
1. **TimescaleDB Setup**
   - Add to docker-compose.yml (follow AIR-002 patterns)
   - Health checks and volume management
   - Connection pooling

2. **Dual-Write Implementation**
   - StorageWriter writes to both Bronze and Silver
   - Per-stream table schemas from registry

### Phase 4: Cross-Stream Analytics
1. **Query API**
   - Axum endpoints (follow AIR-001 design)
   - ASOF JOIN for time correlation
   - Grafana data source integration

---

## Success Criteria Inherited from Memory

### From AIR-001 (apply to AIR-004)
- Performance: < 100ms p99 query latency for 1-day range
- Deployment: Raspberry Pi 5 compatible (< 1.5GB RAM)
- Testing: 90% code coverage
- Message throughput: 1/sec sustained (now: per stream)

### From AIR-003 (apply to AIR-004)
- Config retrieval: < 10ms p95
- Watch notification: < 100ms
- Code maintainability: Prefer thin wrappers over custom implementations
- Hot-reload: No restart required for configuration changes

### New for AIR-004
- Multi-stream support: 3-5 concurrent streams
- Cross-stream query latency: < 200ms p99
- Stream registry: Dynamic source spawning within 1 second
- Schema validation: 100% enforcement before ingestion

---

## Key Insights for AIR-004 Implementation

### 1. Architecture Continuity
The platform has established strong architectural patterns (hexagonal, etcd-first config, Parquet bronze layer). AIR-004 should **extend, not replace** these patterns.

### 2. Leverage Existing Infrastructure
- etcd is already operational and proven
- Docker Compose orchestration patterns established
- config-client crate can be reused for stream registry
- ParquetStore can be extended for multi-stream

### 3. Minimal Custom Code Philosophy
AIR-003's decision to use etcd (260 LOC vs 2000+) demonstrates the platform's preference for battle-tested infrastructure. Apply this to AIR-004:
- Use TimescaleDB for queryable storage (don't build custom time-series DB)
- Use Grafana for dashboards (don't build custom UI)
- Use sqlx for database access (established Rust pattern)

### 4. GitOps-First Configuration
All stream definitions should live in YAML under version control, synced to etcd. This maintains the audit trail and review process established in AIR-003.

### 5. Performance Budget
With Raspberry Pi 5 as the deployment target, AIR-004 must remain within:
- Memory: < 1.5GB total (currently ~500MB for air-quality)
- CPU: Efficient async I/O, minimal blocking
- Disk: Parquet compression, TimescaleDB compression policies

---

## Files Referenced in Memory

### Core Implementation Files
```
/workspaces/neural-data-platform/
├── apps/air-quality-app/src/
│   ├── main.rs (integration orchestration)
│   ├── ingestion/mqtt.rs (MQTT handler)
│   └── pipeline/storage.rs (storage writer)
├── config-client/ (etcd wrapper, 260 LOC)
│   ├── src/client.rs (122 LOC)
│   ├── src/watch.rs (81 LOC)
│   └── src/error.rs (32 LOC)
├── config/
│   ├── base/air-quality/config.yaml
│   └── overlays/{development,production}/
├── docker-compose.yml (orchestration)
└── scripts/sync-config-to-etcd.sh (GitOps sync)
```

### Documentation Files
```
/workspaces/neural-data-platform/product/features/
├── air-001/
│   ├── INDEX.md (feature overview)
│   ├── architecture/01-system-design.md
│   └── specs/01-specification.md
├── air-002/
│   ├── README.md
│   └── architecture/01-system-design.md
├── air-003/
│   ├── architecture/AIR-003-ARCHITECTURE-SUMMARY.md
│   └── specs/01-etcd-specification.md
└── air-004/
    └── architecture/PLATFORM_ARCHITECTURE.md (this feature)
```

---

## Conclusion

ReasoningBank contains comprehensive architecture context from AIR-001 through AIR-003, providing a solid foundation for AIR-004. The platform has established:

1. **Hexagonal architecture** (AIR-001) - domain isolation pattern
2. **etcd-first configuration** (AIR-003) - distributed config with 260 LOC wrapper
3. **Parquet bronze layer** (AIR-001, AIR-002) - time-series storage
4. **GitOps sync pattern** (AIR-003) - YAML source of truth
5. **Docker deployment** (AIR-002) - container orchestration
6. **London School TDD** (AIR-001) - 90% coverage target

**AIR-004 should leverage these patterns while adding**:
- Stream registry (etcd-based, following AIR-003 patterns)
- Generic Source trait (following hexagonal architecture)
- TimescaleDB silver layer (queryable time-series)
- Multi-stream data flow (extending AIR-002 pipeline)
- Cross-stream analytics (new capability)

**Memory Status**: All critical architecture decisions are captured and available for SPARC document generation. No significant gaps in foundational context.

---

**Next Steps**:
1. Use this context to generate AIR-004 SPARC Specification
2. Reference memory entries in architecture decisions
3. Maintain consistency with established patterns
4. Document new patterns in ReasoningBank for future features

---

**Generated by**: Research Agent
**Memory Database**: .swarm/memory.db (65 entries, 0.38 MB)
**Source Documents**: AIR-001 INDEX.md, AIR-003 Architecture Summary, AIR-004 Platform Architecture
