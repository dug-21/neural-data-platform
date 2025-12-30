# DP-003: MQTT Multi-Subscription Acceptance Criteria

## Overview

This document defines testable acceptance criteria for the MQTT multi-subscription feature using Gherkin format (Given-When-Then). Each criterion is linked to requirements in REQUIREMENTS.md.

---

## 1. Configuration Acceptance Criteria

### AC-1.1: Multi-Subscription Configuration Loading

**Requirement**: FR-2.1.1, FR-2.1.2, FR-2.1.3

```gherkin
Feature: Multi-Subscription Configuration

  Scenario: Load multi-subscription MQTT configuration
    Given a stream configuration file with:
      """yaml
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
      """
    When the SourceManager loads the configuration
    Then it should create ONE MqttSource instance
    And the MqttSource should have 2 subscriptions configured
    And subscription[0].stream_id should equal "air-quality"
    And subscription[0].topic_pattern should equal "airgradient/readings/+"
    And subscription[1].stream_id should equal "homeassistant"
    And subscription[1].topic_pattern should equal "homeassistant/+/+/state"

  Scenario: Reject configuration with duplicate stream_ids
    Given a stream configuration with:
      """yaml
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "topic/a/+"
        - stream_id: air-quality
          topic_pattern: "topic/b/+"
      """
    When the SourceManager validates the configuration
    Then it should return a ConfigError
    And the error message should mention "duplicate stream_id"
```

### AC-1.2: Backward Compatibility

**Requirement**: FR-2.1.5, NFR-3.4.1

```gherkin
Feature: Backward Compatible Configuration

  Scenario: Load legacy single topic_pattern configuration
    Given a stream configuration file with:
      """yaml
      sources:
        - type: mqtt
          enabled: true
          params:
            broker_url: "mosquitto"
            port: 1883
            topic_pattern: "airgradient/readings/+"
      """
    When the SourceManager loads the configuration
    Then it should create ONE MqttSource instance
    And the MqttSource should have 1 subscription configured
    And the subscription should use the stream_id from the parent stream config
    And the subscription topic_pattern should equal "airgradient/readings/+"

  Scenario: Existing air-quality configuration continues to work
    Given the existing config/base/streams/air-quality/config.yaml
    When the application starts with this configuration
    Then the MQTT source should connect successfully
    And it should subscribe to "airgradient/readings/+"
    And existing Grafana dashboards should display data correctly
```

### AC-1.3: Per-Subscription Parser Configuration

**Requirement**: FR-2.1.4, FR-2.3.1

```gherkin
Feature: Per-Subscription Parser Configuration

  Scenario: Different parsers for different subscriptions
    Given a configuration with:
      """yaml
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
          parser:
            parser_type: flat_json
            location_id_field: serialno
            skip_fields: [serialno, firmware]
        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
          parser:
            parser_type: flat_json
            location_id_field: entity_id
      """
    When an air-quality message arrives with serialno field
    Then the parser should use "serialno" as location_id
    When a homeassistant message arrives with entity_id field
    Then the parser should use "entity_id" as location_id
```

---

## 2. Routing Acceptance Criteria

### AC-2.1: Topic Pattern Matching

**Requirement**: FR-2.2.1, FR-2.2.2, FR-2.2.3

```gherkin
Feature: Topic Pattern Routing

  Scenario: Route message to correct stream by topic pattern
    Given subscriptions configured:
      | stream_id     | topic_pattern              |
      | air-quality   | airgradient/readings/+     |
      | homeassistant | homeassistant/+/+/state    |
    When a message arrives on topic "airgradient/readings/abc123"
    Then the message should be routed to stream "air-quality"
    And the TimeSeriesPoint should have tag stream_id="air-quality"

  Scenario: Route HomeAssistant message to correct stream
    Given the same subscriptions as above
    When a message arrives on topic "homeassistant/sensor/temperature/state"
    Then the message should be routed to stream "homeassistant"
    And the TimeSeriesPoint should have tag stream_id="homeassistant"

  Scenario: First-match wins for overlapping patterns
    Given subscriptions configured:
      | stream_id | topic_pattern |
      | specific  | sensors/temp/+ |
      | general   | sensors/+/+    |
    When a message arrives on topic "sensors/temp/device1"
    Then the message should be routed to stream "specific"
    And it should NOT be routed to stream "general"
```

