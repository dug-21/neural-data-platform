# DP-003: MQTT Multi-Subscription Test Cases

## Overview

This document provides detailed test cases for the MQTT multi-subscription feature, mapped to acceptance criteria from ACCEPTANCE_CRITERIA.md. Each test case includes implementation guidance following NDP testing patterns.

---

## Test Case Mapping Summary

| AC | Description | Test Count | Test Type |
|----|-------------|------------|-----------|
| AC-1.1 | Multi-Subscription Config Loading | 5 | Unit |
| AC-1.2 | Backward Compatibility | 4 | Unit + E2E |
| AC-1.3 | Per-Subscription Parser Config | 3 | Unit |
| AC-2.1 | Topic Pattern Matching | 5 | Unit |
| AC-2.2 | Unmatched Messages | 2 | Unit |
| AC-3.1 | Consistent Output Schema | 4 | Unit |
| AC-3.2 | Stream ID Tagging | 2 | Unit |
| AC-3.3 | Error Handling | 4 | Unit |
| AC-4.1 | Single Connection Per Broker | 2 | Integration |
| AC-4.2 | Reconnection Behavior | 3 | Integration |
| AC-5.1 | Throughput | 2 | Performance |
| AC-5.2 | Latency | 1 | Performance |
| AC-6.1 | Health Reporting | 2 | Integration |
| AC-6.2 | Structured Logging | 2 | Integration |
| AC-7.1 | End-to-End Data Flow | 3 | E2E |
| AC-7.2 | No Regression | 3 | Unit + E2E |
| **Total** | | **47** | |

---

## 1. Configuration Test Cases

### TC-1.1.1: Load Multi-Subscription Configuration

**Acceptance Criteria**: AC-1.1 (FR-2.1.1, FR-2.1.2, FR-2.1.3)

**Type**: Unit Test

**Location**: `core/src/sources/mqtt.rs` or `core/src/config/mqtt_config.rs`

```rust
#[test]
fn test_load_multi_subscription_config() {
    // ARRANGE
    let yaml = r#"
        sources:
          - type: mqtt
            enabled: true
            params:
              broker_url: "mosquitto"
              port: 1883
              subscriptions:
                - stream_id: air-quality
                  topic_pattern: "airgradient/readings/+"
                - stream_id: homeassistant
                  topic_pattern: "homeassistant/+/+/state"
    "#;

    // ACT
    let config: MqttConfig = serde_yaml::from_str(yaml).unwrap();

    // ASSERT
    assert_eq!(config.subscriptions.len(), 2);
    assert_eq!(config.subscriptions[0].stream_id, "air-quality");
    assert_eq!(config.subscriptions[0].topic_pattern, "airgradient/readings/+");
    assert_eq!(config.subscriptions[1].stream_id, "homeassistant");
    assert_eq!(config.subscriptions[1].topic_pattern, "homeassistant/+/+/state");
}
```

---

### TC-1.1.2: Reject Duplicate Stream IDs

**Acceptance Criteria**: AC-1.1

**Type**: Unit Test

```rust
#[test]
fn test_reject_duplicate_stream_ids() {
    // ARRANGE
    let config = MqttConfig {
        broker_url: "mosquitto".to_string(),
        subscriptions: vec![
            SubscriptionConfig {
                stream_id: "air-quality".to_string(),
                topic_pattern: "topic/a/+".to_string(),
                ..Default::default()
            },
            SubscriptionConfig {
                stream_id: "air-quality".to_string(), // Duplicate!
                topic_pattern: "topic/b/+".to_string(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    // ACT
    let result = config.validate();

    // ASSERT
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("duplicate stream_id"));
}
```

---

### TC-1.1.3: Require Stream ID for Each Subscription

**Acceptance Criteria**: AC-1.1 (FR-2.1.2)

**Type**: Unit Test

```rust
#[test]
fn test_require_stream_id_per_subscription() {
    // ARRANGE
    let yaml = r#"
        subscriptions:
          - topic_pattern: "sensor/+"
    "#;  // Missing stream_id

    // ACT
    let result: Result<SubscriptionConfig, _> = serde_yaml::from_str(yaml);

    // ASSERT
    assert!(result.is_err() || result.unwrap().validate().is_err());
}
```

---

### TC-1.1.4: Require Topic Pattern for Each Subscription

**Acceptance Criteria**: AC-1.1 (FR-2.1.3)

**Type**: Unit Test

```rust
#[test]
fn test_require_topic_pattern_per_subscription() {
    // ARRANGE
    let config = SubscriptionConfig {
        stream_id: "test-stream".to_string(),
        topic_pattern: "".to_string(), // Empty topic
        ..Default::default()
    };

    // ACT
    let result = config.validate();

    // ASSERT
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("topic_pattern"));
}
```

---

### TC-1.1.5: Accept Optional Parser Configuration

**Acceptance Criteria**: AC-1.1 (FR-2.1.4)

**Type**: Unit Test

```rust
#[test]
fn test_parser_config_optional() {
    // ARRANGE - subscription without parser config
    let yaml = r#"
        stream_id: "test-stream"
        topic_pattern: "sensor/+"
    "#;

    // ACT
    let config: SubscriptionConfig = serde_yaml::from_str(yaml).unwrap();

    // ASSERT - should use default parser
    assert!(config.parser.is_none() || config.parser.as_ref().unwrap().is_default());
}
```

---

### TC-1.2.1: Legacy Single Topic Configuration

**Acceptance Criteria**: AC-1.2 (FR-2.1.5, NFR-3.4.1)

**Type**: Unit Test

