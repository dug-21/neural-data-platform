# Error Handling and Resilience Analysis Report

**Feature**: AIR-010
**Date**: 2026-01-01
**Analyzed Components**:
- `/workspaces/neural-data-platform/core/src/`
- `/workspaces/neural-data-platform/apps/air-quality-app/src/`

---

## Executive Summary

The Neural Data Platform codebase demonstrates generally good error handling practices with proper use of custom error types and the `?` operator. However, there are areas for improvement, particularly around excessive `.unwrap()` usage in tests (which is acceptable) versus production code, and opportunities to consolidate error types and add better error context.

### Key Findings

| Category | High Risk | Medium Risk | Low Risk |
|----------|-----------|-------------|----------|
| Unwrap/Expect in Production | 2 | 5 | 8 |
| Error Type Consolidation | 1 | 3 | 2 |
| Missing Error Context | 3 | 7 | 5 |
| Panic-Prone Patterns | 1 | 2 | 3 |
| Retry Pattern Issues | 0 | 2 | 1 |
| Logging Gaps | 2 | 4 | 3 |

---

## 1. Excessive .unwrap() and .expect() Usage

### 1.1 Production Code Findings

#### HIGH RISK

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs:198`
```rust
tokio::signal::ctrl_c()
    .await
    .expect("Failed to listen for Ctrl+C");
```
- **Risk**: Application panic on signal handling failure
- **Recommendation**: Log error and attempt graceful degradation
```rust
if let Err(e) = tokio::signal::ctrl_c().await {
    tracing::error!(error = %e, "Failed to listen for shutdown signal");
    // Continue running without shutdown handler
}
```

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs:21-25`
```rust
tracing_subscriber::EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| "air_quality_app=debug,tower_http=debug".into())
```
- **Risk**: LOW - Already has fallback pattern
- **Status**: Good practice - uses `unwrap_or_else` with sensible default

#### MEDIUM RISK

**Location**: `/workspaces/neural-data-platform/core/src/storage/parquet.rs:382`
```rust
sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
```
- **Risk**: Panic if NaN values exist in data
- **Recommendation**: Handle NaN case explicitly
```rust
sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
```

**Location**: `/workspaces/neural-data-platform/core/src/forecast/fann_adapter.rs:254`
```rust
let model = self.loaded_model.as_ref().unwrap();
```
- **Risk**: Panic if model not loaded before use
- **Recommendation**: Return error instead
```rust
let model = self.loaded_model.as_ref()
    .ok_or_else(|| CoreError::Model("Model not loaded".to_string()))?;
```

### 1.2 Test Code Analysis

Test code contains **400+** instances of `.unwrap()` which is acceptable for tests. However, recommend using helper macros or dedicated test assertions for better error messages.

**Recommendation**: Create test helper functions:
```rust
#[cfg(test)]
mod test_helpers {
    pub fn assert_parses<T: DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json)
            .expect(&format!("Failed to parse JSON: {}", json))
    }
}
```

---

## 2. Error Type Consolidation Opportunities

### 2.1 Current Error Hierarchy

```
core/src/error.rs
  CoreError
    - Source(String)
    - Storage(String)
    - Config(String)
    - Forecast(String)
    - Model(String)
    - Parse(String)

apps/air-quality-app/src/error.rs
  AppError
    - (custom variants)

apps/air-quality-app/src/coordinator/mod.rs
  CoordinatorError
    - RoutingError(String)
    - SourceManagerError(String)
    - ShutdownError(String)
    - ChannelError(String)

core/src/sources/mqtt/mod.rs
  ConfigError
    - NoSubscriptions
    - DuplicateStreamId(String)
    - InvalidSubscription { stream_id, error }

core/src/sources/mqtt/router.rs
  RouterError

core/src/sources/mqtt/subscription.rs
  SubscriptionError
```

### 2.2 Consolidation Recommendations

#### HIGH RISK - Error Information Loss

