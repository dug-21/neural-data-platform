# Consolidated Architecture Decisions - Neural Data Platform

> **Generated**: 2026-01-02
> **Source**: Research across 15 features (air-001 to air-011, dp-001 to dp-004)
> **Rule Applied**: Higher-numbered decisions supersede lower-numbered on conflicts

---

## Executive Summary

This document consolidates all architecture decisions from the NDP feature development history, presenting the **current state** after applying supersession rules. Decisions marked as superseded are noted but not authoritative.

---

## 1. Core Architecture Pattern

### Domain Adapter Pattern (Hexagonal Architecture)
**Source**: AIR-001, confirmed through DP-004

The platform uses **ports and adapters** architecture:
- **Core domain logic** separated from infrastructure
- **Traits as ports**: `Source`, `Store`, `Parser`
- **Implementations as adapters**: MqttSource, HttpPollingSource, ParquetStore, TimescaleStore

**Design Principles**:
1. Domain-agnostic core (`core/` crate)
2. Domain-specific adapters (`domains/`, `apps/`)
3. Configuration-driven behavior (YAML → etcd)
4. Trait-based dependency inversion

---

## 2. Data Layer Architecture

### Bronze → Silver → Gold Data Lake
**Source**: AIR-004, DP-001, DP-004

| Layer | Storage | Purpose | Schema Strategy |
|-------|---------|---------|-----------------|
| **Bronze** | Parquet | Raw archive, audit, replay | Schema-on-read (raw JSON) |
| **Silver** | TimescaleDB | Analytics, dashboards | Schema-on-write (typed columns) |
| **Gold** | Features/ML | Predictions, aggregations | Derived from Silver |

### Bronze Layer Schema (CURRENT - DP-004)
**Status**: Accepted

Store raw JSON payloads, not transformed metrics:

```
timestamp       | DateTime<Utc>  | Ingestion timestamp
source_id       | String         | Source identifier
ndp_id          | String?        | Stable platform-owned identifier
context         | JSON?          | Config-derived metadata snapshot
raw_payload     | JSON           | Exact payload from source (untransformed)
```

**Key Principles**:
- `raw_payload` is sacred - exact byte-for-byte copy
- `context` is snapshot - frozen at ingestion time
- No parsing in Bronze - defer to Silver ETL
- Wide format - one row per message

**Supersedes**: Previous tall schema (one row per metric)

### Silver Layer Schema (CURRENT - AIR-009 ADR-003)
**Status**: Proposed

```sql
- time TIMESTAMPTZ (hypertable dimension)
- ndp_id TEXT (indexed)
- stream_id TEXT (indexed)
- context JSONB (GIN indexed)
- Measurement columns (typed: pm25 FLOAT, temperature FLOAT, etc.)
```

### Context Storage Strategy (CURRENT - AIR-009 ADR-002-AMENDMENT-002)
**Status**: Accepted

**Decision**: Simple blob storage - store context as single JSON blob, no flattening.

**Supersedes**:
- AIR-009 ADR-002 (original): Dot-notation flattening
- AIR-009 ADR-002-AMENDMENT-001: Hybrid promoted fields + blob

---

## 3. Source Identity

### ndp_id: Stable Source Identifier (AIR-009 ADR-001)
**Status**: Proposed

**Decision**: Platform-owned, immutable identifier for each source instance.

**Naming Convention**: `{device-type}-{location-hint}-{sequence}`
- Example: `airgradient-office-001`, `window-office-001`

**Design Principles**:
- `ndp_id` is outside context (pure identity, not attribute)
- `ndp_id` is immutable (never changes after assignment)
- `ndp_id` is required for new records
- `ndp_id` is unique across system

**Problem Solved**: Unstable device identifiers (serial numbers change with replacements)

---

## 4. Configuration Management

### etcd-Based Configuration (AIR-003)
**Status**: Implemented