```rust
#[test]
fn test_legacy_single_topic_config() {
    // ARRANGE - existing air-quality format
    let yaml = r#"
        sources:
          - type: mqtt
            enabled: true
            params:
              broker_url: "mosquitto"
              port: 1883
              topic_pattern: "airgradient/readings/+"
    "#;

    // ACT
    let config: MqttConfig = serde_yaml::from_str(yaml).unwrap();

    // ASSERT
    // Should auto-create single subscription from legacy format
    assert_eq!(config.subscriptions.len(), 1);
    assert_eq!(config.subscriptions[0].topic_pattern, "airgradient/readings/+");
}
```

---

### TC-1.2.2: Legacy Config Uses Parent Stream ID

**Acceptance Criteria**: AC-1.2

**Type**: Unit Test

```rust
#[test]
fn test_legacy_config_uses_parent_stream_id() {
    // ARRANGE
    let stream_config = StreamConfig {
        stream_id: "air-quality".to_string(),
        sources: vec![
            SourceConfig::Mqtt(MqttConfig {
                broker_url: "mosquitto".to_string(),
                topic_pattern: Some("airgradient/readings/+".to_string()),
                subscriptions: vec![], // Empty - legacy format
                ..Default::default()
            }),
        ],
        ..Default::default()
    };

    // ACT
    let mqtt_config = stream_config.sources[0].as_mqtt().unwrap();
    let resolved = mqtt_config.resolve_subscriptions("air-quality");

    // ASSERT
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].stream_id, "air-quality");
}
```

---

### TC-1.2.3: Existing Air-Quality Config Loads

**Acceptance Criteria**: AC-1.2 (NFR-3.4.2)

**Type**: E2E Test

```rust
#[tokio::test]
#[ignore] // Requires etcd
async fn test_existing_air_quality_config_loads() {
    // ARRANGE
    let registry = StreamRegistry::new(&["http://localhost:2379"]).await.unwrap();

    // ACT
    let config = registry.load_stream("air-quality").await.unwrap();

    // ASSERT
    let mqtt_sources: Vec<_> = config.sources.iter()
        .filter(|s| matches!(s, SourceConfig::Mqtt(_)))
        .collect();
    assert!(!mqtt_sources.is_empty());

    // Should parse without error and have valid subscription
    for source in mqtt_sources {
        let mqtt = source.as_mqtt().unwrap();
        assert!(mqtt.topic_pattern.is_some() || !mqtt.subscriptions.is_empty());
    }
}
```

---

### TC-1.2.4: Legacy and New Format Cannot Coexist

**Acceptance Criteria**: AC-1.2 (config clarity)

**Type**: Unit Test

```rust
#[test]
fn test_legacy_and_new_format_exclusive() {
    // ARRANGE - both topic_pattern AND subscriptions
    let config = MqttConfig {
        broker_url: "mosquitto".to_string(),
        topic_pattern: Some("old/topic".to_string()),
        subscriptions: vec![
            SubscriptionConfig {
                stream_id: "new-stream".to_string(),
                topic_pattern: "new/topic".to_string(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    // ACT
    let result = config.validate();

    // ASSERT - should reject ambiguous config
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cannot use both"));
}
```

---

### TC-1.3.1: Different Parsers Per Subscription

**Acceptance Criteria**: AC-1.3 (FR-2.1.4, FR-2.3.1)

**Type**: Unit Test

```rust
#[test]
fn test_different_parsers_per_subscription() {
    // ARRANGE
    let config = MqttConfig {
        subscriptions: vec![
            SubscriptionConfig {
                stream_id: "air-quality".to_string(),
                topic_pattern: "airgradient/+".to_string(),
                parser: Some(ParserConfig {
                    location_id_field: "serialno".to_string(),
                    ..Default::default()
                }),
            },
            SubscriptionConfig {
                stream_id: "homeassistant".to_string(),
                topic_pattern: "homeassistant/+/+/state".to_string(),
                parser: Some(ParserConfig {
                    location_id_field: "entity_id".to_string(),
                    ..Default::default()
                }),
            },
        ],
        ..Default::default()
    };

    // ACT
    let air_parser = config.get_parser_for_stream("air-quality").unwrap();
    let ha_parser = config.get_parser_for_stream("homeassistant").unwrap();

    // ASSERT
    assert_eq!(air_parser.location_id_field, "serialno");
    assert_eq!(ha_parser.location_id_field, "entity_id");
}
```

---

### TC-1.3.2: Parser Uses Correct Location ID Field

**Acceptance Criteria**: AC-1.3

**Type**: Unit Test

```rust
#[test]
fn test_parser_uses_subscription_location_id_field() {
    // ARRANGE
    let parser_config = ParserConfig {
        location_id_field: "entity_id".to_string(),
        ..Default::default()
    };
    let parser = FlatJsonParser::from_config(parser_config).unwrap();

    let payload = json!({
        "entity_id": "sensor.temperature",
        "state": "21.5"
    });

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT
    assert!(!points.is_empty());
    assert_eq!(points[0].location_id, "sensor.temperature");
}
```

---

### TC-1.3.3: Skip Fields Per Subscription

**Acceptance Criteria**: AC-1.3

**Type**: Unit Test

