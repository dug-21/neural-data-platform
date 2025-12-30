# DP-003: TDD Sequence - MQTT Multi-Subscription Support

## Overview

This document defines the Red-Green-Refactor test sequences for each component. Follow this sequence to implement DP-003 using test-driven development.

**TDD Approach**: London School (Outside-In)
- Start with high-level behavior tests
- Mock dependencies when needed
- Drive implementation from failing tests

---

## Phase 1: SubscriptionConfig

### Test Sequence 1.1: Basic Struct Creation

**Complexity**: S (Small)

#### Red (Write Failing Test)

```rust
#[test]
fn test_subscription_config_new_creates_valid_struct() {
    let sub = SubscriptionConfig::new("air-quality", "airgradient/readings/+");

    assert_eq!(sub.stream_id, "air-quality");
    assert_eq!(sub.topic_pattern, "airgradient/readings/+");
    assert!(sub.enabled);
    assert!(sub.parser.is_none());
}
```

#### Green (Minimal Implementation)

```rust
pub struct SubscriptionConfig {
    pub stream_id: String,
    pub topic_pattern: String,
    pub parser: Option<ParserConfig>,
    pub enabled: bool,
}

impl SubscriptionConfig {
    pub fn new(stream_id: impl Into<String>, topic_pattern: impl Into<String>) -> Self {
        Self {
            stream_id: stream_id.into(),
            topic_pattern: topic_pattern.into(),
            parser: None,
            enabled: true,
        }
    }
}
```

#### Refactor

- Add `#[derive(Debug, Clone, PartialEq)]`
- Add rustdoc comments

---

### Test Sequence 1.2: Default Trait

**Complexity**: S

#### Red

```rust
#[test]
fn test_subscription_config_default_has_enabled_true() {
    let sub = SubscriptionConfig::default();

    assert!(sub.enabled);
    assert!(sub.stream_id.is_empty());
    assert!(sub.topic_pattern.is_empty());
}
```

#### Green

```rust
impl Default for SubscriptionConfig {
    fn default() -> Self {
        Self {
            stream_id: String::new(),
            topic_pattern: String::new(),
            parser: None,
            enabled: true,
        }
    }
}
```

---

### Test Sequence 1.3: Builder Methods

**Complexity**: S

#### Red

```rust
#[test]
fn test_subscription_config_with_enabled_sets_value() {
    let sub = SubscriptionConfig::new("test", "test/+").with_enabled(false);

    assert!(!sub.enabled);
}

#[test]
fn test_subscription_config_with_parser_sets_config() {
    let parser = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "id".to_string(),
        ..Default::default()
    };
    let sub = SubscriptionConfig::new("test", "test/+").with_parser(parser.clone());

    assert_eq!(sub.parser, Some(parser));
}
```

#### Green

```rust
impl SubscriptionConfig {
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn with_parser(mut self, parser: ParserConfig) -> Self {
        self.parser = Some(parser);
        self
    }
}
```

---

### Test Sequence 1.4: Serde Deserialization

**Complexity**: S

#### Red

```rust
#[test]
fn test_subscription_serde_deserialize_minimal() {
    let yaml = r#"
stream_id: test
topic_pattern: "test/+"
"#;
    let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(sub.stream_id, "test");
    assert_eq!(sub.topic_pattern, "test/+");
    assert!(sub.enabled); // Default true
}

#[test]
fn test_subscription_serde_deserialize_with_enabled_false() {
    let yaml = r#"
stream_id: test
topic_pattern: "test/+"
enabled: false
"#;
    let sub: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

    assert!(!sub.enabled);
}
```

#### Green

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubscriptionConfig {
    pub stream_id: String,
    pub topic_pattern: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserConfig>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}
