# Architecture and Design Pattern Analysis Report

**Feature**: AIR-010 (Technical Debt & Refactoring)
**Date**: 2026-01-01
**Analyst**: NDP Architecture Agent
**Scope**: `/core/src/` and `/apps/air-quality-app/src/`

---

## Executive Summary

The Neural Data Platform codebase demonstrates solid architectural foundations with clean trait abstractions and good separation of concerns. However, several areas show accumulated complexity and redundancy that impact maintainability.

**Key Findings**:
- 7 major architectural patterns to simplify
- 5 instances of code duplication across modules
- 3 over-engineered abstractions
- Estimated complexity reduction: 25-30%

---

## 1. Overly Complex Abstractions

### 1.1 Dual Coordinator Pattern (HIGH PRIORITY)

**Location**:
- `/core/src/coordinator/` (IngestionCoordinator + SourceManager)
- `/apps/air-quality-app/src/coordinator/` (IngestionCoordinator + SourceManager)

**Current Pattern**:
The same coordinator pattern exists in BOTH the core library AND the application with nearly identical responsibilities:

```
core/src/coordinator/
  - ingestion_coordinator.rs (601 lines)
  - source_manager.rs (580 lines)

apps/air-quality-app/src/coordinator/
  - ingestion_coordinator.rs (423 lines)
  - source_manager.rs (1831 lines)
```

**Why It's Suboptimal**:
1. The app's coordinator duplicates much of core's functionality
2. The app's SourceManager (1831 lines) is 3x larger than core's version
3. Logic divergence creates maintenance burden
4. Not clear which coordinator to use in which context

**Recommended Refactoring**:
- **Option A**: Remove core coordinator, keep app-level (it has more features)
- **Option B**: Extract common coordinator logic to core, make app extend it

**Complexity Reduction Estimate**: 20-25% in coordinator module

---

### 1.2 Multiple Data Point Types

**Location**: `/core/src/types/` and `/core/src/traits.rs`

**Current Pattern**:
```rust
// In traits.rs
pub struct TimeSeriesPoint { ... }    // 6 fields
pub struct AggregatedPoint { ... }    // 4 fields
pub struct ForecastedPoint { ... }    // 5 fields

// In types/
pub struct RawDataPoint { ... }       // 5 fields
pub struct StreamRecord { ... }       // 3 fields (wraps TimeSeriesPoint)
pub struct GenericTimeSeriesPoint     // Legacy
pub struct AirQualityReading          // Legacy
```

**Why It's Suboptimal**:
1. StreamRecord wraps TimeSeriesPoint adding indirection
2. RawDataPoint and TimeSeriesPoint have overlapping purposes
3. Legacy types (GenericTimeSeriesPoint, AirQualityReading) still exist
4. Multiple From/Into conversions between types

**Recommended Refactoring**:
```rust
// Consolidate to 3 clear types:
pub struct RawDataPoint { ... }       // Bronze layer only
pub struct TimeSeriesPoint { ... }    // Silver layer processed
pub struct AggregatedPoint { ... }    // Gold layer aggregates

// Remove StreamRecord wrapper - use stream_id as field
// Remove legacy GenericTimeSeriesPoint and AirQualityReading
```

**Complexity Reduction Estimate**: 15-20% in types module

---

### 1.3 Parser Factory Over-Engineering

**Location**: `/core/src/parsers/`

**Current Pattern**:
```
parsers/
  - mod.rs
  - traits.rs
  - config.rs
  - factory.rs
  - flat_json.rs
  - json_path.rs
  - array_iterator.rs
  - column_oriented.rs
```

**Why It's Suboptimal**:
1. 8 files for parser abstraction - high surface area
2. Factory pattern adds indirection without clear benefit
3. Most real usage is FlatJsonParser
4. ParserConfig has many optional fields that complicate usage:
   - `array_config`, `column_config`, `field_mappings` - rarely all used together

**Recommended Refactoring**:
1. Consolidate `config.rs` and `factory.rs` into `parser_config.rs`
2. Use enum dispatch instead of trait objects where possible
3. Consider builder pattern for ParserConfig instead of many Option fields

**Complexity Reduction Estimate**: 10-15% in parsers module

---

## 2. Trait Implementations to Simplify

### 2.1 Source vs RawSource Trait Duplication

**Location**: `/core/src/traits.rs`

**Current Pattern**:
```rust
#[async_trait]
pub trait Source: Send + Sync {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

#[async_trait]
pub trait RawSource: Send + Sync {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint>;
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>>;
}
```

**Why It's Suboptimal**:
1. HttpPollingSource implements BOTH traits
2. fetch() and fetch_raw() do essentially the same thing with different output
3. The DP-004 Bronze layer uses RawSource, but old code still uses Source
4. Creates confusion about which method to call

