# ADR-002: Entity Schema Configuration Format

**Status**: Proposed
**Date**: 2025-12-30
**Decision Makers**: NDP Architecture Team
**Context**: DP-002 Online Data Dictionary and HomeAssistant Stream Preparation
**Supersedes**: None

---

## Context

HomeAssistant integration (AIR-008) requires a flexible way to define **entity schemas** that map HA entity patterns to NDP field definitions. Unlike traditional streams with fixed fields (e.g., `air-quality` with `pm25`, `temperature`), HomeAssistant streams have:

1. **Dynamic Entities**: New devices added at runtime (window sensors, motion detectors)
2. **Pattern-Based Matching**: Entity IDs follow patterns (`binary_sensor.*_window*`)
3. **Domain-Specific Handling**: Different domains have different data types (`binary_sensor` = boolean, `sensor` = numeric)
4. **Protocol Metadata**: Matter/Thread, Zigbee, Z-Wave, WiFi devices have protocol-specific attributes

### Requirements

1. **Glob Pattern Matching**: Match entity IDs using wildcards (`*`, `?`)
2. **State Mapping**: Convert HA states (`on`/`off`, `open`/`closed`) to numeric values
3. **Field Inheritance**: Entity schemas should inherit from and extend base `fields` definition
4. **Priority Ordering**: More specific patterns should match before generic ones
5. **YAML-Native**: Configuration should follow existing NDP YAML conventions

---

## Decision

**Introduce an `entity_schemas` section in stream configuration with glob pattern matching and state mapping.**

### Configuration Format

```yaml
# config/base/streams/home-events/config.yaml

stream_id: home-events
description: "Home automation events from Matter/Thread sensors via Home Assistant"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

# ==============================================================================
# STANDARD FIELDS (Applies to all records in this stream)
# These fields are ALWAYS present in every record, regardless of entity
# ==============================================================================
fields:
  - name: event_id
    field_type: String
    nullable: false
    description: "Unique event identifier (UUID)"

  - name: timestamp
    field_type: Int
    nullable: false
    unit: "epoch_ms"
    description: "Event timestamp (last_changed from HA)"

  - name: event_type
    field_type: String
    nullable: false
    description: "Event type (state_changed, automation_triggered)"

  - name: entity_id
    field_type: String
    nullable: false
    description: "Home Assistant entity ID (e.g., binary_sensor.window_living_room)"

  - name: entity_domain
    field_type: String
    nullable: false
    description: "Entity domain (binary_sensor, sensor, switch)"

  - name: new_state
    field_type: String
    nullable: false
    description: "New state value (on, off, 23.5, etc.)"

  - name: old_state
    field_type: String
    nullable: true
    description: "Previous state value"

  - name: state_numeric
    field_type: Float
    nullable: true
    description: "Numeric conversion of state (for binary: on=1.0, off=0.0)"

  - name: attributes
    field_type: Json
    nullable: true
    description: "Entity attributes (battery_level, device_class, etc.)"

# ==============================================================================
# ENTITY SCHEMAS (Pattern-based entity handling)
# Defines how to process entities matching glob patterns
# ==============================================================================
entity_schemas:

  # ---------------------------------------------------------------------------
  # WINDOW SENSORS (Matter/Thread)
  # Matches: binary_sensor.window_living_room, binary_sensor.aqara_p2_window_office
  # ---------------------------------------------------------------------------
  - pattern: "binary_sensor.*window*"
    domain: binary_sensor
    device_class: window
    description: "Window open/close sensors (Matter/Thread)"
    protocol: matter_thread
    enabled: true
    priority: 10
    state_mapping:
      "on": 1.0      # Window OPEN
      "off": 0.0     # Window CLOSED
      "open": 1.0    # Alternative state representation
      "closed": 0.0
    attributes_include:
      - battery_level
      - device_class
      - friendly_name
    tags:
      category: "security"
      location_derivation: "entity_id"  # Derive location from entity ID

  # ---------------------------------------------------------------------------
  # DOOR SENSORS
  # Matches: binary_sensor.front_door, binary_sensor.garage_door_sensor
  # ---------------------------------------------------------------------------
  - pattern: "binary_sensor.*door*"
    domain: binary_sensor
    device_class: door
    description: "Door open/close sensors"
    protocol: matter_thread
    enabled: true
    priority: 10
    state_mapping:
      "on": 1.0      # Door OPEN
      "off": 0.0     # Door CLOSED
    attributes_include:
      - battery_level
      - device_class

  # ---------------------------------------------------------------------------
  # MOTION SENSORS
  # Matches: binary_sensor.motion_hallway, binary_sensor.pir_sensor_kitchen
  # ---------------------------------------------------------------------------
  - pattern: "binary_sensor.*motion*"
    domain: binary_sensor
    device_class: motion
    description: "Motion detection sensors"
    protocol: matter_thread
    enabled: true
    priority: 10
    state_mapping:
      "on": 1.0      # Motion DETECTED
      "off": 0.0     # No motion
      "detected": 1.0
      "clear": 0.0

  - pattern: "binary_sensor.*pir*"
    domain: binary_sensor
    device_class: motion
    description: "PIR motion sensors (alias pattern)"
    protocol: zigbee
    enabled: true
    priority: 5  # Lower priority than explicit motion pattern
    state_mapping:
      "on": 1.0
      "off": 0.0

  # ---------------------------------------------------------------------------
  # TEMPERATURE SENSORS
  # Matches: sensor.temperature_living_room, sensor.aqara_temperature
  # ---------------------------------------------------------------------------
  - pattern: "sensor.*temperature*"
    domain: sensor
    device_class: temperature
    description: "Temperature sensors"
    unit_of_measurement: "C"
    enabled: true
    priority: 10
    # No state_mapping - value is already numeric
    state_conversion: float  # Parse state as float directly
    attributes_include:
      - battery_level
      - unit_of_measurement

  # ---------------------------------------------------------------------------
  # HUMIDITY SENSORS
  # ---------------------------------------------------------------------------
  - pattern: "sensor.*humidity*"
    domain: sensor
    device_class: humidity
    description: "Humidity sensors"
    unit_of_measurement: "%"
    enabled: true
    priority: 10
    state_conversion: float

  # ---------------------------------------------------------------------------
  # BATTERY LEVEL (from any device)
  # Lower priority - matches when no more specific pattern matches
  # ---------------------------------------------------------------------------
  - pattern: "sensor.*battery*"
    domain: sensor
    device_class: battery
    description: "Battery level sensors"
    unit_of_measurement: "%"
    enabled: true
    priority: 5
    state_conversion: float

  # ---------------------------------------------------------------------------
  # CATCH-ALL BINARY SENSORS
  # Lowest priority - handles any unmatched binary_sensor entities
  # ---------------------------------------------------------------------------
  - pattern: "binary_sensor.*"
    domain: binary_sensor
    description: "Generic binary sensors (catch-all)"
    enabled: true
    priority: 0  # Lowest priority
    state_mapping:
      "on": 1.0
      "off": 0.0
      "true": 1.0
      "false": 0.0

  # ---------------------------------------------------------------------------
  # CATCH-ALL SENSORS
  # ---------------------------------------------------------------------------
  - pattern: "sensor.*"
    domain: sensor
    description: "Generic sensors (catch-all)"
    enabled: true
    priority: 0
    state_conversion: float_or_null  # Try float, null if fails

# ==============================================================================
# SOURCES
# ==============================================================================
sources:
  - id: homeassistant-mqtt
    type: mqtt
    enabled: true
    config:
      broker_url: "${MQTT_BROKER_URL}"
      port: 1883
      client_id: "ndp-home-events"
      topics:
        - "homeassistant/+/+/state"
        - "homeassistant/+/+/attributes"
      qos: 1
    parser:
      parser_type: home_assistant_mqtt
      topic_pattern: "homeassistant/{domain}/{entity}/{attribute}"
      use_entity_schemas: true  # Enable entity schema matching
```