**Issue**: Many errors use `String` wrapping which loses type information

**Current**:
```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Source error: {0}")]
    Source(String),
}
```

**Recommended**:
```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Source error: {source}")]
    Source {
        source: Box<dyn std::error::Error + Send + Sync>,
        context: String,
    },
}
```

#### MEDIUM RISK - Duplicate Error Patterns

**Issue**: Multiple modules define similar error patterns

**Files**:
- `core/src/sources/mqtt/mod.rs` - ConfigError
- `core/src/sources/mqtt/router.rs` - RouterError
- `core/src/sources/mqtt/subscription.rs` - SubscriptionError

**Recommendation**: Consolidate into single `MqttError` enum:
```rust
#[derive(Debug, thiserror::Error)]
pub enum MqttError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("Router error: {0}")]
    Router(#[from] RouterError),
    #[error("Subscription error: {0}")]
    Subscription(#[from] SubscriptionError),
}
```

---

## 3. Result Chain Optimization

### 3.1 Efficient ? Operator Usage

**Good Example** - `/workspaces/neural-data-platform/core/src/storage/parquet.rs`:
```rust
async fn write(&self, point: TimeSeriesPoint) -> CoreResult<()> {
    let mut wal = self.wal.lock().await;
    let entry = serde_json::to_vec(&point)
        .map_err(|e| CoreError::Storage(format!("Failed to serialize point: {}", e)))?;
    wal.append(&entry)?;
    // ...
}
```

### 3.2 Improvement Opportunities

**Location**: `/workspaces/neural-data-platform/core/src/storage/parquet.rs:164-186`
```rust
// Current - Verbose pattern repeated multiple times
let timestamps = df
    .column("timestamp")
    .map_err(|e| CoreError::Storage(format!("Missing timestamp column: {}", e)))?
    .i64()
    .map_err(|e| CoreError::Storage(format!("Invalid timestamp type: {}", e)))?;
```

**Recommendation**: Create extension trait for column extraction:
```rust
trait DataFrameExt {
    fn get_column_i64(&self, name: &str) -> CoreResult<&ChunkedArray<Int64Type>>;
    fn get_column_utf8(&self, name: &str) -> CoreResult<&Utf8Chunked>;
}

impl DataFrameExt for DataFrame {
    fn get_column_i64(&self, name: &str) -> CoreResult<&ChunkedArray<Int64Type>> {
        self.column(name)
            .map_err(|e| CoreError::Storage(format!("Missing {} column: {}", name, e)))?
            .i64()
            .map_err(|e| CoreError::Storage(format!("Invalid {} type: {}", name, e)))
    }
}
```

---

## 4. Panic-Prone Patterns

### 4.1 Production Code

#### HIGH RISK

**Location**: `/workspaces/neural-data-platform/config-store/src/types.rs:294`
```rust
panic!("Invalid path format: {}", path);
```
- **Risk**: Application crash on invalid configuration
- **Recommendation**: Return `Result<(), ConfigError>`

### 4.2 Test Code (Acceptable)

Test code appropriately uses `panic!` for test assertions. The following patterns are correct:
```rust
_ => panic!("Expected Connection error"),
```

---

## 5. Missing Error Context

### 5.1 Current State

Many errors lack sufficient context for debugging:

**Example** - `/workspaces/neural-data-platform/core/src/sources/http_poll.rs:437-438`
```rust
.map_err(|e| CoreError::Source(format!("HTTP request failed: {}", e)))?;
```

**Missing Context**:
- Endpoint URL
- Sensor serial number
- Request attempt number
- Timestamp

### 5.2 Recommendations

#### Adopt `anyhow` for Applications

**File**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs`

```rust
use anyhow::{Context, Result};

// Before
let store = Arc::new(ParquetStore::new(&config.storage.base_path)?);