**Key Structure**:
```
/streams/{stream-id}/config   → StreamConfig JSON
/streams/{stream-id}/schema   → SchemaDefinition JSON
/streams/{stream-id}/sources  → SourceConfig[] JSON
```

**Architecture Patterns**:
- Watch-based hot reload (< 100ms propagation)
- GitOps integration (YAML → etcd sync via `deploy.sh`)
- Environment variable override (Env > etcd > Defaults)

### GitOps Configuration Pattern (DP-001 ADR-003)
**Status**: Accepted

Split configurations:
- **Static** (Git-managed): Docker configs, provisioned dashboards
- **Dynamic** (GitOps → etcd): Stream configs, runtime parameters

---

## 5. Ingestion Pipeline

### Channel Ownership (AIR-005 context, AIR-011 refinement)
**Status**: Accepted

**Decision**: IngestionCoordinator owns master mpsc channel.
- Sources send to coordinator's channel
- Coordinator routes to storage layers
- Single point of coordination

### Parser Architecture (CURRENT - AIR-006)
**Status**: Proposed

**Decision**: Unified `Parser` trait system, delete `ResponseParser`.

```rust
pub trait Parser: Send + Sync {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &str;
    fn config(&self) -> &ParserConfig;
}
```

**Implementations**:
- `FlatJsonParser` - Single JSON object
- `JsonPathParser` - Specific field extraction
- `ArrayIteratorParser` - Array-wrapped responses (AIR-006)
- `ColumnOrientedParser` - Column-oriented data like NWS (AIR-007)

**Supersedes**: `ResponseParser` struct-based system (AIR-005)

### Parser Decoupling (AIR-011 ADR-001)
**Status**: Implemented (via trait separation)

**Problem**: Double polling caused memory accumulation and Pi lockups.

**Actual Implementation** (differs from original proposal):
- Feature gating (`#[cfg(feature = "etl")]`) was NOT implemented
- Instead, decoupling achieved via **dual-trait architecture**:

**Two Trait Paths** (defined in code):

| Trait | Method | Output | Use Case |
|-------|--------|--------|----------|
| `Source` | `fetch()` | `Vec<TimeSeriesPoint>` | Parsed data (Silver layer) |
| `RawSource` | `fetch_raw()` / `fetch_raw_batch()` | `RawDataPoint` | Raw JSON (Bronze layer) |

### Active Ingestion Flow (CURRENT)

**CRITICAL**: Only `RawSource::fetch_raw_batch()` is called in production.

**Verification** (from `apps/air-quality-app/src/main.rs`):
```
Line 123: "Note: Legacy MQTT path removed. MQTT is now managed by IngestionCoordinator"
Line 279-280: "Single ingestion path using RawDataPoint with 5-column schema"
```

**Production Flow**:
```
main.rs
  └→ IngestionCoordinator
       └→ app's SourceManager (apps/air-quality-app/src/coordinator/source_manager.rs)
            └→ source.fetch_raw_batch()  ← ONLY this is called
                 └→ RawDataPoint (no parsing)
                      └→ ParquetStore (Bronze layer)
```

**Code That Exists But Is NOT Called**:
| Code Location | Uses | Status |
|---------------|------|--------|
| `apps/.../ingestion/mqtt_handler.rs` | `source.fetch()` | **DEAD CODE** - module exported but never imported |
| `core/src/coordinator/source_manager.rs` | `.fetch()` | **NOT USED** - app has its own source_manager |
| Parser implementations | All parser code | **NOT INVOKED** - reserved for future Silver ETL |

**Key Insight**: The dual-trait architecture exists, but currently **only the RawSource path is wired up**. The Source trait path with parsers is available for future Silver layer ETL but is not connected to the ingestion pipeline.

**Bronze Path (ACTIVE)**: `fetch_raw_batch()` → `RawDataPoint` → Parquet (raw JSON preserved)
**Silver Path (FUTURE)**: `fetch()` → `TimeSeriesPoint` → TimescaleDB (parsed fields) - NOT YET IMPLEMENTED