### Pattern Matching Syntax

Entity patterns use **glob syntax** (not regex) for simplicity:

| Pattern | Matches | Does Not Match |
|---------|---------|----------------|
| `binary_sensor.*window*` | `binary_sensor.window_living_room`, `binary_sensor.aqara_p2_window_office` | `sensor.window_temperature` |
| `sensor.temperature_*` | `sensor.temperature_kitchen`, `sensor.temperature_outdoor` | `sensor.humidity_kitchen` |
| `binary_sensor.door_?` | `binary_sensor.door_1`, `binary_sensor.door_a` | `binary_sensor.door_front` |
| `*battery*` | `sensor.battery_level`, `sensor.phone_battery` | - |

**Wildcards**:
- `*` - Matches zero or more characters
- `?` - Matches exactly one character

### Priority Resolution

When an entity matches multiple patterns, the highest priority wins:

```
Entity: binary_sensor.motion_hallway

Patterns:
  1. binary_sensor.*motion*  (priority: 10) <- MATCHES
  2. binary_sensor.*pir*     (priority: 5)  <- Does not match
  3. binary_sensor.*         (priority: 0)  <- Would match, but lower priority

Result: Uses binary_sensor.*motion* schema
```

### State Mapping vs State Conversion

**State Mapping** (for discrete states):
```yaml
state_mapping:
  "on": 1.0
  "off": 0.0
```
Lookup table for string-to-numeric conversion.

**State Conversion** (for numeric states):
```yaml
state_conversion: float  # or: int, float_or_null
```
Direct type conversion of state value.

---

## Rationale

### Why Glob Patterns Over Regex

