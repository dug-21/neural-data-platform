# Crate Layout Patterns for Gold Layer

**Created**: 2026-02-03
**Author**: NDP Architect Agent
**Feature**: fe-001 (Gold Layer Foundation via V1.1)
**Status**: Analysis Complete - Recommendations Provided

---

## Executive Summary

This document analyzes the current NDP crate structure and recommends the module/crate layout for the Gold Layer (V1.1 through V2.0). The recommendation is to **extend `core/src/` with a new `gold/` module** rather than creating a separate crate, maintaining consistency with the established pattern for Bronze and Silver layers.

---

## 1. Current Crate Architecture

### 1.1 Workspace Structure

```
/workspaces/neural-data-platform/
├── Cargo.toml                    # Workspace root (8 members)
├── core/                         # platform-core library crate
├── crates/
│   └── ndp-types/               # Shared type definitions
├── config-client/               # etcd configuration client
├── domains/
│   └── air-quality/             # Domain-specific logic
├── apps/
│   ├── air-quality-app/         # Main ingestion application
│   └── silver-etl/              # Silver layer ETL binary + library
└── tools/
    └── ndp-validate/            # Validation CLI tool
```

### 1.2 Dependency Graph

```
                    ┌─────────────────────┐
                    │   ndp-types         │
                    │   (crates/)         │
                    │ - SourceType        │
                    │ - FieldType         │
                    │ - DqRule enums      │
                    │ - Validation traits │
                    └─────────┬───────────┘
                              │
                              │ workspace dependency
                              ▼
┌───────────────────────────────────────────────────────────────────┐
│                        platform-core                               │
│                        (core/)                                     │
│ ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────┐ ┌────────┐ │
│ │ sources │ │ storage  │ │ parsers   │ │ silver     │ │ traits │ │
│ │ - mqtt  │ │ - parquet│ │ - factory │ │ - transform│ │ - Source│
│ │ - http  │ │ - wal    │ │ - flat_json│ │ - dq_eval │ │ - Store │
│ │ - csv   │ │          │ │ - column  │ │ - outputs  │ │ - Fore- │
│ └─────────┘ └──────────┘ └───────────┘ └────────────┘ │   cast  │
│ ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────┐ └────────┘ │
│ │ config  │ │ types    │ │ coordinator│ │ subscribers│            │
│ │ - silver│ │ - stream │ │ - ingest  │ │ - bronze   │            │
│ │   _etl  │ │   config │ │ - source  │ │ - silver   │            │
│ │ - mock  │ │ - raw_data│ │   manager │ │ - processor│            │
│ └─────────┘ └──────────┘ └───────────┘ └────────────┘            │
│ ┌─────────┐ ┌──────────┐ ┌───────────┐ ┌────────────┐            │
│ │ dimensions│ │ mcp      │ │ outputs   │ │ processors │            │
│ │ - ddl   │ │ - tools  │ │ - mqtt    │ │ - threshold│            │
│ │ - loader│ │ - handler│ │           │ │            │            │
│ └─────────┘ └──────────┘ └───────────┘ └────────────┘            │
└───────────────────────────────────────────────────────────────────┘
                              │
                              │ depends on
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ air-quality     │ │ config-client   │ │ silver-etl      │
│ (domains/)      │ │                 │ │ (apps/)         │
│ - parser        │ │ - etcd wrapper  │ │ - sql_gen       │
│ - types         │ │ - ConfigLoader  │ │ - schema_gen    │
│ - adapter       │ │   trait         │ │ - daemon        │
│ - validation    │ │                 │ │ - pre_transform │
└─────────────────┘ └─────────────────┘ └─────────────────┘
              │               │               │
              └───────────────┴───────────────┘
                              │
                              ▼
                    ┌─────────────────┐
                    │ air-quality-app │
                    │ (apps/)         │
                    │ - main binary   │
                    │ - MCP binary    │
                    │ - REST handlers │
                    └─────────────────┘
```

### 1.3 Core Module Organization

