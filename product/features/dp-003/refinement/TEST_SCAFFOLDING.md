# DP-003: Test Scaffolding

## Overview

This document defines the test module structure and organization for the MQTT multi-subscription feature. The scaffolding follows NDP patterns established in AIR-005 with London School TDD principles.

---

## 1. Test Module Structure

### 1.1 Unit Tests (Inline Modules)

Unit tests are placed inline with implementation files using `#[cfg(test)]` modules.

```
core/src/
  sources/
    mqtt.rs                     # Existing - 15 tests preserved
    mqtt_multi.rs               # New multi-subscription support
      mod tests                 # 40-50 unit tests
        mod config_tests        # Configuration parsing (10-12 tests)
        mod router_tests        # Topic routing (12-15 tests)
        mod subscription_tests  # Subscription management (8-10 tests)
        mod parser_tests        # Parser integration (8-10 tests)
  config/
    mqtt_config.rs              # New config types with validation
      mod tests                 # Config validation tests
```

### 1.2 Integration Tests (External Test Crate)

Integration tests that require external infrastructure (MQTT broker).

```
tests/
  integration/
    mqtt/
      mod.rs                    # Module exports
      mqtt_multi_subscription_integration_test.rs  # Connection tests (8-10 tests)
      mqtt_routing_integration_test.rs             # Message flow tests (4-5 tests)
```

### 1.3 End-to-End Tests

Full pipeline tests requiring complete deployment.

```
tests/
  e2e/
    mqtt/
      mqtt_pipeline_test.rs     # Full pipeline verification (2-3 tests)
```

### 1.4 Performance Tests

Benchmarks for throughput and latency requirements.

```
benches/
  mqtt_throughput.rs            # Throughput benchmarks
  mqtt_latency.rs               # Latency benchmarks
```

---

## 2. Unit Test Module Organization

### 2.1 Configuration Tests Module

**File**: `core/src/sources/mqtt_multi.rs` or `core/src/config/mqtt_config.rs`

```rust
#[cfg(test)]
mod tests {
    mod config_tests {
        use super::super::*;

        // TC-1.1.x: Multi-subscription config loading
        #[test]
        fn test_load_multi_subscription_config() { /* ... */ }

        #[test]
        fn test_reject_duplicate_stream_ids() { /* ... */ }

        #[test]
        fn test_require_stream_id_per_subscription() { /* ... */ }

        #[test]
        fn test_require_topic_pattern_per_subscription() { /* ... */ }

        #[test]
        fn test_parser_config_optional() { /* ... */ }

        // TC-1.2.x: Backward compatibility
        #[test]
        fn test_legacy_single_topic_config() { /* ... */ }

        #[test]
        fn test_legacy_config_uses_parent_stream_id() { /* ... */ }

        #[test]
        fn test_legacy_and_new_format_exclusive() { /* ... */ }

        // TC-1.3.x: Per-subscription parser
        #[test]
        fn test_different_parsers_per_subscription() { /* ... */ }

        #[test]
        fn test_parser_uses_subscription_location_id_field() { /* ... */ }

        #[test]
        fn test_skip_fields_per_subscription() { /* ... */ }
    }
}
```

### 2.2 Topic Router Tests Module

**File**: `core/src/sources/mqtt_multi.rs`

```rust
#[cfg(test)]
mod tests {
    mod router_tests {
        use super::super::*;

        // TC-2.1.x: Topic pattern matching
        #[test]
        fn test_route_single_level_wildcard() { /* ... */ }

        #[test]
        fn test_route_multi_level_wildcard() { /* ... */ }

        #[test]
        fn test_route_to_correct_stream() { /* ... */ }

        #[test]
        fn test_first_match_wins() { /* ... */ }

        #[test]
        fn test_add_stream_id_tag() { /* ... */ }

        // TC-2.2.x: Unmatched messages
        #[test]
        fn test_unmatched_messages_logged() { /* ... */ }

        #[tokio::test]
        async fn test_continue_after_unmatched() { /* ... */ }

        // Edge cases
        #[test]
        fn test_empty_topic_no_match() { /* ... */ }

        #[test]
        fn test_exact_topic_match() { /* ... */ }

        #[test]
        fn test_multiple_wildcards_in_pattern() { /* ... */ }

        #[test]
        fn test_hash_at_end_only() { /* ... */ }

        #[test]
        fn test_plus_matches_non_empty_level() { /* ... */ }
    }
}
```

