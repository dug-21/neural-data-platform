# ADR-003: MQTT Topic-to-Stream Routing Algorithm

## Status

**Proposed** | 2025-12-30

## Context

With multiple subscriptions on a single MQTT connection, incoming messages must be routed to the correct stream based on the topic they arrive on. This requires:

1. **Topic Pattern Matching**: Determine which subscription matches the incoming topic
2. **Stream Selection**: Route to the correct stream_id for storage
3. **Parser Selection**: Use the correct parser for that stream
4. **Error Handling**: Handle messages that match no or multiple subscriptions

### MQTT Topic Pattern Syntax

MQTT supports two wildcards:

| Wildcard | Description | Example |
|----------|-------------|---------|
| `+` | Single-level wildcard | `sensors/+/temp` matches `sensors/room1/temp` |
| `#` | Multi-level wildcard | `sensors/#` matches `sensors/room1/temp` and `sensors/room1/humidity` |

### Example Scenario

```yaml
subscriptions:
  - stream_id: air-quality
    topic_pattern: "airgradient/readings/+"

  - stream_id: homeassistant
    topic_pattern: "homeassistant/+/+/state"

  - stream_id: hvac
    topic_pattern: "homeassistant/climate/#"
```

Incoming messages:

| Topic | Expected Stream |
|-------|-----------------|
| `airgradient/readings/ABC123` | air-quality |
| `homeassistant/sensor/temp/state` | homeassistant |
| `homeassistant/climate/living_room/temperature` | hvac |
| `unknown/topic` | (no match - dead letter) |

### Design Considerations

1. **Pattern Overlap**: `homeassistant/#` would match `homeassistant/sensor/temp/state`
2. **Matching Priority**: Most specific pattern should win
3. **Performance**: Routing happens for every message (must be fast)
4. **Dead Letters**: Messages matching no pattern need handling

## Decision

### Routing Strategy: Explicit Pattern Matching with Priority

Implement a topic router that:
1. Converts MQTT patterns to regex at config load time
2. Matches incoming topics against patterns in order
3. Uses first match (order matters for overlapping patterns)
4. Sends unmatched messages to dead letter queue

### Implementation

#### TopicRouter Structure

```rust
use regex::Regex;

/// Routes MQTT topics to streams based on pattern matching
pub struct TopicRouter {
    /// Compiled patterns in priority order
    routes: Vec<RouteEntry>,
}

struct RouteEntry {
    /// Original MQTT pattern (for logging)
    pattern: String,
    /// Compiled regex for matching
    regex: Regex,
    /// Target stream
    stream_id: String,
    /// Parser for this stream
    parser: Arc<dyn Parser + Send + Sync>,
}

impl TopicRouter {
    /// Create router from subscription configs
    pub fn new(subscriptions: Vec<SubscriptionConfig>) -> Result<Self, RouterError> {
        let routes = subscriptions
            .into_iter()
            .filter(|s| s.enabled)
            .map(|sub| {
                let regex = mqtt_pattern_to_regex(&sub.topic_pattern)?;
                Ok(RouteEntry {
                    pattern: sub.topic_pattern.clone(),
                    regex,
                    stream_id: sub.stream_id.clone(),
                    parser: create_parser(&sub.parser)?,
                })
            })
            .collect::<Result<Vec<_>, RouterError>>()?;

        Ok(Self { routes })
    }

    /// Route a topic to its stream
    pub fn route(&self, topic: &str) -> Option<&RouteEntry> {
        self.routes.iter().find(|r| r.regex.is_match(topic))
    }
}
```

#### Pattern-to-Regex Conversion