```rust
#[test]
fn test_skip_fields_per_subscription() {
    // ARRANGE
    let parser_config = ParserConfig {
        location_id_field: "serialno".to_string(),
        skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
        ..Default::default()
    };
    let parser = FlatJsonParser::from_config(parser_config).unwrap();

    let payload = json!({
        "serialno": "abc123",
        "firmware": "3.4.1",
        "pm02": 15.5,
        "atmp": 22.0
    });

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();
    let metrics: Vec<_> = points.iter()
        .map(|p| p.tags.get("metric").unwrap().as_str())
        .collect();

    // ASSERT
    assert!(!metrics.contains(&"serialno"));
    assert!(!metrics.contains(&"firmware"));
    assert!(metrics.contains(&"pm02"));
    assert!(metrics.contains(&"atmp"));
}
```

---

## 2. Topic Routing Test Cases

### TC-2.1.1: Route by Single-Level Wildcard (+)

**Acceptance Criteria**: AC-2.1 (FR-2.2.2, FR-2.2.3)

**Type**: Unit Test

```rust
#[test]
fn test_route_single_level_wildcard() {
    // ARRANGE
    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "air-quality".to_string(),
            topic_pattern: "airgradient/readings/+".to_string(),
            ..Default::default()
        },
    ]);

    // ACT & ASSERT
    assert_eq!(router.match_topic("airgradient/readings/abc123"), Some("air-quality"));
    assert_eq!(router.match_topic("airgradient/readings/xyz789"), Some("air-quality"));
    assert_eq!(router.match_topic("airgradient/readings/"), None); // Empty not matched
    assert_eq!(router.match_topic("airgradient/readings/a/b"), None); // Multi-level not matched
}
```

---

### TC-2.1.2: Route by Multi-Level Wildcard (#)

**Acceptance Criteria**: AC-2.1

**Type**: Unit Test

```rust
#[test]
fn test_route_multi_level_wildcard() {
    // ARRANGE
    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "homeassistant".to_string(),
            topic_pattern: "homeassistant/#".to_string(),
            ..Default::default()
        },
    ]);

    // ACT & ASSERT
    assert_eq!(router.match_topic("homeassistant/sensor/temp/state"), Some("homeassistant"));
    assert_eq!(router.match_topic("homeassistant/binary_sensor/motion/state"), Some("homeassistant"));
    assert_eq!(router.match_topic("homeassistant"), Some("homeassistant")); // Just prefix
    assert_eq!(router.match_topic("other/topic"), None);
}
```

---

### TC-2.1.3: Route to Correct Stream by Pattern

**Acceptance Criteria**: AC-2.1 (FR-2.2.3)

**Type**: Unit Test

```rust
#[test]
fn test_route_to_correct_stream() {
    // ARRANGE
    let router = TopicRouter::new(vec![
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
    ]);

    // ACT & ASSERT
    assert_eq!(router.match_topic("airgradient/readings/abc123"), Some("air-quality"));
    assert_eq!(router.match_topic("homeassistant/sensor/temp/state"), Some("homeassistant"));
}
```

---

### TC-2.1.4: First-Match Wins for Overlapping Patterns

**Acceptance Criteria**: AC-2.1 (FR-2.2.4)

**Type**: Unit Test

```rust
#[test]
fn test_first_match_wins() {
    // ARRANGE - specific pattern before general
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

    // ACT
    let result = router.match_topic("sensors/temp/device1");

    // ASSERT - should match first pattern (specific)
    assert_eq!(result, Some("specific"));
}
```

---

### TC-2.1.5: Add Stream ID Tag to Routed Points

**Acceptance Criteria**: AC-2.1, AC-3.2 (FR-2.3.3)

**Type**: Unit Test

```rust
#[test]
fn test_add_stream_id_tag() {
    // ARRANGE
    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "air-quality".to_string(),
            topic_pattern: "airgradient/+".to_string(),
            ..Default::default()
        },
    ]);

    let mut point = TimeSeriesPoint {
        timestamp: Utc::now(),
        location_id: "abc123".to_string(),
        value: 15.5,
        tags: HashMap::new(),
    };

    // ACT
    let stream_id = router.match_topic("airgradient/abc123").unwrap();
    point.tags.insert("stream_id".to_string(), stream_id.to_string());

    // ASSERT
    assert_eq!(point.tags.get("stream_id"), Some(&"air-quality".to_string()));
}
```

---

### TC-2.2.1: Log Unmatched Messages at DEBUG

**Acceptance Criteria**: AC-2.2 (FR-2.2.5)

**Type**: Unit Test (with log capture)

```rust
#[test]
fn test_unmatched_messages_logged() {
    // ARRANGE
    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "air-quality".to_string(),
            topic_pattern: "airgradient/+".to_string(),
            ..Default::default()
        },
    ]);

    // ACT
    let result = router.match_topic("unknown/topic/path");

    // ASSERT
    assert!(result.is_none());
    // Log verification would use tracing-test or similar
    // Expected: DEBUG log with "unknown/topic/path"
}
```

---

### TC-2.2.2: Continue Processing After Unmatched Message

**Acceptance Criteria**: AC-2.2

**Type**: Unit Test

```rust
#[tokio::test]
async fn test_continue_after_unmatched() {
    // ARRANGE
    let router = TopicRouter::new(vec![
        SubscriptionConfig {
            stream_id: "air-quality".to_string(),
            topic_pattern: "airgradient/+".to_string(),
            ..Default::default()
        },
    ]);

    // ACT - process unmatched, then matched
    let result1 = router.match_topic("unknown/topic");
    let result2 = router.match_topic("airgradient/abc123");

    // ASSERT - router continues working
    assert!(result1.is_none());
    assert_eq!(result2, Some("air-quality"));
}
```

---

## 3. Message Processing Test Cases

### TC-3.1.1: Output Schema Consistency - Air Quality

