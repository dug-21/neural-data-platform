# Configuration Parser Algorithm - DP-003

## Overview

The configuration parser handles loading MQTT source configurations from YAML, with backward compatibility for legacy single-topic format and migration to the new multi-subscription format.

## Data Structures

```
STRUCTURE SubscriptionConfig:
    stream_id: String              // Target stream (e.g., "air-quality")
    topic_pattern: String          // MQTT pattern (e.g., "airgradient/readings/+")
    parser: Optional<ParserConfig> // Parser for this subscription
    enabled: Boolean               // Whether subscription is active (default: true)

STRUCTURE MqttConfig:
    // Connection settings
    broker_url: String             // MQTT broker hostname (e.g., "mosquitto")
    port: Integer                  // MQTT port (default: 1883)
    client_id: String              // Unique client identifier
    qos: Integer                   // Quality of Service (0, 1, or 2)

    // Reconnection settings
    reconnect_delay_secs: Integer  // Initial reconnect delay (default: 1)
    max_reconnect_delay_secs: Integer  // Maximum backoff (default: 30)

    // Buffer settings
    buffer_capacity: Integer       // Internal buffer size (default: 1000)

    // NEW: Multiple subscriptions
    subscriptions: Array<SubscriptionConfig>

    // DEPRECATED: Legacy single topic
    topic_pattern: Optional<String>

STRUCTURE ParserConfig:
    parser_type: String            // "flat_json", "array_iterator", etc.
    location_id_field: String      // Field containing location ID
    skip_fields: Array<String>     // Fields to exclude from metrics
    default_tags: Map<String, String>  // Tags to add to all points
```

---

## Algorithm 1: Parse YAML Configuration

Parses YAML configuration file into MqttConfig structure.

### Input/Output

```
INPUT:
    yaml_content: String    // Raw YAML file content

OUTPUT:
    MqttConfig OR Error
```

### Algorithm

```
ALGORITHM: parse_mqtt_config
INPUT: yaml_content (String)
OUTPUT: MqttConfig OR Error

BEGIN
    // Step 1: Parse YAML into generic structure
    TRY
        raw_config <- yaml_parse(yaml_content)
    CATCH parse_error
        RETURN Error("Invalid YAML syntax: {parse_error}")
    END TRY

    // Step 2: Extract source configuration
    sources <- raw_config["sources"]
    IF sources is None OR sources is empty THEN
        RETURN Error("No sources defined in configuration")
    END IF

    // Step 3: Find MQTT source
    mqtt_source <- None
    FOR EACH source IN sources DO
        IF source["type"] == "mqtt" THEN
            mqtt_source <- source
            BREAK
        END IF
    END FOR

    IF mqtt_source is None THEN
        RETURN Error("No MQTT source found in configuration")
    END IF

    // Step 4: Parse connection settings
    params <- mqtt_source["params"]
    IF params is None THEN
        RETURN Error("MQTT source missing 'params' section")
    END IF

    config <- MqttConfig {
        broker_url: params["broker_url"] OR Error("Missing broker_url"),
        port: params["port"] OR 1883,
        client_id: params["client_id"] OR Error("Missing client_id"),
        qos: params["qos"] OR 1,
        reconnect_delay_secs: params["reconnect_delay_secs"] OR 1,
        max_reconnect_delay_secs: params["max_reconnect_delay_secs"] OR 30,
        buffer_capacity: params["buffer_capacity"] OR 1000,
        subscriptions: [],
        topic_pattern: None
    }

    // Step 5: Parse subscriptions (new format)
    IF params["subscriptions"] exists THEN
        FOR EACH sub IN params["subscriptions"] DO
            subscription <- parse_subscription(sub)
            IF subscription is Error THEN
                RETURN subscription
            END IF
            config.subscriptions.append(subscription)
        END FOR
    END IF

    // Step 6: Parse legacy topic_pattern (backward compatibility)
    IF params["topic_pattern"] exists THEN
        config.topic_pattern <- params["topic_pattern"]
    END IF

    // Step 7: Handle source-level parser (legacy format)
    IF mqtt_source["parser"] exists AND config.subscriptions is empty THEN
        // Legacy format: parser at source level
        config.legacy_parser <- parse_parser_config(mqtt_source["parser"])
    END IF

    // Step 8: Validate configuration
    validation <- validate_mqtt_config(config)
    IF validation is Error THEN
        RETURN validation
    END IF

    RETURN config
END
```