### 2.3 Subscription Management Tests Module

**File**: `core/src/sources/mqtt_multi.rs`

```rust
#[cfg(test)]
mod tests {
    mod subscription_tests {
        use super::super::*;

        // Subscription lifecycle
        #[test]
        fn test_subscription_creation() { /* ... */ }

        #[test]
        fn test_subscription_validation() { /* ... */ }

        #[tokio::test]
        async fn test_concurrent_subscription_access() { /* ... */ }

        // Health tracking
        #[tokio::test]
        async fn test_subscription_health_tracking() { /* ... */ }

        #[tokio::test]
        async fn test_message_counter_per_subscription() { /* ... */ }

        // QoS handling
        #[test]
        fn test_qos_per_subscription() { /* ... */ }

        #[test]
        fn test_default_qos_applied() { /* ... */ }
    }
}
```

### 2.4 Parser Integration Tests Module

**File**: `core/src/sources/mqtt_multi.rs`

```rust
#[cfg(test)]
mod tests {
    mod parser_tests {
        use super::super::*;
        use crate::parsers::*;

        // TC-3.1.x: Output schema consistency
        #[test]
        fn test_air_quality_output_schema() { /* ... */ }

        #[test]
        fn test_homeassistant_output_schema() { /* ... */ }

        #[test]
        fn test_both_streams_identical_schema() { /* ... */ }

        #[test]
        fn test_multiple_metrics_per_message() { /* ... */ }

        // TC-3.2.x: Stream tagging
        #[test]
        fn test_all_points_have_stream_id() { /* ... */ }

        #[test]
        fn test_all_points_have_source_tag() { /* ... */ }

        // TC-3.3.x: Error handling
        #[test]
        fn test_malformed_json_handled() { /* ... */ }

        #[test]
        fn test_missing_location_id_uses_default() { /* ... */ }

        #[tokio::test]
        async fn test_parser_error_continues_processing() { /* ... */ }
    }
}
```

---

## 3. Integration Test Module Organization

### 3.1 Connection Management Tests

**File**: `tests/integration/mqtt/mqtt_multi_subscription_integration_test.rs`

```rust
//! Integration tests for MQTT multi-subscription connection management
//!
//! Requires: Docker Compose with Mosquitto broker on port 11883

use neural_core::sources::{MqttConfig, MqttMultiSource, SubscriptionConfig};
use rumqttc::QoS;
use std::time::Duration;

mod connection_tests {
    use super::*;

    // TC-4.1.x: Single connection
    #[tokio::test]
    #[ignore] // Requires MQTT broker
    async fn test_single_connection_multiple_subscriptions() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_subscribe_all_topics_on_connect() { /* ... */ }

    // TC-4.2.x: Reconnection
    #[tokio::test]
    #[ignore]
    async fn test_resubscribe_after_reconnect() { /* ... */ }

    #[test]
    fn test_exponential_backoff() { /* ... */ }

    #[test]
    fn test_backoff_never_exceeds_max() { /* ... */ }
}
```

### 3.2 Message Flow Tests

**File**: `tests/integration/mqtt/mqtt_routing_integration_test.rs`

```rust
//! Integration tests for MQTT message routing
//!
//! Requires: Docker Compose with Mosquitto broker on port 11883

use neural_core::sources::{MqttConfig, MqttMultiSource};
use tokio::sync::mpsc;

mod routing_tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_message_routed_to_correct_stream() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_concurrent_message_handling() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_high_throughput_stress() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_parser_error_recovery() { /* ... */ }
}
```

---

## 4. E2E Test Module Organization

### 4.1 Pipeline Verification Tests

**File**: `tests/e2e/mqtt/mqtt_pipeline_test.rs`