**Acceptance Criteria**: AC-3.1 (FR-2.3.2)

**Type**: Unit Test

```rust
#[test]
fn test_air_quality_output_schema() {
    // ARRANGE
    let parser = create_air_quality_parser();
    let payload = json!({
        "serialno": "abc123",
        "pm02": 15.5,
        "atmp": 22.0
    });

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT - Bronze layer schema
    for point in &points {
        assert!(point.timestamp > DateTime::<Utc>::MIN_UTC);
        assert!(!point.location_id.is_empty());
        assert!(point.tags.contains_key("metric"));
        // value is f64 (always present in TimeSeriesPoint)
    }

    // Verify specific values
    let pm_point = points.iter().find(|p| p.tags.get("metric") == Some(&"pm02".to_string())).unwrap();
    assert_eq!(pm_point.location_id, "abc123");
    assert_eq!(pm_point.value, 15.5);
}
```

---

### TC-3.1.2: Output Schema Consistency - HomeAssistant

**Acceptance Criteria**: AC-3.1

**Type**: Unit Test

```rust
#[test]
fn test_homeassistant_output_schema() {
    // ARRANGE
    let parser = create_homeassistant_parser();
    let payload = json!({
        "entity_id": "sensor.temp",
        "state": "21.5"
    });

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT - Same schema as air-quality
    assert!(!points.is_empty());
    let point = &points[0];
    assert_eq!(point.location_id, "sensor.temp");
    assert!(point.tags.contains_key("metric"));
    // Numeric state parsed as value
}
```

---

### TC-3.1.3: Both Streams Produce Identical Schema

**Acceptance Criteria**: AC-3.1 (NFR-3.4.3)

**Type**: Unit Test

```rust
#[test]
fn test_both_streams_identical_schema() {
    // ARRANGE
    let air_parser = create_air_quality_parser();
    let ha_parser = create_homeassistant_parser();

    let air_payload = json!({"serialno": "abc123", "pm02": 15.5});
    let ha_payload = json!({"entity_id": "sensor.temp", "state": "21.5"});

    // ACT
    let air_points = air_parser.parse(&air_payload, Utc::now()).unwrap();
    let ha_points = ha_parser.parse(&ha_payload, Utc::now()).unwrap();

    // ASSERT - identical structure
    let air_point = &air_points[0];
    let ha_point = &ha_points[0];

    // Both have same fields
    assert!(air_point.tags.contains_key("metric") == ha_point.tags.contains_key("metric"));
    // Both have timestamp, location_id, value (struct fields)
}
```

---

### TC-3.1.4: Multiple Metrics Per Message

**Acceptance Criteria**: AC-3.1

**Type**: Unit Test

```rust
#[test]
fn test_multiple_metrics_per_message() {
    // ARRANGE
    let parser = create_air_quality_parser();
    let payload = json!({
        "serialno": "abc123",
        "pm02": 15.5,
        "atmp": 22.0,
        "rhum": 55.0,
        "rco2": 450
    });

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT - one point per metric
    assert_eq!(points.len(), 4);
    let metrics: Vec<_> = points.iter()
        .map(|p| p.tags.get("metric").unwrap().clone())
        .collect();
    assert!(metrics.contains(&"pm02".to_string()));
    assert!(metrics.contains(&"atmp".to_string()));
    assert!(metrics.contains(&"rhum".to_string()));
    assert!(metrics.contains(&"rco2".to_string()));
}
```

---

### TC-3.2.1: All Points Receive Stream ID Tag

**Acceptance Criteria**: AC-3.2 (FR-2.3.3)

**Type**: Unit Test

```rust
#[test]
fn test_all_points_have_stream_id() {
    // ARRANGE
    let subscription = SubscriptionConfig {
        stream_id: "air-quality".to_string(),
        topic_pattern: "airgradient/+".to_string(),
        ..Default::default()
    };

    let parser = create_parser_for_subscription(&subscription);
    let payload = json!({
        "serialno": "abc123",
        "pm02": 15.5,
        "atmp": 22.0
    });

    // ACT
    let mut points = parser.parse(&payload, Utc::now()).unwrap();
    for point in &mut points {
        point.tags.insert("stream_id".to_string(), subscription.stream_id.clone());
    }

    // ASSERT - ALL points have stream_id
    for point in &points {
        assert_eq!(point.tags.get("stream_id"), Some(&"air-quality".to_string()));
    }
}
```

---

### TC-3.2.2: All Points Have Source Tag

**Acceptance Criteria**: AC-3.2

**Type**: Unit Test

```rust
#[test]
fn test_all_points_have_source_tag() {
    // ARRANGE
    let parser_config = ParserConfig {
        default_tags: [("source".to_string(), "mqtt".to_string())].into_iter().collect(),
        ..Default::default()
    };
    let parser = FlatJsonParser::from_config(parser_config).unwrap();

    let payload = json!({"serialno": "abc123", "pm02": 15.5});

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT
    for point in &points {
        assert_eq!(point.tags.get("source"), Some(&"mqtt".to_string()));
    }
}
```

---

### TC-3.3.1: Malformed JSON Does Not Crash

**Acceptance Criteria**: AC-3.3 (FR-2.3.5, NFR-3.2.2)

**Type**: Unit Test

```rust
#[test]
fn test_malformed_json_handled() {
    // ARRANGE
    let source = create_mqtt_source();
    let payload = b"not valid json {";

    // ACT
    let result = source.parse_payload(payload);

    // ASSERT - returns error, doesn't panic
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("parse") || err.to_string().contains("JSON"));
}
```

---