```

---

### Test Sequence 1.5: Validation

**Complexity**: S

#### Red

```rust
#[test]
fn test_validate_empty_stream_id_error() {
    let sub = SubscriptionConfig {
        stream_id: "".to_string(),
        topic_pattern: "test/+".to_string(),
        parser: None,
        enabled: true,
    };

    assert!(matches!(sub.validate(), Err(SubscriptionError::EmptyStreamId)));
}

#[test]
fn test_validate_empty_topic_pattern_error() {
    let sub = SubscriptionConfig {
        stream_id: "test".to_string(),
        topic_pattern: "".to_string(),
        parser: None,
        enabled: true,
    };

    assert!(matches!(sub.validate(), Err(SubscriptionError::EmptyTopicPattern)));
}

#[test]
fn test_validate_valid_subscription_ok() {
    let sub = SubscriptionConfig::new("test", "test/+");

    assert!(sub.validate().is_ok());
}
```

#### Green

```rust
pub fn validate(&self) -> Result<(), SubscriptionError> {
    if self.stream_id.is_empty() {
        return Err(SubscriptionError::EmptyStreamId);
    }
    if self.topic_pattern.is_empty() {
        return Err(SubscriptionError::EmptyTopicPattern);
    }
    Ok(())
}
```

---

## Phase 2: TopicRouter

### Test Sequence 2.1: Pattern to Regex - Literal

**Complexity**: S

#### Red

```rust
#[test]
fn test_pattern_literal_exact_match() {
    let regex = mqtt_pattern_to_regex("sensors/room1/temp").unwrap();

    assert!(regex.is_match("sensors/room1/temp"));
    assert!(!regex.is_match("sensors/room2/temp"));
    assert!(!regex.is_match("sensors/room1/humidity"));
}
```

#### Green

```rust
pub fn mqtt_pattern_to_regex(pattern: &str) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("pattern cannot be empty".to_string());
    }

    let escaped = regex::escape(pattern);
    let regex_str = format!("^{}$", escaped);

    Regex::new(&regex_str).map_err(|e| e.to_string())
}
```

---

### Test Sequence 2.2: Pattern to Regex - Single Level Wildcard

**Complexity**: M

#### Red

```rust
#[test]
fn test_pattern_single_level_wildcard_matches() {
    let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();

    assert!(regex.is_match("sensors/room1/temp"));
    assert!(regex.is_match("sensors/kitchen/temp"));
}

#[test]
fn test_pattern_single_level_wildcard_requires_segment() {
    let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();

    assert!(!regex.is_match("sensors/temp")); // Missing level
    assert!(!regex.is_match("sensors//temp")); // Empty level
}

#[test]
fn test_pattern_single_level_wildcard_single_level_only() {
    let regex = mqtt_pattern_to_regex("sensors/+/temp").unwrap();

    assert!(!regex.is_match("sensors/room1/sub/temp")); // Too many levels
}
```

#### Green

```rust
pub fn mqtt_pattern_to_regex(pattern: &str) -> Result<Regex, String> {
    if pattern.is_empty() {
        return Err("pattern cannot be empty".to_string());
    }

    let mut regex_str = String::from("^");
    let segments: Vec<&str> = pattern.split('/').collect();

    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            regex_str.push('/');
        }

        match *segment {
            "+" => regex_str.push_str("[^/]+"),
            _ => regex_str.push_str(&regex::escape(segment)),
        }
    }

    regex_str.push('$');
    Regex::new(&regex_str).map_err(|e| e.to_string())
}
```

---

### Test Sequence 2.3: Pattern to Regex - Multi Level Wildcard

**Complexity**: M

#### Red

```rust
#[test]
fn test_pattern_multi_level_wildcard_matches_zero_levels() {
    let regex = mqtt_pattern_to_regex("sensors/#").unwrap();

    assert!(regex.is_match("sensors/"));
}

#[test]
fn test_pattern_multi_level_wildcard_matches_one_level() {
    let regex = mqtt_pattern_to_regex("sensors/#").unwrap();

    assert!(regex.is_match("sensors/room1"));
}