```rust
//! End-to-end tests for MQTT pipeline
//!
//! Requires: Full deployment (MQTT + Parquet + DuckDB)
//! Run: ./deploy/pi/deploy.sh start

mod pipeline_tests {
    use super::*;

    // TC-7.1.x: Data flow verification
    #[tokio::test]
    #[ignore]
    async fn test_air_quality_to_parquet() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_homeassistant_to_separate_partition() { /* ... */ }

    #[tokio::test]
    #[ignore]
    async fn test_grafana_data_available() { /* ... */ }
}
```

---

## 5. Shared Test Utilities

### 5.1 Test Helpers Module

**File**: `core/src/sources/mqtt_test_helpers.rs` (compile-time gated)

```rust
//! Test utilities for MQTT multi-subscription testing
//! Only compiled with #[cfg(test)]

#[cfg(test)]
pub mod test_helpers {
    use super::*;
    use crate::parsers::{FlatJsonParser, ParserConfig, ParserType};
    use std::collections::HashMap;

    /// Create a default parser for testing
    pub fn create_default_parser() -> Box<dyn Parser + Send + Sync> {
        let config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec![
                "serialno".to_string(),
                "firmware".to_string(),
                "model".to_string(),
                "ledMode".to_string(),
            ],
            default_tags: [("source".to_string(), "mqtt".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        Box::new(FlatJsonParser::from_config(config).unwrap())
    }

    /// Create a HomeAssistant parser for testing
    pub fn create_homeassistant_parser() -> Box<dyn Parser + Send + Sync> {
        let config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "entity_id".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec!["entity_id".to_string()],
            default_tags: [("source".to_string(), "mqtt".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        Box::new(FlatJsonParser::from_config(config).unwrap())
    }

    /// Create a test point
    pub fn create_test_point(location_id: &str, metric: &str, value: f64) -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: location_id.to_string(),
            value,
            tags: HashMap::from([
                ("metric".to_string(), metric.to_string()),
                ("source".to_string(), "mqtt".to_string()),
            ]),
        }
    }

    /// Create a multi-subscription config for testing
    pub fn create_test_multi_config() -> MqttConfig {
        MqttConfig {
            broker_url: "localhost".to_string(),
            port: 11883, // Test port
            subscriptions: vec![
                SubscriptionConfig {
                    stream_id: "air-quality".to_string(),
                    topic_pattern: "airgradient/readings/+".to_string(),
                    ..Default::default()
                },
                SubscriptionConfig {
                    stream_id: "homeassistant".to_string(),
                    topic_pattern: "homeassistant/+/+/state".to_string(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }
}
```

### 5.2 Integration Test Fixtures Module

**File**: `tests/integration/mqtt/fixtures.rs`

```rust
//! Test fixtures for MQTT integration tests

use rumqttc::{AsyncClient, MqttOptions, QoS};
use std::time::Duration;

/// Create a test publisher client
pub async fn create_test_publisher() -> AsyncClient {
    let mut mqtt_options = MqttOptions::new("test-publisher", "localhost", 11883);
    mqtt_options.set_keep_alive(Duration::from_secs(5));

    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    // Spawn event loop handler
    tokio::spawn(async move {
        loop {
            if event_loop.poll().await.is_err() {
                break;
            }
        }
    });

    // Wait for connection
    tokio::time::sleep(Duration::from_millis(100)).await;
    client
}

/// Publish a test message
pub async fn publish_test_message(client: &AsyncClient, topic: &str, payload: &str) {
    client
        .publish(topic, QoS::AtLeastOnce, false, payload.as_bytes().to_vec())
        .await
        .expect("Failed to publish test message");
}
```

---

## 6. Test Categories and Attributes

### 6.1 Test Attributes

```rust
// Unit test - runs always
#[test]
fn test_unit_example() { /* ... */ }

// Async unit test
#[tokio::test]
async fn test_async_unit_example() { /* ... */ }

// Integration test - requires Docker
#[tokio::test]
#[ignore] // Run with: cargo test -- --ignored
async fn test_integration_example() { /* ... */ }

// Performance test - run separately
#[tokio::test]
#[ignore]
async fn test_performance_example() { /* ... */ }

// Test expecting panic
#[test]
#[should_panic(expected = "connection refused")]
fn test_panic_example() { /* ... */ }
```