### AC-2.2: Unmatched Messages

**Requirement**: FR-2.2.5

```gherkin
Feature: Unmatched Message Handling

  Scenario: Log unmatched messages without crashing
    Given subscriptions configured for "airgradient/readings/+"
    When a message arrives on topic "unknown/topic/path"
    Then the message should be logged at DEBUG level
    And the log should include the topic "unknown/topic/path"
    And the MqttSource should continue running
    And no error should be raised
```

---

## 3. Message Processing Acceptance Criteria

### AC-3.1: Consistent Output Schema

**Requirement**: FR-2.3.2, NFR-3.4.3

```gherkin
Feature: Bronze Layer Schema Consistency

  Scenario: All subscriptions produce consistent Parquet schema
    Given subscriptions for "air-quality" and "homeassistant"
    When air-quality message arrives:
      """json
      {"serialno": "abc123", "pm02": 15.5, "atmp": 22.0}
      """
    Then output TimeSeriesPoints should have:
      | field       | type      | value       |
      | timestamp   | DateTime  | <current>   |
      | location_id | String    | abc123      |
      | metric      | String    | pm02        |
      | value       | Float64   | 15.5        |
    And another point with metric="atmp", value=22.0

  Scenario: HomeAssistant produces same schema
    When homeassistant message arrives:
      """json
      {"entity_id": "sensor.temp", "state": "21.5"}
      """
    Then output TimeSeriesPoint should have:
      | field       | type      | value       |
      | timestamp   | DateTime  | <current>   |
      | location_id | String    | sensor.temp |
      | metric      | String    | state       |
      | value       | Float64   | 21.5        |
```

### AC-3.2: Stream ID Tagging

**Requirement**: FR-2.3.3

```gherkin
Feature: Stream ID Tag Injection

  Scenario: All points receive stream_id tag
    Given a subscription with stream_id="air-quality"
    When a message produces multiple TimeSeriesPoints
    Then ALL points should have tag stream_id="air-quality"
    And ALL points should have tag source="mqtt"
```

### AC-3.3: Error Handling

**Requirement**: FR-2.3.5, NFR-3.2.2

```gherkin
Feature: Parser Error Resilience

  Scenario: Malformed JSON does not crash subscription
    Given a running MqttSource with air-quality subscription
    When an invalid JSON message arrives: "not valid json {"
    Then an error should be logged with topic and error details
    And the MqttSource should continue processing
    And subsequent valid messages should be processed normally

  Scenario: Missing required field does not crash
    Given parser configured with location_id_field="serialno"
    When a message arrives without serialno field:
      """json
      {"pm02": 15.5}
      """
    Then the parser should use default_location_id or "unknown"
    And the point should still be created
    And processing should continue
```

---

## 4. Connection Management Acceptance Criteria

### AC-4.1: Single Connection Per Broker

**Requirement**: FR-2.4.1

```gherkin
Feature: Connection Efficiency

  Scenario: Multiple subscriptions share one connection
    Given 5 subscriptions to different topic patterns
    When MqttSource starts
    Then only ONE TCP connection should be opened to the broker
    And all 5 topic patterns should be subscribed on that connection
```

### AC-4.2: Reconnection Behavior

**Requirement**: FR-2.4.2, FR-2.4.3

```gherkin
Feature: Reconnection Handling

  Scenario: Re-subscribe to all topics after reconnection
    Given MqttSource with 3 subscriptions is running
    When the broker connection is lost
    Then reconnection should use exponential backoff
    And upon successful reconnection
    Then ALL 3 topic patterns should be re-subscribed
    And no messages should be lost during brief outage (QoS 1)

  Scenario: Exponential backoff limits
    Given reconnect_delay_secs=1 and max_reconnect_delay_secs=30
    When connection attempts fail repeatedly
    Then delays should be: 1s, 2s, 4s, 8s, 16s, 30s, 30s, 30s...
    And delay should never exceed max_reconnect_delay_secs
```

---

## 5. Performance Acceptance Criteria

### AC-5.1: Throughput

**Requirement**: NFR-3.1.2

