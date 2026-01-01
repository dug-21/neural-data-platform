# AIR-011: Implementation Plan - Eliminate Duplicative Parser Processing

## Overview

This document provides a detailed TDD-based implementation plan to eliminate the double-polling and parser waste causing Pi memory pressure and lockups.

## Root Cause Summary

The current architecture has **two independent polling mechanisms** running simultaneously:

1. **Internal `polling_loop()`** (via `source.start()`):
   - Spawns a background tokio task
   - Calls `poll_all_sensors()` / `poll_all_endpoints()` at `poll_interval`
   - Invokes parser to convert JSON -> `TimeSeriesPoint`
   - Sends parsed points to internal `mpsc::channel`
   - **These channels are NEVER consumed** (memory leak)

2. **SourceManager polling** (via `run_http_polling_source()`):
   - Has its own `tokio::time::interval(1 second)` loop
   - Calls `source.fetch_raw_batch()` (no parsing!)
   - Sends `RawDataPoint` to ingestion channel
   - **This is the actual Bronze layer path**

### The Problem

```
source.start()                     SourceManager loop
    |                                    |
    v                                    v
polling_loop()                    interval.tick()
    |                                    |
    v                                    v
poll_all_sensors()               fetch_raw_batch()
    |                                    |
    v                                    v
[PARSER INVOKED]                  [NO PARSING]
    |                                    |
    v                                    v
internal channel (UNBOUNDED!)     ingestion channel
    |                                    |
    v                                    v
NEVER CONSUMED (leak)             Bronze storage (actual path)
```

## Implementation Phases

### Phase 1: Stop Calling `source.start()` in SourceManager

**Files to Modify:**
- `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

**Specific Changes:**

#### 1.1 Remove `source.start()` from `run_http_polling_source()` (Lines 446-450)

```rust
// BEFORE (lines 446-450):
// Start the source
source
    .start()
    .await
    .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

// AFTER:
// AIR-011: Do NOT call source.start() - it spawns a background polling_loop
// that parses JSON into TimeSeriesPoints and sends to an unconsumed channel.
// We only use fetch_raw_batch() which fetches raw JSON for Bronze storage.
// The is_running flag is set in the constructor - no start() needed.
```

#### 1.2 Remove `source.start()` from `run_generic_http_polling_source()` (Lines 847-851)

```rust
// BEFORE (lines 847-851):
// Start the source
source
    .start()
    .await
    .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

// AFTER:
// AIR-011: Do NOT call source.start() - same rationale as HttpPollingSource
```

#### 1.3 Update `source.stop()` calls to no-op or remove

Since we're not calling `start()`, the `is_running` flag was never set, so `stop()` will be a no-op. However, we should keep the call for defensive cleanup.

**Verification:**
- Run existing tests: `cargo test -p air-quality-app`
- Verify no `polling_loop` task is spawned

---

### Phase 2: Remove Parser Dependency from Source Constructors

**Files to Modify:**
- `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

**Rationale:** Since we never parse during ingestion, parsers should not be required in constructors. This:
- Reduces memory allocation (no parser, no channel)
- Clarifies that sources only do raw fetching
- Preserves parsers for future Silver ETL

#### 2.1 Create Raw-Only Constructors in `http_poll.rs`

Add new constructor methods that don't require a parser:

```rust
impl HttpPollingSource {
    /// Create a raw-only HTTP polling source (AIR-011)
    ///
    /// This constructor creates a source optimized for Bronze layer ingestion:
    /// - No parser required (raw JSON stored as-is)
    /// - No internal channel allocation
    /// - No polling_loop capability
    pub fn new_raw_only(
        config: HttpPollingConfig,
        stream_id: Option<String>,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> CoreResult<Self> {
        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CoreError::Source(format!("Failed to create HTTP client: {}", e)))?;

        // AIR-011: No channel allocation - we only use fetch_raw_batch()
        // Create dummy channel that will never be used
        let (sender, receiver) = mpsc::channel(1); // Minimal allocation

        Ok(Self {
            config,
            parser: Arc::new(NoOpParser), // Placeholder parser
            client,
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            last_successful_poll: Arc::new(Mutex::new(HashMap::new())),
            stream_id,
            ndp_id,
            context,
        })
    }
}
```

