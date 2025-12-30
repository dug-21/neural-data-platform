# DP-003: Mock Setup

## Overview

This document defines the mock implementations and test infrastructure needed for TDD implementation of the MQTT multi-subscription feature. Following NDP patterns from AIR-005, we use trait-based mocking with the `mockall` crate and behavior verification.

---

## 1. Mock Strategy

### 1.1 Unit Test Mocks

Unit tests use mocks to isolate components and verify interactions:

| Component | Mock Type | Purpose |
|-----------|-----------|---------|
| MqttClient | MockMqttClient | Verify subscribe/publish/disconnect calls |
| Parser | MockParser | Verify parse calls and routing |
| TopicRouter | Real impl | Simple enough to test directly |
| EventLoop | Not mocked | Use real async channels |

### 1.2 Integration Test Infrastructure

Integration tests use real components with test infrastructure:

| Component | Implementation | Notes |
|-----------|----------------|-------|
| MQTT Broker | Mosquitto Docker | Port 11883 (test) |
| Parser | Real FlatJsonParser | Verify actual parsing |
| Storage | In-memory channel | Capture output |

---

## 2. MockMqttClient

### 2.1 Trait Definition

```rust
// core/src/sources/mqtt_client.rs
use async_trait::async_trait;
use rumqttc::QoS;

/// Abstraction over MQTT client for testability
#[async_trait]
pub trait MqttClientTrait: Send + Sync {
    /// Subscribe to a topic pattern
    async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), MqttError>;

    /// Subscribe to multiple topics atomically
    async fn subscribe_many(&self, topics: Vec<(&str, QoS)>) -> Result<(), MqttError>;

    /// Disconnect from broker
    async fn disconnect(&self) -> Result<(), MqttError>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}
```

### 2.2 Mock Implementation

```rust
// core/src/sources/mqtt_client.rs
#[cfg(test)]
pub mod test_mocks {
    use super::*;
    use mockall::automock;
    use mockall::predicate::*;

    #[automock]
    #[async_trait]
    pub trait MqttClientTrait: Send + Sync {
        async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), MqttError>;
        async fn subscribe_many(&self, topics: Vec<(&str, QoS)>) -> Result<(), MqttError>;
        async fn disconnect(&self) -> Result<(), MqttError>;
        fn is_connected(&self) -> bool;
    }

    impl MockMqttClientTrait {
        /// Create a mock that expects specific subscriptions
        pub fn expecting_subscriptions(topics: Vec<&str>) -> Self {
            let mut mock = MockMqttClientTrait::new();

            for topic in topics {
                let topic_owned = topic.to_string();
                mock.expect_subscribe()
                    .withf(move |t, _| t == topic_owned)
                    .times(1)
                    .returning(|_, _| Ok(()));
            }

            mock.expect_is_connected()
                .returning(|| true);

            mock
        }

        /// Create a mock that simulates connection failure
        pub fn failing_connection() -> Self {
            let mut mock = MockMqttClientTrait::new();

            mock.expect_subscribe()
                .returning(|_, _| Err(MqttError::ConnectionFailed("test failure".to_string())));

            mock.expect_is_connected()
                .returning(|| false);

            mock
        }
    }
}
```