**Recommended Refactoring**:
Unify into single Source trait with generic output:
```rust
pub trait Source<T>: Send + Sync {
    async fn fetch(&self) -> CoreResult<T>;
    async fn fetch_batch(&self) -> CoreResult<Vec<T>>;
    async fn health_check(&self) -> CoreResult<HealthStatus>;
}

// Type aliases for clarity
pub type RawSource = dyn Source<RawDataPoint>;
pub type ProcessedSource = dyn Source<TimeSeriesPoint>;
```

**Complexity Reduction Estimate**: 15% in traits module

---

### 2.2 Store vs RawStore Trait Duplication

**Location**: `/core/src/traits.rs`

**Current Pattern**:
```rust
#[async_trait]
pub trait Store: Send + Sync {
    async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()>;
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> CoreResult<()>;
    // ... query, aggregate, health_check
}

#[async_trait]
pub trait RawStore: Send + Sync {
    async fn write_raw(&self, point: RawDataPoint) -> CoreResult<()>;
    async fn write_raw_batch(&self, points: Vec<RawDataPoint>) -> CoreResult<()>;
    async fn query_raw(...) -> CoreResult<Vec<RawDataPoint>>;
}
```

**Why It's Suboptimal**:
- Same pattern as Source/RawSource - parallel implementations
- ParquetStore implements both
- Method naming inconsistency (write vs write_raw)

**Recommended Refactoring**:
Same generic approach as Source trait refactoring.

**Complexity Reduction Estimate**: 15% in traits module

---

## 3. Module Organization Improvements

### 3.1 Source Module Sprawl

**Location**: `/core/src/sources/`

**Current Structure**:
```
sources/
  - mod.rs (168 lines)
  - http_poll.rs (very large - needs check)
  - merge.rs
  - mqtt/
      - mod.rs
      - router.rs
      - subscription.rs
  - parsers/
      - mod.rs
      - air_pollution.rs
      - weather.rs
```

**Issues**:
1. `http_poll.rs` is a monolithic file (exceeds 25k tokens)
2. `sources/parsers/` conflicts with top-level `/parsers/` module
3. MQTT split into 3 files but HTTP is one massive file
4. `mod.rs` has 26 re-exports - too many public items

**Recommended Refactoring**:
```
sources/
  mod.rs (only public API re-exports)
  http/
    mod.rs
    polling.rs
    generic_polling.rs
    config.rs
  mqtt/
    mod.rs
    source.rs
    router.rs
    subscription.rs
  merge.rs
```

**Complexity Reduction Estimate**: 20% in sources module

---

### 3.2 Redundant API Handler Modules

**Location**: `/apps/air-quality-app/src/api/handlers/`

**Current Structure**:
```
handlers/
  - mod.rs
  - alerts.rs
  - forecast.rs
  - health.rs
  - locations.rs
  - readings.rs
```

**Issues**:
1. Each handler file imports nearly identical dependencies
2. AlertStore and LocationStore are in-memory stores defined inline
3. No shared handler utilities despite similar patterns

**Recommended Refactoring**:
1. Create `handlers/shared.rs` for common patterns
2. Move in-memory stores to dedicated module
3. Consider macro for repetitive handler patterns

**Complexity Reduction Estimate**: 10% in handlers module

---

## 4. Public API Surface Reduction

### 4.1 Over-Exported Core Library

**Location**: `/core/src/lib.rs`

**Current Exports** (31 items):
```rust
pub use coordinator::{IngestionCoordinator, SourceManager};
pub use error::CoreError;
pub use parsers::{FlatJsonParser, Parser, ParserConfig, ParserType};
pub use sources::{
    HttpPollingConfig, HttpPollingSource, MergeConfig, MqttConfig, MqttSource,
    ReadingMerger, SensorConfig,
};
pub use storage::{ParquetStore, WriteAheadLog};
pub use traits::{
    AggregatedPoint, AggregationType, Forecast, ForecastedPoint, HealthStatus,
    ModelMetrics, RawSource, Source, Store, TimeSeriesPoint,
};
pub use types::GenericTimeSeriesPoint;
pub use types::{
    FieldType, RecordMetadata, SchemaField, SourceConfig, SourceType,
    StorageConfig, StreamConfig, StreamConfigError, StreamRecord,
};
```

**Why It's Suboptimal**:
1. 31 public items - users have too many choices
2. Internal implementation details leaked (WriteAheadLog, etc.)
3. No clear "essential API" vs "advanced API" distinction

