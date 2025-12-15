# AIR-004: Generic Multi-Stream Data Platform - SPARC Refinement

## Document Status

**Status**: Refinement Phase Complete (Revised with Existing Codebase Integration)
**Version**: 2.0.0
**Last Updated**: 2025-12-15
**Related Documents**:
- [Specification](../specification/SPECIFICATION.md)
- [Pseudocode](../pseudocode/PSEUDOCODE.md)
- [Platform Architecture](../architecture/PLATFORM_ARCHITECTURE.md)
- [Dependency Map](../DEPENDENCY_MAP.md)
- [Completion](../completion/COMPLETION.md)

---

## 1. Overview

This document defines the iterative refinement strategy for implementing the Generic Multi-Stream Data Platform. It establishes Test-Driven Development (TDD) practices, quality gates, and incremental implementation milestones to ensure a robust, maintainable codebase **while building upon existing patterns from AIR-001, AIR-002, and AIR-003**.

### 1.1 Integration with Existing Codebase

AIR-004 extends the proven patterns from previous features:
- **AIR-001**: Configuration loading, etcd integration, MQTT handling
- **AIR-002**: MQTT→Parquet pipeline, batch processing
- **AIR-003**: REST API, health monitoring, metrics instrumentation

**Key Principle**: We are **extending and generalizing** existing functionality, not rewriting from scratch.

---

## 2. Existing Test Coverage

### 2.1 Current Test Patterns in Codebase

The neural-data-platform already has comprehensive test coverage with established patterns:

#### **Unit Tests Pattern** (`#[cfg(test)] mod tests`)

```rust
// File: domains/air-quality/src/parser.rs (495 lines, ~200 lines of tests)
#[cfg(test)]
mod tests {
    use super::*;

    /// Test data - Complete MQTT payload (from actual sensor)
    const MQTT_COMPLETE_PAYLOAD: &str = r#"{
        "wifi": -42,
        "serialno": "airgradient:123456",
        "rco2": 650,
        "pm01": 5,
        "pm02": 12
    }"#;

    #[test]
    fn test_parse_mqtt_complete_payload_success() {
        let result = parse_mqtt_payload(MQTT_COMPLETE_PAYLOAD);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.device.serialno, "airgradient:123456");
        assert_eq!(reading.device.wifi, Some(-42));
    }
}
```

**Characteristics**:
- Inline `#[cfg(test)]` modules in source files
- Const test data for reusability
- Descriptive test names (`test_parse_mqtt_complete_payload_success`)
- Clear arrange-act-assert structure

#### **Integration Tests Pattern** (`tests/*.rs`)

```rust
// File: apps/air-quality-app/tests/integration_test.rs (786 lines)
use neural_core::{ParquetStore, TimeSeriesPoint};
use tempfile::TempDir;

#[tokio::test]
async fn test_parquet_write_and_query() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Write test points
    let points = vec![/* ... */];
    store.write_batch(points).await.unwrap();

    // Query and verify
    let results = store.query(/* ... */).await.unwrap();
    assert_eq!(results.len(), 2);
}
```

**Characteristics**:
- Tests in `tests/` directory (external to crate)
- `#[tokio::test]` for async tests
- `tempfile::TempDir` for isolated storage testing
- Comprehensive edge case coverage (786 lines covering: WAL replay, aggregations, concurrent access, stress tests)

### 2.2 Existing Test Files by Category

| Category | Files | Coverage |
|----------|-------|----------|
| **Domain Unit Tests** | `domains/air-quality/src/*.rs` | Parser (200 lines), Types (150 lines), Adapter (180 lines), Validation (200 lines) |
| **Integration Tests** | `apps/air-quality-app/tests/*.rs` | Storage (786 lines), Config loading (250 lines), MQTT pipeline (150 lines) |
| **E2E Tests** | `tests/integration/*.rs` | Cross-component workflows (500+ lines) |

### 2.3 Test Infrastructure Currently Used

**Dependencies** (`apps/air-quality-app/Cargo.toml`):
```toml
[dev-dependencies]
mockall = "0.13"           # Mock objects (London School TDD)
axum-test = "14.0"         # HTTP testing
tokio-test = "0.4"         # Async test utilities
tempfile = "3.8"           # Temporary directories for integration tests
```