The `core/` crate (`platform-core`) follows a **capability-based module structure**:

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `traits/` | Port definitions (hexagonal) | `Source`, `Store`, `RawSource`, `RawStore`, `Forecast` |
| `types/` | Domain types | `StreamConfig`, `TimeSeriesPoint`, `RawDataPoint`, `SchemaField` |
| `sources/` | Source adapters | `MqttSource`, `HttpPollingSource`, `CsvSource` |
| `storage/` | Storage adapters | `ParquetStore`, `WriteAheadLog` |
| `parsers/` | Parsing framework | `Parser` trait, `FlatJsonParser`, `ArrayIteratorParser` |
| `config/` | Configuration types | `SilverEtlConfig`, `MockConfigLoader` |
| `silver/` | Silver layer logic | `transform_to_silver`, `DqEvaluator`, `TimescaleOutput` |
| `coordinator/` | Orchestration | `IngestionCoordinator`, `SourceManager` |
| `subscribers/` | Event subscribers | `BronzeSubscriber`, `SilverSubscriber`, `ProcessorSubscriber` |
| `processors/` | Data processors | `ThresholdProcessor` |
| `outputs/` | Output sinks | `MqttOutput` |
| `dimensions/` | Dimension tables | `CsvDimensionLoader`, `DdlGenerator` |
| `mcp/` | MCP server tools | Tool handlers, protocol types |
| `event_bus/` | Event distribution | `EventBus`, `EventBusConfig` |
| `error/` | Error types | `CoreError`, `CoreResult` |

---

## 2. Bronze and Silver Layer Organization

### 2.1 Bronze Layer (current)

Bronze layer logic is distributed across:

| Location | Responsibility |
|----------|----------------|
| `core/src/storage/parquet.rs` | `ParquetStore` - Parquet write operations |
| `core/src/storage/wal.rs` | Write-ahead log for crash recovery |
| `core/src/types/raw_data_point.rs` | `RawDataPoint` - Bronze record type |
| `core/src/subscribers/bronze.rs` | `BronzeSubscriber` - Event handling |

**Key Insight**: Bronze is primarily a **storage concern** - no transformation logic, just raw append.

### 2.2 Silver Layer (current)

Silver layer is organized as a **module within core**:

```
core/src/silver/
├── mod.rs              # Module exports
├── types.rs            # SilverRecord, DqResult, DqViolation
├── transform.rs        # transform_to_silver function
├── dq_evaluator.rs     # DQ rule evaluation logic
└── outputs/
    ├── mod.rs
    └── timescale.rs    # TimescaleOutput adapter
```

**Additional Silver logic** exists in `apps/silver-etl/`:

```
apps/silver-etl/src/
├── lib.rs              # Library exports
├── main.rs             # CLI binary
├── config.rs           # Configuration loading
├── etl.rs              # EtlRunner orchestration
├── sql_gen.rs          # SQL generation from config
├── schema_gen.rs       # Schema generation
├── dq.rs               # DQ SQL generation
├── pre_transform.rs    # Array explosion, pre-transforms
├── persistence.rs      # ETL run tracking
├── metrics.rs          # Prometheus metrics
└── daemon.rs           # Long-running ETL daemon
```

**Key Insight**: Silver uses a **hybrid pattern**:
- **Core types and transforms** in `core/src/silver/`
- **ETL orchestration and SQL** in `apps/silver-etl/`

This separation allows:
1. Core library to remain infrastructure-agnostic
2. ETL app to include DuckDB dependency (heavy, optional)
3. Other apps to use Silver types without ETL machinery

---

## 3. Hexagonal Architecture Patterns

### 3.1 Trait Definitions (Ports)

All major interfaces are defined as traits in `core/src/traits.rs`:

```rust
// Source Port - All data sources implement this
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

// RawSource Port - Bronze layer sources
#[async_trait]
pub trait RawSource: Send + Sync {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint>;
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>>;
}

// Store Port - All storage backends
#[async_trait]
pub trait Store: Send + Sync {
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;
    async fn query(...) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn aggregate(...) -> CoreResult<Vec<AggregatedPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}
```

### 3.2 Adapter Pattern

Each adapter implements traits without knowing about others:

```rust
// HttpPollingSource adapts HTTP API to Source/RawSource traits
pub struct HttpPollingSource { ... }

impl RawSource for HttpPollingSource {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        // HTTP request, no parsing beyond raw JSON
    }
}

// TimescaleOutput adapts PostgreSQL to SilverOutput trait
pub struct TimescaleOutput { ... }

impl SilverOutput for TimescaleOutput {
    async fn write_batch(&self, records: &[SilverRecord]) -> Result<()> {
        // Insert into TimescaleDB
    }
}
```

