# AIR-011: System Design - Parser Decoupling

**Version**: 1.0.0
**Date**: 2026-01-01
**Author**: NDP Architecture Agent
**Status**: Proposed

---

## Table of Contents

1. [Overview](#1-overview)
2. [Current Architecture (Problem State)](#2-current-architecture-problem-state)
3. [Target Architecture (Solution State)](#3-target-architecture-solution-state)
4. [Component Changes](#4-component-changes)
5. [Module Structure](#5-module-structure)
6. [Interface Design for Future ETL](#6-interface-design-for-future-etl)
7. [Data Flow Diagrams](#7-data-flow-diagrams)
8. [Migration Plan](#8-migration-plan)
9. [Testing Strategy](#9-testing-strategy)
10. [Risk Assessment](#10-risk-assessment)

---

## 1. Overview

### 1.1 Purpose

This document details the system design for AIR-011: eliminating duplicative parser processing from the ingestion pipeline while preserving parsers for future Silver layer ETL.

### 1.2 Design Goals

| Goal | Priority | Rationale |
|------|----------|-----------|
| Eliminate double polling | P0 | Critical for Pi stability |
| Remove parser memory pressure | P0 | Prevents lockups |
| Preserve parser code | P1 | Investment protection |
| Minimize code changes | P1 | Reduce risk |
| Enable future ETL | P2 | Silver layer preparation |

### 1.3 Scope

**In Scope**:
- Source constructor modifications (remove parser requirement)
- SourceManager changes (stop creating parsers)
- Feature gating for ETL module
- Documentation updates

**Out of Scope**:
- Silver layer ETL implementation
- New parser development
- Bronze storage format changes
- MQTT source changes (already raw-only)

---

## 2. Current Architecture (Problem State)

### 2.1 Double Polling Problem

```
                    CURRENT STATE (PROBLEMATIC)
    ============================================================

    SourceManager Loop                    HttpPollingSource Internal Loop
    (fetch_raw_batch)                     (polling_loop via start())
           |                                       |
           v                                       v
    +---------------+                      +---------------+
    | HTTP Request  |                      | HTTP Request  |
    | GET /api/data |                      | GET /api/data |
    +-------+-------+                      +-------+-------+
            |                                      |
            v                                      v
    +---------------+                      +---------------+
    | Raw JSON      |                      | Parser.parse()|
    | ~100KB        |                      | JSON -> Points|
    +-------+-------+                      +-------+-------+
            |                                      |
            v                                      v
    +---------------+                      +---------------+
    | RawDataPoint  |                      | Vec<TSPoint>  |
    | -> Bronze     |                      | -> Channel    |
    | Parquet       |                      | (NEVER READ!) |
    +---------------+                      +---------------+
           OK                                   WASTE

    Problem: Two HTTP requests per poll interval
    Problem: Parser output accumulates, never consumed
    Problem: Memory grows until Pi locks up
```

### 2.2 Code Path Analysis

```rust
// Current flow in SourceManager::spawn_http_poll_source()

// Step 1: Create parser (WASTEFUL)
let parser = self.create_parser_from_params(&config.params, "http_poll")?;

// Step 2: Create source with parser
let mut http_source = HttpPollingSource::new(http_config, parser)?;

// Step 3: Start source - spawns internal polling_loop (DOUBLE POLL!)
http_source.start().await?;

// Step 4: Spawn fetch loop using fetch_raw_batch (CORRECT PATH)
tokio::spawn(async move {
    loop {
        match http_source.fetch_raw_batch().await {  // Raw JSON
            Ok(raw_points) => { /* store to Bronze */ }
            Err(e) => { /* handle error */ }
        }
    }
});

// Issue: start() spawns polling_loop() which also polls and parses
// Issue: Parser results sent to internal channel, never consumed
```

### 2.3 Memory Accumulation

```
Time (hours) | Channel Size | Memory Used | Status
-------------|--------------|-------------|--------
0            | 0            | 200MB       | OK
1            | 3,600        | 210MB       | OK
4            | 14,400       | 250MB       | Warning
8            | 28,800       | 340MB       | Critical
12           | 43,200       | 450MB       | Near OOM
16           | 57,600       | LOCKUP      | FAILURE
```

Calculation: ~1000 TimeSeriesPoints per poll x 6 polls/hour x 16 hours = 96,000 points
At ~200 bytes/point = ~19MB in channel alone (plus allocator overhead)

---

## 3. Target Architecture (Solution State)

### 3.1 Simplified Ingestion Path

```
                    TARGET STATE (AIR-011)
    ============================================================

    SourceManager Loop                    HttpPollingSource
    (fetch_raw_batch ONLY)                (No internal polling)
           |
           v
    +---------------+
    | HTTP Request  |    <-- Single request per poll interval
    | GET /api/data |
    +-------+-------+
            |
            v
    +---------------+
    | Raw JSON      |    <-- No parsing
    | ~100KB        |
    +-------+-------+
            |
            v
    +---------------+
    | RawDataPoint  |    <-- Direct to Bronze
    | -> Bronze     |
    | Parquet       |
    +---------------+
           OK

    Result: Single HTTP request per poll
    Result: No parser execution
    Result: Stable memory usage
```

### 3.2 Component Relationships

```
+------------------------------------------------------------------+
|                        air-quality-app                            |
|                                                                  |
|  +---------------------------+    +---------------------------+  |
|  |    IngestionCoordinator   |    |     StorageWriter         |  |
|  |    (unchanged)            |    |     (unchanged)           |  |
|  +-------------+-------------+    +---------------------------+  |
|                |                                                  |
|                v                                                  |
|  +---------------------------+                                   |
|  |      SourceManager        |                                   |
|  |  - NO parser creation     |                                   |
|  |  - spawn_http_poll_source |                                   |
|  |    (simplified)           |                                   |
|  +-------------+-------------+                                   |
|                |                                                  |
+----------------|--------------------------------------------------+
                 |
                 v
+------------------------------------------------------------------+
|                         neural-core                               |
|                                                                  |
|  +---------------------------+    +---------------------------+  |
|  |    HttpPollingSource      |    |    parsers/ (DORMANT)     |  |
|  |  - new(config) NO PARSER  |    |  - flat_json.rs           |  |
|  |  - NO start()             |    |  - json_path.rs           |  |
|  |  - fetch_raw_batch() ONLY |    |  - array_iterator.rs      |  |
|  +---------------------------+    |  - column_oriented.rs     |  |
|                                   |  - factory.rs             |  |
|  +---------------------------+    |  - config.rs              |  |
|  |    RawSource trait        |    |  - traits.rs              |  |
|  |  - fetch_raw_batch()      |    |                           |  |
|  |  - health_check()         |    |  [Feature-gated for ETL]  |  |
|  +---------------------------+    +---------------------------+  |
|                                                                  |
+------------------------------------------------------------------+
```

---

## 4. Component Changes

### 4.1 HttpPollingSource Changes

**File**: `core/src/sources/http_poll.rs`

```rust
// BEFORE: Constructor requires parser
impl HttpPollingSource {
    pub fn new(
        config: HttpPollingConfig,
        parser: Box<dyn Parser + Send + Sync>,
    ) -> CoreResult<Self> {
        // ...
    }
}

// AFTER: Constructor is raw-only
impl HttpPollingSource {
    /// Create a new raw HTTP polling source
    ///
    /// This source fetches raw JSON responses and returns them as RawDataPoint.
    /// Parsing is NOT performed during ingestion - parsers are reserved for
    /// Silver layer ETL (see `core/src/parsers/` with `etl` feature).
    pub fn new(config: HttpPollingConfig) -> CoreResult<Self> {
        // No parser field
        // No internal channel for TimeSeriesPoints
    }
}
```

**Removed Fields**:
```rust
// REMOVE from HttpPollingSource struct:
parser: Box<dyn Parser + Send + Sync>,
points_tx: mpsc::Sender<TimeSeriesPoint>,
points_rx: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
```

**Removed Methods**:
```rust
// REMOVE or deprecate:
pub async fn start(&mut self) -> CoreResult<()>  // Spawns polling_loop
fn polling_loop(/* ... */)                        // Internal polling
async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>>  // Returns parsed
```

**Retained Methods**:
```rust
// KEEP (these form the RawSource interface):
pub async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>>
pub async fn health_check(&self) -> CoreResult<HealthStatus>
```

### 4.2 SourceManager Changes

**File**: `core/src/coordinator/source_manager.rs`

```rust
// REMOVE: Parser creation method
fn create_parser_from_params(/* ... */) -> CoreResult<Box<dyn Parser>>

// MODIFY: spawn_http_poll_source
async fn spawn_http_poll_source(&self, source_id: String, config: SourceConfig) -> CoreResult<()> {
    // REMOVE: parser creation
    // let parser = self.create_parser_from_params(&config.params, "http_poll")?;

    // MODIFY: source creation without parser
    let http_source = HttpPollingSource::new(http_config)?;

    // REMOVE: start() call - no internal polling
    // http_source.start().await?;

    // KEEP: fetch loop using fetch_raw_batch
    tokio::spawn(async move {
        loop {
            match http_source.fetch_raw_batch().await {
                Ok(raw_points) => { /* ... */ }
                Err(e) => { /* ... */ }
            }
        }
    });
}
```

### 4.3 Parser Module Feature Gate

**File**: `core/Cargo.toml`

```toml
[features]
default = []
etl = []  # Enable parsers for Silver layer ETL
```

**File**: `core/src/lib.rs`

```rust
// Parsers are feature-gated for ETL use only
// Not compiled by default (ingestion doesn't need them)
#[cfg(feature = "etl")]
pub mod parsers;

// Re-exports only when ETL feature enabled
#[cfg(feature = "etl")]
pub use parsers::{
    Parser, ParserConfig, ParserType,
    FlatJsonParser, JsonPathParser, ArrayIteratorParser, ColumnOrientedParser,
    create_parser_from_config,
};
```

### 4.4 Import Cleanup

**Files to Update**:

| File | Change |
|------|--------|
| `core/src/sources/http_poll.rs` | Remove `use crate::parsers::*` |
| `core/src/sources/mqtt/mod.rs` | Remove `use crate::parsers::*` |
| `core/src/coordinator/source_manager.rs` | Remove parser imports |
| `apps/air-quality-app/src/main.rs` | Remove parser configuration |

---

## 5. Module Structure

### 5.1 Current Module Tree

```
core/src/
+-- lib.rs
+-- coordinator/
|   +-- mod.rs
|   +-- source_manager.rs      # Creates parsers (CHANGE)
+-- parsers/                   # Active (DEACTIVATE)
|   +-- mod.rs
|   +-- traits.rs
|   +-- config.rs
|   +-- factory.rs
|   +-- flat_json.rs
|   +-- json_path.rs
|   +-- array_iterator.rs
|   +-- column_oriented.rs
+-- sources/
|   +-- mod.rs
|   +-- http_poll.rs           # Uses parsers (CHANGE)
|   +-- mqtt/
|       +-- mod.rs             # Uses parsers (CHANGE)
+-- storage/
+-- traits.rs
+-- types/
+-- error.rs
```

### 5.2 Target Module Tree

```
core/src/
+-- lib.rs                     # Feature-gate parsers module
+-- coordinator/
|   +-- mod.rs
|   +-- source_manager.rs      # NO parser creation
+-- parsers/                   # DORMANT (feature = "etl")
|   +-- mod.rs                 # Documented as ETL-only
|   +-- traits.rs
|   +-- config.rs
|   +-- factory.rs
|   +-- flat_json.rs
|   +-- json_path.rs
|   +-- array_iterator.rs
|   +-- column_oriented.rs
+-- sources/
|   +-- mod.rs
|   +-- http_poll.rs           # Raw-only, no parser
|   +-- mqtt/
|       +-- mod.rs             # Raw-only, no parser
+-- storage/
+-- traits.rs                  # RawSource trait emphasized
+-- types/
+-- error.rs
```

### 5.3 Parser Module Documentation

**File**: `core/src/parsers/mod.rs`

```rust
//! # Parser Module - Reserved for Silver Layer ETL
//!
//! **STATUS**: DORMANT - Not used during Bronze layer ingestion
//!
//! This module contains parsers for converting raw JSON API responses into
//! structured `TimeSeriesPoint` data. These parsers are **not executed**
//! during normal data ingestion (AIR-011).
//!
//! ## When Parsers Are Used
//!
//! Parsers will be activated for Silver layer ETL (future DP-00x features):
//! - Bronze (Parquet) -> Parser -> Silver (TimescaleDB)
//! - Batch processing of historical data
//! - Data quality transformations
//!
//! ## Enabling Parsers
//!
//! Parsers are behind the `etl` feature flag:
//!
//! ```toml
//! # Cargo.toml
//! neural-core = { version = "0.5", features = ["etl"] }
//! ```
//!
//! ## Available Parsers
//!
//! | Parser | Use Case | Configuration |
//! |--------|----------|---------------|
//! | `FlatJsonParser` | Simple key-value JSON | `ParserType::FlatJson` |
//! | `JsonPathParser` | JSONPath extraction | `ParserType::JsonPath` |
//! | `ArrayIteratorParser` | Array-based data | `ParserType::ArrayIterator` |
//! | `ColumnOrientedParser` | Columnar formats | `ParserType::ColumnOriented` |
//!
//! ## See Also
//!
//! - [ADR-001: Parser Archive Strategy](../../../product/features/air-011/architecture/ADR-001-parser-archive.md)
//! - [Silver Layer Design](future: DP-00x)
```

---

## 6. Interface Design for Future ETL

### 6.1 ETL Parser Trait

The existing `Parser` trait remains unchanged for future ETL use:

```rust
// core/src/parsers/traits.rs (unchanged, feature-gated)

/// Main parser trait - all parsers must implement this
///
/// Used by Silver layer ETL to convert raw JSON to TimeSeriesPoints.
/// NOT used during Bronze layer ingestion.
pub trait Parser: Send + Sync {
    /// Parse raw JSON payload into time series points
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Return parser name for logging/debugging
    fn name(&self) -> &str;

    /// Return parser configuration for introspection
    fn config(&self) -> &ParserConfig;

    /// Parse with context for ndp_id and context injection
    fn parse_with_context(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
        context: &ParseContext,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

### 6.2 Future ETL Service Interface

When Silver layer is implemented (DP-00x), it will use parsers like this:

```rust
// Future: domains/etl/src/bronze_to_silver.rs

use neural_core::parsers::{Parser, create_parser_from_config, ParserConfig};
use neural_core::storage::ParquetStore;

/// ETL service for Bronze -> Silver transformation
pub struct BronzeToSilverETL {
    bronze_store: Arc<ParquetStore>,
    silver_store: Arc<TimescaleStore>,  // Future
    parser: Box<dyn Parser>,
}

impl BronzeToSilverETL {
    pub fn new(
        bronze_store: Arc<ParquetStore>,
        silver_store: Arc<TimescaleStore>,
        parser_config: ParserConfig,
    ) -> CoreResult<Self> {
        // Parser instantiation happens HERE (ETL time), not during ingestion
        let parser = create_parser_from_config(parser_config)?;

        Ok(Self {
            bronze_store,
            silver_store,
            parser,
        })
    }

    /// Process a batch of Bronze records to Silver
    pub async fn process_batch(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> CoreResult<usize> {
        // 1. Read raw JSON from Bronze Parquet
        let raw_records = self.bronze_store.query_raw(start, end).await?;

        // 2. Parse each record using the configured parser
        let mut points = Vec::new();
        for record in raw_records {
            let parsed = self.parser.parse(&record.payload, record.timestamp)?;
            points.extend(parsed);
        }

        // 3. Write to Silver (TimescaleDB)
        self.silver_store.write_batch(&points).await?;

        Ok(points.len())
    }
}
```

### 6.3 Configuration for ETL

Stream configurations will include parser settings for ETL:

```yaml
# config/base/streams/outdoor-weather/config.yaml
stream_id: outdoor-weather
description: OpenWeatherMap weather data
version: "1.0.0"
enabled: true

# Bronze layer settings (ingestion)
sources:
  - source_type: http_poll
    enabled: true
    params:
      base_url_template: "https://api.openweathermap.org/data/2.5/weather"
      poll_interval_secs: 600
      # NO parser config here - Bronze stores raw

# Silver layer settings (future ETL)
etl:
  enabled: false  # Enable when Silver layer ready
  parser:
    parser_type: flat_json
    location_id_field: "id"
    skip_fields: ["sys", "cod", "base"]
    field_mappings:
      - source_field: "main.temp"
        target_field: "temperature"
        unit: "kelvin"
      - source_field: "main.humidity"
        target_field: "humidity"
        unit: "percent"
```

---

## 7. Data Flow Diagrams

### 7.1 Ingestion Flow (Target State)

```
    +-------------+
    | HTTP API    |
    | (External)  |
    +------+------+
           |
           | HTTP Response (raw JSON)
           v
    +------+------+
    | HttpPolling |
    | Source      |
    +------+------+
           |
           | fetch_raw_batch()
           v
    +------+------+
    | RawDataPoint|
    | {           |
    |   stream_id |
    |   timestamp |
    |   payload   | <-- raw JSON bytes
    |   metadata  |
    | }           |
    +------+------+
           |
           | mpsc channel
           v
    +------+------+
    | Ingestion   |
    | Router      |
    +------+------+
           |
           | route by stream_id
           v
    +------+------+
    | Storage     |
    | Writer      |
    +------+------+
           |
           | write_batch()
           v
    +------+------+
    | Parquet     |
    | Store       |
    | (Bronze)    |
    +-------------+
```

### 7.2 ETL Flow (Future State)

```
    +-------------+
    | Bronze      |
    | Parquet     |
    | (Raw JSON)  |
    +------+------+
           |
           | query_raw(start, end)
           v
    +------+------+
    | ETL Service |
    | (DP-00x)    |
    +------+------+
           |
           | for each raw record:
           v
    +------+------+
    | Parser      | <-- Parsers activated HERE
    | (feature    |
    |  = "etl")   |
    +------+------+
           |
           | Vec<TimeSeriesPoint>
           v
    +------+------+
    | Silver      |
    | Store       |
    | (TimescaleDB|
    +-------------+
```

### 7.3 Component Interaction Sequence

```
    SourceManager          HttpPollingSource         ParquetStore
         |                        |                       |
         | new(config)            |                       |
         |----------------------->|                       |
         |                        |                       |
         | [NO start() call]      |                       |
         |                        |                       |
    loop |                        |                       |
    -----+                        |                       |
         | fetch_raw_batch()      |                       |
         |----------------------->|                       |
         |                        | HTTP GET              |
         |                        |-----> External API    |
         |                        |<----- JSON Response   |
         |                        |                       |
         | Vec<RawDataPoint>      |                       |
         |<-----------------------|                       |
         |                        |                       |
         | write_raw_batch()      |                       |
         |----------------------------------------------->|
         |                        |                       |
         | [repeat every poll_interval]                   |
    -----+                                                |
```

---

## 8. Migration Plan

### 8.1 Phase 1: Feature Gate (Non-Breaking)

**Duration**: 1 day
**Risk**: Low

1. Add `etl` feature to `core/Cargo.toml`
2. Wrap `pub mod parsers` with `#[cfg(feature = "etl")]`
3. Update `lib.rs` re-exports with feature gate
4. Verify `cargo build` works without feature
5. Verify `cargo build --features etl` works with feature

### 8.2 Phase 2: Source Decoupling (Breaking Internal)

**Duration**: 2 days
**Risk**: Medium

1. Modify `HttpPollingSource::new()` to remove parser parameter
2. Remove `start()` method (or deprecate)
3. Remove internal `polling_loop()` implementation
4. Update `SourceManager::spawn_http_poll_source()`
5. Remove `create_parser_from_params()` from SourceManager
6. Update unit tests

### 8.3 Phase 3: Import Cleanup

**Duration**: 1 day
**Risk**: Low

1. Remove parser imports from source files
2. Update any documentation referencing parsers
3. Add module-level documentation to `parsers/mod.rs`
4. Verify no parser code paths in ingestion

### 8.4 Phase 4: Verification

**Duration**: 2 days
**Risk**: None

1. Run full test suite: `cargo test`
2. Run clippy: `cargo clippy`
3. Build release: `cargo build --release`
4. Deploy to test Pi
5. Monitor for 24+ hours
6. Verify memory stability
7. Verify no parser tracing logs during ingestion

---

## 9. Testing Strategy

### 9.1 Unit Tests

```rust
// core/src/sources/http_poll.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_source_creation_without_parser() {
        let config = HttpPollingConfig {
            base_url_template: "http://example.com".to_string(),
            poll_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(10),
            sensors: vec![],
            buffer_capacity: 100,
        };

        // Should not require parser
        let source = HttpPollingSource::new(config);
        assert!(source.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_returns_raw_json() {
        // Mock HTTP response
        let source = create_test_source_with_mock_http();

        let raw_points = source.fetch_raw_batch().await.unwrap();

        // Verify raw JSON is preserved
        for point in raw_points {
            assert!(point.payload.is_object() || point.payload.is_array());
            // Payload should be raw, not parsed into TimeSeriesPoint fields
        }
    }
}
```

### 9.2 Integration Tests

```rust
// tests/integration/air_011_parser_removal.rs

#[tokio::test]
async fn test_ingestion_without_parser_execution() {
    // Setup: Create test environment with tracing
    let subscriber = tracing_subscriber::fmt()
        .with_test_writer()
        .finish();

    // Start ingestion with mock HTTP endpoint
    let config = load_test_config();
    let coordinator = IngestionCoordinator::new(config).await.unwrap();

    // Run for short duration
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify: No parser-related logs
    // Verify: Raw JSON stored in Bronze
    // Verify: No TimeSeriesPoint channel growth

    coordinator.shutdown().await.unwrap();
}
```

### 9.3 Memory Stability Test

```rust
// tests/stability/memory_test.rs

#[tokio::test]
#[ignore] // Long-running test
async fn test_24_hour_memory_stability() {
    let initial_memory = get_process_memory();

    // Start ingestion
    let coordinator = start_test_coordinator().await;

    // Run for 24 hours (or shorter in CI)
    for hour in 0..24 {
        tokio::time::sleep(Duration::from_secs(3600)).await;

        let current_memory = get_process_memory();
        let growth = current_memory - initial_memory;

        // Memory growth should be bounded (< 50MB over 24 hours)
        assert!(growth < 50_000_000, "Memory grew by {} bytes at hour {}", growth, hour);
    }

    coordinator.shutdown().await.unwrap();
}
```

---

## 10. Risk Assessment

### 10.1 Risk Matrix

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Parser tests fail | Medium | Low | Tests still run with `--features etl` |
| External code depends on parsers | Low | Medium | Public API unchanged, feature-gated |
| Future ETL blocked | Low | Low | Parsers preserved, just dormant |
| Regression in raw storage | Low | High | Existing `fetch_raw_batch` tests |
| Feature flag complexity | Low | Low | Simple boolean flag |

### 10.2 Rollback Plan

If issues arise after deployment:

1. **Immediate**: Revert to previous Docker image
2. **Short-term**: Re-enable parser module (remove feature gate)
3. **Investigation**: Enable parser tracing to identify issues
4. **Fix Forward**: Address specific issues rather than full revert

### 10.3 Success Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Pi uptime | > 24 hours | Monitoring dashboard |
| Memory usage | Stable (< 10% growth/day) | `docker stats` |
| Parser CPU | 0% during ingestion | `perf` profiling |
| Test coverage | > 80% on modified code | `cargo tarpaulin` |
| Build time | No increase | CI metrics |

---

## Appendix A: File Change Summary

| File | Change Type | Description |
|------|-------------|-------------|
| `core/Cargo.toml` | Modify | Add `etl` feature |
| `core/src/lib.rs` | Modify | Feature-gate parsers |
| `core/src/parsers/mod.rs` | Modify | Add documentation |
| `core/src/sources/http_poll.rs` | Major | Remove parser, simplify |
| `core/src/coordinator/source_manager.rs` | Major | Remove parser creation |
| `core/src/sources/mqtt/mod.rs` | Minor | Remove parser imports |
| `apps/air-quality-app/src/main.rs` | Minor | Remove parser config |

---

## Appendix B: Related Documents

- [ADR-001: Parser Archive Strategy](./ADR-001-parser-archive.md)
- [AIR-011 SCOPE.md](../SCOPE.md)
- [AIR-011 STATUS.md](../STATUS.md)
- [Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [AIR-005 Ingestion Coordinator Design](/workspaces/neural-data-platform/docs/architecture/AIR-005_INGESTION_COORDINATOR_DESIGN.md)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-01 | NDP Architecture Agent | Initial design |