**Test Commands**:
```bash
# All workspace tests
cargo test --workspace --all-features

# Specific crate tests
cargo test -p air-quality-app

# Integration tests only
cargo test --test '*' -- --test-threads=1

# With coverage
cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

---

## 3. TDD Implementation Strategy (Extending Existing Tests)

### 3.1 Testing Philosophy

**Red-Green-Refactor Cycle** (with existing test migration):
1. **Red**: Write failing test OR ensure existing test fails with new code
2. **Green**: Implement to pass (preserve existing test behavior)
3. **Refactor**: Improve while keeping ALL tests green (old + new)

**Test Pyramid** (Current State):
```
            /\
           /E2E\          <- 5% (500+ lines in tests/integration)
          /------\
         /  Integ \       <- 20% (1200+ lines in apps/tests)
        /----------\
       /   Unit     \     <- 75% (800+ lines in domain/src)
      /--------------\
```

### 3.2 Regression Testing Strategy

**CRITICAL**: Before any refactoring, establish regression baseline:

```bash
# 1. Run all existing tests and capture results
cargo test --workspace --all-features 2>&1 | tee baseline-tests.log

# 2. Generate coverage baseline
cargo llvm-cov --workspace --all-features --lcov --output-path baseline-coverage.info

# 3. Document current test metrics
cargo test --workspace --all-features -- --list | wc -l  # Count tests
```

**Regression Test Checklist** (Run before each commit):
- [ ] All existing unit tests pass (air-quality domain)
- [ ] All existing integration tests pass (air-quality-app)
- [ ] Config hierarchy tests pass (etcd loading)
- [ ] MQTT pipeline tests pass (integration_test.rs)
- [ ] WAL replay tests pass (persistence)
- [ ] Aggregation query tests pass (storage)
- [ ] Concurrent access tests pass (thread safety)

### 3.3 Migration Testing Pattern

When refactoring existing components (e.g., MqttSource), use **parallel implementation testing**:

```rust
// File: core/src/sources/mqtt_source_tests.rs

#[cfg(test)]
mod migration_tests {
    use super::*;

    /// Test that new MqttSource maintains compatibility with AIR-002 behavior
    #[tokio::test]
    async fn test_backward_compatibility_with_air002() {
        // Setup: Create source using existing AIR-002 config format
        let old_config = AirQualityConfig::from_file("test-configs/air-002.yaml").unwrap();

        // Act: Initialize new generic MqttSource
        let source = MqttSource::from_stream_config(&old_config.to_stream_config()).await.unwrap();

        // Assert: Should connect and subscribe to same topics
        assert_eq!(source.stream_id(), "air-quality");
        assert_eq!(source.broker_url(), "mqtt://localhost:1883");

        // Verify existing tests still pass
        let mut rx = source.subscribe().await.unwrap();
        // ... existing test logic from AIR-002
    }

    /// Test that existing AIR-002 MQTT handler code paths still work
    #[tokio::test]
    async fn test_existing_mqtt_handler_integration() {
        // Use actual AIR-002 mqtt_handler code
        let handler = existing_air002_mqtt_handler();

        // Inject new generic source
        let source = MqttSource::new(test_config());
        handler.attach_source(source).await.unwrap();

        // Verify existing AIR-002 behavior preserved
        handler.publish_test_message().await;
        let received = handler.wait_for_storage().await;
        assert!(received.is_ok(), "AIR-002 pipeline should work unchanged");
    }
}
```

**Migration Testing Workflow**:
1. **Before Refactoring**: Run existing tests to establish baseline
2. **During Refactoring**: Keep old implementation alongside new (feature flags)
3. **Compatibility Tests**: Verify new code handles old data/configs
4. **Gradual Cutover**: Switch one integration test at a time
5. **After Refactoring**: Remove old code only when ALL tests pass

---

## 4. TDD Implementation Order (Building on Existing Code)

### 4.1 Phase 0: Baseline Verification (Day 1)

**Goal**: Ensure existing tests pass and establish metrics

```bash
# Run all tests
cargo test --workspace --all-features

# Generate baseline coverage report
cargo llvm-cov --workspace --all-features --html --output-dir baseline-coverage/