### 3.3 Configuration-Driven Behavior

The Silver ETL demonstrates configuration-driven design:

```yaml
# config/base/streams/air-quality/config.yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp
  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

The `SilverEtlConfig` struct in `core/src/config/silver_etl.rs` provides:
- Type-safe deserialization
- Validation methods
- Default values

---

## 4. Gold Layer Recommendations

### 4.1 Recommended Structure: Module in Core

**Decision**: Add `core/src/gold/` module rather than separate crate.

**Rationale**:
1. **Consistency**: Follows Silver layer pattern (`core/src/silver/`)
2. **Shared types**: Gold needs `StreamConfig`, `SilverRecord`, error types
3. **Feature gating**: Optional TimescaleDB dependency via Cargo features
4. **Testing**: Unit tests can share fixtures with Bronze/Silver
5. **Simplicity**: No additional workspace member management

### 4.2 Proposed Gold Module Structure

```
core/src/gold/
├── mod.rs                      # Module exports
├── types.rs                    # Gold-specific types
│   ├── GoldRecord              # Aligned, feature-enriched record
│   ├── EventRecord             # Unified event abstraction
│   ├── FeatureVector           # ML-ready feature set
│   └── ObjectiveTarget         # Declared objective
├── config/
│   ├── mod.rs
│   ├── gold_etl.rs            # GoldEtlConfig (v11-A01)
│   ├── alignment.rs           # AlignmentConfig (v11-A03)
│   └── objectives.rs          # ObjectivesConfig (v11-A05)
├── interpreter/
│   ├── mod.rs
│   ├── aggregate.rs           # Continuous aggregate SQL generation
│   ├── alignment.rs           # Cross-stream alignment SQL
│   ├── transitions.rs         # State transition materialization
│   └── threshold_crossing.rs  # Threshold event generation (v11-012)
├── features/
│   ├── mod.rs
│   ├── registry.rs            # FeatureTypeRegistry (v11-A06)
│   ├── lag.rs                 # Lag feature generator
│   ├── rolling.rs             # Rolling statistics generator
│   └── trend.rs               # Trend computation
├── events/
│   ├── mod.rs
│   ├── unified.rs             # Unified events view (v11-013)
│   └── aggregation.rs         # Hourly event aggregation
└── outputs/
    ├── mod.rs
    └── timescale.rs           # GoldTimescaleOutput
```

### 4.3 Feature Gating

Add to `core/Cargo.toml`:

```toml
[features]
default = []
timescale = ["tokio-postgres", "bb8", "bb8-postgres"]
gold = ["timescale"]  # Gold requires TimescaleDB

[dependencies]
# ... existing deps ...
```

This allows apps to opt-in to Gold capabilities:

```toml
# apps/gold-etl/Cargo.toml (future)
[dependencies]
neural-core = { path = "../../core", package = "platform-core", features = ["gold"] }
```

### 4.4 Gold ETL Application (Future)

Following the `apps/silver-etl/` pattern, create `apps/gold-etl/`:

```
apps/gold-etl/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Library exports
│   ├── main.rs                # CLI binary
│   ├── config.rs              # Configuration loading
│   ├── etl.rs                 # GoldEtlRunner orchestration
│   ├── sql_gen.rs             # SQL generation from gold_etl config
│   ├── schema_gen.rs          # Gold schema generation
│   ├── daemon.rs              # Long-running Gold ETL daemon
│   └── metrics.rs             # Prometheus metrics
└── tests/
    └── integration/