### 6.2 Test Naming Convention

```
test_<component>_<scenario>_<expected_outcome>
```

Examples:
- `test_topic_router_wildcard_plus_matches_single_level`
- `test_config_parser_duplicate_stream_id_returns_error`
- `test_mqtt_source_reconnect_resubscribes_all_topics`

---

## 7. Test Execution Commands

### 7.1 Development Workflow

```bash
# Run all unit tests (fast)
cargo test --package neural-core --lib

# Run specific module tests
cargo test --package neural-core sources::mqtt_multi::tests::config_tests
cargo test --package neural-core sources::mqtt_multi::tests::router_tests

# Run with output
cargo test mqtt -- --nocapture

# Run single test
cargo test test_load_multi_subscription_config -- --exact
```

### 7.2 Integration Tests

```bash
# Start test infrastructure
docker-compose -f deploy/docker-compose.test.yml up -d

# Run integration tests
cargo test --test '*integration*' -- --ignored

# Run specific integration test
cargo test test_single_connection_multiple_subscriptions -- --ignored
```

### 7.3 Coverage Report

```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/ \
  --packages neural-core --lib

# View coverage
open coverage/tarpaulin-report.html
```

---

## 8. Backward Compatibility Tests

### 8.1 Preserved Tests

All 15 existing tests in `core/src/sources/mqtt.rs` must pass unchanged:

```rust
// These tests must remain in mqtt.rs and continue passing:
- test_mqtt_source_creation
- test_health_check_before_start
- test_parse_payload_success
- test_parse_payload_invalid_json
- test_parse_payload_partial_data
- test_parse_payload_all_fields
- test_exponential_backoff_calculation
- test_topic_pattern_substitution
- test_fetch_returns_cached_points
- test_parse_payload_extracts_all_numeric_fields
- test_field_names_not_renamed_at_ingestion
- test_non_metric_fields_excluded
- test_all_numeric_types_extracted
```

### 8.2 Regression Test

```rust
#[test]
fn test_all_existing_mqtt_tests_pass() {
    // This is a meta-test verified by running:
    // cargo test --package neural-core sources::mqtt::tests
    // All tests must pass without modification
}
```

---

## 9. CI/CD Test Integration

### 9.1 GitHub Actions Jobs

```yaml
# .github/workflows/test.yml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: cargo test --workspace --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      mosquitto:
        image: eclipse-mosquitto:2.0
        ports:
          - 11883:1883
    steps:
      - uses: actions/checkout@v4
      - name: Run integration tests
        run: cargo test --workspace --test '*integration*' -- --ignored
```

---

## 10. Summary

### Test Count by Category

| Category | Test Count | Location |
|----------|------------|----------|
| Config Unit Tests | 10-12 | `mqtt_multi.rs::config_tests` |
| Router Unit Tests | 12-15 | `mqtt_multi.rs::router_tests` |
| Subscription Unit Tests | 8-10 | `mqtt_multi.rs::subscription_tests` |
| Parser Unit Tests | 8-10 | `mqtt_multi.rs::parser_tests` |
| Connection Integration | 3-4 | `mqtt_multi_subscription_integration_test.rs` |
| Routing Integration | 4-5 | `mqtt_routing_integration_test.rs` |
| E2E Pipeline | 2-3 | `mqtt_pipeline_test.rs` |
| Performance | 3-4 | `benches/mqtt_*.rs` |
| **Total** | **50-63** | |

### Coverage Targets

| Component | Target | Priority |
|-----------|--------|----------|
| `MqttConfig` | 95% | High |
| `SubscriptionConfig` | 95% | High |
| `TopicRouter` | 90% | High |
| `MqttMultiSource` | 85% | High |
| Error handling | 80% | Medium |

---

## References

- TEST_STRATEGY.md - Overall test strategy
- TEST_CASES.md - Detailed test cases
- `docs/testing/AIR-005-TEST-DESIGN.md` - London School TDD patterns
- `core/src/sources/mqtt.rs` - Existing MQTT tests (15 tests)