#### 2.2 Create `NoOpParser` for Compatibility

```rust
/// No-operation parser for raw-only sources (AIR-011)
///
/// This parser exists only to satisfy the type system.
/// It should never be called in raw-only mode.
#[derive(Debug)]
pub struct NoOpParser;

impl Parser for NoOpParser {
    fn parse(&self, _data: &Value, _timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // AIR-011: Should never be called
        warn!("NoOpParser::parse() called - this indicates a bug in AIR-011 implementation");
        Ok(vec![])
    }

    fn name(&self) -> &str {
        "noop"
    }
}
```

#### 2.3 Update SourceManager to Use Raw-Only Constructors

In `source_manager.rs`, modify `run_http_polling_source()`:

```rust
// BEFORE:
let mut source = HttpPollingSource::with_raw_config(
    config,
    parser,  // <-- AIR-011: Remove this
    Some(stream_id.clone()),
    ndp_id.clone(),
    context.clone(),
)

// AFTER:
let source = HttpPollingSource::new_raw_only(
    config,
    Some(stream_id.clone()),
    ndp_id.clone(),
    context.clone(),
)
```

**Note:** Remove `mut` since we're not calling `start()` anymore.

---

### Phase 3: Remove Parser Creation from SourceManager

**Files to Modify:**
- `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

#### 3.1 Remove Parser Creation in `run_http_polling_source()` (Lines 413-434)

```rust
// BEFORE (lines 413-434):
// Create parser from config (FlatJson for AirGradient sensors)
let parser_config = ParserConfig {
    parser_type: ParserType::FlatJson,
    location_id_field: "serialno".to_string(),
    // ... 15+ lines of parser config
};
let parser = create_parser_from_config(parser_config).map_err(|e| {
    SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
})?;

// AFTER:
// AIR-011: Parser creation removed - Bronze layer stores raw JSON
// Parsers preserved in core/src/parsers/ for future Silver layer ETL
```

#### 3.2 Remove Parser Creation in `run_generic_http_polling_source()` (Lines 832-835)

```rust
// BEFORE (lines 832-835):
// Create parser from config (uses the actual parser type from YAML, not hardcoded FlatJson)
let parser = create_parser_from_config(parser_config).map_err(|e| {
    SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
})?;

// AFTER:
// AIR-011: Parser creation removed - parser_config preserved for logging/debugging only
debug!("Stream {} would use parser type {:?} (disabled for Bronze layer)",
       stream_id, parser_config.parser_type);
```

#### 3.3 Update Imports

Remove unused imports from `source_manager.rs`:

```rust
// Remove:
use neural_core::parsers::{create_parser_from_config, ParserConfig, ParserType};
```

---

### Phase 4: Archive Parsers for Silver ETL (Optional)

**Files to Consider:**
- `/workspaces/neural-data-platform/core/src/parsers/`
- `/workspaces/neural-data-platform/core/Cargo.toml`

**Option A: Feature Flag (Recommended)**

Keep parsers but gate them behind a feature flag:

```toml
# core/Cargo.toml
[features]
default = []
parsers = []  # Enable for Silver ETL, disable for Bronze-only builds
```

**Option B: Move to Separate Crate**

Create `neural-etl` crate for Silver layer:
```
/crates/neural-etl/
  Cargo.toml
  src/
    lib.rs
    parsers/
      mod.rs
      flat_json.rs
      array_iterator.rs
      column_oriented.rs
      jsonpath.rs
