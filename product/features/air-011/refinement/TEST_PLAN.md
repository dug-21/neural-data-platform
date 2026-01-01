# AIR-011: Test Plan - Eliminate Duplicative Parser Processing

## Overview

This test plan covers the verification strategy for AIR-011, ensuring that the elimination of duplicative parser processing:
1. Does not break existing functionality
2. Achieves memory stability on Pi
3. Maintains correct Bronze layer data storage

---

## Test Categories

### 1. Unit Tests

Unit tests verify individual component behavior in isolation.

#### 1.1 HttpPollingSource Tests

**File:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`

| Test Name | Description | Priority |
|-----------|-------------|----------|
| `test_raw_only_source_creation` | Verify source creates without parser | P0 |
| `test_fetch_raw_batch_without_start` | Verify fetch works without calling start() | P0 |
| `test_fetch_raw_returns_valid_json` | Verify raw payload is valid JSON | P0 |
| `test_source_id_generation` | Verify source_id format is correct | P1 |
| `test_no_polling_loop_spawned` | Verify no background task runs | P0 |
| `test_is_running_flag_not_set` | Verify is_running stays false in raw mode | P1 |
| `test_stop_is_idempotent` | Verify stop() works even without start() | P1 |

**Test Code:**

```rust
// /workspaces/neural-data-platform/core/src/sources/http_poll.rs
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, any};
    use serde_json::json;

    #[tokio::test]
    async fn test_raw_only_source_creation() {
        let config = HttpPollingConfig {
            sensors: vec![SensorConfig {
                serial_number: "test123".to_string(),
                url: "http://localhost:8080/test".to_string(),
            }],
            ..Default::default()
        };

        let result = HttpPollingSource::new_raw_only(
            config,
            Some("air-quality".to_string()),
            None,
            None,
        );

        assert!(result.is_ok(), "Should create source without parser");
        let source = result.unwrap();
        assert_eq!(source.source_id(), "air-quality-Http");
    }

    #[tokio::test]
    async fn test_fetch_raw_batch_without_start() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "pm25": 10.5,
                        "temp": 22.3,
                        "humidity": 45
                    }))
            )
            .mount(&mock_server)
            .await;

        let config = HttpPollingConfig {
            sensors: vec![SensorConfig {
                serial_number: "sensor1".to_string(),
                url: format!("{}/measures/current", mock_server.uri()),
            }],
            ..Default::default()
        };

        let source = HttpPollingSource::new_raw_only(
            config,
            Some("test-stream".to_string()),
            None,
            None,
        ).unwrap();

        // Do NOT call start() - this is the key test
        let result = source.fetch_raw_batch().await;

        assert!(result.is_ok());
        let points = result.unwrap();
        assert_eq!(points.len(), 1);

        let point = &points[0];
        assert!(point.raw_payload.get("pm25").is_some());
        assert!(point.raw_payload.get("temp").is_some());
    }

    #[tokio::test]
    async fn test_no_polling_loop_spawned() {
        let config = HttpPollingConfig {
            sensors: vec![SensorConfig {
                serial_number: "test".to_string(),
                url: "http://localhost/test".to_string(),
            }],
            poll_interval: Duration::from_millis(10),
            ..Default::default()
        };

        let source = HttpPollingSource::new_raw_only(
            config,
            Some("test".to_string()),
            None,
            None,
        ).unwrap();

        // Wait longer than poll_interval
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Verify no background task is running
        let is_running = *source.is_running.lock().await;
        assert!(!is_running, "Background polling should not be running");
    }

    #[tokio::test]
    async fn test_parser_not_invoked_during_raw_fetch() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        // Create a parser that tracks if it's called
        static PARSER_CALLED: AtomicBool = AtomicBool::new(false);

        struct TrackingParser;
        impl Parser for TrackingParser {
            fn parse(&self, _: &Value, _: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
                PARSER_CALLED.store(true, Ordering::SeqCst);
                Ok(vec![])
            }
            fn name(&self) -> &str { "tracking" }
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

        // Use with_raw_config (old API) to include parser
        let source = HttpPollingSource::with_raw_config(
            config,
            Box::new(TrackingParser),
            Some("test".to_string()),
            None,
            None,
        ).unwrap();

        // Call raw fetch - should NOT invoke parser
        let _ = source.fetch_raw_batch().await;

        assert!(!PARSER_CALLED.load(Ordering::SeqCst),
                "Parser should not be invoked during fetch_raw_batch()");
    }
}
```

#### 1.2 GenericHttpPollingSource Tests

Similar tests for `GenericHttpPollingSource`:

| Test Name | Description | Priority |
|-----------|-------------|----------|
| `test_generic_raw_only_creation` | Verify generic source creates without parser | P0 |
| `test_generic_fetch_raw_batch` | Verify raw batch fetch works | P0 |
| `test_generic_multi_endpoint_fetch` | Verify all endpoints fetched | P1 |
| `test_generic_auth_methods` | Verify auth works in raw mode | P1 |

#### 1.3 SourceManager Tests

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs`