### 2.3 Usage Example

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::sources::mqtt_client::test_mocks::MockMqttClientTrait;
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_mqtt_source_subscribes_to_all_topics() {
        // ARRANGE
        let mut mock_client = MockMqttClientTrait::new();

        // Expect subscriptions for both patterns
        mock_client
            .expect_subscribe()
            .with(eq("airgradient/readings/+"), eq(QoS::AtLeastOnce))
            .times(1)
            .returning(|_, _| Ok(()));

        mock_client
            .expect_subscribe()
            .with(eq("homeassistant/+/+/state"), eq(QoS::AtLeastOnce))
            .times(1)
            .returning(|_, _| Ok(()));

        mock_client
            .expect_is_connected()
            .returning(|| true);

        let config = MqttConfig {
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
        };

        let source = MqttMultiSource::with_client(config, Box::new(mock_client));

        // ACT
        source.start().await.unwrap();

        // ASSERT - mockall verifies expectations automatically
    }
}
```

---

## 3. MockParser

### 3.1 Trait Definition

The `Parser` trait already exists in `core/src/parsers/mod.rs`:

```rust
// Already defined in neural-core
pub trait Parser: Send + Sync {
    fn parse(&self, json: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

### 3.2 Mock Implementation

```rust
// core/src/parsers/test_mocks.rs
#[cfg(test)]
pub mod test_mocks {
    use super::*;
    use mockall::automock;

    #[automock]
    pub trait Parser: Send + Sync {
        fn parse(&self, json: &Value, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>>;
    }

    impl MockParser {
        /// Create a mock that returns fixed points
        pub fn returning_points(points: Vec<TimeSeriesPoint>) -> Self {
            let mut mock = MockParser::new();
            mock.expect_parse()
                .returning(move |_, _| Ok(points.clone()));
            mock
        }

        /// Create a mock that fails with specific error
        pub fn failing_with(error: CoreError) -> Self {
            let mut mock = MockParser::new();
            mock.expect_parse()
                .returning(move |_, _| Err(error.clone()));
            mock
        }

        /// Create a mock that tracks parse calls
        pub fn tracking_calls() -> (Self, Arc<Mutex<Vec<Value>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let calls_clone = calls.clone();

            let mut mock = MockParser::new();
            mock.expect_parse()
                .returning(move |json, _| {
                    calls_clone.lock().unwrap().push(json.clone());
                    Ok(vec![])
                });

            (mock, calls)
        }
    }
}
```

### 3.3 Usage Example

```rust
#[tokio::test]
async fn test_message_routed_to_correct_parser() {
    // ARRANGE
    let air_parser = MockParser::returning_points(vec![create_test_point("sensor1", "pm02", 15.0)]);
    let ha_parser = MockParser::returning_points(vec![create_test_point("sensor.temp", "state", 21.5)]);

    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "air-quality".to_string(),
            topic_pattern: "airgradient/+".to_string(),
            parser: Some(Box::new(air_parser)),
        },
        SubscriptionConfig {
            stream_id: "homeassistant".to_string(),
            topic_pattern: "homeassistant/#".to_string(),
            parser: Some(Box::new(ha_parser)),
        },
    ]);

    // ACT
    let result = router.route_message("airgradient/abc123", &json!({"pm02": 15.0}));

    // ASSERT
    assert_eq!(result.unwrap().stream_id, "air-quality");
}
```

---

## 4. Test Infrastructure Setup

### 4.1 Docker Compose for Tests

**File**: `deploy/docker-compose.test.yml`

```yaml
version: '3.8'

services:
  mosquitto-test:
    image: eclipse-mosquitto:2.0
    container_name: mosquitto-test
    ports:
      - "11883:1883"
      - "19001:9001"
    volumes:
      - ./tests/fixtures/mqtt/mosquitto.conf:/mosquitto/config/mosquitto.conf:ro
      - ./tests/fixtures/mqtt/passwd:/mosquitto/config/passwd:ro
    healthcheck:
      test: ["CMD", "mosquitto_pub", "-h", "localhost", "-t", "test", "-m", "health"]
      interval: 5s
      timeout: 3s
      retries: 3
    networks:
      - test-network

networks:
  test-network:
    driver: bridge
```

### 4.2 Mosquitto Test Configuration

**File**: `tests/fixtures/mqtt/mosquitto.conf`

```
# Test Mosquitto Configuration
listener 1883
allow_anonymous true
persistence false
log_dest stdout
log_type all

# WebSocket listener
listener 9001
protocol websockets

# Limits
max_connections 100
max_queued_messages 1000
```

### 4.3 Test Helper Functions

**File**: `tests/integration/mqtt/test_helpers.rs`

```rust
//! Test helper functions for MQTT integration tests

use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::time::Duration;
use tokio::sync::oneshot;

/// Test configuration constants
pub const TEST_BROKER_HOST: &str = "localhost";
pub const TEST_BROKER_PORT: u16 = 11883;

/// Wait for broker to be ready
pub async fn wait_for_broker() -> bool {
    for _ in 0..30 {
        let mut options = MqttOptions::new("health-check", TEST_BROKER_HOST, TEST_BROKER_PORT);
        options.set_keep_alive(Duration::from_secs(2));

        let (client, mut event_loop) = AsyncClient::new(options, 10);

        match tokio::time::timeout(Duration::from_secs(1), event_loop.poll()).await {
            Ok(Ok(Event::Incoming(Packet::ConnAck(_)))) => {
                let _ = client.disconnect().await;
                return true;
            }
            _ => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    false
}

/// Create a test publisher with unique client ID
pub async fn create_test_publisher(name: &str) -> (AsyncClient, oneshot::Sender<()>) {
    let client_id = format!("test-pub-{}-{}", name, uuid::Uuid::new_v4());
    let mut options = MqttOptions::new(&client_id, TEST_BROKER_HOST, TEST_BROKER_PORT);
    options.set_keep_alive(Duration::from_secs(10));

    let (client, mut event_loop) = AsyncClient::new(options, 100);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    // Spawn event loop
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = event_loop.poll() => {
                    if result.is_err() {
                        break;
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    // Wait for connection
    tokio::time::sleep(Duration::from_millis(100)).await;
    (client, shutdown_tx)
}

/// Create a test subscriber that collects messages
pub async fn create_test_subscriber(
    name: &str,
    topics: Vec<&str>,
) -> (mpsc::Receiver<(String, Vec<u8>)>, oneshot::Sender<()>) {
    let client_id = format!("test-sub-{}-{}", name, uuid::Uuid::new_v4());
    let mut options = MqttOptions::new(&client_id, TEST_BROKER_HOST, TEST_BROKER_PORT);
    options.set_keep_alive(Duration::from_secs(10));

    let (client, mut event_loop) = AsyncClient::new(options, 100);
    let (msg_tx, msg_rx) = mpsc::channel(100);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    // Subscribe to topics
    for topic in topics {
        client.subscribe(topic, QoS::AtLeastOnce).await.unwrap();
    }

    // Spawn event loop
    tokio::spawn(async move {
        loop {
            tokio::select! {
                result = event_loop.poll() => {
                    match result {
                        Ok(Event::Incoming(Packet::Publish(publish))) => {
                            let _ = msg_tx.send((publish.topic.clone(), publish.payload.to_vec())).await;
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
                _ = &mut shutdown_rx => {
                    break;
                }
            }
        }
    });

    // Wait for subscriptions
    tokio::time::sleep(Duration::from_millis(200)).await;
    (msg_rx, shutdown_tx)
}

/// Publish a message and wait for it to be delivered
pub async fn publish_and_wait(
    client: &AsyncClient,
    topic: &str,
    payload: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    client
        .publish(topic, QoS::AtLeastOnce, false, payload.as_bytes().to_vec())
        .await?;

    // Small delay to ensure message is processed
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok(())
}

/// Clean up test topics (publish retained messages with empty payload)
pub async fn cleanup_topics(client: &AsyncClient, topics: Vec<&str>) {
    for topic in topics {
        let _ = client.publish(topic, QoS::AtLeastOnce, true, vec![]).await;
    }
}
```

---

## 5. Topic Router (Real Implementation)

The TopicRouter is simple enough to use the real implementation in tests:

### 5.1 TopicRouter Implementation

```rust
// core/src/sources/topic_router.rs

use std::collections::HashMap;

/// Routes MQTT messages to streams based on topic patterns
#[derive(Debug)]
pub struct TopicRouter {
    subscriptions: Vec<SubscriptionConfig>,
}

impl TopicRouter {
    pub fn new(subscriptions: Vec<SubscriptionConfig>) -> Self {
        Self { subscriptions }
    }

    /// Match a topic against subscription patterns (first match wins)
    pub fn match_topic(&self, topic: &str) -> Option<&str> {
        for sub in &self.subscriptions {
            if self.pattern_matches(&sub.topic_pattern, topic) {
                return Some(&sub.stream_id);
            }
        }
        None
    }

    /// Check if an MQTT topic pattern matches a topic
    fn pattern_matches(&self, pattern: &str, topic: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let topic_parts: Vec<&str> = topic.split('/').collect();

        let mut p_idx = 0;
        let mut t_idx = 0;

        while p_idx < pattern_parts.len() && t_idx < topic_parts.len() {
            match pattern_parts[p_idx] {
                "#" => return true, // Multi-level wildcard matches rest
                "+" => {
                    // Single-level wildcard matches one non-empty level
                    if topic_parts[t_idx].is_empty() {
                        return false;
                    }
                    p_idx += 1;
                    t_idx += 1;
                }
                exact => {
                    if exact != topic_parts[t_idx] {
                        return false;
                    }
                    p_idx += 1;
                    t_idx += 1;
                }
            }
        }

        // Both should be exhausted for exact match
        p_idx == pattern_parts.len() && t_idx == topic_parts.len()
    }

    /// Get subscription config for a stream ID
    pub fn get_subscription(&self, stream_id: &str) -> Option<&SubscriptionConfig> {
        self.subscriptions.iter().find(|s| s.stream_id == stream_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let router = TopicRouter::new(vec![SubscriptionConfig {
            stream_id: "test".to_string(),
            topic_pattern: "sensors/temp/device1".to_string(),
            ..Default::default()
        }]);

        assert_eq!(router.match_topic("sensors/temp/device1"), Some("test"));
        assert_eq!(router.match_topic("sensors/temp/device2"), None);
    }

    #[test]
    fn test_single_level_wildcard() {
        let router = TopicRouter::new(vec![SubscriptionConfig {
            stream_id: "test".to_string(),
            topic_pattern: "sensors/+/data".to_string(),
            ..Default::default()
        }]);

        assert_eq!(router.match_topic("sensors/temp/data"), Some("test"));
        assert_eq!(router.match_topic("sensors/humidity/data"), Some("test"));
        assert_eq!(router.match_topic("sensors/temp/value"), None);
        assert_eq!(router.match_topic("sensors/a/b/data"), None);
    }

    #[test]
    fn test_multi_level_wildcard() {
        let router = TopicRouter::new(vec![SubscriptionConfig {
            stream_id: "test".to_string(),
            topic_pattern: "homeassistant/#".to_string(),
            ..Default::default()
        }]);

        assert_eq!(router.match_topic("homeassistant/sensor/temp/state"), Some("test"));
        assert_eq!(router.match_topic("homeassistant/a/b/c/d"), Some("test"));
        assert_eq!(router.match_topic("other/topic"), None);
    }

    #[test]
    fn test_first_match_wins() {
        let router = TopicRouter::new(vec![
            SubscriptionConfig {
                stream_id: "specific".to_string(),
                topic_pattern: "sensors/temp/+".to_string(),
                ..Default::default()
            },
            SubscriptionConfig {
                stream_id: "general".to_string(),
                topic_pattern: "sensors/+/+".to_string(),
                ..Default::default()
            },
        ]);

        assert_eq!(router.match_topic("sensors/temp/device1"), Some("specific"));
        assert_eq!(router.match_topic("sensors/humidity/device1"), Some("general"));
    }
}
```

---

## 6. Subscription Stats Tracker

### 6.1 Implementation

```rust
// core/src/sources/subscription_stats.rs

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Statistics for a single subscription
#[derive(Debug, Default)]
pub struct SubscriptionStats {
    pub message_count: AtomicU64,
    pub error_count: AtomicU64,
    pub last_message_timestamp: AtomicU64,
}

impl SubscriptionStats {
    pub fn increment_messages(&self) {
        self.message_count.fetch_add(1, Ordering::Relaxed);
        self.last_message_timestamp.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            Ordering::Relaxed,
        );
    }

    pub fn increment_errors(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_message_count(&self) -> u64 {
        self.message_count.load(Ordering::Relaxed)
    }

    pub fn get_error_count(&self) -> u64 {
        self.error_count.load(Ordering::Relaxed)
    }
}

/// Tracks statistics for all subscriptions
#[derive(Debug, Default)]
pub struct SubscriptionStatsTracker {
    stats: HashMap<String, Arc<SubscriptionStats>>,
}

impl SubscriptionStatsTracker {
    pub fn new(stream_ids: Vec<String>) -> Self {
        let stats = stream_ids
            .into_iter()
            .map(|id| (id, Arc::new(SubscriptionStats::default())))
            .collect();
        Self { stats }
    }

    pub fn get_stats(&self, stream_id: &str) -> Option<Arc<SubscriptionStats>> {
        self.stats.get(stream_id).cloned()
    }

    pub fn get_all_stats(&self) -> HashMap<String, (u64, u64)> {
        self.stats
            .iter()
            .map(|(id, stats)| {
                (id.clone(), (stats.get_message_count(), stats.get_error_count()))
            })
            .collect()
    }
}
```

---

## 7. Integration Test Fixtures

### 7.1 Test Setup Macro

```rust
// tests/integration/mqtt/mod.rs

/// Macro to set up MQTT integration test environment
#[macro_export]
macro_rules! mqtt_integration_test {
    ($name:ident, $test_fn:expr) => {
        #[tokio::test]
        #[ignore] // Requires Docker
        async fn $name() {
            use crate::integration::mqtt::test_helpers::*;

            // Ensure broker is running
            if !wait_for_broker().await {
                panic!("MQTT broker not available. Run: docker-compose -f deploy/docker-compose.test.yml up -d");
            }

            // Run the test
            $test_fn.await;
        }
    };
}

// Usage:
mqtt_integration_test!(test_my_feature, async {
    // Test implementation
});
```

### 7.2 Async Test Utilities

```rust
// tests/integration/mqtt/async_utils.rs

use std::time::Duration;
use tokio::time::timeout;

/// Wait for a condition with timeout
pub async fn wait_for<F, Fut>(
    condition: F,
    timeout_duration: Duration,
    poll_interval: Duration,
) -> bool
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = std::time::Instant::now();
    while start.elapsed() < timeout_duration {
        if condition().await {
            return true;
        }
        tokio::time::sleep(poll_interval).await;
    }
    false
}

/// Assert with timeout
pub async fn assert_eventually<F, Fut>(condition: F, timeout_secs: u64, message: &str)
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    if !wait_for(condition, Duration::from_secs(timeout_secs), Duration::from_millis(100)).await {
        panic!("Assertion timed out after {}s: {}", timeout_secs, message);
    }
}
```

---

## 8. Mock Factory Functions

### 8.1 Factory Module

```rust
// core/src/sources/test_factories.rs

#[cfg(test)]
pub mod factories {
    use super::*;

    /// Factory for creating test MqttConfig instances
    pub struct MqttConfigFactory;

    impl MqttConfigFactory {
        /// Create minimal valid config
        pub fn minimal() -> MqttConfig {
            MqttConfig {
                broker_url: "localhost".to_string(),
                port: 11883,
                subscriptions: vec![SubscriptionConfig {
                    stream_id: "test".to_string(),
                    topic_pattern: "test/+".to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }
        }

        /// Create multi-subscription config
        pub fn multi_subscription() -> MqttConfig {
            MqttConfig {
                broker_url: "localhost".to_string(),
                port: 11883,
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

        /// Create legacy single-topic config
        pub fn legacy() -> MqttConfig {
            MqttConfig {
                broker_url: "localhost".to_string(),
                port: 11883,
                topic_pattern: Some("airgradient/readings/+".to_string()),
                subscriptions: vec![],
                ..Default::default()
            }
        }

        /// Create config with custom parser settings
        pub fn with_parser(parser_config: ParserConfig) -> MqttConfig {
            MqttConfig {
                broker_url: "localhost".to_string(),
                port: 11883,
                subscriptions: vec![SubscriptionConfig {
                    stream_id: "test".to_string(),
                    topic_pattern: "test/+".to_string(),
                    parser: Some(parser_config),
                    ..Default::default()
                }],
                ..Default::default()
            }
        }
    }

    /// Factory for creating test TimeSeriesPoint instances
    pub struct PointFactory;

    impl PointFactory {
        pub fn air_quality(location_id: &str, metric: &str, value: f64) -> TimeSeriesPoint {
            TimeSeriesPoint {
                timestamp: Utc::now(),
                location_id: location_id.to_string(),
                value,
                tags: HashMap::from([
                    ("metric".to_string(), metric.to_string()),
                    ("source".to_string(), "mqtt".to_string()),
                    ("stream_id".to_string(), "air-quality".to_string()),
                ]),
            }
        }

        pub fn homeassistant(entity_id: &str, state: f64) -> TimeSeriesPoint {
            TimeSeriesPoint {
                timestamp: Utc::now(),
                location_id: entity_id.to_string(),
                value: state,
                tags: HashMap::from([
                    ("metric".to_string(), "state".to_string()),
                    ("source".to_string(), "mqtt".to_string()),
                    ("stream_id".to_string(), "homeassistant".to_string()),
                ]),
            }
        }
    }
}
```

---

## 9. Test Dependencies

### 9.1 Cargo.toml Dev Dependencies

```toml
[dev-dependencies]
# Async testing
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread", "time"] }

# Mocking
mockall = "0.12"

# HTTP mocking (for integration with HTTP sources)
wiremock = "0.6"

# Assertions
assert_matches = "1.5"

# Test utilities
uuid = { version = "1", features = ["v4"] }
tempfile = "3"

# Tracing for log capture in tests
tracing-test = "0.2"

# Property-based testing (future)
# proptest = "1"
```

---

## 10. Summary

### Mock Components

| Component | Type | Purpose |
|-----------|------|---------|
| `MockMqttClientTrait` | mockall mock | Verify subscribe/disconnect calls |
| `MockParser` | mockall mock | Verify parse calls, error handling |
| `TopicRouter` | Real impl | Simple enough for direct testing |
| `SubscriptionStats` | Real impl | Atomic counters, no external deps |

### Test Infrastructure

| Component | Implementation | Port |
|-----------|----------------|------|
| Mosquitto broker | Docker | 11883 |
| Test publisher | rumqttc | - |
| Test subscriber | rumqttc | - |

### Factory Functions

| Factory | Creates |
|---------|---------|
| `MqttConfigFactory` | MqttConfig variants |
| `PointFactory` | TimeSeriesPoint instances |

---

## References

- TEST_SCAFFOLDING.md - Test module organization
- TEST_CASES.md - Detailed test cases
- `docs/testing/AIR-005-TEST-DESIGN.md` - London School TDD patterns
- `core/src/traits.rs` - Existing mock patterns
