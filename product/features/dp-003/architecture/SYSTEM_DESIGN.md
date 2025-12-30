# DP-003: MQTT Multi-Subscription System Design

## Overview

This document describes the system architecture for supporting multiple MQTT subscriptions per broker connection, enabling config-driven multi-stream ingestion.

## Component Architecture

### High-Level Component Diagram

```
                                    +------------------+
                                    |   YAML Config    |
                                    | (GitOps managed) |
                                    +--------+---------+
                                             |
                                             v
+------------------+              +----------+---------+
|   Mosquitto      |              |   StreamRegistry   |
|   MQTT Broker    |              |      (etcd)        |
+--------+---------+              +----------+---------+
         |                                   |
         | MQTT Publish                      | Config Load
         |                                   |
         v                                   v
+--------+---------------------------------+---------+
|                    MqttSource                      |
|  +------------------------------------------+     |
|  |              TopicRouter                 |     |
|  |  +-------------+  +-------------+        |     |
|  |  | air-quality |  | homeassistant|       |     |
|  |  | pattern     |  | pattern      |       |     |
|  |  +------+------+  +------+-------+       |     |
|  +---------|----------------|---------------+     |
|            |                |                      |
|  +---------v-+     +--------v-------+             |
|  | FlatJson  |     | FlatJson       |             |
|  | Parser    |     | Parser         |             |
|  +-----+-----+     +-------+--------+             |
|        |                   |                       |
+--------|-------------------|----------------------+
         |                   |
         v                   v
+--------+-------------------+--------+
|          IngestionRouter           |
|  stream_id -> storage_channel map  |
+--------+-------------------+--------+
         |                   |
         v                   v
+--------+------+   +--------+-------+
| air-quality   |   | homeassistant  |
| Parquet Store |   | Parquet Store  |
+---------------+   +----------------+
```

### Data Flow Sequence

```
1. MQTT Message Arrives
   Topic: airgradient/readings/ABC123
   Payload: {"pm02": 12.5, "rco2": 450, "serialno": "ABC123"}

2. TopicRouter Matches Pattern
   Pattern: airgradient/readings/+
   Stream: air-quality

3. Parser Transforms Data
   Input: JSON payload
   Output: Vec<TimeSeriesPoint>
     - {metric: "pm02", value: 12.5, location_id: "ABC123"}
     - {metric: "rco2", value: 450, location_id: "ABC123"}

4. Points Tagged with Stream ID
   Each point gets: tags.stream_id = "air-quality"

5. IngestionRouter Routes to Storage
   Lookup: stream_id -> storage_channel
   Send to: air-quality channel

6. ParquetStore Writes
   Path: data/bronze/air-quality/2024/12/30/data.parquet
```

## Component Details

### MqttConfig (Proposed)

```rust
/// Configuration for MQTT source with multiple subscriptions
#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    // Connection settings
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub qos: u8,

    // Reconnection settings
    pub reconnect_delay_secs: u64,
    pub max_reconnect_delay_secs: u64,

    // Buffer settings
    pub buffer_capacity: usize,

    // NEW: Multiple subscriptions
    pub subscriptions: Vec<SubscriptionConfig>,

    // DEPRECATED: Legacy single pattern (backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_pattern: Option<String>,
}

/// Configuration for a single MQTT subscription
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionConfig {
    /// Stream ID for routing
    pub stream_id: String,

    /// MQTT topic pattern (supports + and # wildcards)
    pub topic_pattern: String,

    /// Parser configuration
    pub parser: Option<ParserConfig>,

    /// Enable/disable this subscription
    #[serde(default = "default_true")]
    pub enabled: bool,
}
```

### Comparison: Current vs Proposed

#### Current MqttConfig

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,       // Single pattern
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}
```

#### Proposed MqttConfig

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub subscriptions: Vec<SubscriptionConfig>,  // Multiple subscriptions
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,

    // Backward compatibility
    pub topic_pattern: Option<String>,
}
```

### TopicRouter Component

```rust
/// Routes MQTT topics to streams based on pattern matching
pub struct TopicRouter {
    routes: Vec<RouteEntry>,
}

struct RouteEntry {
    pattern: String,          // Original MQTT pattern
    regex: Regex,             // Compiled regex
    stream_id: String,        // Target stream
    parser: Arc<dyn Parser>,  // Parser for this stream
}

impl TopicRouter {
    /// Create router from subscriptions
    pub fn new(subs: Vec<SubscriptionConfig>) -> Result<Self>;

    /// Find matching route for topic
    pub fn route(&self, topic: &str) -> Option<&RouteEntry>;

    /// Get all subscribed topic patterns
    pub fn topic_patterns(&self) -> Vec<&str>;
}
```

### MqttSource Changes

```rust
pub struct MqttSource {
    config: MqttConfig,
    router: TopicRouter,              // NEW: Topic routing
    client: Option<AsyncClient>,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
    dead_letter_tx: mpsc::Sender<DeadLetterItem>,  // NEW: Dead letters
}

impl MqttSource {
    /// Create with multiple subscriptions
    pub fn new(config: MqttConfig) -> Result<Self> {
        let router = TopicRouter::new(config.get_subscriptions())?;
        // ...
    }

    /// Subscribe to all configured topics
    async fn subscribe_all(&self, client: &AsyncClient) -> Result<()> {
        for pattern in self.router.topic_patterns() {
            client.subscribe(pattern, self.config.qos).await?;
        }
        Ok(())
    }

    /// Process incoming message with routing
    async fn process_message(&self, topic: &str, payload: &[u8]) -> Result<()> {
        match self.router.route(topic) {
            Some(route) => {
                let points = route.parser.parse(payload)?;
                // Tag with stream_id and send
            }
            None => {
                // Send to dead letter
            }
        }
    }
}
```