#[test]
fn test_pattern_multi_level_wildcard_matches_many_levels() {
    let regex = mqtt_pattern_to_regex("sensors/#").unwrap();

    assert!(regex.is_match("sensors/room1/temp"));
    assert!(regex.is_match("sensors/room1/sub/deep/temp"));
}

#[test]
fn test_pattern_multi_level_wildcard_no_match_different_prefix() {
    let regex = mqtt_pattern_to_regex("sensors/#").unwrap();

    assert!(!regex.is_match("other/room1/temp"));
}
```

#### Green

```rust
// Update mqtt_pattern_to_regex
match *segment {
    "+" => regex_str.push_str("[^/]+"),
    "#" => {
        regex_str.push_str(".*");
        break; // # matches rest
    }
    _ => regex_str.push_str(&regex::escape(segment)),
}
```

---

### Test Sequence 2.4: Pattern to Regex - Combined Wildcards

**Complexity**: M

#### Red

```rust
#[test]
fn test_pattern_combined_wildcards() {
    let regex = mqtt_pattern_to_regex("home/+/devices/#").unwrap();

    assert!(regex.is_match("home/room1/devices/"));
    assert!(regex.is_match("home/room1/devices/light"));
    assert!(regex.is_match("home/room1/devices/light/status"));
    assert!(!regex.is_match("home/devices/light")); // Missing single level
    assert!(!regex.is_match("home/room1/room2/devices/light")); // Extra level before devices
}
```

#### Green

(Already handled by previous implementation)

---

### Test Sequence 2.5: Pattern Validation

**Complexity**: S

#### Red

```rust
#[test]
fn test_pattern_empty_error() {
    let result = mqtt_pattern_to_regex("");
    assert!(result.is_err());
}

#[test]
fn test_pattern_starts_with_slash_error() {
    let result = validate_topic_pattern("/sensors/temp");
    assert!(result.is_err());
}

#[test]
fn test_pattern_hash_not_at_end_error() {
    let result = validate_topic_pattern("sensors/#/temp");
    assert!(result.is_err());
}

#[test]
fn test_pattern_multiple_hash_error() {
    let result = validate_topic_pattern("sensors/#/#");
    assert!(result.is_err());
}