```

**Recommendation:** Use Option A for now - less disruption, easy to enable for Silver ETL work.

---

### Phase 5: Memory Optimization - Remove Unused Channels

**Files to Modify:**
- `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`

#### 5.1 Make `polling_loop()` Private and Remove from Raw Path

The `polling_loop()` method should remain for backward compatibility but be marked as deprecated or moved behind the `parsers` feature flag.

```rust
#[cfg(feature = "parsers")]
impl HttpPollingSource {
    /// Background polling task - DEPRECATED for Bronze ingestion
    ///
    /// This method is only relevant when using Source::fetch() pattern
    /// which parses data. For Bronze layer, use fetch_raw_batch() directly.
    #[deprecated(since = "0.2.0", note = "Use fetch_raw_batch() for Bronze layer")]
    async fn polling_loop(&self) -> CoreResult<()> {
        // ... existing implementation
    }
}
```

#### 5.2 Make `start()` No-Op for Raw Mode

```rust
impl HttpPollingSource {
    pub async fn start(&mut self) -> CoreResult<()> {
        // AIR-011: Check if this is a raw-only source
        if self.is_raw_only {
            info!("Raw-only source - start() is a no-op");
            return Ok(());
        }

        // ... existing implementation for backward compatibility
    }
}
```

---

## Implementation Order

Execute phases in this order to minimize risk:

| Order | Phase | Risk | Rollback Ease |
|-------|-------|------|---------------|
| 1 | Phase 1: Remove `start()` calls | Low | Very Easy |
| 2 | Phase 3: Remove parser creation | Low | Easy |
| 3 | Phase 5: Memory optimization | Medium | Moderate |
| 4 | Phase 2: Raw-only constructors | Medium | Moderate |
| 5 | Phase 4: Archive parsers | Low | N/A |

---

## TDD Test Cases

### Test 1: Sources Work Without Parsers

**File:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs` (test module)

```rust
#[tokio::test]
async fn test_raw_only_source_creation() {
    // Arrange
    let config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test123".to_string(),
            url: "http://localhost:8080/test".to_string(),
        }],
        ..Default::default()
    };

    // Act
    let source = HttpPollingSource::new_raw_only(
        config,
        Some("air-quality".to_string()),
        None,
        None,
    );

    // Assert
    assert!(source.is_ok());
    let source = source.unwrap();
    assert_eq!(source.source_id(), "air-quality-Http");
}
```

### Test 2: fetch_raw_batch Works Without start()

```rust
#[tokio::test]
async fn test_fetch_raw_batch_without_start() {
    // Arrange - Create mock HTTP server
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/measures/current"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"pm25": 10.5, "temp": 22.3})))
        .mount(&mock_server)
        .await;

    let config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test".to_string(),
            url: format!("{}/measures/current", mock_server.uri()),
        }],
        ..Default::default()
    };

    let source = HttpPollingSource::new_raw_only(config, Some("test".to_string()), None, None)
        .unwrap();

    // Act - Call fetch_raw_batch() without calling start()
    let result = source.fetch_raw_batch().await;

    // Assert
    assert!(result.is_ok());
    let points = result.unwrap();
    assert_eq!(points.len(), 1);
    assert!(points[0].raw_payload.get("pm25").is_some());
}
```

### Test 3: No Background Task Spawned

```rust
#[tokio::test]
async fn test_no_polling_loop_spawned() {
    // Arrange
    let config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test".to_string(),
            url: "http://localhost/test".to_string(),
        }],
        poll_interval: Duration::from_millis(10), // Very short interval
        ..Default::default()
    };

    let source = HttpPollingSource::new_raw_only(config, Some("test".to_string()), None, None)
        .unwrap();

    // Act - Wait for potential polling loop to trigger
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Assert - is_running should still be false (no background task)
    let is_running = *source.is_running.lock().await;
    assert!(!is_running, "No background task should be running");
}
```

### Test 4: Memory Stability Test

```rust
#[tokio::test]
async fn test_memory_does_not_grow_unbounded() {
    // Arrange
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"data": "x".repeat(1000)}))) // 1KB response
        .mount(&mock_server)
        .await;

    let config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test".to_string(),
            url: format!("{}/test", mock_server.uri()),
        }],
        buffer_capacity: 10, // Small buffer
        ..Default::default()
    };

    let source = HttpPollingSource::new_raw_only(config, Some("test".to_string()), None, None)
        .unwrap();

    // Act - Fetch many times
    for _ in 0..100 {
        let _ = source.fetch_raw_batch().await;
    }

    // Assert - Internal channel should not accumulate
    // (This is implicit - if old implementation, channel would have 100 items)
    let mut receiver = source.receiver.lock().await;
    let drained: Vec<_> = std::iter::from_fn(|| receiver.try_recv().ok()).collect();
    assert!(drained.is_empty(), "No parsed points should accumulate");
}
```

