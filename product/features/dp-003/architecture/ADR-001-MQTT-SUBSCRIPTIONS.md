# ADR-001: MQTT Multi-Subscription Architecture

## Status

**Proposed** | 2025-12-30

## Context

The Neural Data Platform currently uses `MqttSource` to ingest data from MQTT brokers. The existing implementation supports only ONE topic pattern per source instance:

```rust
// Current MqttConfig (single topic)
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,  // <-- SINGLE pattern
    pub qos: QoS,
    // ...
}
```

This limitation prevents:

1. **HomeAssistant Integration**: Cannot add `homeassistant/+/+/state` alongside `airgradient/readings/+`
2. **Config-Driven Growth**: Adding new MQTT streams requires code changes or multiple source instances
3. **Resource Inefficiency**: Multiple MQTT connections to the same broker wastes resources

### Design Options Considered

#### Option A: Multiple Source Instances (Rejected)

Spawn separate `MqttSource` instances for each stream:

```yaml
# Each stream has its own MQTT source
sources:
  - type: mqtt
    stream_id: air-quality
    topic_pattern: "airgradient/readings/+"

  - type: mqtt
    stream_id: homeassistant
    topic_pattern: "homeassistant/+/+/state"
```

**Pros:**
- No code changes to MqttSource
- Isolation between streams

**Cons:**
- Multiple broker connections (resource waste)
- Client ID conflicts with MQTT broker
- Inconsistent with HTTP polling pattern (single source, multiple endpoints)
- Harder to coordinate reconnection/backoff

#### Option B: Single Source with Subscription Array (Selected)

Refactor `MqttSource` to accept multiple subscriptions:

```yaml
sources:
  - type: mqtt
    broker_url: "mosquitto"
    subscriptions:
      - stream_id: air-quality
        topic_pattern: "airgradient/readings/+"

      - stream_id: homeassistant
        topic_pattern: "homeassistant/+/+/state"
```

**Pros:**
- Single broker connection (resource efficient)
- Matches HTTP polling pattern (`GenericHttpPollingConfig.endpoints`)
- Centralized reconnection logic
- Config-driven stream addition

**Cons:**
- Requires MqttSource refactor
- Topic-to-stream routing complexity

#### Option C: MQTT Bridge/Router (Rejected)

Use external MQTT bridge (Mosquitto bridge) to route topics to separate brokers:

**Pros:**
- No NDP code changes

**Cons:**
- External infrastructure complexity
- Harder to maintain stream-to-topic mapping
- Additional latency

## Decision

**Implement Option B: Single Source with Subscription Array**

This decision aligns with:
1. HTTP polling pattern (already uses `endpoints: Vec<EndpointConfig>`)
2. Domain Adapter pattern (source configuration is config-driven)
3. Resource efficiency (single broker connection)

## Consequences

### Positive

1. **Config-Driven Streams**: Add new MQTT streams via YAML without code changes
2. **Resource Efficiency**: Single connection to MQTT broker
3. **Consistent Architecture**: Matches HTTP polling multi-endpoint pattern
4. **Centralized Retry Logic**: One reconnection strategy for all subscriptions

### Negative

1. **Refactoring Required**: MqttConfig and MqttSource need changes
2. **Topic Routing Complexity**: Must match incoming messages to correct stream
3. **Shared Failure Domain**: Broker disconnect affects all subscriptions

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Client ID conflicts | Use single client ID with subscription management |
| Message routing errors | Explicit topic-to-stream mapping with validation |
| Backward compatibility | Support legacy `topic_pattern` field temporarily |

## Implementation Notes

### Reference: HTTP Polling Pattern

The `GenericHttpPollingConfig` provides a proven pattern:

```rust
// From core/src/sources/http_poll.rs
pub struct GenericHttpPollingConfig {
    pub endpoints: Vec<EndpointConfig>,  // Multiple endpoints
    pub poll_interval: Duration,
    // ...
}

pub struct EndpointConfig {
    pub endpoint_id: String,
    pub url: String,
    pub location_id: String,
    pub parser_name: String,
    // ...
}
```

The MQTT implementation should mirror this:

```rust
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub subscriptions: Vec<SubscriptionConfig>,  // Multiple subscriptions
    pub qos: QoS,
    // ...
}

pub struct SubscriptionConfig {
    pub stream_id: String,
    pub topic_pattern: String,
    pub parser_name: Option<String>,
}
```

## Related Documents

- ADR-002-CONFIG-FORMAT.md: New YAML configuration structure
- ADR-003-TOPIC-ROUTING.md: Message-to-stream routing algorithm
- SYSTEM_DESIGN.md: Component diagram and data flow

## References

- SCOPE.md: Feature requirements
- core/src/sources/mqtt.rs: Current implementation
- core/src/sources/http_poll.rs: Reference pattern