```rust
/// Convert MQTT topic pattern to regex
///
/// - `+` becomes `[^/]+` (single level)
/// - `#` becomes `.*` (multi-level, must be at end)
fn mqtt_pattern_to_regex(pattern: &str) -> Result<Regex, RouterError> {
    // Validate pattern
    if pattern.contains('#') && !pattern.ends_with('#') && !pattern.ends_with("/#") {
        return Err(RouterError::InvalidPattern(
            format!("# wildcard must be at end of pattern: {}", pattern)
        ));
    }

    let mut regex_str = String::from("^");

    for (i, segment) in pattern.split('/').enumerate() {
        if i > 0 {
            regex_str.push('/');
        }

        match segment {
            "+" => regex_str.push_str("[^/]+"),
            "#" => {
                regex_str.push_str(".*");
                break; // # matches rest of topic
            }
            _ => {
                // Escape regex special characters
                regex_str.push_str(&regex::escape(segment));
            }
        }
    }

    regex_str.push('$');

    Regex::new(&regex_str)
        .map_err(|e| RouterError::RegexError(e.to_string()))
}
```

#### Pattern Matching Examples

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_level_wildcard() {
        let regex = mqtt_pattern_to_regex("sensors/+/temperature").unwrap();

        assert!(regex.is_match("sensors/room1/temperature"));
        assert!(regex.is_match("sensors/kitchen/temperature"));
        assert!(!regex.is_match("sensors/temperature")); // missing level
        assert!(!regex.is_match("sensors/room1/sub/temperature")); // too many levels
    }

    #[test]
    fn test_multi_level_wildcard() {
        let regex = mqtt_pattern_to_regex("homeassistant/#").unwrap();

        assert!(regex.is_match("homeassistant/sensor/temp"));
        assert!(regex.is_match("homeassistant/binary_sensor/door/state"));
        assert!(regex.is_match("homeassistant")); // # can match zero levels
        assert!(!regex.is_match("other/topic"));
    }

    #[test]
    fn test_airgradient_pattern() {
        let regex = mqtt_pattern_to_regex("airgradient/readings/+").unwrap();

        assert!(regex.is_match("airgradient/readings/ABC123"));
        assert!(regex.is_match("airgradient/readings/sensor_01"));
        assert!(!regex.is_match("airgradient/readings")); // missing serial
        assert!(!regex.is_match("airgradient/readings/a/b")); // too many
    }

    #[test]
    fn test_homeassistant_state_pattern() {
        let regex = mqtt_pattern_to_regex("homeassistant/+/+/state").unwrap();

        assert!(regex.is_match("homeassistant/sensor/temperature/state"));
        assert!(regex.is_match("homeassistant/binary_sensor/door/state"));
        assert!(!regex.is_match("homeassistant/sensor/state")); // missing level
        assert!(!regex.is_match("homeassistant/sensor/room/temp/state")); // too many
    }
}
```

### Message Processing Flow

```rust
impl MqttSource {
    /// Process incoming MQTT message
    async fn process_message(
        &self,
        topic: &str,
        payload: &[u8],
        router: &TopicRouter,
    ) -> Result<(), ProcessError> {
        // 1. Route to stream
        let route = match router.route(topic) {
            Some(r) => r,
            None => {
                // Send to dead letter queue
                self.dead_letter_tx.send(DeadLetterItem {
                    topic: topic.to_string(),
                    payload: payload.to_vec(),
                    error: "No matching subscription".to_string(),
                    timestamp: Utc::now(),
                }).await?;
                return Ok(());
            }
        };

        // 2. Parse payload
        let json: Value = serde_json::from_slice(payload)?;
        let timestamp = Utc::now();
        let points = route.parser.parse(&json, timestamp)?;

        // 3. Tag with stream_id
        let tagged_points: Vec<TimeSeriesPoint> = points
            .into_iter()
            .map(|mut p| {
                p.tags.insert("stream_id".to_string(), route.stream_id.clone());
                p.tags.insert("topic".to_string(), topic.to_string());
                p
            })
            .collect();

        // 4. Send to router for storage
        for point in tagged_points {
            self.ingestion_tx.send(point).await?;
        }

        Ok(())
    }
}
```

### Priority and Overlap Resolution

**Rule: First Match Wins**

Subscriptions are matched in the order they appear in the configuration. For overlapping patterns, place more specific patterns first:

```yaml
subscriptions:
  # More specific pattern first
  - stream_id: hvac
    topic_pattern: "homeassistant/climate/+/state"

  # Less specific pattern second
  - stream_id: homeassistant
    topic_pattern: "homeassistant/+/+/state"
```

With this order:
- `homeassistant/climate/living_room/state` -> hvac (first match)
- `homeassistant/sensor/temp/state` -> homeassistant (doesn't match climate)

## Consequences

### Positive

1. **Fast Matching**: Regex compiled once at config load
2. **Deterministic**: First-match rule is predictable
3. **Flexible**: Supports all MQTT wildcard patterns
4. **Observable**: Dead letter queue captures routing failures

### Negative

1. **Order Sensitivity**: Config order affects routing
2. **No Multi-Routing**: Message goes to exactly one stream
3. **Pattern Validation**: Invalid patterns fail at startup

### Error Handling

| Scenario | Handling |
|----------|----------|
| No pattern matches | Dead letter queue |
| Parse failure | Log error, skip message |
| Storage failure | Retry with backoff |
| Invalid pattern | Fail at startup |

## Performance Considerations

### Benchmarks (Expected)

| Operation | Expected Time |
|-----------|---------------|
| Regex match (single pattern) | < 1 microsecond |
| Route lookup (10 patterns) | < 10 microseconds |
| Full message processing | < 1 millisecond |

### Optimization Strategies

1. **Pre-compiled Regex**: Build once at startup
2. **Pattern Ordering**: Put high-volume patterns first
3. **Batch Processing**: Process multiple messages before routing

## Related Documents

- ADR-001-MQTT-SUBSCRIPTIONS.md: Architecture decision
- ADR-002-CONFIG-FORMAT.md: Configuration format
- SYSTEM_DESIGN.md: Component diagram

## References

- MQTT 3.1.1 Specification: Topic wildcards
- Rust regex crate: Pattern matching performance