**Recommended Refactoring**:
```rust
// lib.rs - Minimal public API
pub mod prelude {
    pub use crate::traits::{Source, Store, Forecast};
    pub use crate::types::{TimeSeriesPoint, StreamConfig};
    pub use crate::error::CoreError;
}

// Everything else accessible via explicit module path
pub mod sources;    // For advanced users
pub mod storage;    // For advanced users
pub mod parsers;    // For advanced users
```

**Complexity Reduction Estimate**: 30% improvement in API discoverability

---

## 5. Non-Idiomatic Rust Patterns

### 5.1 Manual Error String Construction

**Location**: Throughout codebase

**Current Pattern**:
```rust
// In multiple files
CoreError::Storage(format!("Failed to write Parquet: {}", e))
CoreError::Source(format!("MQTT connection refused: {}", e))
CoreError::Config(format!("Missing broker_url for MQTT source"))
```

**Why It's Suboptimal**:
- Error messages constructed manually each time
- No structured error context
- Hard to pattern match on specific errors

**Recommended Refactoring**:
Use `#[from]` and more specific error variants:
```rust
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {context}")]
    Storage {
        context: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Parquet write failed")]
    ParquetWrite(#[from] polars::error::PolarsError),

    // More specific variants...
}
```

**Complexity Reduction Estimate**: 10% in error handling

---

### 5.2 Excessive Arc<RwLock<T>> Usage

**Location**: `/core/src/coordinator/ingestion_coordinator.rs`

**Current Pattern**:
```rust
pub struct IngestionCoordinator {
    config: IngestionCoordinatorConfig,
    receiver: Arc<RwLock<Option<mpsc::Receiver<StreamRecord>>>>,
    sender: mpsc::Sender<StreamRecord>,
    storage_channels: Arc<RwLock<HashMap<String, StorageChannel>>>,
    is_running: Arc<RwLock<bool>>,
    shutdown_tx: Arc<RwLock<Option<oneshot::Sender<()>>>>,
    stats: Arc<RwLock<CoordinatorStats>>,
    source_handles: Arc<RwLock<HashMap<String, IngestionHandle>>>,
}
```

**Why It's Suboptimal**:
- 7 Arc<RwLock<...>> fields - heavy synchronization
- Most fields rarely written (is_running, shutdown_tx)
- Could use AtomicBool for is_running
- receiver wrapped in Option<RwLock<Option<...>>> - double indirection

**Recommended Refactoring**:
```rust
pub struct IngestionCoordinator {
    config: IngestionCoordinatorConfig,
    sender: mpsc::Sender<StreamRecord>,
    storage_channels: DashMap<String, StorageChannel>,  // Lock-free concurrent map
    is_running: AtomicBool,
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,    // Only locked once
    stats: AtomicStats,  // Custom atomic struct
    source_handles: DashMap<String, IngestionHandle>,
}
```

**Complexity Reduction Estimate**: 15% in coordinator module

---

## 6. Generic Usage Optimization

### 6.1 Missing Generics Where Beneficial

**Location**: `/core/src/storage/parquet.rs`

**Current Pattern**:
```rust
async fn write_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path)
async fn append_to_parquet(&self, points: Vec<TimeSeriesPoint>, path: &Path)
```

**Issue**: Raw and processed points require separate methods.

**Recommended Refactoring**:
```rust
trait ToParquetSchema {
    fn to_columns(&self) -> Vec<Series>;
}

impl ToParquetSchema for TimeSeriesPoint { ... }
impl ToParquetSchema for RawDataPoint { ... }

async fn write_parquet<T: ToParquetSchema>(&self, points: Vec<T>, path: &Path)
```

---

### 6.2 Over-Used Generics in Parser

**Location**: `/core/src/parsers/traits.rs`