**Parser configs remain in stream config** for future use when Silver ETL activates the `Source` trait path.

---

## 6. HTTP Polling Architecture

### Generic HTTP Polling (AIR-005)
**Status**: Implemented

**Components**:
- `ResponseParser` trait (→ superseded by `Parser` trait in AIR-006)
- `AuthMethod` enum: None, QueryParam, Header, BasicAuth
- `RetryConfig`: Exponential backoff with jitter
- `EndpointConfig`: Generic endpoint configuration

**Error Classification**:
| Type | Response | Action |
|------|----------|--------|
| Transient | 5xx, network | Exponential backoff |
| Rate Limited | 429 | Parse Retry-After, respect quota |
| Permanent | 401, 403, 404 | Log error, skip endpoint |

### Health Check Pattern
Staleness detection: Alert if no data for 2× poll_interval

---

## 7. MQTT Architecture

### Multi-Subscription Pattern (DP-003 ADR-001)
**Status**: Proposed

**Decision**: Single MqttSource with subscription array (not multiple connections).

```yaml
sources:
  - type: mqtt
    params:
      broker_url: "mosquitto"
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
```

### Topic Routing Algorithm (DP-003 ADR-003)
**Status**: Proposed

- MQTT pattern → compiled regex at config load
- First match wins (order matters)
- Dead letter queue for unmatched messages
- Performance: <1 microsecond per match

**Pattern Conversion**:
- `+` → `[^/]+` (single level)
- `#` → `.*` (multi-level, must be at end)

---

## 8. Stream Configuration

### Stream Configuration Files (CURRENT)
**Status**: Implemented

**File Location**:
```
config/base/streams/{stream-id}/config.yaml
```

Each stream has a **single config.yaml** containing:
- Stream metadata (stream_id, description, version, retention)
- Field definitions (fields section)
- Source configurations (sources array with type, params, parser)
- Entity schemas for Data Dictionary (entity_schemas section)

**Example Structure** (air-quality/config.yaml):
```yaml
stream_id: "air-quality"
description: "AirGradient sensor readings"
version: "1.0.0"
enabled: true
retention_days: 365

fields:
  pm25: { type: "float", unit: "µg/m³" }
  ...

sources:
  - type: mqtt
    enabled: true
    ndp_id: "aq_airgradient_1"
    context: { device_type: airgradient, ... }
    broker_url: "mosquitto"
    topic_pattern: "airgradient/readings/+"
    parser:
      parser_type: flat_json
      ...

entity_schemas:
  - schema_name: airgradient
    description: "AirGradient indoor sensors"
    attributes: [...]
```

### etcd Key Structure (Runtime)
**Status**: Implemented

Configs are synced from YAML to etcd at runtime:
```
/air-quality/streams/{stream-id}/config   → Full configuration JSON
/air-quality/streams/{stream-id}/enabled  → Stream enabled flag
```

**Dynamic Registration**: Watch API for hot-reload when configs change

### Stream Separation Strategy
**Source**: AIR-005, AIR-007

**Decision**: Separate streams for different data structures/update frequencies.

Examples:
- `outdoor-weather` + `outdoor-air-quality` (OpenWeatherMap)
- `nws-gridpoints-forecast` + `nws-station-observations` (NWS)

**Rationale**:
- Different polling intervals
- Different data structures (column-oriented vs flat)
- Different retention policies
- Independent failure domains

---

## 9. Analytics Layer

### Virtual Silver Layer (DP-001 ADR-001)
**Status**: Accepted (for DuckDB analytics)

**Decision**: DuckDB views over Bronze Parquet (query-time DQ).
- No ETL complexity
- No data duplication
- Always fresh data

**Note**: Coexists with TimescaleDB Silver for different use cases.

### Data Dictionary (DP-002)
**Status**: Implemented