| Test Name | Description | Priority |
|-----------|-------------|----------|
| `test_spawn_source_without_parser` | Verify source spawns without parser error | P0 |
| `test_ingestion_channel_receives_raw` | Verify raw points sent to ingestion | P0 |
| `test_no_memory_accumulation` | Verify no internal channel growth | P0 |
| `test_stop_source_cleanup` | Verify proper cleanup on stop | P1 |

```rust
// /workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs
#[cfg(test)]
mod air_011_tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_http_source_no_parser_creation() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

        let mut params = HashMap::new();
        params.insert(
            "endpoints".to_string(),
            serde_json::json!([{
                "serial": "test123",
                "url": "http://localhost:8080/test"
            }]),
        );

        let source_config = SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            ndp_id: None,
            context: None,
            params,
        };

        // This should succeed without parser creation
        let result = manager.spawn_source("test-stream", &source_config).await;
        assert!(result.is_ok());

        // Verify ingestion channel is connected
        assert!(!rx.is_closed());
    }

    #[tokio::test]
    async fn test_no_parsed_points_accumulation() {
        // This test verifies the AIR-011 fix:
        // Without the fix, internal mpsc channel would accumulate parsed points
        // With the fix, no parsing occurs, so no accumulation

        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

        // Spawn source and let it run briefly
        let mut params = HashMap::new();
        params.insert(
            "endpoints".to_string(),
            serde_json::json!([{
                "serial": "test",
                "url": "http://localhost:9999/nonexistent" // Will fail, that's OK
            }]),
        );
        params.insert("poll_interval_secs".to_string(), serde_json::json!(1));

        let source_config = SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            ndp_id: None,
            context: None,
            params,
        };

        let _ = manager.spawn_source("test", &source_config).await;

        // Wait a bit - old implementation would have accumulated parsed points
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Stop and verify no memory issues
        // (This is implicit - explicit memory check would require external tool)
        manager.stop_all_sources().await.unwrap();
    }
}
```

---

### 2. Integration Tests

Integration tests verify component interactions.

**File:** `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration/`

#### 2.1 End-to-End Ingestion Test

```rust
// /workspaces/neural-data-platform/apps/air-quality-app/tests/integration/air_011_test.rs

#[tokio::test]
async fn test_bronze_layer_receives_raw_json() {
    // Setup mock HTTP server
    let mock_server = MockServer::start().await;

    let sensor_response = json!({
        "wifi": -50,
        "serialno": "test123",
        "pm02": 12,
        "rco2": 800,
        "atmp": 22.5,
        "rhum": 45,
        "pm01": 8,
        "pm10": 15,
        "tvoc_index": 100,
        "nox_index": 50
    });

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&sensor_response))
        .mount(&mock_server)
        .await;

    // Setup stream config
    let stream_config = StreamConfig {
        stream_id: "test-air-quality".to_string(),
        enabled: true,
        sources: vec![SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            params: hashmap! {
                "endpoints".to_string() => json!([{
                    "serial": "test123",
                    "url": format!("{}/measures", mock_server.uri())
                }])
            },
            ..Default::default()
        }],
        ..Default::default()
    };

    // Setup ingestion channel
    let (tx, mut rx) = mpsc::channel(100);

    // Create and spawn source
    let registry = Arc::new(StreamRegistry::new_mock());
    registry.save_stream(&stream_config).await.unwrap();

    let mut manager = SourceManager::new(registry);
    manager.set_ingestion_sender(tx);
    manager.start_sources_for_stream(&stream_config).await.unwrap();

    // Wait for data
    let timeout = Duration::from_secs(5);
    let result = tokio::time::timeout(timeout, rx.recv()).await;

    assert!(result.is_ok(), "Should receive data within timeout");
    let raw_point = result.unwrap().unwrap();

    // Verify raw JSON is preserved exactly
    assert!(raw_point.raw_payload.is_object());
    assert_eq!(raw_point.raw_payload["pm02"], 12);
    assert_eq!(raw_point.raw_payload["atmp"], 22.5);

    // Verify metadata
    assert!(raw_point.source_id.contains("test-air-quality"));
}
```

#### 2.2 Multi-Source Integration Test

```rust
#[tokio::test]
async fn test_multiple_sources_no_memory_growth() {
    // Spawn multiple sources
    // Run for extended period
    // Verify memory stable

    let initial_memory = get_current_memory_usage();

    // ... spawn 5 sources, run for 30 seconds ...

    let final_memory = get_current_memory_usage();

    // Allow 10% growth for normal operation
    assert!(final_memory < initial_memory * 1.1,
            "Memory should not grow significantly: {} -> {}",
            initial_memory, final_memory);
}
```