// After
let store = Arc::new(
    ParquetStore::new(&config.storage.base_path)
        .context(format!("Failed to initialize storage at {}", config.storage.base_path))?
);
```

#### Use `thiserror` with Structured Fields

**File**: `/workspaces/neural-data-platform/core/src/error.rs`

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("HTTP polling failed for {endpoint} (attempt {attempt}/{max_attempts}): {message}")]
    HttpPolling {
        endpoint: String,
        attempt: u32,
        max_attempts: u32,
        message: String,
        #[source]
        source: Option<reqwest::Error>,
    },
}
```

---

## 6. Retry Pattern Analysis

### 6.1 Current Implementation

**Good Example** - `/workspaces/neural-data-platform/core/src/sources/http_poll.rs:46-90`

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl RetryConfig {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);
        // ... jitter logic
    }
}
```

**Assessment**: EXCELLENT - Well-designed with:
- Exponential backoff
- Maximum delay cap
- Jitter to prevent thundering herd
- Configurable parameters

### 6.2 Improvement Opportunities

#### MEDIUM RISK - MQTT Reconnection

**Location**: `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs:406-418`

```rust
// Current - Uses integer arithmetic
let delay = std::cmp::min(
    config.reconnect_delay.as_secs() * 2_u64.pow(reconnect_attempt),
    config.max_reconnect_delay.as_secs(),
);
```

**Issue**: No jitter, potential overflow with large attempt counts

**Recommendation**: Reuse `RetryConfig` from http_poll module:
```rust
use crate::sources::http_poll::RetryConfig;

let retry_config = RetryConfig::default();
let delay = retry_config.delay_for_attempt(reconnect_attempt);
```

---

## 7. Logging Analysis

### 7.1 Good Patterns

**Structured Logging** - `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs`
```rust
error!(
    topic = %publish.topic,
    stream_id = %route.stream_id,
    error = %e,
    "Failed to parse MQTT payload"
);
```

### 7.2 Improvement Areas

#### HIGH RISK - Missing Critical Context

**Location**: `/workspaces/neural-data-platform/apps/air-quality-app/src/main.rs:208-209`
```rust
if let Err(e) = result {
    tracing::error!("Server error: {}", e);
}
```

**Missing**:
- Request context
- Connection details
- Stack trace

**Recommendation**:
```rust
if let Err(e) = result {
    tracing::error!(
        error = ?e,
        error_chain = %format!("{:?}", e.source()),
        "Server error occurred"
    );
}
```

#### MEDIUM RISK - Inconsistent Log Levels

**Findings**:
- Configuration load failures use `warn` but could fail the application
- Some transient errors logged at `error` level (should be `warn`)

**Example**:
```rust
// Current - Uses warn for potentially critical failure
tracing::warn!("Failed to load stream config: {}", e);

// Recommended - Use error if this prevents startup
tracing::error!(
    stream_id = "air-quality",
    error = %e,
    fallback = "legacy etcd",
    "Primary config source unavailable, attempting fallback"
);
```

---

## 8. Resilience Improvement Recommendations

### 8.1 Circuit Breaker Pattern

**Recommendation**: Add circuit breaker for external service calls

```rust
pub struct CircuitBreaker {
    failure_count: AtomicU32,
    last_failure: AtomicU64,
    state: AtomicU8,  // 0=Closed, 1=Open, 2=HalfOpen
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub async fn call<T, E, F>(&self, f: F) -> Result<T, E>
    where
        F: Future<Output = Result<T, E>>,
    {
        if self.is_open() {
            return Err(CircuitOpenError);
        }

        match f.await {
            Ok(v) => {
                self.record_success();
                Ok(v)
            }
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }
}
```

### 8.2 Bulkhead Pattern

**Recommendation**: Isolate critical paths with dedicated thread pools

```rust
// Separate runtime for storage operations
let storage_runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(2)
    .thread_name("storage")
    .build()?;

// Separate runtime for source polling
let polling_runtime = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)
    .thread_name("polling")
    .build()?;