### TC-3.3.2: Missing Required Field Uses Default

**Acceptance Criteria**: AC-3.3

**Type**: Unit Test

```rust
#[test]
fn test_missing_location_id_uses_default() {
    // ARRANGE
    let parser_config = ParserConfig {
        location_id_field: "serialno".to_string(),
        default_location_id: Some("unknown".to_string()),
        ..Default::default()
    };
    let parser = FlatJsonParser::from_config(parser_config).unwrap();

    // Missing serialno
    let payload = json!({"pm02": 15.5});

    // ACT
    let points = parser.parse(&payload, Utc::now()).unwrap();

    // ASSERT - uses default
    assert!(!points.is_empty());
    assert_eq!(points[0].location_id, "unknown");
}
```

---

### TC-3.3.3: Parser Error Does Not Stop Subscription

**Acceptance Criteria**: AC-3.3 (FR-2.3.5)

**Type**: Unit Test

```rust
#[tokio::test]
async fn test_parser_error_continues_processing() {
    // ARRANGE
    let source = create_mqtt_source();

    // ACT - process invalid, then valid
    let result1 = source.parse_payload(b"invalid json");
    let result2 = source.parse_payload(br#"{"serialno":"abc","pm02":15.5}"#);

    // ASSERT - continues after error
    assert!(result1.is_err());
    assert!(result2.is_ok());
    assert!(!result2.unwrap().is_empty());
}
```

---

### TC-3.3.4: Error Logging Includes Context

**Acceptance Criteria**: AC-3.3 (NFR-3.3.2)

**Type**: Unit Test (with log verification)

```rust
#[test]
fn test_error_includes_context() {
    // This test verifies error messages include:
    // - stream_id
    // - topic
    // - error details

    // ARRANGE
    let parser = create_parser_for_subscription(&SubscriptionConfig {
        stream_id: "air-quality".to_string(),
        topic_pattern: "test/+".to_string(),
        ..Default::default()
    });

    // ACT - parse invalid data
    let result = parser.parse(&json!({"not": "numeric"}), Utc::now());

    // ASSERT - error context (would check logs in real test)
    // Expected log: ERROR stream_id="air-quality" topic="test/abc" error="..."
}
```

---

## 4. Connection Management Test Cases

### TC-4.1.1: Single Connection for Multiple Subscriptions

**Acceptance Criteria**: AC-4.1 (FR-2.4.1)

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore] // Requires MQTT broker
async fn test_single_connection_multiple_subscriptions() {
    // ARRANGE
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        port: 11883, // Test port
        subscriptions: vec![
            SubscriptionConfig { stream_id: "stream1".to_string(), topic_pattern: "topic1/+".to_string(), ..Default::default() },
            SubscriptionConfig { stream_id: "stream2".to_string(), topic_pattern: "topic2/+".to_string(), ..Default::default() },
            SubscriptionConfig { stream_id: "stream3".to_string(), topic_pattern: "topic3/+".to_string(), ..Default::default() },
        ],
        ..Default::default()
    };

    // ACT
    let mut source = MqttSource::new(config, create_default_parser());
    source.start().await.unwrap();

    // ASSERT
    // Verify via broker: only 1 connection from client_id
    // All 3 topics subscribed on same connection
    // Would use mosquitto_sub --topic '$SYS/broker/clients/connected' to verify

    source.stop().await.unwrap();
}
```

---

### TC-4.1.2: Subscribe All Topics on Connect

**Acceptance Criteria**: AC-4.1 (FR-2.2.1)

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore] // Requires MQTT broker
async fn test_subscribe_all_topics_on_connect() {
    // ARRANGE
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        subscriptions: vec![
            SubscriptionConfig { stream_id: "s1".to_string(), topic_pattern: "t1/+".to_string(), ..Default::default() },
            SubscriptionConfig { stream_id: "s2".to_string(), topic_pattern: "t2/+".to_string(), ..Default::default() },
        ],
        ..Default::default()
    };

    // ACT
    let mut source = MqttSource::new(config, create_default_parser());
    source.start().await.unwrap();

    // Wait for subscriptions
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ASSERT - publish to both topics, both should be received
    let mqtt_client = create_test_publisher().await;
    mqtt_client.publish("t1/device1", QoS::AtLeastOnce, false, "{}").await.unwrap();
    mqtt_client.publish("t2/device2", QoS::AtLeastOnce, false, "{}").await.unwrap();

    // Verify source received both
    tokio::time::sleep(Duration::from_millis(500)).await;
    let points = source.fetch().await.unwrap();
    // Both messages should be received (actual verification depends on parser)
}
```

---

### TC-4.2.1: Re-subscribe All Topics After Reconnect