```

**Key Difference from Silver ETL**: Gold ETL reads from Silver (TimescaleDB), not Bronze (Parquet). This eliminates DuckDB dependency.

---

## 5. Type Organization

### 5.1 Where Types Should Live

| Type Category | Location | Rationale |
|---------------|----------|-----------|
| **Enums for JSON Schema** | `crates/ndp-types/` | Shared across all crates, JSON Schema generation |
| **Config structs** | `core/src/gold/config/` | Feature-gated, serde derives |
| **Domain types** | `core/src/gold/types.rs` | Internal to gold module |
| **Output traits** | `core/src/gold/outputs/mod.rs` | Hexagonal port definition |

### 5.2 Example: GoldEtlConfig

```rust
// core/src/gold/config/gold_etl.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub aggregates: AggregatesConfig,
    pub features: FeaturesConfig,
    pub transitions: Option<TransitionsConfig>,
    pub alignment: Option<AlignmentConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct AggregatesConfig {
    pub granularities: Vec<String>,  // ["1 hour", "1 day"]
    pub default_metrics: Vec<String>, // ["mean", "std", "min", "max"]
    pub fields: HashMap<String, FieldAggregateConfig>,
}
```

### 5.3 Example: Unified Event Type

```rust
// core/src/gold/events/unified.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedEvent {
    pub event_id: String,
    pub event_time: DateTime<Utc>,
    pub stream_id: String,
    pub entity_id: String,
    pub event_type: EventType,
    pub details: Value,  // Type-specific payload as JSON
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    StateTransition,
    ThresholdCrossing,
    // V1.2+
    Anomaly,
    TrendChange,
}
```

---

## 6. Versioned Architecture (V1.1 through V2.0)

### 6.1 V1.1 Scope (Gold Layer Foundation)

Core module structure:

```
core/src/gold/
├── config/
│   ├── gold_etl.rs            # v11-A01
│   ├── alignment.rs           # v11-A03
│   └── objectives.rs          # v11-A05
├── interpreter/
│   ├── aggregate.rs           # v11-A02
│   └── alignment.rs           # v11-A04
├── features/
│   └── registry.rs            # v11-A06 (basic)
├── events/
│   ├── transitions.rs         # v11-006
│   ├── threshold_crossing.rs  # v11-012
│   └── unified.rs             # v11-013
└── outputs/
    └── timescale.rs
```

### 6.2 V1.2 Scope (Pattern Detection)

Extend with:

```
core/src/gold/
├── correlation/               # NEW
│   ├── mod.rs
│   ├── scanner.rs             # Granger causality scanner (v12-002)
│   ├── response.rs            # Response window analyzer (v12-003)
│   ├── lag.rs                 # Lag optimizer (v12-004)
│   ├── aggregator.rs          # Correlation aggregator (v12-005)
│   └── candidates.rs          # Candidate registry (v12-008)
└── events/
    └── anomaly.rs             # NEW - Anomaly detection
```

### 6.3 V1.3 Scope (Prediction & Actions)

Extend with:

```
core/src/gold/
├── causal/                    # NEW
│   ├── mod.rs
│   ├── validation.rs          # PC algorithm (v13-001)
│   └── experiments.rs         # Natural experiment detection
├── models/                    # NEW
│   ├── mod.rs
│   ├── zoo.rs                 # Model registry (v13-002)
│   ├── tournament.rs          # Model selection (v13-003)
│   └── prediction.rs          # Prediction service (v13-004)
├── actions/                   # NEW
│   ├── mod.rs
│   ├── framework.rs           # Action framework (v13-005)
│   ├── scoring.rs             # Action scoring (v13-006)
│   ├── outcomes.rs            # Outcome tracker (v13-007)
│   └── autonomy.rs            # Autonomy controller (v13-009)
└── events/
    └── trend.rs               # NEW - Trend change events
```

### 6.4 V2.0 Scope (Multi-Stream Intelligence)

Extend with:

```
core/src/gold/
├── domains/                   # NEW
│   ├── mod.rs
│   ├── registry.rs            # Domain registry
│   └── objectives.rs          # Multi-stream objectives
└── financial/                 # NEW (optional feature)
    ├── mod.rs
    └── sources/               # FRED, Alpaca adapters
```

---

## 7. Cross-Cutting Concerns

### 7.1 Error Handling

Follow existing pattern in `core/src/error.rs`:

```rust
// Add Gold-specific errors
#[derive(Debug, Error)]
pub enum CoreError {
    // ... existing variants ...

    #[error("Gold ETL error: {0}")]
    GoldEtl(String),

    #[error("Feature computation error: {0}")]
    FeatureComputation(String),

    #[error("Event generation error: {0}")]
    EventGeneration(String),

    #[error("Alignment error: {0}")]
    Alignment(String),
}
```

### 7.2 Logging

Follow existing `tracing` pattern:

```rust
use tracing::{debug, info, warn, error, instrument};