---

## Algorithm 2: Parse Subscription Configuration

Parses a single subscription entry.

### Input/Output

```
INPUT:
    raw_sub: Map     // Raw YAML subscription object

OUTPUT:
    SubscriptionConfig OR Error
```

### Algorithm

```
ALGORITHM: parse_subscription
INPUT: raw_sub (Map)
OUTPUT: SubscriptionConfig OR Error

BEGIN
    // Step 1: Extract required fields
    stream_id <- raw_sub["stream_id"]
    IF stream_id is None OR stream_id is empty THEN
        RETURN Error("Subscription missing stream_id")
    END IF

    topic_pattern <- raw_sub["topic_pattern"]
    IF topic_pattern is None OR topic_pattern is empty THEN
        RETURN Error("Subscription '{stream_id}' missing topic_pattern")
    END IF

    // Step 2: Extract optional fields with defaults
    enabled <- raw_sub["enabled"]
    IF enabled is None THEN
        enabled <- true    // Default: enabled
    END IF

    // Step 3: Parse parser configuration if present
    parser_config <- None
    IF raw_sub["parser"] exists THEN
        parser_config <- parse_parser_config(raw_sub["parser"])
        IF parser_config is Error THEN
            RETURN Error("Invalid parser config for '{stream_id}': {parser_config.message}")
        END IF
    END IF

    // Step 4: Create subscription
    RETURN SubscriptionConfig {
        stream_id: stream_id,
        topic_pattern: topic_pattern,
        parser: parser_config,
        enabled: enabled
    }
END
```

---

## Algorithm 3: Get All Subscriptions (Backward Compatibility)

Returns all subscriptions, including legacy format conversion.

### Input/Output

```
INPUT:
    config: MqttConfig

OUTPUT:
    Array<SubscriptionConfig>
```

### Algorithm

```
ALGORITHM: get_subscriptions
INPUT: config (MqttConfig)
OUTPUT: Array<SubscriptionConfig>

BEGIN
    subscriptions <- copy(config.subscriptions)

    // Step 1: Check for legacy topic_pattern
    IF config.topic_pattern is not None THEN
        // Log deprecation warning
        log_warn("DEPRECATED: topic_pattern field is deprecated, use subscriptions array")

        // Step 2: Check if pattern already exists in subscriptions
        pattern_exists <- false
        FOR EACH sub IN subscriptions DO
            IF sub.topic_pattern == config.topic_pattern THEN
                pattern_exists <- true
                BREAK
            END IF
        END FOR

        // Step 3: Add legacy pattern as subscription if not duplicate
        IF NOT pattern_exists THEN
            legacy_sub <- SubscriptionConfig {
                stream_id: "legacy",          // Default stream ID for legacy
                topic_pattern: config.topic_pattern,
                parser: config.legacy_parser, // Use source-level parser
                enabled: true
            }
            subscriptions.append(legacy_sub)
            log_info("Converted legacy topic_pattern to subscription: {config.topic_pattern}")
        END IF
    END IF

    RETURN subscriptions
END
```

---

## Algorithm 4: Validate Configuration

Validates the complete MQTT configuration.

### Input/Output

```
INPUT:
    config: MqttConfig

OUTPUT:
    Success OR Error
```

### Algorithm