**Acceptance Criteria**: AC-4.2 (FR-2.4.2)

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore] // Requires MQTT broker with restart capability
async fn test_resubscribe_after_reconnect() {
    // ARRANGE
    let config = MqttConfig {
        broker_url: "localhost".to_string(),
        subscriptions: vec![
            SubscriptionConfig { stream_id: "s1".to_string(), topic_pattern: "t1/+".to_string(), ..Default::default() },
            SubscriptionConfig { stream_id: "s2".to_string(), topic_pattern: "t2/+".to_string(), ..Default::default() },
        ],
        reconnect_delay: Duration::from_millis(100),
        ..Default::default()
    };

    let mut source = MqttSource::new(config, create_default_parser());
    source.start().await.unwrap();

    // ACT - simulate broker restart
    restart_test_broker().await;
    tokio::time::sleep(Duration::from_secs(2)).await; // Wait for reconnect

    // ASSERT - all topics should be resubscribed
    let mqtt_client = create_test_publisher().await;
    mqtt_client.publish("t1/test", QoS::AtLeastOnce, false, r#"{"value":1}"#).await.unwrap();
    mqtt_client.publish("t2/test", QoS::AtLeastOnce, false, r#"{"value":2}"#).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    let points = source.fetch().await.unwrap();
    // Should receive from both topics after reconnect
}
```

---

### TC-4.2.2: Exponential Backoff Calculation

**Acceptance Criteria**: AC-4.2 (FR-2.4.3)

**Type**: Unit Test

```rust
#[test]
fn test_exponential_backoff() {
    // ARRANGE
    let base_delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(30);

    // ACT & ASSERT
    let calculate_delay = |attempt: u32| -> Duration {
        let delay_secs = std::cmp::min(
            base_delay.as_secs() * 2_u64.pow(attempt),
            max_delay.as_secs(),
        );
        Duration::from_secs(delay_secs)
    };

    assert_eq!(calculate_delay(0), Duration::from_secs(1));
    assert_eq!(calculate_delay(1), Duration::from_secs(2));
    assert_eq!(calculate_delay(2), Duration::from_secs(4));
    assert_eq!(calculate_delay(3), Duration::from_secs(8));
    assert_eq!(calculate_delay(4), Duration::from_secs(16));
    assert_eq!(calculate_delay(5), Duration::from_secs(30)); // Capped
    assert_eq!(calculate_delay(6), Duration::from_secs(30)); // Still capped
}
```

---

### TC-4.2.3: Max Backoff Limit

**Acceptance Criteria**: AC-4.2

**Type**: Unit Test

```rust
#[test]
fn test_backoff_never_exceeds_max() {
    // ARRANGE
    let config = MqttConfig {
        reconnect_delay: Duration::from_secs(1),
        max_reconnect_delay: Duration::from_secs(30),
        ..Default::default()
    };

    // ACT - simulate many reconnect attempts
    for attempt in 0..100 {
        let delay = std::cmp::min(
            config.reconnect_delay.as_secs() * 2_u64.pow(attempt),
            config.max_reconnect_delay.as_secs(),
        );

        // ASSERT - never exceeds max
        assert!(delay <= config.max_reconnect_delay.as_secs());
    }
}
```

---

## 5. Performance Test Cases

### TC-5.1.1: Throughput >= 1000 Messages/Second

**Acceptance Criteria**: AC-5.1 (NFR-3.1.2)

**Type**: Performance/Load Test

```rust
#[tokio::test]
#[ignore] // Performance test, run separately
async fn test_throughput_1000_msgs_per_second() {
    // ARRANGE
    let config = MqttConfig {
        buffer_capacity: 5000,
        subscriptions: vec![
            SubscriptionConfig {
                stream_id: "perf-test".to_string(),
                topic_pattern: "perf/+".to_string(),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    let mut source = MqttSource::new(config, create_default_parser());
    source.start().await.unwrap();

    // ACT - publish 1000 messages
    let start = Instant::now();
    let publisher = create_test_publisher().await;

    for i in 0..1000 {
        let payload = format!(r#"{{"id":{},"pm02":{},"serialno":"perf"}}"#, i, i as f64 * 0.1);
        publisher.publish(format!("perf/{}", i), QoS::AtMostOnce, false, payload).await.unwrap();
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;
    let elapsed = start.elapsed();

    // ASSERT
    let points = source.fetch().await.unwrap();
    let throughput = points.len() as f64 / elapsed.as_secs_f64();

    assert!(throughput >= 1000.0, "Throughput {:.0} msg/s < 1000 required", throughput);
}
```

---

### TC-5.1.2: No Message Loss Under Load

**Acceptance Criteria**: AC-5.1

**Type**: Performance Test

```rust
#[tokio::test]
#[ignore]
async fn test_no_message_loss() {
    // ARRANGE
    let message_count = 1000;
    let mut source = create_mqtt_source_with_subscription("load-test", "load/+");
    source.start().await.unwrap();

    // ACT - publish known number of messages
    let publisher = create_test_publisher().await;
    for i in 0..message_count {
        publisher.publish(format!("load/{}", i), QoS::AtLeastOnce, false,
            format!(r#"{{"n":{},"serialno":"test"}}"#, i)).await.unwrap();
    }

    // Wait for all messages
    tokio::time::sleep(Duration::from_secs(5)).await;

    // ASSERT - all messages received
    let points = source.fetch().await.unwrap();
    assert_eq!(points.len(), message_count, "Lost {} messages", message_count - points.len());
}
```

---

### TC-5.2.1: Processing Latency < 100ms p95

**Acceptance Criteria**: AC-5.2 (NFR-3.1.1)

**Type**: Performance Test

```rust
#[tokio::test]
#[ignore]
async fn test_processing_latency_p95() {
    // ARRANGE
    let mut source = create_mqtt_source();
    source.start().await.unwrap();

    // ACT - measure latency for 100 messages
    let mut latencies = Vec::new();
    let publisher = create_test_publisher().await;

    for i in 0..100 {
        let send_time = Instant::now();
        let payload = format!(r#"{{"n":{},"serialno":"lat"}}"#, i);
        publisher.publish("latency/test", QoS::AtMostOnce, false, payload).await.unwrap();

        // Wait and fetch
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = source.fetch().await.unwrap();
        latencies.push(send_time.elapsed());
    }

    // Calculate p95
    latencies.sort();
    let p95_index = (latencies.len() as f64 * 0.95) as usize;
    let p95_latency = latencies[p95_index];

    // ASSERT
    assert!(p95_latency < Duration::from_millis(100),
        "p95 latency {:?} >= 100ms required", p95_latency);
}
```

---

## 6. Observability Test Cases

### TC-6.1.1: Per-Subscription Health Status

**Acceptance Criteria**: AC-6.1 (NFR-3.5.1)

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore]
async fn test_per_subscription_health() {
    // ARRANGE
    let config = MqttConfig {
        subscriptions: vec![
            SubscriptionConfig { stream_id: "air-quality".to_string(), topic_pattern: "air/+".to_string(), ..Default::default() },
            SubscriptionConfig { stream_id: "homeassistant".to_string(), topic_pattern: "ha/+".to_string(), ..Default::default() },
        ],
        ..Default::default()
    };

    let mut source = MqttSource::new(config, create_default_parser());
    source.start().await.unwrap();

    // Send some messages
    let publisher = create_test_publisher().await;
    publisher.publish("air/test", QoS::AtMostOnce, false, r#"{"serialno":"a","pm02":1}"#).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // ACT
    let health = source.health_check().await.unwrap();

    // ASSERT - should report per-subscription status
    // Expected: details["air-quality"] = { healthy: true, message_count: 1 }
    // Expected: details["homeassistant"] = { healthy: true, message_count: 0 }
}
```

---

### TC-6.1.2: Message Count Per Subscription

**Acceptance Criteria**: AC-6.1

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore]
async fn test_message_count_per_subscription() {
    // ARRANGE
    let mut source = create_mqtt_source_with_multi_subscriptions();
    source.start().await.unwrap();

    // Send different counts to each subscription
    let publisher = create_test_publisher().await;
    for _ in 0..5 {
        publisher.publish("stream1/test", QoS::AtMostOnce, false, r#"{"serialno":"a","v":1}"#).await.unwrap();
    }
    for _ in 0..3 {
        publisher.publish("stream2/test", QoS::AtMostOnce, false, r#"{"serialno":"b","v":2}"#).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // ACT
    let stats = source.get_subscription_stats().await;

    // ASSERT
    assert_eq!(stats.get("stream1").unwrap().message_count, 5);
    assert_eq!(stats.get("stream2").unwrap().message_count, 3);
}
```

---

### TC-6.2.1: Logs Include Stream Context

**Acceptance Criteria**: AC-6.2 (NFR-3.5.4)

**Type**: Integration Test (with log capture)

```rust
#[tokio::test]
#[ignore]
async fn test_logs_include_stream_id() {
    // This test would use tracing-test or similar to capture logs

    // ARRANGE
    // let (logs, _guard) = capture_logs();
    let mut source = create_mqtt_source_with_subscription("air-quality", "air/+");
    source.start().await.unwrap();

    // ACT - process a message
    let publisher = create_test_publisher().await;
    publisher.publish("air/test", QoS::AtMostOnce, false, r#"{"serialno":"a","pm02":1}"#).await.unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;

    // ASSERT
    // let log_entry = logs.iter().find(|l| l.contains("air-quality")).unwrap();
    // assert!(log_entry.contains("stream_id=air-quality"));
    // assert!(log_entry.contains("topic=air/test"));
}
```

---

### TC-6.2.2: Connection Events Logged

**Acceptance Criteria**: AC-6.2 (NFR-3.5.3)

**Type**: Integration Test

```rust
#[tokio::test]
#[ignore]
async fn test_connection_events_logged() {
    // ARRANGE - capture logs
    let mut source = MqttSource::new(MqttConfig::default(), create_default_parser());

    // ACT
    source.start().await.unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;

    // ASSERT - verify INFO log "Connected to MQTT broker"
    // Would check captured logs for expected messages
}
```

---

## 7. End-to-End Test Cases

### TC-7.1.1: Air-Quality Data to Correct Partition

**Acceptance Criteria**: AC-7.1 (FR-2.3.2, NFR-3.4.2)

**Type**: E2E Test

```rust
#[tokio::test]
#[ignore] // Requires full deployment
async fn test_air_quality_to_parquet() {
    // ARRANGE - full pipeline running
    // Assume deploy/pi/deploy.sh start has been run

    // ACT - publish air quality message
    let publisher = create_production_publisher().await;
    publisher.publish(
        "airgradient/readings/e2e-test",
        QoS::AtLeastOnce,
        false,
        r#"{"serialno":"e2e-test","pm02":15.5,"atmp":22.0}"#
    ).await.unwrap();

    // Wait for pipeline processing
    tokio::time::sleep(Duration::from_secs(10)).await;

    // ASSERT - data in correct Parquet partition
    let conn = duckdb::Connection::open_in_memory().unwrap();
    let query = format!(
        "SELECT * FROM read_parquet('data/bronze/air-quality/{}/*.parquet') WHERE location_id = 'e2e-test'",
        Utc::now().format("%Y-%m-%d")
    );

    let mut stmt = conn.prepare(&query).unwrap();
    let mut rows = stmt.query([]).unwrap();
    let row = rows.next().unwrap().unwrap();

    assert!(row.get::<_, f64>(3).unwrap() > 0.0); // value column
}
```

---

### TC-7.1.2: HomeAssistant Data to Separate Partition

**Acceptance Criteria**: AC-7.1

**Type**: E2E Test

```rust
#[tokio::test]
#[ignore]
async fn test_homeassistant_to_separate_partition() {
    // ARRANGE
    let publisher = create_production_publisher().await;

    // ACT
    publisher.publish(
        "homeassistant/sensor/e2e_temp/state",
        QoS::AtLeastOnce,
        false,
        r#"{"entity_id":"sensor.e2e_temp","state":"21.5"}"#
    ).await.unwrap();

    tokio::time::sleep(Duration::from_secs(10)).await;

    // ASSERT - data in homeassistant partition
    let conn = duckdb::Connection::open_in_memory().unwrap();

    // Should be in homeassistant partition
    let ha_query = format!(
        "SELECT COUNT(*) FROM read_parquet('data/bronze/homeassistant/{}/*.parquet') WHERE location_id = 'sensor.e2e_temp'",
        Utc::now().format("%Y-%m-%d")
    );
    let ha_count: i64 = conn.query_row(&ha_query, [], |r| r.get(0)).unwrap();
    assert!(ha_count > 0, "Data not found in homeassistant partition");

    // Should NOT be in air-quality partition
    let air_query = format!(
        "SELECT COUNT(*) FROM read_parquet('data/bronze/air-quality/{}/*.parquet') WHERE location_id = 'sensor.e2e_temp'",
        Utc::now().format("%Y-%m-%d")
    );
    let air_count: i64 = conn.query_row(&air_query, [], |r| r.get(0)).unwrap_or(0);
    assert_eq!(air_count, 0, "Data incorrectly found in air-quality partition");
}
```

---

### TC-7.1.3: Grafana Dashboard Data Available

**Acceptance Criteria**: AC-7.1

**Type**: E2E Test (Manual or Automated)

```rust
#[tokio::test]
#[ignore]
async fn test_grafana_data_available() {
    // This would typically be a manual or Playwright test

    // ARRANGE - send test data
    let publisher = create_production_publisher().await;
    publisher.publish("airgradient/readings/grafana-test",
        QoS::AtLeastOnce, false,
        r#"{"serialno":"grafana-test","pm02":25.0}"#).await.unwrap();

    tokio::time::sleep(Duration::from_secs(30)).await;

    // ACT - query Grafana API
    let client = reqwest::Client::new();
    let response = client
        .get("http://localhost:3000/api/datasources/proxy/1/query")
        .query(&[("db", "duckdb"), ("q", "SELECT * FROM air_quality LIMIT 1")])
        .send()
        .await
        .unwrap();

    // ASSERT
    assert!(response.status().is_success());
    let body: serde_json::Value = response.json().await.unwrap();
    assert!(!body["results"].as_array().unwrap().is_empty());
}
```

---

### TC-7.2.1: All Existing MQTT Tests Pass

**Acceptance Criteria**: AC-7.2 (NFR-3.4.2)

**Type**: Unit Test (Regression)

```rust
// This is verified by running:
// cargo test --package neural-core sources::mqtt::tests

// All 15 existing tests in core/src/sources/mqtt.rs must pass:
// - test_mqtt_source_creation
// - test_health_check_before_start
// - test_parse_payload_success
// - test_parse_payload_invalid_json
// - test_parse_payload_partial_data
// - test_parse_payload_all_fields
// - test_exponential_backoff_calculation
// - test_topic_pattern_substitution
// - test_fetch_returns_cached_points
// - test_parse_payload_extracts_all_numeric_fields
// - test_field_names_not_renamed_at_ingestion
// - test_non_metric_fields_excluded
// - test_all_numeric_types_extracted
```

---

### TC-7.2.2: All Air-Quality App Tests Pass

**Acceptance Criteria**: AC-7.2

**Type**: Unit Test (Regression)

```rust
// This is verified by running:
// cargo test --package air-quality-app

// All existing tests including:
// - mqtt_routing_integration_test.rs tests
// - coordinator tests
// - router tests
```

---

### TC-7.2.3: Source Trait Contract Preserved

**Acceptance Criteria**: AC-7.2 (NFR-3.4.3)

**Type**: Unit Test

```rust
#[tokio::test]
async fn test_source_trait_contract() {
    // ARRANGE
    let config = MqttConfig::default();
    let source: Box<dyn Source> = Box::new(MqttSource::new(config, create_default_parser()));

    // ACT & ASSERT - Source trait methods work
    let points = source.fetch().await.unwrap();
    assert!(points.is_empty()); // No messages yet

    let health = source.health_check().await.unwrap();
    assert!(!health.healthy); // Not started
}
```

---

## Test Execution Summary

### Unit Tests (Run Always)

```bash
# Run all unit tests
cargo test --workspace --lib

# Run MQTT-specific tests
cargo test --package neural-core mqtt
cargo test --package air-quality-app mqtt
```

### Integration Tests (Require Docker)

```bash
# Start test infrastructure
docker-compose -f deploy/docker-compose.test.yml up -d

# Run integration tests
cargo test --workspace --test '*integration*' -- --ignored
```

### E2E Tests (Require Full Deployment)

```bash
# Start production stack
./deploy/pi/deploy.sh start

# Run E2E tests
cargo test --workspace --test '*e2e*' -- --ignored
```

### Performance Tests (Run Separately)

```bash
# Run performance benchmarks
cargo test --workspace perf -- --ignored --nocapture
cargo bench mqtt
```

---

## References

- TEST_STRATEGY.md - Overall test strategy
- REQUIREMENTS.md - Functional requirements (FR-x.x.x)
- ACCEPTANCE_CRITERIA.md - Acceptance criteria (AC-x.x)
- `docs/testing/AIR-005-TEST-DESIGN.md` - London School TDD patterns
- `core/src/sources/mqtt.rs` - Existing MQTT tests (15 tests)
