# ADR-002: MQTT Multi-Subscription Configuration Format

## Status

**Proposed** | 2025-12-30

## Context

With the decision to support multiple subscriptions per MQTT source (ADR-001), we need to define the configuration format that:

1. Supports multiple topic patterns with stream routing
2. Maintains backward compatibility with existing configs
3. Follows NDP configuration conventions
4. Enables config-driven stream addition

### Current Configuration Format

**Rust Struct (core/src/sources/mqtt.rs):**

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,        // Single pattern
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}
```

**YAML Config (config/base/streams/air-quality/config.yaml):**

```yaml
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "air-quality-app"
      topic_pattern: "airgradient/readings/+"  # Single pattern
      qos: 1
      # ...
    parser:
      parser_type: flat_json
      location_id_field: serialno
```

### Design Considerations

1. **Stream Isolation**: Each subscription should route to a specific stream
2. **Parser Configuration**: Different streams may need different parsers
3. **Backward Compatibility**: Existing configs with `topic_pattern` should continue working
4. **Validation**: Config should be validated at load time

## Decision

### New Configuration Format

#### Rust Structs

```rust
/// Configuration for a single MQTT subscription
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionConfig {
    /// Stream ID for routing (e.g., "air-quality", "homeassistant")
    pub stream_id: String,

    /// MQTT topic pattern with wildcards (e.g., "airgradient/readings/+")
    pub topic_pattern: String,

    /// Parser configuration (optional, uses default if not specified)
    pub parser: Option<ParserConfig>,

    /// Whether this subscription is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Configuration for MQTT source with multiple subscriptions
#[derive(Debug, Clone, Deserialize)]
pub struct MqttConfig {
    /// MQTT broker hostname or IP
    pub broker_url: String,

    /// MQTT broker port (default: 1883)
    #[serde(default = "default_port")]
    pub port: u16,

    /// MQTT client ID (must be unique per broker)
    pub client_id: String,

    /// QoS level (0, 1, or 2)
    #[serde(default = "default_qos")]
    pub qos: u8,

    /// Reconnection delay in seconds
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay_secs: u64,

    /// Maximum reconnection delay in seconds
    #[serde(default = "default_max_reconnect_delay")]
    pub max_reconnect_delay_secs: u64,

    /// Internal buffer capacity
    #[serde(default = "default_buffer_capacity")]
    pub buffer_capacity: usize,

    /// Multiple subscriptions (NEW)
    #[serde(default)]
    pub subscriptions: Vec<SubscriptionConfig>,

    /// Legacy single topic pattern (DEPRECATED, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_pattern: Option<String>,
}
```

#### YAML Configuration

**New Format (config/base/streams/air-quality/config.yaml):**

```yaml
# Air Quality Stream - MQTT source with subscriptions array
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "ndp-mqtt-client"
      qos: 1
      reconnect_delay_secs: 1
      max_reconnect_delay_secs: 30
      buffer_capacity: 1000

      # NEW: Multiple subscriptions
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
          enabled: true
          parser:
            parser_type: flat_json
            location_id_field: serialno
            skip_fields:
              - serialno
              - firmware
              - model
              - ledMode
            default_tags:
              source: mqtt
              stream_id: air-quality
```

**Shared Broker Config (config/base/mqtt-shared.yaml):**

```yaml
# Shared MQTT broker configuration
# Referenced by multiple stream configs
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
              stream_id: air-quality

        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
          enabled: false  # Enable when HA integration is ready
          parser:
            parser_type: flat_json
            default_tags:
              source: mqtt
              stream_id: homeassistant
```

### Backward Compatibility Strategy

```rust
impl MqttConfig {
    /// Get all subscriptions (including legacy topic_pattern)
    pub fn get_subscriptions(&self) -> Vec<SubscriptionConfig> {
        let mut subs = self.subscriptions.clone();

        // Support legacy single topic_pattern
        if let Some(pattern) = &self.topic_pattern {
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

## Consequences

### Positive

1. **Clear Stream Routing**: Each subscription explicitly maps to a stream_id
2. **Per-Subscription Parsers**: Different streams can use different parsers
3. **Enable/Disable Granularity**: Individual subscriptions can be toggled
4. **Backward Compatible**: Legacy `topic_pattern` still works

### Negative

1. **Config Complexity**: More configuration options to understand
2. **Validation Required**: Must validate stream_id uniqueness, topic patterns

### Validation Rules

```rust
impl MqttConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let subs = self.get_subscriptions();

        // Rule 1: At least one subscription required
        if subs.is_empty() {
            return Err(ConfigError::NoSubscriptions);
        }

        // Rule 2: Stream IDs must be unique
        let mut seen = HashSet::new();
        for sub in &subs {
            if !seen.insert(&sub.stream_id) {
                return Err(ConfigError::DuplicateStreamId(sub.stream_id.clone()));
            }
        }

        // Rule 3: Topic patterns must be valid
        for sub in &subs {
            validate_topic_pattern(&sub.topic_pattern)?;
        }

        Ok(())
    }
}
```

## Migration Path

### Phase 1: Add Subscriptions Support (Non-Breaking)

- Add `subscriptions` field to MqttConfig
- Keep `topic_pattern` for backward compatibility
- `get_subscriptions()` merges both

### Phase 2: Migrate Existing Configs

- Update stream YAML files to use `subscriptions` array
- Test with both old and new format
- Update documentation

### Phase 3: Deprecate Legacy Field (Future)

- Log deprecation warning when `topic_pattern` is used
- Remove in future major version

## Related Documents

- ADR-001-MQTT-SUBSCRIPTIONS.md: Decision to use subscription array
- ADR-003-TOPIC-ROUTING.md: How to route messages
- HTTP polling reference: core/src/sources/http_poll.rs

## References

- Current config: config/base/streams/air-quality/config.yaml
- HomeAssistant config: config/base/streams/homeassistant/config.yaml