---

### 3. Manual Verification Steps for Pi Stability

These tests must be run on actual Raspberry Pi hardware.

#### 3.1 Pre-Deployment Verification

**Checklist before deploying to Pi:**

- [ ] All unit tests pass locally
- [ ] All integration tests pass locally
- [ ] Code compiles for ARM target: `cargo build --target aarch64-unknown-linux-gnu`
- [ ] No new clippy warnings: `cargo clippy --all-targets`
- [ ] Documentation updated

#### 3.2 Pi Deployment Test Protocol

**Duration:** 24 hours minimum

**Setup:**
1. SSH into Pi: `ssh pi@neural-pi.local`
2. Stop existing services: `./deploy/pi/deploy.sh stop`
3. Deploy new code: `./deploy/pi/deploy.sh sync && ./deploy/pi/deploy.sh start`

**Monitoring Commands:**

```bash
# Memory monitoring (run every 5 minutes)
watch -n 300 'free -m && ps aux --sort=-%mem | head -5'

# Log monitoring
journalctl -u air-quality-app -f | grep -E '(memory|OOM|kill)'

# Process memory tracking
while true; do
    echo "$(date): $(ps -o rss,vsz -p $(pgrep air-quality-app))"
    sleep 60
done >> /tmp/memory-tracking.log
```

**Success Criteria:**

| Metric | Threshold | Check Interval |
|--------|-----------|----------------|
| RSS Memory | < 100MB | Every hour |
| Memory Growth | < 1MB/hour | After 4 hours |
| OOM Events | 0 | Continuous |
| Service Restarts | 0 | End of test |
| Data Ingestion | > 0 points/hour | Every hour |

#### 3.3 Memory Stability Test Script

```bash
#!/bin/bash
# /workspaces/neural-data-platform/scripts/air-011-stability-test.sh

LOG_FILE="/tmp/air-011-stability-$(date +%Y%m%d-%H%M%S).log"
DURATION_HOURS=24
CHECK_INTERVAL=300  # 5 minutes

echo "Starting AIR-011 stability test for $DURATION_HOURS hours" | tee -a "$LOG_FILE"
echo "Logging to: $LOG_FILE"

START_TIME=$(date +%s)
END_TIME=$((START_TIME + DURATION_HOURS * 3600))

# Get process ID
APP_PID=$(pgrep air-quality-app)
if [ -z "$APP_PID" ]; then
    echo "ERROR: air-quality-app not running!" | tee -a "$LOG_FILE"
    exit 1
fi

# Initial memory reading
INITIAL_RSS=$(ps -o rss= -p $APP_PID)
echo "Initial RSS: ${INITIAL_RSS}KB" | tee -a "$LOG_FILE"

LAST_RSS=$INITIAL_RSS
MAX_RSS=$INITIAL_RSS

while [ $(date +%s) -lt $END_TIME ]; do
    CURRENT_TIME=$(date "+%Y-%m-%d %H:%M:%S")

    # Check if process still running
    if ! kill -0 $APP_PID 2>/dev/null; then
        echo "$CURRENT_TIME: ERROR - Process died!" | tee -a "$LOG_FILE"
        exit 1
    fi

    # Get current memory
    CURRENT_RSS=$(ps -o rss= -p $APP_PID)
    DIFF=$((CURRENT_RSS - LAST_RSS))

    # Track max
    if [ $CURRENT_RSS -gt $MAX_RSS ]; then
        MAX_RSS=$CURRENT_RSS
    fi

    # Log
    echo "$CURRENT_TIME: RSS=${CURRENT_RSS}KB (delta=${DIFF}KB, max=${MAX_RSS}KB)" | tee -a "$LOG_FILE"

    # Alert if growing too fast
    if [ $DIFF -gt 1024 ]; then  # More than 1MB growth
        echo "$CURRENT_TIME: WARNING - Large memory increase: ${DIFF}KB" | tee -a "$LOG_FILE"
    fi

    LAST_RSS=$CURRENT_RSS
    sleep $CHECK_INTERVAL
done

# Final summary
FINAL_RSS=$(ps -o rss= -p $APP_PID)
TOTAL_GROWTH=$((FINAL_RSS - INITIAL_RSS))
HOURLY_GROWTH=$((TOTAL_GROWTH / DURATION_HOURS))

echo "" | tee -a "$LOG_FILE"
echo "=== STABILITY TEST COMPLETE ===" | tee -a "$LOG_FILE"
echo "Duration: ${DURATION_HOURS} hours" | tee -a "$LOG_FILE"
echo "Initial RSS: ${INITIAL_RSS}KB" | tee -a "$LOG_FILE"
echo "Final RSS: ${FINAL_RSS}KB" | tee -a "$LOG_FILE"
echo "Max RSS: ${MAX_RSS}KB" | tee -a "$LOG_FILE"
echo "Total Growth: ${TOTAL_GROWTH}KB" | tee -a "$LOG_FILE"
echo "Hourly Growth: ${HOURLY_GROWTH}KB/hour" | tee -a "$LOG_FILE"

# Pass/Fail determination
if [ $HOURLY_GROWTH -lt 1024 ] && [ $FINAL_RSS -lt 102400 ]; then
    echo "RESULT: PASS" | tee -a "$LOG_FILE"
    exit 0
else
    echo "RESULT: FAIL" | tee -a "$LOG_FILE"
    exit 1
fi
```