# Run existing benchmarks (if any)
cargo bench --bench config_retrieval
```

**Deliverables**:
- [ ] Baseline test results documented
- [ ] Current coverage metrics recorded
- [ ] Existing performance benchmarks captured
- [ ] Known failing tests documented (technical debt)

### 4.2 Phase 1: Core Types with Existing Pattern (2 days)

**Goal**: Implement foundation types following existing patterns from `air-quality` domain

**TDD Approach**:
```rust
// File: core/src/types/stream_record.rs (follow air-quality/src/types.rs pattern)

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamRecord {
    pub stream_id: String,
    pub timestamp: DateTime<Utc>,
    pub data: Value,
    pub metadata: Option<RecordMetadata>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_roundtrip_preserves_data() {
        // Same pattern as air-quality types tests
        let record = StreamRecord {
            stream_id: "test-stream".to_string(),
            timestamp: Utc::now(),
            data: serde_json::json!({"value": 42.5}),
            metadata: None,
        };

        let json = serde_json::to_string(&record).unwrap();
        let deserialized: StreamRecord = serde_json::from_str(&json).unwrap();

        assert_eq!(record, deserialized);
    }

    #[test]
    fn test_validates_stream_id_format() {
        // Reuse validation patterns from air-quality domain
        assert!(StreamRecord::validate_stream_id("air-quality").is_ok());
        assert!(StreamRecord::validate_stream_id("Air-Quality").is_err());
    }
}
```

**Quality Gate**:
- [ ] All unit tests pass (existing + new)
- [ ] No clippy warnings: `cargo clippy --all-targets -- -D warnings`
- [ ] Code formatted: `cargo fmt -- --check`
- [ ] Documentation for public types

### 4.3 Phase 2: Stream Registry with etcd Integration (3 days)

**Goal**: Extend existing etcd config loading from AIR-001

**Reference Existing Code**:
- `config-client/src/etcd.rs` - Existing etcd client
- `apps/air-quality-app/tests/etcd_config_test.rs` - Existing etcd tests

**Integration Test Strategy** (extend existing pattern):
```rust
// File: tests/integration/registry_tests.rs

use config_client::EtcdClient;
use testcontainers::clients::Cli;
use testcontainers_modules::etcd::Etcd;

#[tokio::test]
async fn test_registry_detects_config_changes() {
    // Use same testcontainer pattern as existing etcd tests
    let docker = Cli::default();
    let etcd_container = docker.run(Etcd::default());
    let etcd_url = format!("http://127.0.0.1:{}", etcd_container.get_host_port_ipv4(2379));

    let registry = StreamRegistry::connect(&etcd_url).await.unwrap();

    // Create stream (similar to AIR-001 config creation)
    let config = StreamConfig {
        stream_id: "test-stream".to_string(),
        source_type: SourceType::Mqtt,
        // ... config fields
    };
    registry.create_stream(&config).await.unwrap();

    // Watch for changes (extend AIR-001 watch mechanism)
    let mut events = registry.watch_streams().await.unwrap();

    // Update config
    registry.update_stream("test-stream", updated_config).await.unwrap();

    // Assert event received (same timeout pattern as existing tests)
    let event = tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .expect("No event received");

    assert_eq!(event.stream_id, "test-stream");
}
```

### 4.4 Phase 3: Source Implementations (4 days)

**Goal**: Refactor existing MqttSource, add HttpPoller and WebhookHandler

**Refactoring MqttSource** (from `apps/air-quality-app/src/ingestion/mqtt_handler.rs`):

```rust
// File: core/src/sources/mqtt_source.rs

use async_trait::async_trait;
use rumqttc::{AsyncClient, EventLoop, QoS};

#[async_trait]
pub trait Source: Send + Sync + 'static {
    async fn subscribe(&self) -> Result<Receiver<StreamRecord>>;
    fn stream_id(&self) -> &str;
    fn source_type(&self) -> SourceType;
    async fn health_check(&self) -> Result<HealthStatus>;
}

pub struct MqttSource {
    stream_id: String,
    client: AsyncClient,
    eventloop: EventLoop,
    topics: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Contract test - ensures MqttSource meets Source trait requirements
    #[tokio::test]
    async fn test_mqtt_source_meets_contract() {
        let source = MqttSource::new(test_mqtt_config());

        // Source contract validation
        assert!(!source.stream_id().is_empty());
        assert_eq!(source.source_type(), SourceType::Mqtt);

        let health = source.health_check().await.unwrap();
        assert!(matches!(health, HealthStatus::Healthy | HealthStatus::Degraded));
    }