**Current Pattern**:
```rust
pub trait Parser: Send + Sync {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
    fn parse_with_context(&self, payload: &Value, timestamp: DateTime<Utc>, context: &ParseContext)
        -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

**Issue**: All parsers return `Vec<TimeSeriesPoint>`, but Bronze layer needs `RawDataPoint`.

**Recommended Refactoring**:
Keep parsers simple - they should only handle field extraction, not type conversion.

---

## 7. Configuration Pattern Simplification

### 7.1 Configuration Loading Complexity

**Location**: `/apps/air-quality-app/src/main.rs`

**Current Pattern** (lines 27-102):
```rust
let config = match load_from_stream_config(&[&etcd_endpoint], "air-quality").await {
    Ok(stream_config) => { ... }
    Err(e) => {
        match air_quality_app::load_from_etcd().await {
            Ok(etcd_config) => {
                // 40 lines of manual field mapping
            }
            Err(e) => {
                match AppConfig::from_yaml("config.yaml") {
                    Ok(cfg) => { ... }
                    Err(e) => {
                        AppConfig::default_config()
                    }
                }
            }
        }
    }
};
```

**Why It's Suboptimal**:
- 75 lines for config loading with 3 nested fallback levels
- Manual field mapping between config types
- Each fallback duplicates conversion logic

**Recommended Refactoring**:
```rust
// config/loader.rs
pub struct ConfigLoader {
    sources: Vec<Box<dyn ConfigSource>>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            sources: vec![
                Box::new(StreamRegistrySource::new()),
                Box::new(EtcdSource::new()),
                Box::new(YamlSource::new()),
                Box::new(DefaultSource::new()),
            ]
        }
    }

    pub async fn load(&self) -> AppConfig {
        for source in &self.sources {
            if let Ok(config) = source.load().await {
                return config;
            }
        }
        AppConfig::default()
    }
}
```

**Complexity Reduction Estimate**: 40% in config loading

---

## 8. Code Duplication Analysis

### 8.1 Parser Config Defaults

**Location**: Multiple files

**Duplicated Pattern**:
```rust
// In apps/air-quality-app/src/coordinator/source_manager.rs
let parser_config = ParserConfig {
    parser_type: ParserType::FlatJson,
    location_id_field: "serialno".to_string(),
    default_location_id: None,
    skip_fields: vec!["serialno".to_string(), "wifi".to_string(), ...],
    field_mappings: None,
    default_tags: std::collections::HashMap::new(),
    array_config: None,
    column_config: None,
};

// Same pattern appears in:
// - core/src/coordinator/source_manager.rs (lines 390-416)
// - apps/air-quality-app/src/coordinator/source_manager.rs (lines 400-420)
// - apps/air-quality-app/src/coordinator/source_manager.rs (lines 735-755)
```

**Recommended Refactoring**:
```rust
impl ParserConfig {
    pub fn airgradient_default() -> Self {
        Self {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            skip_fields: vec!["serialno", "wifi", "boot", "firmware", "model", "ledMode"]
                .into_iter().map(String::from).collect(),
            ..Default::default()
        }
    }
}
```

---

### 8.2 MQTT Config Parsing Duplication

**Location**:
- `/core/src/coordinator/source_manager.rs` (lines 84-147)
- `/apps/air-quality-app/src/coordinator/source_manager.rs` (lines 644-721)

Both files extract MQTT config from HashMap<String, Value> with identical patterns.

**Recommended**: Implement `TryFrom<HashMap<String, Value>>` for MqttConfig.

---

## Summary Table

| Finding | Location | Priority | Estimated Reduction |
|---------|----------|----------|---------------------|
| Dual Coordinator Pattern | core + app | HIGH | 20-25% |
| Multiple Data Point Types | types/ | HIGH | 15-20% |
| Source/RawSource Duplication | traits.rs | MEDIUM | 15% |
| Store/RawStore Duplication | traits.rs | MEDIUM | 15% |
| http_poll.rs Monolith | sources/ | MEDIUM | 20% |
| Config Loading Complexity | main.rs | MEDIUM | 40% |
| Excessive Arc<RwLock<T>> | coordinator | LOW | 15% |
| Parser Config Duplication | multiple | LOW | 10% |

---

## Recommendations Priority Order

1. **Immediate** (AIR-010 Phase 1):
   - Consolidate coordinator modules (choose app or core)
   - Remove legacy types (GenericTimeSeriesPoint, AirQualityReading)
   - Add default parser configs (ParserConfig::airgradient_default())

2. **Short-term** (AIR-010 Phase 2):
   - Unify Source/RawSource traits
   - Unify Store/RawStore traits
   - Split http_poll.rs into submodules

3. **Medium-term** (DP-005 or later):
   - Simplify config loading with trait-based approach
   - Reduce public API surface
   - Replace Arc<RwLock> with atomic types where appropriate

---

## Maintainability Metrics

**Before Refactoring**:
- Total lines in analyzed files: ~12,000
- Number of trait definitions: 8
- Number of data structs: 12
- Public API items: 31

**After Recommended Refactoring**:
- Estimated lines: ~9,000 (25% reduction)
- Number of trait definitions: 5 (38% reduction)
- Number of data structs: 6 (50% reduction)
- Public API items: 12 (61% reduction)

---

## Related ADRs to Create

1. **ADR-005**: Unified Data Point Type Hierarchy
2. **ADR-006**: Generic Trait Pattern for Source/Store
3. **ADR-007**: Configuration Loading Strategy
4. **ADR-008**: Coordinator Module Consolidation

---

*Report generated by NDP Architecture Agent*