#[instrument(skip(self, config))]
pub async fn generate_alignment_sql(&self, config: &AlignmentConfig) -> Result<String> {
    info!(streams = ?config.streams.len(), "Generating alignment SQL");
    // ...
}
```

### 7.3 Testing

Follow existing London School TDD pattern with mocks:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub GoldOutput {}

        #[async_trait]
        impl GoldOutput for GoldOutput {
            async fn write_aligned(&self, records: &[AlignedRecord]) -> Result<()>;
            async fn write_events(&self, events: &[UnifiedEvent]) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn test_alignment_generation() {
        let mut mock_output = MockGoldOutput::new();
        mock_output.expect_write_aligned()
            .times(1)
            .returning(|_| Ok(()));
        // ...
    }
}
```

---

## 8. Migration Path

### 8.1 Phase 1: Module Structure (Week 1)

1. Create `core/src/gold/mod.rs` with feature gate
2. Add `gold` feature to `core/Cargo.toml`
3. Create config types (`GoldEtlConfig`, `AlignmentConfig`, `ObjectivesConfig`)
4. Create JSON Schema files in `config/schemas/`

### 8.2 Phase 2: Interpreter Foundation (Week 2)

1. Implement `GoldEtlInterpreter` basic structure
2. Generate first continuous aggregate SQL
3. Unit tests with mock output

### 8.3 Phase 3: Cross-Stream Alignment (Week 3)

1. Implement `AlignmentInterpreter`
2. Generate aligned view SQL
3. Integration test with test TimescaleDB

### 8.4 Phase 4: Events and Features (Week 4-5)

1. Implement state transition materializer
2. Implement threshold crossing generator
3. Implement unified events view
4. Fast-follower validation test

---

## 9. Alternatives Considered

### 9.1 Separate Gold Crate

**Option**: Create `crates/ndp-gold/` as independent crate.

**Pros**:
- Clean separation
- Independent versioning
- Smaller compile units

**Cons**:
- More workspace complexity
- Duplicate type imports
- Harder to share test fixtures
- Inconsistent with Bronze/Silver pattern

**Decision**: Rejected - Module in core is simpler and consistent.

### 9.2 Gold in silver-etl App

**Option**: Extend `apps/silver-etl/` to handle Gold.

**Pros**:
- Single ETL application
- Shared orchestration code

**Cons**:
- Growing complexity
- Mixed responsibilities
- Harder to test in isolation

**Decision**: Rejected - Separate `apps/gold-etl/` maintains clear boundaries.

### 9.3 DuckDB for Gold Layer

**Option**: Use DuckDB continuous views for Gold aggregates.

**Pros**:
- Already in stack (DP-001)
- Good performance

**Cons**:
- No continuous aggregates (must re-query)
- Limited to batch processing
- Memory pressure on Pi

**Decision**: Rejected per existing architecture - TimescaleDB for Silver/Gold.

---

## 10. Summary

### Key Recommendations

| Aspect | Recommendation | Rationale |
|--------|----------------|-----------|
| **Gold types** | `core/src/gold/` module | Consistency with Silver pattern |
| **Feature gating** | `gold` feature requiring `timescale` | Optional heavy dependency |
| **Config types** | `core/src/gold/config/` | Serde derives, validation |
| **Enums for schema** | `crates/ndp-types/` | JSON Schema generation |
| **ETL orchestration** | `apps/gold-etl/` (future) | Clean binary separation |
| **Testing** | London School TDD with mocks | Project standard |

### Next Steps

1. **Create skeleton**: `core/src/gold/mod.rs` with feature gate
2. **Define config types**: Start with `GoldEtlConfig`
3. **Implement interpreter**: Basic SQL generation
4. **Validate with fast-follower test**: Add stream via config only

---

## References

- [FEATURE-ROADMAP.md](/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md) - Full Gold layer roadmap
- [PLATFORM_ARCHITECTURE_OVERVIEW.md](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) - Current architecture
- [CONSOLIDATED_ARCHITECTURE_DECISIONS.md](/workspaces/neural-data-platform/docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md) - ADR history
- [core/Cargo.toml](/workspaces/neural-data-platform/core/Cargo.toml) - Core crate configuration
- [apps/silver-etl/src/lib.rs](/workspaces/neural-data-platform/apps/silver-etl/src/lib.rs) - Silver ETL pattern