    /// Backward compatibility test - works with AIR-002 pipeline
    #[tokio::test]
    async fn test_air002_pipeline_compatibility() {
        // Use existing AIR-002 MQTT test infrastructure
        let source = MqttSource::new(air002_mqtt_config());
        let mut rx = source.subscribe().await.unwrap();

        // Publish using existing test helper
        publish_air002_test_message().await;

        // Should receive in AIR-002 format
        let record = tokio::time::timeout(Duration::from_secs(5), rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(record.stream_id, "air-quality");
    }
}
```

**Contract Testing Pattern** (reusable for all sources):
```rust
// File: core/src/sources/source_contract_tests.rs

/// Generic contract test for any Source implementation
pub async fn validate_source_contract<S: Source>(source: S) {
    // 1. stream_id is non-empty
    assert!(!source.stream_id().is_empty());

    // 2. source_type is valid
    let source_type = source.source_type();
    assert!(matches!(
        source_type,
        SourceType::Mqtt | SourceType::HttpPoll | SourceType::Webhook
    ));

    // 3. health_check returns valid status
    let health = source.health_check().await.unwrap();
    assert!(matches!(
        health,
        HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy
    ));
}

#[tokio::test]
async fn test_mqtt_source_contract() {
    validate_source_contract(MqttSource::new(test_config())).await;
}

#[tokio::test]
async fn test_http_poller_contract() {
    validate_source_contract(HttpPollingSource::new(test_config())).await;
}
```

### 4.5 Phase 4: Storage Layer Extension (4 days)

**Goal**: Extend existing ParquetStore for multi-stream support

**Reference Existing Tests**: `apps/air-quality-app/tests/integration_test.rs` (786 lines)

**Test Strategy**: Preserve all existing storage tests while adding multi-stream capability

```rust
// File: tests/integration/multi_stream_storage_tests.rs

#[tokio::test]
async fn test_multi_stream_storage_isolation() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write to multiple streams
    let streams = vec!["air-quality", "weather", "traffic"];
    for stream_id in &streams {
        let points = vec![
            TimeSeriesPoint {
                timestamp: now,
                location_id: format!("{}-sensor", stream_id),
                value: 42.0,
                tags: HashMap::from([("stream".to_string(), stream_id.to_string())]),
            }
        ];
        store.write_batch(points).await.unwrap();
    }

    // Verify each stream isolated (reuse existing query pattern)
    for stream_id in &streams {
        let results = store.query(
            &format!("{}-sensor", stream_id),
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        ).await.unwrap();

        assert_eq!(results.len(), 1, "Should have exactly one point for {}", stream_id);
    }
}

/// REGRESSION TEST: Ensure existing AIR-002 tests still pass
#[tokio::test]
async fn test_air002_storage_still_works() {
    // Run exact test from integration_test.rs
    test_parquet_write_and_query().await;
    test_data_persistence_after_restart().await;
    test_wal_replay_correctness().await;
    test_aggregation_mean().await;
}
```

### 4.6 Phase 5: Ingestion Coordinator (3 days)

**Goal**: Build coordinator that orchestrates all components

**End-to-End Test** (extends existing pipeline tests):
```rust
// File: tests/e2e/full_pipeline_test.rs

#[tokio::test]
async fn test_multi_stream_pipeline() {
    // Setup infrastructure (reuse existing patterns)
    let temp_dir = TempDir::new().unwrap();
    let etcd = start_test_etcd().await;
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Create multiple stream configs
    for stream_id in &["air-quality", "weather"] {
        etcd.create_stream(stream_id, test_config(stream_id)).await;
    }

    // Start coordinator
    let coordinator = IngestionCoordinator::start(test_config()).await.unwrap();

    // Verify both streams ingesting
    // ... test logic

    // CRITICAL: Ensure AIR-002 pipeline still works
    assert!(verify_air002_pipeline_intact().await);
}
```

---

## 5. Code Quality Guidelines (Enforced by CI)

### 5.1 Rust-Specific Standards (Already Established)

**Error Handling** (following existing patterns):
```rust
// Use thiserror (already in workspace dependencies)
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("Failed to write to Parquet: {0}")]
    ParquetWrite(#[from] parquet::errors::ParquetError),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),
}

// Use anyhow for application errors (existing pattern)
fn main() -> anyhow::Result<()> {
    // ...
}
```

**Async Best Practices** (following tokio workspace standard):
```rust
// Already using tokio = { version = "1.40", features = ["full"] }
use async_trait::async_trait;  // Already in dependencies