## Error Handling

### Dead Letter Queue

Messages that don't match any subscription pattern are sent to a dead letter queue:

```rust
pub struct DeadLetterItem {
    pub topic: String,
    pub payload: Vec<u8>,
    pub error: String,
    pub timestamp: DateTime<Utc>,
}
```

Dead letters are logged and can be monitored:

```
WARN Dead letter: topic=unknown/topic, error=No matching subscription
```

### Reconnection Handling

```rust
impl MqttSource {
    async fn handle_disconnect(&self) {
        // Exponential backoff
        let delay = calculate_backoff(self.reconnect_attempt);

        // Reconnect
        let (client, event_loop) = create_connection(&self.config)?;

        // Resubscribe to ALL patterns
        self.subscribe_all(&client).await?;

        // Continue processing
    }
}
```

## Configuration Examples

### Single Stream (Current Pattern)

```yaml
# config/base/streams/air-quality/config.yaml
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "air-quality-app"
      topic_pattern: "airgradient/readings/+"  # Legacy single pattern
      qos: 1
```

### Multiple Streams (New Pattern)

```yaml
# config/base/mqtt-sources.yaml (shared config)
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "ndp-mqtt-shared"
      qos: 1
      reconnect_delay_secs: 1
      max_reconnect_delay_secs: 30
      buffer_capacity: 2000

      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
          enabled: true
          parser:
            parser_type: flat_json
            location_id_field: serialno
            skip_fields: [serialno, firmware, model, ledMode]
            default_tags:
              source: mqtt

        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
          enabled: true
          parser:
            parser_type: flat_json
            default_tags:
              source: mqtt

        - stream_id: hvac
          topic_pattern: "homeassistant/climate/#"
          enabled: false  # Not yet implemented
```

## Backward Compatibility

### Strategy

1. **Legacy Field Support**: `topic_pattern` field still works
2. **Automatic Conversion**: Legacy config converted to subscription
3. **Deprecation Warning**: Log warning when legacy field used

### Implementation

```rust
impl MqttConfig {
    pub fn get_subscriptions(&self) -> Vec<SubscriptionConfig> {
        let mut subs = self.subscriptions.clone();

        // Support legacy single topic_pattern
        if let Some(pattern) = &self.topic_pattern {
            tracing::warn!(
                "DEPRECATED: topic_pattern field is deprecated, use subscriptions array"
            );

            if !subs.iter().any(|s| s.topic_pattern == *pattern) {
                subs.push(SubscriptionConfig {
                    stream_id: "legacy".to_string(),
                    topic_pattern: pattern.clone(),
                    parser: None,
                    enabled: true,
                });
            }
        }

        subs
    }
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_pattern_to_regex() {
        // Test single-level wildcard
        let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();
        assert!(regex.is_match("sensors/room1/temp"));
        assert!(!regex.is_match("sensors/temp"));
    }

    #[test]
    fn test_router_matches_correct_stream() {
        let router = TopicRouter::new(vec![
            SubscriptionConfig {
                stream_id: "air-quality".to_string(),
                topic_pattern: "airgradient/readings/+".to_string(),
                parser: None,
                enabled: true,
            },
        ]).unwrap();

        let route = router.route("airgradient/readings/ABC123").unwrap();
        assert_eq!(route.stream_id, "air-quality");
    }

    #[test]
    fn test_no_match_returns_none() {
        let router = TopicRouter::new(vec![
            SubscriptionConfig {
                stream_id: "test".to_string(),
                topic_pattern: "test/+".to_string(),
                parser: None,
                enabled: true,
            },
        ]).unwrap();

        assert!(router.route("unknown/topic").is_none());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_multi_subscription_routing() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig {
                stream_id: "air-quality".to_string(),
                topic_pattern: "airgradient/readings/+".to_string(),
                parser: None,
                enabled: true,
            },
            SubscriptionConfig {
                stream_id: "homeassistant".to_string(),
                topic_pattern: "homeassistant/+/+/state".to_string(),
                parser: None,
                enabled: true,
            },
        ],
        // ...
    };

    let source = MqttSource::new(config).unwrap();

    // Verify both subscriptions are active
    let patterns = source.router.topic_patterns();
    assert!(patterns.contains(&"airgradient/readings/+"));
    assert!(patterns.contains(&"homeassistant/+/+/state"));
}
```

## Deployment Considerations

### Rolling Update Strategy

1. **Phase 1**: Deploy new code with backward compatibility
2. **Phase 2**: Update configs to use `subscriptions` array
3. **Phase 3**: Remove deprecated `topic_pattern` (future release)

### Monitoring

Add metrics for:
- Messages per stream
- Dead letter rate
- Routing latency
- Pattern match performance

```rust
// Example metrics
mqtt_messages_routed{stream_id="air-quality"} 1234
mqtt_messages_dead_letter 5
mqtt_routing_latency_ms{quantile="0.99"} 0.5
```

## Related Documents

- ADR-001-MQTT-SUBSCRIPTIONS.md: Architecture decision
- ADR-002-CONFIG-FORMAT.md: Configuration format
- ADR-003-TOPIC-ROUTING.md: Routing algorithm
- core/src/sources/mqtt.rs: Current implementation
- core/src/sources/http_poll.rs: Reference pattern