```
ALGORITHM: validate_mqtt_config
INPUT: config (MqttConfig)
OUTPUT: Success OR Error

BEGIN
    // Step 1: Get all subscriptions (including legacy)
    subscriptions <- get_subscriptions(config)

    // Step 2: Validate at least one subscription exists
    IF subscriptions is empty THEN
        RETURN Error("ConfigError::NoSubscriptions - At least one subscription required")
    END IF

    // Step 3: Validate stream_id uniqueness
    seen_stream_ids <- empty set
    FOR EACH sub IN subscriptions DO
        IF sub.stream_id IN seen_stream_ids THEN
            RETURN Error("ConfigError::DuplicateStreamId - Stream ID '{sub.stream_id}' used multiple times")
        END IF
        seen_stream_ids.add(sub.stream_id)
    END FOR

    // Step 4: Validate each topic pattern
    FOR EACH sub IN subscriptions DO
        validation <- validate_topic_pattern(sub.topic_pattern)
        IF validation is Error THEN
            RETURN Error("ConfigError::InvalidTopicPattern for '{sub.stream_id}': {validation.message}")
        END IF
    END FOR

    // Step 5: Validate connection settings
    IF config.broker_url is empty THEN
        RETURN Error("ConfigError::MissingBrokerUrl")
    END IF

    IF config.client_id is empty THEN
        RETURN Error("ConfigError::MissingClientId")
    END IF

    IF config.qos < 0 OR config.qos > 2 THEN
        RETURN Error("ConfigError::InvalidQoS - Must be 0, 1, or 2")
    END IF

    IF config.port < 1 OR config.port > 65535 THEN
        RETURN Error("ConfigError::InvalidPort")
    END IF

    // Step 6: Validate buffer settings
    IF config.buffer_capacity < 1 THEN
        RETURN Error("ConfigError::InvalidBufferCapacity - Must be >= 1")
    END IF

    IF config.reconnect_delay_secs < 1 THEN
        RETURN Error("ConfigError::InvalidReconnectDelay - Must be >= 1")
    END IF

    // Step 7: All validations passed
    RETURN Success
END
```

---

## Algorithm 5: Validate Topic Pattern

Validates an MQTT topic pattern.

### Input/Output

```
INPUT:
    pattern: String

OUTPUT:
    Success OR Error
```

### Algorithm

```
ALGORITHM: validate_topic_pattern
INPUT: pattern (String)
OUTPUT: Success OR Error

BEGIN
    // Rule 1: Pattern cannot be empty
    IF pattern is empty THEN
        RETURN Error("Pattern cannot be empty")
    END IF

    // Rule 2: Pattern cannot start with '/'
    IF pattern starts with '/' THEN
        RETURN Error("Pattern cannot start with /")
    END IF

    // Rule 3: Pattern cannot end with '/' (except "/#")
    IF pattern ends with '/' AND NOT pattern ends with '/#' THEN
        RETURN Error("Pattern cannot end with /")
    END IF

    // Rule 4: No empty segments (double slashes)
    IF pattern contains '//' THEN
        RETURN Error("Pattern cannot contain empty segments (//)")
    END IF

    // Rule 5: '#' must be at end of pattern
    IF pattern contains '#' THEN
        IF NOT (pattern == '#' OR pattern ends with '/#') THEN
            RETURN Error("# wildcard must be at end of pattern")
        END IF

        // Rule 6: Only one '#' allowed
        count <- count occurrences of '#' in pattern
        IF count > 1 THEN
            RETURN Error("Multiple # wildcards not allowed")
        END IF
    END IF

    // Rule 7: '+' cannot be mixed with other characters in segment
    segments <- split pattern by '/'
    FOR EACH segment IN segments DO
        IF segment contains '+' AND segment != '+' THEN
            RETURN Error("+ wildcard must be alone in segment, found: {segment}")
        END IF
    END FOR

    RETURN Success
END
```

---

## Configuration Format Examples

### New Format (Recommended)

```yaml
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
          enabled: true
          parser:
            parser_type: flat_json
            location_id_field: entity_id
            default_tags:
              source: mqtt
              stream_id: homeassistant
```

### Legacy Format (Backward Compatible)

```yaml
sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "air-quality-app"
      topic_pattern: "airgradient/readings/+"  # DEPRECATED
      qos: 1
    parser:
      parser_type: flat_json
      location_id_field: serialno
```

---

## Migration Path

### Phase 1: Detection and Warning

```
ALGORITHM: check_legacy_format
INPUT: config (MqttConfig)
OUTPUT: Array<MigrationWarning>

BEGIN
    warnings <- empty array

    // Check for deprecated topic_pattern
    IF config.topic_pattern is not None THEN
        warnings.append(MigrationWarning {
            type: "deprecated_field",
            message: "topic_pattern is deprecated, use subscriptions array",
            line: get_yaml_line("topic_pattern"),
            suggestion: generate_migration_yaml(config)
        })
    END IF

    // Check for source-level parser with subscriptions
    IF config.legacy_parser exists AND config.subscriptions is not empty THEN
        warnings.append(MigrationWarning {
            type: "conflicting_config",
            message: "Source-level parser ignored when subscriptions array is present",
            suggestion: "Move parser config into each subscription"
        })
    END IF

    RETURN warnings
END
```