#[async_trait]
pub trait Source: Send + Sync + 'static {
    async fn subscribe(&self) -> Result<Receiver<StreamRecord>>;
}
```

### 5.2 Logging Standards (Existing tracing infrastructure)

```rust
// Already configured: tracing = "0.1", tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
use tracing::{info, warn, error, debug, instrument};

#[instrument(skip(self), fields(stream_id = %self.stream_id))]
async fn process_record(&self, record: StreamRecord) -> Result<()> {
    debug!(record_ts = %record.timestamp, "Processing record");

    match self.write_bronze(&record).await {
        Ok(_) => info!("Bronze write complete"),
        Err(e) => {
            error!(error = %e, "Bronze write failed");
            return Err(e);
        }
    }

    Ok(())
}
```

### 5.3 Existing CI Quality Gates

From `.github/workflows/air-001-ci.yml`:

```yaml
# 1. Compilation check
- name: Check compilation
  run: cargo check --workspace --all-features

# 2. Formatting (enforced)
- name: Check formatting
  run: cargo fmt --all -- --check

# 3. Clippy (all warnings as errors)
- name: Run clippy
  run: cargo clippy --workspace --all-features --all-targets -- -D warnings

# 4. Test suite
- name: Run tests
  run: cargo test --workspace --all-features

# 5. Code coverage
- name: Generate code coverage
  run: cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info
```

**Quality Gate Requirements** (already enforced):
- ✅ All tests pass
- ✅ cargo fmt passes
- ✅ cargo clippy with `-D warnings` (no warnings allowed)
- ✅ Coverage uploaded to Codecov

---

## 6. Performance Optimization Plan (Based on Existing Benchmarks)

### 6.1 Existing Performance Targets from AIR-001/002/003

| Metric | Source | Target | Current (AIR-002) |
|--------|--------|--------|-------------------|
| **Config retrieval** | AIR-001 etcd | <10ms p95 | ~8ms (achieved) |
| **MQTT message ingestion** | AIR-002 | 1 msg/sec sustained | 1.2 msg/sec (achieved) |
| **Parquet batch write** | AIR-002 | <5s for 100 records | ~2s (achieved) |
| **Memory usage (Raspberry Pi 5)** | AIR-001 | <1.5GB total | ~800MB (achieved) |
| **Query latency** | AIR-002 | <100ms p95 | ~50ms (achieved) |

### 6.2 Benchmarking Infrastructure (Already Exists)

```rust
// File: benches/ingestion_throughput.rs (create if needed, pattern from config-store)

use criterion::{criterion_group, criterion_main, Criterion, Throughput};

fn benchmark_parquet_writes(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let temp_dir = tempfile::TempDir::new().unwrap();
    let store = rt.block_on(async {
        ParquetStore::new(temp_dir.path()).unwrap()
    });

    let records = generate_test_records(1000);

    let mut group = c.benchmark_group("parquet_writes");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("batch_1000", |b| {
        b.to_async(&rt).iter(|| async {
            store.write_batch(&records).await.unwrap()
        })
    });

    group.finish();
}

criterion_group!(benches, benchmark_parquet_writes);
criterion_main!(benches);
```

**Cargo.toml addition** (following config-store pattern):
```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "ingestion_throughput"
harness = false
```

### 6.3 AIR-004 Performance Targets (Building on existing)

| Metric | Baseline (AIR-002) | AIR-004 Target | Approach |
|--------|-------------------|----------------|----------|
| Config load (multi-stream) | 8ms (single) | <15ms (10 streams) | etcd batch get |
| Ingestion throughput | 1 msg/sec | 10 msg/sec | Parallel sources |
| Memory per stream | 800MB (1 stream) | <100MB per stream | Streaming writes |
| Query multi-stream | 50ms (1 stream) | <200ms (10 streams) | Indexed partitions |

---

## 7. Quality Gates (Extending Existing CI)

### 7.1 Current CI Pipeline (`.github/workflows/air-001-ci.yml`)

**Existing Jobs**:
1. ✅ **check**: Compilation verification
2. ✅ **fmt**: Code formatting check
3. ✅ **clippy**: Lint checking (`-D warnings`)
4. ✅ **test**: Full test suite
5. ✅ **coverage**: Code coverage reporting
6. ✅ **build-multiarch**: x86_64 + aarch64 builds
7. ✅ **security-audit**: cargo audit
8. ✅ **check-todos**: No TODO/FIXME in staged files

### 7.2 AIR-004 Additions to CI

```yaml
# Add to .github/workflows/air-004-ci.yml