### Test 5: Parser Not Invoked

```rust
#[tokio::test]
async fn test_parser_not_invoked_during_fetch_raw() {
    // Arrange - Use a parser that panics if called
    struct PanicParser;
    impl Parser for PanicParser {
        fn parse(&self, _: &Value, _: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
            panic!("Parser should not be invoked!");
        }
        fn name(&self) -> &str { "panic" }
    }

    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"test": 1})))
        .mount(&mock_server)
        .await;

    let config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test".to_string(),
            url: format!("{}/test", mock_server.uri()),
        }],
        ..Default::default()
    };

    // Use with_raw_config with panic parser
    let source = HttpPollingSource::with_raw_config(
        config,
        Box::new(PanicParser),
        Some("test".to_string()),
        None,
        None,
    ).unwrap();

    // Act & Assert - Should not panic
    let result = source.fetch_raw_batch().await;
    assert!(result.is_ok());
}
```

---

## Rollback Plan

### Immediate Rollback (Phase 1 only)

If issues arise after Phase 1:

```rust
// Restore source.start() call in source_manager.rs
source
    .start()
    .await
    .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;
```

**Git command:**
```bash
git checkout HEAD~1 -- apps/air-quality-app/src/coordinator/source_manager.rs
```

### Full Rollback

```bash
# Revert all AIR-011 changes
git revert --no-commit <commit-hash-1> <commit-hash-2> ...
git commit -m "revert(air-011): rollback due to stability issues"
```

### Partial Rollback - Keep Raw Path, Restore Parser

If raw fetch works but we need parsers for debugging:

```rust
// In source_manager.rs, add parser back for logging only
let parser_config = /* ... */;
debug!("Would parse with {:?}", parser_config);
// But don't create or invoke parser
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking backward compatibility | Medium | High | Feature flags, deprecation warnings |
| Missed parser use case | Low | Medium | Comprehensive grep for parser usage |
| Test failures | Medium | Low | Run full test suite before each phase |
| Pi still locks up | Low | High | Memory profiling before/after deployment |
| Silver ETL needs parsers | Certain | Medium | Keep parsers, just don't invoke in Bronze |

### Pre-Implementation Checklist

- [ ] Run `cargo test --all` - all tests pass
- [ ] Run `cargo clippy` - no new warnings
- [ ] Review memory usage baseline on Pi
- [ ] Backup current working deployment
- [ ] Prepare rollback branch

### Post-Implementation Verification

- [ ] All tests pass
- [ ] No new clippy warnings
- [ ] Pi runs stable for 4+ hours
- [ ] Memory usage stable (no growth trend)
- [ ] Bronze layer data correct
- [ ] Logs show no parser invocations

---

## Code Locations Summary

| Component | File | Lines | Change Type |
|-----------|------|-------|-------------|
| SourceManager HTTP | `source_manager.rs` | 446-450 | Remove start() |
| SourceManager Generic | `source_manager.rs` | 847-851 | Remove start() |
| Parser creation HTTP | `source_manager.rs` | 413-434 | Remove block |
| Parser creation Generic | `source_manager.rs` | 832-835 | Remove block |
| HttpPollingSource | `http_poll.rs` | 363-416 | Add raw constructor |
| GenericHttpPollingSource | `http_poll.rs` | 846-900 | Add raw constructor |
| NoOpParser | `http_poll.rs` | New | Add struct |
| Imports | `source_manager.rs` | 6 | Remove parser imports |

---

## Next Steps

1. Create branch: `feature/air-011`
2. Implement Phase 1 (remove start() calls)
3. Run test suite
4. Deploy to Pi for 4-hour stability test
5. If stable, proceed to Phase 3
6. Document changes in ADR

---

*Last Updated: 2026-01-01*
*Author: SPARC Refinement Agent*