#[test]
fn test_pattern_plus_mixed_segment_error() {
    let result = validate_topic_pattern("sensors/room+/temp");
    assert!(result.is_err());
}
```

#### Green

```rust
fn validate_topic_pattern(pattern: &str) -> Result<(), SubscriptionError> {
    if pattern.starts_with('/') {
        return Err(SubscriptionError::InvalidTopicPattern(
            "pattern cannot start with /".to_string(),
        ));
    }
    // ... more validation as in FILE_CHANGES.md
}
```

---

### Test Sequence 2.6: Router Creation

**Complexity**: M

#### Red

```rust
#[test]
fn test_router_new_single_subscription() {
    let subs = vec![
        SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    assert_eq!(router.route_count(), 1);
}

#[test]
fn test_router_new_multiple_subscriptions() {
    let subs = vec![
        SubscriptionConfig::new("a", "topic/a/+"),
        SubscriptionConfig::new("b", "topic/b/#"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    assert_eq!(router.route_count(), 2);
}

#[test]
fn test_router_new_skips_disabled() {
    let subs = vec![
        SubscriptionConfig::new("disabled", "test/+").with_enabled(false),
        SubscriptionConfig::new("enabled", "test/#"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    assert_eq!(router.route_count(), 1);
}

#[test]
fn test_router_new_empty_error() {
    let subs: Vec<SubscriptionConfig> = vec![];
    let result = TopicRouter::new(subs);

    assert!(matches!(result, Err(RouterError::NoEnabledSubscriptions)));
}
```

#### Green

```rust
impl TopicRouter {
    pub fn new(subscriptions: Vec<SubscriptionConfig>) -> Result<Self, RouterError> {
        let mut routes = Vec::new();

        for sub in subscriptions {
            if !sub.enabled {
                continue;
            }

            let regex = mqtt_pattern_to_regex(&sub.topic_pattern)
                .map_err(|e| RouterError::InvalidPattern {
                    pattern: sub.topic_pattern.clone(),
                    error: e,
                })?;

            routes.push(RouteEntry {
                pattern: sub.topic_pattern,
                regex,
                stream_id: sub.stream_id,
                parser_config: sub.parser,
                enabled: true,
            });
        }

        if routes.is_empty() {
            return Err(RouterError::NoEnabledSubscriptions);
        }

        Ok(Self { routes })
    }
}
```

---

### Test Sequence 2.7: Router Matching

**Complexity**: M

#### Red

```rust
#[test]
fn test_router_route_returns_matching_stream() {
    let subs = vec![
        SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    let route = router.route("airgradient/readings/ABC123");

    assert!(route.is_some());
    assert_eq!(route.unwrap().stream_id, "air-quality");
}

#[test]
fn test_router_route_returns_none_for_no_match() {
    let subs = vec![
        SubscriptionConfig::new("test", "test/+"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    let route = router.route("unknown/topic");

    assert!(route.is_none());
}

#[test]
fn test_router_route_first_match_wins() {
    let subs = vec![
        SubscriptionConfig::new("specific", "homeassistant/climate/+/state"),
        SubscriptionConfig::new("general", "homeassistant/+/+/state"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    // Climate should match first (specific)
    let route = router.route("homeassistant/climate/living_room/state");
    assert_eq!(route.unwrap().stream_id, "specific");

    // Sensor should match second (general)
    let route = router.route("homeassistant/sensor/temp/state");
    assert_eq!(route.unwrap().stream_id, "general");
}

#[test]
fn test_router_route_empty_topic_returns_none() {
    let subs = vec![
        SubscriptionConfig::new("test", "test/+"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    let route = router.route("");

    assert!(route.is_none());
}
```

#### Green

```rust
impl TopicRouter {
    pub fn route(&self, topic: &str) -> Option<&RouteEntry> {
        if topic.is_empty() {
            return None;
        }

        for route in &self.routes {
            if route.regex.is_match(topic) {
                return Some(route);
            }
        }

        None
    }
}
```

---

### Test Sequence 2.8: Topic Patterns Getter

**Complexity**: S

#### Red

```rust
#[test]
fn test_router_topic_patterns_returns_all() {
    let subs = vec![
        SubscriptionConfig::new("a", "topic/a/+"),
        SubscriptionConfig::new("b", "topic/b/#"),
    ];
    let router = TopicRouter::new(subs).unwrap();

    let patterns = router.topic_patterns();

    assert_eq!(patterns.len(), 2);
    assert!(patterns.contains(&"topic/a/+"));
    assert!(patterns.contains(&"topic/b/#"));
}
```

#### Green

```rust
pub fn topic_patterns(&self) -> Vec<&str> {
    self.routes.iter().map(|r| r.pattern.as_str()).collect()
}
```

---

## Phase 3: MqttConfig Refactor

### Test Sequence 3.1: Subscriptions Field

**Complexity**: S

#### Red

```rust
#[test]
fn test_mqtt_config_with_subscriptions_array() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("air", "air/+"),
        ],
        topic_pattern: None,
        ..Default::default()
    };

    let subs = config.get_subscriptions();

    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].stream_id, "air");
}
```

#### Green

Add `subscriptions: Vec<SubscriptionConfig>` field to MqttConfig.

---

### Test Sequence 3.2: Backward Compatibility - Legacy topic_pattern

**Complexity**: M

#### Red

```rust
#[test]
fn test_mqtt_config_legacy_topic_pattern_converted() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![],
        topic_pattern: Some("legacy/topic/+".to_string()),
        ..Default::default()
    };

    let subs = config.get_subscriptions();

    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].stream_id, "legacy");
    assert_eq!(subs[0].topic_pattern, "legacy/topic/+");
}
```

#### Green

```rust
impl MqttConfig {
    pub fn get_subscriptions(&self) -> Vec<SubscriptionConfig> {
        let mut subs = self.subscriptions.clone();

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

---

### Test Sequence 3.3: Mixed Format Handling

**Complexity**: S

#### Red

```rust
#[test]
fn test_mqtt_config_subscriptions_takes_precedence() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("new", "new/+"),
        ],
        topic_pattern: Some("old/+".to_string()),
        ..Default::default()
    };

    let subs = config.get_subscriptions();

    // Both should be present
    assert_eq!(subs.len(), 2);
}

#[test]
fn test_mqtt_config_no_duplicate_patterns() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("existing", "same/+"),
        ],
        topic_pattern: Some("same/+".to_string()), // Same pattern
        ..Default::default()
    };

    let subs = config.get_subscriptions();

    // Legacy should not be added if pattern already exists
    assert_eq!(subs.len(), 1);
}
```

#### Green

Already handled by the any() check in get_subscriptions().

---

### Test Sequence 3.4: Validation

**Complexity**: M

#### Red

```rust
#[test]
fn test_mqtt_config_validate_no_subscriptions_error() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![],
        topic_pattern: None,
        ..Default::default()
    };

    assert!(matches!(config.validate(), Err(ConfigError::NoSubscriptions)));
}

#[test]
fn test_mqtt_config_validate_duplicate_stream_id_error() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("same", "topic/a/+"),
            SubscriptionConfig::new("same", "topic/b/+"), // Duplicate stream_id
        ],
        topic_pattern: None,
        ..Default::default()
    };

    assert!(matches!(config.validate(), Err(ConfigError::DuplicateStreamId(_))));
}

#[test]
fn test_mqtt_config_validate_valid_ok() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("air", "air/+"),
            SubscriptionConfig::new("home", "home/+"),
        ],
        topic_pattern: None,
        ..Default::default()
    };

    assert!(config.validate().is_ok());
}
```

#### Green

```rust
impl MqttConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        let subs = self.get_subscriptions();

        if subs.is_empty() {
            return Err(ConfigError::NoSubscriptions);
        }

        let mut seen = std::collections::HashSet::new();
        for sub in &subs {
            if !seen.insert(&sub.stream_id) {
                return Err(ConfigError::DuplicateStreamId(sub.stream_id.clone()));
            }
        }

        Ok(())
    }
}
```

---

## Phase 4: MqttSource Integration

### Test Sequence 4.1: MqttSource with Router

**Complexity**: M

#### Red

```rust
#[tokio::test]
async fn test_mqtt_source_new_builds_router() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("air", "air/+"),
        ],
        ..Default::default()
    };

    let source = MqttSource::new(config, create_default_parser());

    assert!(source.is_ok());
}

#[tokio::test]
async fn test_mqtt_source_new_invalid_config_error() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![], // No subscriptions
        topic_pattern: None,
        ..Default::default()
    };

    let source = MqttSource::new(config, create_default_parser());

    assert!(source.is_err());
}
```

#### Green

Update MqttSource::new to build TopicRouter.

---

### Test Sequence 4.2: Topic Patterns for Subscription

**Complexity**: S

#### Red

```rust
#[tokio::test]
async fn test_mqtt_source_topic_patterns() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("a", "topic/a/+"),
            SubscriptionConfig::new("b", "topic/b/#"),
        ],
        ..Default::default()
    };

    let source = MqttSource::new(config, create_default_parser()).unwrap();
    let patterns = source.router.topic_patterns();

    assert!(patterns.contains(&"topic/a/+"));
    assert!(patterns.contains(&"topic/b/#"));
}
```

#### Green

Already implemented via TopicRouter integration.

---

### Test Sequence 4.3: Message Routing with Stream Tagging

**Complexity**: L

#### Red

```rust
#[tokio::test]
async fn test_mqtt_source_routes_messages_with_stream_id() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
        ],
        ..Default::default()
    };

    let source = MqttSource::new(config, create_default_parser()).unwrap();

    // Simulate message processing (internal method)
    let topic = "airgradient/readings/ABC123";
    let payload = r#"{"pm02": 12.5, "serialno": "ABC123"}"#;

    // Process and check points have stream_id tag
    let points = source.process_message(topic, payload.as_bytes()).await.unwrap();

    assert!(points.iter().all(|p| p.tags.get("stream_id") == Some(&"air-quality".to_string())));
}
```

#### Green

Update process_events to tag points with stream_id from matched route.

---

### Test Sequence 4.4: Unmatched Topic Handling

**Complexity**: M

#### Red

```rust
#[tokio::test]
async fn test_mqtt_source_unmatched_topic_dead_letter() {
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 1883,
        client_id: "test".to_string(),
        subscriptions: vec![
            SubscriptionConfig::new("known", "known/+"),
        ],
        ..Default::default()
    };

    let source = MqttSource::new(config, create_default_parser()).unwrap();

    let topic = "unknown/topic";
    let payload = r#"{"value": 1}"#;

    // Should return empty (dead letter logged)
    let points = source.process_message(topic, payload.as_bytes()).await.unwrap();

    assert!(points.is_empty());
    // In a real test, verify dead letter was logged
}
```

#### Green

Route returns None, log warning, return empty points.

---

## Phase 5: Config Parsing Integration

### Test Sequence 5.1: YAML Parsing with Subscriptions

**Complexity**: S

#### Red

```rust
#[test]
fn test_yaml_config_with_subscriptions() {
    let yaml = r#"
broker_url: "localhost"
port: 1883
client_id: "test"
subscriptions:
  - stream_id: air-quality
    topic_pattern: "airgradient/readings/+"
    enabled: true
"#;

    let config: MqttConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.subscriptions.len(), 1);
    assert_eq!(config.subscriptions[0].stream_id, "air-quality");
}
```

#### Green

MqttConfig already derives Deserialize with proper fields.

---

### Test Sequence 5.2: YAML Parsing with Legacy Format

**Complexity**: S

#### Red

```rust
#[test]
fn test_yaml_config_legacy_format() {
    let yaml = r#"
broker_url: "localhost"
port: 1883
client_id: "test"
topic_pattern: "legacy/+"
"#;

    let config: MqttConfig = serde_yaml::from_str(yaml).unwrap();

    // Legacy should be converted
    let subs = config.get_subscriptions();
    assert_eq!(subs.len(), 1);
    assert_eq!(subs[0].topic_pattern, "legacy/+");
}
```

#### Green

Already implemented via get_subscriptions().

---

## Summary: Test Count by Phase

| Phase | Component | Unit Tests | Integration Tests |
|-------|-----------|------------|-------------------|
| 1 | SubscriptionConfig | 12-15 | 0 |
| 2 | TopicRouter | 18-22 | 0 |
| 3 | MqttConfig | 8-10 | 0 |
| 4 | MqttSource | 5-8 | 8-10 |
| 5 | Config Parsing | 3-5 | 2-3 |
| **Total** | | **46-60** | **10-13** |

---

## TDD Checklist

For each test:

- [ ] Write test FIRST (Red)
- [ ] Verify test fails for the right reason
- [ ] Write minimal code to pass (Green)
- [ ] Verify test passes
- [ ] Refactor if needed
- [ ] Verify test still passes
- [ ] Run `cargo clippy`
- [ ] Run `cargo fmt`
- [ ] Commit

---

## Related Documents

- IMPLEMENTATION_PLAN.md: Phase organization
- FILE_CHANGES.md: Full code listings
- TOPIC_ROUTER.md: Algorithm design