jobs:
  # ... existing jobs ...

  regression-tests:
    name: Regression Test Suite
    runs-on: ubuntu-latest
    steps:
      - name: Run AIR-001 tests
        run: cargo test -p config-client --all-features

      - name: Run AIR-002 tests
        run: cargo test -p air-quality-app --test integration_test

      - name: Run AIR-003 tests
        run: cargo test -p air-quality-app --test server_test

      - name: Fail if any regression
        if: failure()
        run: |
          echo "❌ Regression detected in existing functionality"
          exit 1

  benchmark-comparison:
    name: Performance Regression Check
    runs-on: ubuntu-latest
    steps:
      - name: Run benchmarks
        run: cargo bench --bench ingestion_throughput -- --save-baseline current

      - name: Compare with baseline
        run: |
          # Compare current vs baseline
          # Fail if >10% regression
```

### 7.3 Pre-Merge Requirements (Updated)

| Check | Required | Enforced By | Notes |
|-------|----------|-------------|-------|
| All tests pass | Yes | GitHub Actions | Includes regression tests |
| Coverage >= 75% | Yes | Codecov | Existing: ~80% |
| No clippy warnings | Yes | GitHub Actions | `-D warnings` |
| Code formatted | Yes | GitHub Actions | `cargo fmt --check` |
| Documentation | Yes | Code review | Public APIs must have docs |
| No unwrap() in production | Yes | Clippy lint | Use `?` operator |
| **Regression tests pass** | **Yes** | **New CI job** | **AIR-001/002/003 tests** |
| **Benchmarks stable** | **Yes** | **New CI job** | **<10% regression** |

---

## 8. Refactoring Guidelines (Preserving Existing Behavior)

### 8.1 Safe Refactoring Pattern (Feature Flags)

When refactoring existing components (e.g., MqttSource):

```rust
// Cargo.toml
[features]
default = ["new-mqtt-source"]
new-mqtt-source = []      # AIR-004 generic source
legacy-mqtt-source = []   # AIR-002 implementation

// In code
#[cfg(feature = "new-mqtt-source")]
mod mqtt_source_generic {
    // New implementation
}

#[cfg(feature = "legacy-mqtt-source")]
mod mqtt_source_legacy {
    // Keep old AIR-002 code intact
}

// Tests can run both
#[cfg(test)]
mod tests {
    #[test]
    fn test_both_implementations_equivalent() {
        #[cfg(feature = "new-mqtt-source")]
        let result_new = new_implementation();

        #[cfg(feature = "legacy-mqtt-source")]
        let result_legacy = legacy_implementation();

        assert_eq!(result_new, result_legacy);
    }
}
```

### 8.2 Breaking Change Protocol

1. Create deprecation notice with migration path
2. Add `#[deprecated]` attribute to old API
3. Maintain both APIs for 1 minor version
4. Run regression tests on BOTH paths
5. Remove in next major version

```rust
#[deprecated(since = "0.3.0", note = "Use `write_records` instead. Migration guide: docs/migration.md")]
pub async fn write(&self, record: StreamRecord) -> Result<()> {
    self.write_records(&[record]).await
}
```

---

## 9. Implementation Milestones (With Regression Checks)

### 9.1 Milestone Checklist

**M0: Baseline Established** (Day 1)
- [ ] All AIR-001 tests passing
- [ ] All AIR-002 tests passing
- [ ] All AIR-003 tests passing
- [ ] Baseline coverage captured (currently ~80%)
- [ ] Performance benchmarks documented

**M1: Foundation Complete** (Days 2-3)
- [ ] Core types implemented with tests
- [ ] Registry client functional
- [ ] 80% unit test coverage (new code)
- [ ] **All existing tests still passing**

**M2: Sources Complete** (Days 4-7)
- [ ] Source trait finalized
- [ ] MqttSource refactored (backward compatible)
- [ ] HttpPollingSource implemented
- [ ] WebhookHandler implemented
- [ ] Contract tests passing
- [ ] **AIR-002 MQTT pipeline tests still passing**

**M3: Storage Complete** (Days 8-11)
- [ ] ParquetStore multi-stream support
- [ ] TimescaleDB adapter working
- [ ] DDL generation functional
- [ ] Dual-write coordination tested
- [ ] **All AIR-002 storage tests still passing**