**Schema** (TimescaleDB `data_dictionary` schema):
- `streams` - Stream metadata (stream_id, description, version, retention)
- `fields` - Field definitions per stream
- `sources` - Source configurations per stream
- `entity_schemas` - Entity definitions with device_class
- `entity_schema_attributes` - Attribute definitions per entity
- `sync_status` - Audit trail of sync operations

**Views**:
- `v_data_dictionary` - Joined view of streams → entities → attributes
- `stream_overview` - Stream summary with field/source/schema counts

**Dynamic Loading Mechanism**:
1. SQL schema created at container start (`init-scripts/01-create-data-dictionary.sql`)
2. `deploy.sh sync` or `deploy.sh start` triggers sync
3. `sync_to_data_dictionary()` function parses all `config/base/streams/*/config.yaml` files
4. Extracts `entity_schemas` sections and generates INSERT statements
5. Full refresh: deletes existing data, re-inserts from config files
6. Records sync status with counts and timestamps

**Config → Database Flow**:
```
config/base/streams/{stream}/config.yaml
        ↓ (deploy.sh sync_to_data_dictionary)
    Generated SQL INSERT statements
        ↓ (psql to TimescaleDB)
    data_dictionary.* tables populated
```

---

## 10. Error Handling

### Dead Letter Queue Pattern (AIR-002 ADR-003)
**Status**: Implemented

**Format**: `/data/dlq/YYYY-MM-DD.jsonl`

**Benefits**:
- Data preservation
- Debugging capability
- Replay potential

### Dual-Write Semantics (AIR-004)
**Status**: Proposed

- Bronze write: **AUTHORITATIVE** (must succeed)
- Silver write: **BEST-EFFORT** (can fail, can rebuild)

---

## 11. Resource Constraints

### Raspberry Pi 5 Budget
**Source**: Multiple features

| Constraint | Target |
|------------|--------|
| Total Memory | < 2GB |
| Single App | < 512MB |
| Config Propagation | < 100ms |
| Cross-stream Query | < 100ms p99 |

### Container Memory Allocation (DP-001)
| Service | Limit |
|---------|-------|
| mosquitto | 128MB |
| etcd | 256MB |
| air-quality-app | 512MB |
| duckdb | 512MB |
| grafana | 256MB |
| timescaledb | 256MB |

---

## 12. Deployment Architecture

### Docker-Based Deployment (CURRENT)
**Status**: Implemented

**Primary Entry Point**: `deploy/pi/deploy.sh`

**Commands**:
| Command | Action |
|---------|--------|
| `./deploy.sh` | Full deploy (build + start) |
| `./deploy.sh start` | Start all services |
| `./deploy.sh stop` | Stop all services |
| `./deploy.sh logs` | View container logs |
| `./deploy.sh status` | Check service health |
| `./deploy.sh update` | Pull latest and rebuild |

**Startup Sequence**:
1. `dc up -d` - Start Docker Compose services
2. `sync_config()` - Sync YAML configs to etcd
3. `init_streams()` - Initialize stream configurations
4. `sync_to_data_dictionary()` - Populate TimescaleDB data dictionary

**Container Stack** (docker-compose.yml):
- `mosquitto` - MQTT broker (128MB)
- `etcd` - Configuration store (256MB)
- `air-quality-app` - Rust ingestion application (512MB)
- `pi5-timescaledb` - TimescaleDB for Silver layer (256MB)
- `grafana` - Dashboards (256MB)

**Configuration Flow**:
```
config/base/streams/*.yaml
        ↓ (sync_config → etcd)
    Runtime configuration
        ↓ (watch API)
    Hot-reload to air-quality-app
```

**Init Scripts** (TimescaleDB):
- `01-create-data-dictionary.sql` - Data dictionary schema
- `02-create-users.sql` - Database users and permissions

---

## 13. Superseded Decisions Summary