### Phase 2: Automatic Migration Helper

```
ALGORITHM: generate_migration_yaml
INPUT: legacy_config (MqttConfig)
OUTPUT: String (new format YAML)

BEGIN
    new_config <- MqttConfig {
        broker_url: legacy_config.broker_url,
        port: legacy_config.port,
        client_id: legacy_config.client_id,
        qos: legacy_config.qos,
        reconnect_delay_secs: legacy_config.reconnect_delay_secs,
        max_reconnect_delay_secs: legacy_config.max_reconnect_delay_secs,
        buffer_capacity: legacy_config.buffer_capacity,
        subscriptions: [{
            stream_id: infer_stream_id(legacy_config.topic_pattern),
            topic_pattern: legacy_config.topic_pattern,
            parser: legacy_config.legacy_parser,
            enabled: true
        }]
    }

    RETURN yaml_serialize(new_config)
END

FUNCTION: infer_stream_id
INPUT: topic_pattern (String)
OUTPUT: String

BEGIN
    // Extract first segment as stream hint
    // "airgradient/readings/+" -> "airgradient"
    // "homeassistant/+/+/state" -> "homeassistant"

    segments <- split topic_pattern by '/'
    IF segments[0] contains '+' OR segments[0] contains '#' THEN
        RETURN "legacy"
    END IF
    RETURN segments[0]
END
```

---

## Error Handling

### Configuration Errors

| Error Type | Cause | Recovery |
|------------|-------|----------|
| `NoSubscriptions` | No subscriptions and no legacy pattern | Add subscriptions to config |
| `DuplicateStreamId` | Same stream_id used twice | Use unique stream IDs |
| `InvalidTopicPattern` | Malformed MQTT pattern | Fix pattern syntax |
| `MissingBrokerUrl` | broker_url not specified | Add broker_url to params |
| `MissingClientId` | client_id not specified | Add client_id to params |
| `InvalidQoS` | QoS not 0, 1, or 2 | Use valid QoS value |

### Error Messages

```
EXAMPLES:

Error: ConfigError::NoSubscriptions
Message: "At least one subscription is required. Add a subscriptions array or topic_pattern."

Error: ConfigError::DuplicateStreamId("air-quality")
Message: "Stream ID 'air-quality' is used by multiple subscriptions. Each subscription must have a unique stream_id."

Error: ConfigError::InvalidTopicPattern("sensors/#/temp")
Message: "Invalid topic pattern 'sensors/#/temp': # wildcard must be at end of pattern."
```

---

## Complexity Analysis

| Algorithm | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| `parse_mqtt_config` | O(n) where n = config size | O(n) for parsed structure |
| `parse_subscription` | O(1) | O(1) |
| `get_subscriptions` | O(k) where k = subscription count | O(k) |
| `validate_mqtt_config` | O(k * m) where m = avg pattern length | O(k) for seen set |
| `validate_topic_pattern` | O(m) where m = pattern length | O(m) for segments |

---

## Test Cases

```
TEST: parse_new_format_config
    INPUT: YAML with subscriptions array
    EXPECTED: MqttConfig with populated subscriptions array

TEST: parse_legacy_format_config
    INPUT: YAML with topic_pattern field
    EXPECTED:
        - MqttConfig with topic_pattern set
        - get_subscriptions() returns 1 subscription
        - Deprecation warning logged

TEST: parse_mixed_format_error
    INPUT: YAML with both subscriptions AND topic_pattern
    EXPECTED:
        - Warning about mixed format
        - Legacy pattern NOT added (subscriptions take precedence)

TEST: validation_duplicate_stream_id
    INPUT: Config with two subscriptions having same stream_id
    EXPECTED: Error("DuplicateStreamId")

TEST: validation_invalid_pattern
    INPUT: Config with pattern "sensors/#/temp"
    EXPECTED: Error("InvalidTopicPattern")

TEST: validation_empty_subscriptions
    INPUT: Config with empty subscriptions and no topic_pattern
    EXPECTED: Error("NoSubscriptions")
```

---

## Related Documents

- ADR-002-CONFIG-FORMAT.md: Configuration format decisions
- TOPIC_ROUTER.md: Pattern validation and routing
- MESSAGE_PROCESSOR.md: How config is used at runtime