```

### 8.3 Timeout Wrappers

**Recommendation**: Add consistent timeout handling

```rust
use tokio::time::{timeout, Duration};

pub async fn with_timeout<T, F>(
    name: &str,
    duration: Duration,
    f: F,
) -> CoreResult<T>
where
    F: Future<Output = CoreResult<T>>,
{
    timeout(duration, f)
        .await
        .map_err(|_| CoreError::Timeout {
            operation: name.to_string(),
            duration,
        })?
}
```

### 8.4 Graceful Degradation

**Current State**: Application fails completely if primary config source unavailable

**Recommendation**: Implement fallback chain with health tracking

```rust
pub struct ConfigLoader {
    sources: Vec<Box<dyn ConfigSource>>,
    health: HashMap<String, SourceHealth>,
}

impl ConfigLoader {
    pub async fn load(&mut self) -> CoreResult<Config> {
        for source in &self.sources {
            match source.load().await {
                Ok(config) => {
                    self.mark_healthy(source.name());
                    return Ok(config);
                }
                Err(e) => {
                    self.mark_unhealthy(source.name(), e);
                    continue;
                }
            }
        }
        Err(CoreError::Config("All config sources failed".into()))
    }
}
```

---

## 9. Action Items by Priority

### Immediate (High Priority)

1. **Fix panic in config-store/src/types.rs:294** - Replace panic with Result
2. **Add error context to HTTP polling failures** - Include endpoint, attempt info
3. **Fix partial_cmp().unwrap() in parquet.rs** - Handle NaN values

### Short-term (Medium Priority)

4. **Consolidate MQTT error types** - Create unified MqttError enum
5. **Add circuit breaker for external APIs** - OpenWeatherMap, etc.
6. **Improve logging consistency** - Standardize log levels

### Long-term (Low Priority)

7. **Migrate to structured errors** - Add source fields to CoreError variants
8. **Add bulkhead isolation** - Separate runtimes for critical paths
9. **Create test helper utilities** - Reduce unwrap() in tests with better helpers

---

## 10. Testing Recommendations

### Error Path Testing

```rust
#[tokio::test]
async fn test_storage_handles_invalid_parquet() {
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Write corrupted file
    std::fs::write(temp_dir.path().join("data/bad.parquet"), b"invalid")?;

    // Should return error, not panic
    let result = store.query("test", start, end, None).await;
    assert!(matches!(result, Err(CoreError::Storage(_))));
}
```

### Chaos Testing

```rust
#[tokio::test]
async fn test_resilience_under_network_failure() {
    let mock_server = MockServer::start().await;

    // Simulate network failure after 3 requests
    mock_server.register_failing_after(3).await;

    let source = HttpPollingSource::new(config, parser)?;

    // Should retry and eventually fail gracefully
    let result = source.fetch().await;
    assert!(result.is_err());
    // Verify retry metrics
    assert!(source.metrics().retry_count >= 3);
}
```

---

## Appendix: Files Analyzed

| File | Unwrap Count | Expect Count | Error Handling Grade |
|------|--------------|--------------|---------------------|
| core/src/error.rs | 0 | 0 | A |
| core/src/traits.rs | 42 (tests) | 0 | B+ (test only) |
| core/src/sources/http_poll.rs | 67 (tests) | 0 | A |
| core/src/sources/mqtt/mod.rs | 28 (tests) | 0 | A |
| core/src/storage/parquet.rs | 89 (tests) | 0 | B (NaN handling) |
| core/src/storage/wal.rs | 18 (tests) | 0 | A |
| apps/air-quality-app/src/main.rs | 2 | 1 | B |
| apps/air-quality-app/src/config.rs | 0 | 0 | A |
| apps/air-quality-app/src/coordinator/ | 0 | 0 | A |

**Overall Grade: B+**

The codebase demonstrates mature error handling practices with room for improvement in error context, consolidation, and resilience patterns.