| Original | Superseded By | Reason |
|----------|---------------|--------|
| AIR-002 ADR-005: Minimal YAML config | AIR-003: etcd config | Standardization |
| AIR-005: ResponseParser struct | AIR-006: Parser trait | Unification, config-driven |
| AIR-009 ADR-002: Dot-notation flattening | ADR-002-AMEND-002: Simple blob | Simplicity, reconstruction |
| AIR-009 ADR-002-AMEND-001: Hybrid promoted fields | ADR-002-AMEND-002: Simple blob | Unnecessary complexity |
| Previous tall Bronze schema | DP-004: Wide raw JSON | Schema-on-read, replay |
| DP-001: DuckDB for Silver | DP-002: TimescaleDB for Silver | Time-series optimization |

---

## 14. Pending/Proposed Decisions

| Decision | Feature | Status | Notes |
|----------|---------|--------|-------|
| ndp_id implementation | AIR-009 | Proposed | Stable identifiers |
| MQTT multi-subscription | DP-003 | Proposed | Topic routing |
| ColumnOrientedParser | AIR-007 | Proposed | NWS data support |

**Recently Implemented** (moved from proposed):
- Parser decoupling (AIR-011) - Via dual-trait architecture
- Data Dictionary (DP-002) - Dynamic sync from config
- Bronze raw JSON schema (DP-004) - RawDataPoint implemented

---

## 15. Key Patterns Reference (Updated)

| Pattern | Description | Source |
|---------|-------------|--------|
| Domain Adapter | Hexagonal architecture with traits as ports | AIR-001 |
| Channel Ownership | Coordinator owns master mpsc channel | AIR-005, AIR-011 |
| Watch-Based Config | Hot reload via etcd watch API | AIR-003 |
| Stream Registry | Dynamic stream configuration | AIR-004 |
| Schema-on-Read | Raw storage, transform at query | DP-004 |
| Dual-Write | Bronze authoritative, Silver best-effort | AIR-004 |
| Dead Letter Queue | Preserve failed messages | AIR-002 |
| Parser Trait | Unified config-driven parsing | AIR-006 |
| Dual-Trait Architecture | Source (parsed) vs RawSource (raw) for layer separation | AIR-011 |
| Topic Routing | Pattern-based MQTT routing | DP-003 |
| GitOps Split | Static (Git) + Dynamic (etcd) config | DP-001 |
| Config-Driven Data Dictionary | YAML entity_schemas → TimescaleDB sync | DP-002 |

---

## 16. File References

### Core Architecture Documents
- `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
- `docs/architecture/COMPONENT_DEPENDENCY_MAP.md`

### Feature Architecture (by Phase)

**Air Quality Phase**:
- `product/features/air-001/architecture/01-system-design.md`
- `product/features/air-004/architecture/ADR-001-MULTISTREAM-FOUNDATION.md`
- `product/features/air-006/architecture/ARCHITECTURE.md` (Parser unification)
- `product/features/air-009/architecture/ADR-002-AMENDMENT-002-simple-blob.md` (Context storage)
- `product/features/air-011/architecture/ADR-001-parser-archive.md` (Parser decoupling)

**Data Platform Phase**:
- `product/features/dp-001/architecture/ARCHITECTURE_DOCUMENTATION_SUMMARY.md`
- `product/features/dp-002/architecture/ADR-001-TIMESCALEDB-SCHEMA.md`
- `product/features/dp-003/architecture/ADR-001-MQTT-SUBSCRIPTIONS.md`
- `product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md`

---

## Review Checklist

Before storing as patterns, verify:

- [ ] All superseded decisions correctly identified
- [ ] Current state reflects highest-numbered decisions
- [ ] No conflicting decisions remain
- [ ] Implementation status accurate
- [ ] Resource constraints still valid
- [ ] Pattern names suitable for semantic search

---

*This document consolidates architecture research for pattern storage review. Do not modify without re-running feature analysis.*