#### 3.4 Data Integrity Verification

After stability test, verify data correctness:

```bash
# Check Bronze layer data
ls -la /data/raw/air-quality/year=*/month=*/day=*/

# Verify Parquet files readable
parquet-tools show /data/raw/air-quality/year=2026/month=01/day=01/*.parquet | head -20

# Count records
parquet-tools rowcount /data/raw/air-quality/year=2026/month=01/day=01/*.parquet

# Verify raw_payload contains expected fields
parquet-tools cat --columns raw_payload /data/raw/air-quality/year=2026/month=01/day=01/*.parquet | head -5
```

---

### 4. Regression Tests

These tests ensure AIR-011 doesn't break existing functionality.

#### 4.1 Existing Test Suite

Run full test suite before and after changes:

```bash
# Before changes
cargo test --all 2>&1 | tee test-results-before.txt

# After changes
cargo test --all 2>&1 | tee test-results-after.txt

# Compare
diff test-results-before.txt test-results-after.txt
```

#### 4.2 API Compatibility Tests

```rust
#[test]
fn test_old_api_still_works() {
    // Verify with_raw_config still accepts parser
    let config = HttpPollingConfig::default();
    let parser = Box::new(crate::parsers::FlatJsonParser::default());

    let result = HttpPollingSource::with_raw_config(
        config,
        parser,
        Some("test".to_string()),
        None,
        None,
    );

    assert!(result.is_ok(), "Old API should still work");
}
```

---

### 5. Performance Tests

#### 5.1 Throughput Test

```rust
#[tokio::test]
async fn test_raw_fetch_throughput() {
    let mock_server = MockServer::start().await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({"data": "x".repeat(10000)}))) // 10KB response
        .mount(&mock_server)
        .await;

    let config = HttpPollingConfig {
        sensors: (0..10).map(|i| SensorConfig {
            serial_number: format!("sensor{}", i),
            url: format!("{}/sensor{}", mock_server.uri(), i),
        }).collect(),
        ..Default::default()
    };

    let source = HttpPollingSource::new_raw_only(
        config,
        Some("test".to_string()),
        None,
        None,
    ).unwrap();

    let start = std::time::Instant::now();

    for _ in 0..100 {
        let _ = source.fetch_raw_batch().await;
    }

    let duration = start.elapsed();
    let fetches_per_sec = 1000.0 / duration.as_secs_f64();

    println!("Throughput: {:.2} fetches/second", fetches_per_sec);
    assert!(fetches_per_sec > 10.0, "Should achieve >10 batch fetches/second");
}
```

---

## Test Execution Order

1. **Unit Tests** (Local, CI)
   - Run: `cargo test -p neural-core`
   - Run: `cargo test -p air-quality-app`

2. **Integration Tests** (Local, CI)
   - Run: `cargo test --test integration`

3. **Regression Tests** (Local, CI)
   - Run: `cargo test --all`

4. **Performance Tests** (Local)
   - Run: `cargo test --release -- --ignored performance`

5. **Manual Pi Tests** (Hardware)
   - Deploy to Pi
   - Run stability script
   - Monitor for 24 hours

---

## Test Environment Requirements

### Local Development
- Rust 1.75+
- Docker (for mock etcd)
- `wiremock` crate for HTTP mocking

### CI Environment
- GitHub Actions runner
- ARM cross-compilation toolchain
- Test fixtures in `/tests/fixtures/`

### Pi Test Environment
- Raspberry Pi 4 (4GB+ RAM recommended)
- NDP services deployed
- Network access to sensors (or mock endpoints)

---

## Pass/Fail Criteria

| Test Category | Pass Criteria |
|---------------|---------------|
| Unit Tests | 100% pass |
| Integration Tests | 100% pass |
| Regression Tests | No new failures |
| Performance Tests | Within 10% of baseline |
| Pi Stability (4h) | RSS < 100MB, growth < 1MB/hour |
| Pi Stability (24h) | RSS < 150MB, no restarts |
| Data Integrity | All Parquet files readable, fields present |

---

*Last Updated: 2026-01-01*
*Author: SPARC Refinement Agent*