**M4: Integration Complete** (Days 12-14)
- [ ] Ingestion Coordinator running
- [ ] End-to-end tests passing
- [ ] Performance benchmarks met
- [ ] Documentation complete
- [ ] **All regression tests passing**
- [ ] **No performance degradation vs AIR-002**

### 9.2 Definition of Done (Updated)

A feature is complete when:
1. All acceptance tests pass
2. Unit test coverage >= 80%
3. Integration tests exist for external dependencies
4. **All regression tests pass (AIR-001/002/003)**
5. **Performance benchmarks meet targets**
6. Documentation updated
7. Code reviewed and approved
8. No new clippy warnings
9. Changelog entry added
10. **Migration guide written (if breaking changes)**

---

## 10. Risk Mitigation Through Testing (Extending Existing Tests)

### 10.1 Existing Failure Mode Tests (From integration_test.rs)

The codebase already has comprehensive failure testing:

```rust
// From apps/air-quality-app/tests/integration_test.rs

/// Test handling of NaN values
#[tokio::test]
async fn test_invalid_nan_values() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let point = TimeSeriesPoint {
        value: f64::NAN,
        // ...
    };

    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle NaN values");
}

/// Test concurrent writes to different locations
#[tokio::test]
async fn test_concurrent_writes_different_locations() {
    let store = Arc::new(ParquetStore::new(temp_dir.path()).unwrap());

    // Spawn concurrent write tasks
    let mut handles = vec![];
    for i in 0..5 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone.write_batch(vec![point]).await
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}
```

**Pattern to extend for AIR-004**: Apply same failure testing to new components.

### 10.2 Additional Failure Scenarios for AIR-004

```rust
#[tokio::test]
async fn test_handles_etcd_connection_loss() {
    let registry = RegistryClient::new(test_config()).await.unwrap();

    // Simulate etcd going down
    stop_etcd_container();

    // Registry should use cached config (graceful degradation)
    let config = registry.get_stream("air-quality").await.unwrap();
    assert!(config.is_some(), "Should fall back to cache");

    // New stream creation should fail gracefully
    let result = registry.create_stream("new-stream", test_config()).await;
    assert!(matches!(result, Err(RegistryError::ConnectionLost)));
}

#[tokio::test]
async fn test_multi_stream_isolation_on_failure() {
    let coordinator = IngestionCoordinator::start(test_config()).await.unwrap();

    // Stream 1 fails to connect
    inject_mqtt_broker_failure("stream-1");

    // Stream 2 should continue working
    verify_stream_healthy("stream-2").await;

    // Coordinator should report partial health
    let health = coordinator.health_check().await.unwrap();
    assert_eq!(health.status, HealthStatus::Degraded);
    assert_eq!(health.healthy_streams, 1);
    assert_eq!(health.total_streams, 2);
}
```

---

## 11. Summary

This refinement document establishes:

1. **TDD Practices**: Red-Green-Refactor with **regression testing**
2. **Existing Test Patterns**: Leverage 2000+ lines of existing tests
3. **Backward Compatibility**: Ensure AIR-001/002/003 continue working
4. **Code Standards**: Follow established Rust patterns (thiserror, tokio, tracing)
5. **Performance Targets**: Build on proven AIR-002 benchmarks
6. **Quality Gates**: Extend existing CI with regression checks
7. **Refactoring Guidelines**: Safe patterns with feature flags
8. **Risk Mitigation**: Reuse existing failure mode tests

### 11.1 Key Principles for AIR-004

1. **Don't Rewrite What Works**: Extend, don't replace AIR-001/002/003
2. **Test Migrations**: Run old tests against new code
3. **Gradual Cutover**: Feature flags for safe transitions
4. **Performance Baseline**: No regressions vs AIR-002
5. **Regression Suite**: All previous features must pass

### 11.2 Next Steps

1. ✅ Run baseline test suite: `cargo test --workspace`
2. ✅ Capture baseline metrics: `cargo llvm-cov`
3. ✅ Begin Phase 1: Core types (follow `air-quality/src/types.rs` pattern)
4. ✅ Implement regression CI job
5. ✅ Progress through iterations with continuous regression testing

---

**Document Version: 2.0.0**
**Last Updated: 2025-12-15**
**SPARC Phase: Refinement (Complete - Revised with Codebase Integration)**