| Criterion | Glob | Regex |
|-----------|------|-------|
| **Readability** | High | Medium |
| **User Familiarity** | File patterns | Developer-only |
| **Error Prone** | Low | High (escaping issues) |
| **Performance** | Fast (simple matching) | Variable |
| **Sufficient for Use Case** | Yes | Overkill |

**Decision**: Glob patterns are sufficient for entity ID matching and more user-friendly.

### Why Separate entity_schemas from fields

| Aspect | Using entity_schemas | Extending fields |
|--------|---------------------|------------------|
| **Separation of Concerns** | Entity handling vs field definition | Mixed |
| **Pattern Matching** | Native support | Would need conditional logic |
| **Protocol Metadata** | Natural place | Awkward in field definition |
| **Migration** | Additive (new section) | Changes existing structure |

**Decision**: Separate section maintains clean separation between schema and entity handling.

### Why Priority-Based Resolution

**Alternative**: First-match wins (order-dependent)

**Problem**: YAML map order not guaranteed in all parsers; harder to reason about

**Solution**: Explicit `priority` field makes resolution deterministic and debuggable

---

## Consequences

### Positive

1. **Flexible Entity Handling**: New entity types added via config, not code
2. **Protocol Awareness**: Matter/Thread, Zigbee, Z-Wave can have different handling
3. **State Normalization**: All states converted to numeric for analytics
4. **Priority Control**: Specific patterns override generic catch-alls
5. **Backward Compatible**: Existing streams without entity_schemas unchanged

### Negative

1. **Pattern Complexity**: Many overlapping patterns could be confusing
2. **No Runtime Validation**: Invalid patterns discovered at match time
3. **Ordering Sensitivity**: Priority values must be carefully managed

### Risks

1. **Pattern Conflicts**: Two patterns with same priority, both matching
   - **Mitigation**: Lint tool to detect conflicts; warning on duplicate priority
2. **State Mapping Gaps**: Unknown state value not in mapping
   - **Mitigation**: Log warning, use null for state_numeric

---

## Alternatives Considered

### Alternative 1: JSON Schema Validation

Use JSON Schema to define entity patterns and mappings.

```json
{
  "entityPatterns": [
    {
      "pattern": "binary_sensor\\..*window.*",
      "type": "object",
      "properties": {
        "state": { "enum": ["on", "off"] }
      }
    }
  ]
}
```

**Rejected because**:
- Verbose for simple pattern matching
- JSON Schema regex escaping is error-prone
- Not aligned with existing YAML conventions

### Alternative 2: Domain-Only Matching

Match only on domain (binary_sensor, sensor, switch) without entity ID patterns.

```yaml
domain_schemas:
  binary_sensor:
    state_mapping:
      "on": 1.0
      "off": 0.0
```

**Rejected because**:
- Too coarse - can't distinguish window sensors from door sensors
- Loses device class information
- No protocol differentiation

### Alternative 3: Per-Entity Configuration

Explicit configuration for each entity.

```yaml
entities:
  binary_sensor.window_living_room:
    device_class: window
    protocol: matter_thread
  binary_sensor.window_kitchen:
    device_class: window
    protocol: matter_thread
```

**Rejected because**:
- Doesn't scale with many entities
- Requires config update for each new device
- Maintenance burden

---

## Migration Strategy

### Existing Streams

Existing streams (`air-quality`, `weather`) have no `entity_schemas` section and continue to work unchanged. The parser only uses entity_schemas when `use_entity_schemas: true` is set.

### New HomeAssistant Stream

1. Create `config/base/streams/home-events/config.yaml` with entity_schemas
2. Deploy via existing GitOps workflow
3. Entities matching patterns are processed; others logged but skipped

### Adding New Entity Types

1. Add new pattern to entity_schemas section
2. Run `deploy.sh sync` to update etcd
3. New entities immediately matched on next event

---

## Implementation Impact

### Files to Create

- `core/src/sources/parsers/entity_matcher.rs` - Glob pattern matching
- `core/src/types/entity_schema.rs` - EntitySchema struct

### Files to Modify

- `core/src/sources/parsers/home_assistant.rs` - Use entity_schemas for parsing
- `core/src/types/stream_config.rs` - Add EntitySchema to StreamConfig

### Configuration Changes

- Add `entity_schemas` section to home-events stream config
- Update schema validation to support new section

---

## Related Decisions

- **ADR-001 (DP-002)**: TimescaleDB Schema Design (entity_schemas table)
- **ADR-003 (DP-002)**: Sync Mechanism (syncs entity_schemas to TimescaleDB)
- **ADR-005 (AIR-008)**: Hybrid Event-State Model

---

## References

- [Glob Pattern Syntax](https://en.wikipedia.org/wiki/Glob_(programming))
- [Home Assistant Entity ID Naming](https://www.home-assistant.io/docs/configuration/customizing-devices/#entity_id)
- [Matter Device Classes](https://www.home-assistant.io/integrations/matter/#supported-device-types)

---

**Last Updated**: 2025-12-30
**Next Review**: After home-events stream implementation