```gherkin
Feature: Message Throughput

  Scenario: Handle high message rate
    Given MqttSource with air-quality subscription
    When 1000 messages arrive within 1 second
    Then all messages should be processed
    And processing should complete within 2 seconds
    And no messages should be dropped
```

### AC-5.2: Latency

**Requirement**: NFR-3.1.1

```gherkin
Feature: Processing Latency

  Scenario: Low latency message processing
    Given MqttSource is running and idle
    When a message arrives on subscribed topic
    Then the message should be parsed and sent to ingestion channel
    And end-to-end latency (receive to channel send) should be < 100ms (p95)
```

---

## 6. Observability Acceptance Criteria

### AC-6.1: Health Reporting

**Requirement**: NFR-3.5.1

```gherkin
Feature: Subscription Health Status

  Scenario: Report per-subscription health
    Given MqttSource with 2 subscriptions
    When health_check is called
    Then response should include:
      | subscription | status  | message_count | last_message_at |
      | air-quality  | healthy | 150           | <timestamp>     |
      | homeassistant| healthy | 42            | <timestamp>     |
```

### AC-6.2: Structured Logging

**Requirement**: NFR-3.5.3, NFR-3.5.4

```gherkin
Feature: Structured Logging

  Scenario: Logs include stream context
    Given MqttSource processing messages
    When a message is processed for stream "air-quality"
    Then the log entry should include:
      | field     | value       |
      | stream_id | air-quality |
      | topic     | <topic>     |
      | source    | mqtt        |

  Scenario: Connection events are logged
    When MqttSource connects to broker
    Then INFO log should include "Connected to MQTT broker"
    And log should include broker_url and port
    When connection is lost
    Then WARN log should include "Disconnected from MQTT broker"
```

---

## 7. Integration Acceptance Criteria

### AC-7.1: End-to-End Data Flow

**Requirement**: FR-2.3.2, NFR-3.4.2

```gherkin
Feature: End-to-End Integration

  Scenario: Air-quality data flows to Parquet
    Given running application with multi-subscription MQTT
    When AirGradient sensor publishes to "airgradient/readings/abc123":
      """json
      {"serialno": "abc123", "pm02": 12.5, "atmp": 22.0, "rhum": 55.0}
      """
    Then data should appear in Parquet file at:
      data/bronze/air-quality/YYYY-MM-DD/part-*.parquet
    And querying with DuckDB should return the metrics

  Scenario: HomeAssistant data flows to separate Parquet
    Given running application with multi-subscription MQTT
    When HomeAssistant publishes to "homeassistant/sensor/temp/state":
      """json
      {"entity_id": "sensor.temp", "state": "21.5"}
      """
    Then data should appear in Parquet file at:
      data/bronze/homeassistant/YYYY-MM-DD/part-*.parquet
    And it should NOT appear in air-quality partition
```

### AC-7.2: No Regression

**Requirement**: NFR-3.4.2

```gherkin
Feature: Regression Prevention

  Scenario: Existing air-quality tests pass
    Given the updated MqttSource implementation
    When running: cargo test --package neural-core
    Then all existing mqtt tests should pass
    When running: cargo test --package air-quality-app
    Then all existing tests should pass

  Scenario: Existing dashboards work
    Given updated application deployed
    When Grafana queries air-quality data
    Then dashboards should render correctly
    And data schema should be unchanged
```

---

## 8. Test Matrix Summary

| Category | Test Count | Automation |
|----------|------------|------------|
| Configuration | 5 | Unit Tests |
| Routing | 4 | Unit Tests |
| Parsing | 4 | Unit Tests |
| Connection | 3 | Integration Tests |
| Performance | 2 | Load Tests |
| Observability | 2 | Integration Tests |
| End-to-End | 3 | E2E Tests |
| **Total** | **23** | |

---

## 9. Definition of Done

The feature is complete when:

1. [ ] All 23 acceptance criteria pass
2. [ ] Unit test coverage >= 80%
3. [ ] Integration tests pass in CI
4. [ ] No regression in existing air-quality functionality
5. [ ] Documentation updated (README, CHANGELOG)
6. [ ] Code review approved
7. [ ] Performance benchmarks meet NFR targets
8. [ ] Deployed to development environment
9. [ ] Manual smoke test completed
