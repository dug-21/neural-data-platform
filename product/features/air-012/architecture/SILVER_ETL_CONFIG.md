# air-012: Silver ETL Configuration Design

## Overview

This document specifies the ETL configuration for transforming Home Assistant state events from Bronze (Parquet) to Silver (TimescaleDB). The configuration follows the pattern established in `config/base/streams/air-quality/config.yaml`.

---

## Proposed Stream Configuration

The complete stream config should be placed at:
```
config/base/streams/home-assistant-state/config.yaml
```

### Full Configuration

```yaml
# Home Assistant State Events Stream Configuration
# GitOps managed - synced to etcd at /streams/home-assistant-state/*

stream_id: "home-assistant-state"
description: "Window/door state events from Home Assistant via MQTT"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: "daily"

# Data schema fields (simple for state events)
fields:
  state:
    type: "string"
    description: "Binary state: 'on' (open) or 'off' (closed)"
    nullable: false

# Data sources configuration
sources:
  - type: mqtt
    enabled: true
    ndp_id_pattern: "homeassistant/binary_sensor/{ndp_id}/state"
    context:
      device_type: binary_sensor
      location:
        type: indoor
        path: /home
      environment: indoor
    broker_url: "192.168.52.103"
    port: 1883
    client_id: "ndp-home-assistant-state"
    topic_pattern: "homeassistant/binary_sensor/+/state"
    qos: 1
    reconnect_delay_secs: 1
    max_reconnect_delay_secs: 30
    buffer_capacity: 100
    parser:
      parser_type: raw_text
      ndp_id_from_topic: true
      topic_regex: "homeassistant/binary_sensor/(?P<ndp_id>[^/]+)/state"
      default_tags:
        source: mqtt
        stream_id: home-assistant-state
        ha_domain: binary_sensor

# Entity schemas for data dictionary
entity_schemas:
  - schema_name: ha_binary_sensor
    description: Home Assistant binary sensors (doors, windows, motion)
    device_class: binary_sensor
    attributes:
      - name: state
        type: string
        description: "Binary state value"
        nullable: false
        valid_values: ["on", "off"]

# =============================================================================
# Silver ETL Configuration
# =============================================================================
# Transforms Bronze Parquet data to Silver TimescaleDB
silver_etl:
  enabled: true
  target_table: silver.state_events
  description: "State change events from Home Assistant binary sensors"
  grain: "One row per state change event (sparse - only fires on change)"

  # Timestamp mapping from Bronze to Silver
  timestamp:
    source_field: timestamp
    target_field: event_time
    transform: microseconds_to_timestamp

  # Identity fields (used in deduplication)
  identity_fields:
    - source: ndp_id
      target: ndp_id

  # Field mappings with transforms and DQ rules
  field_mappings:
    # State field - extracted from raw payload
    - source_path: raw_payload
      target_column: state
      type: text
      description: "Binary state: 'on' (open) or 'off' (closed)"
      nullable: false
      transform: trim_lowercase
      dq_rules:
        - rule: value_check
          valid_values: ["on", "off"]
          action: reject
          message: "invalid_state_value"

    # Source entity ID - extracted from MQTT topic
    - source_path: topic
      target_column: source_entity_id
      type: text
      description: "Full Home Assistant entity ID from MQTT topic"
      nullable: true
      transform: extract_entity_id
      transform_config:
        regex: "homeassistant/(?P<domain>[^/]+)/(?P<entity>[^/]+)/state"
        output_template: "{domain}.{entity}"

  # Additional DQ rules (stream-level)
  dq_rules:
    # Freshness check - state events should arrive within minutes
    - rule: freshness_check
      field: event_time
      max_age: "5 minutes"
      max_future: "1 minute"
      reference: ingestion_time
      action: flag
      message: "timestamp_out_of_range"

    # Gap detection - alert on unexpected gaps (but not as aggressively as observations)
    # Note: State events are sparse by nature, so long gaps are normal
    - rule: gap_check
      field: event_time
      partition_by: [ndp_id]
      max_gap: "36 hours"
      action: flag
      message: "large_gap_possible_sensor_issue"

  # DQ output configuration
  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false
    transparency:
      enabled: true
      table: silver.dq_transparency
      include_sample_payload: true
      max_samples_per_rule: 10

  # Deduplication strategy
  deduplication:
    enabled: true
    key_columns: [event_time, ndp_id]
    strategy: upsert
    # MQTT may deliver duplicates on reconnection
    dedupe_window: "1 minute"

  # Incremental processing
  incremental:
    enabled: true
    watermark_column: event_time
    lag_interval: 5 minutes
```

---

## Field Mapping Details

### State Field

| Aspect | Configuration |
|--------|---------------|
| **Source** | `raw_payload` (entire MQTT message body) |
| **Target** | `state` column |
| **Transform** | `trim_lowercase` - trim whitespace, lowercase |
| **Validation** | Must be exactly "on" or "off" |
| **Action on Invalid** | Reject (don't insert) |

The Home Assistant MQTT messages are simple:
```
Topic: homeassistant/binary_sensor/door_backslider/state
Payload: off
```

The entire payload is the state value.

### Source Entity ID

| Aspect | Configuration |
|--------|---------------|
| **Source** | `topic` (MQTT topic path) |
| **Target** | `source_entity_id` column |
| **Transform** | Extract using regex |
| **Output Format** | `binary_sensor.door_backslider` |

Example transformation:
```
Input:  "homeassistant/binary_sensor/door_backslider/state"
Output: "binary_sensor.door_backslider"
```

### ndp_id

| Aspect | Configuration |
|--------|---------------|
| **Source** | Extracted from topic at ingestion |
| **Target** | `ndp_id` column |
| **Example** | `door_backslider`, `door_officewindow` |

The `ndp_id` is extracted from the MQTT topic by the source adapter:
```
Topic: homeassistant/binary_sensor/door_backslider/state
ndp_id: door_backslider
```

---

## DQ Rules Comparison

### air-quality vs home-assistant-state

| Rule Type | air-quality | home-assistant-state |
|-----------|-------------|----------------------|
| **Range Check** | Yes (PM2.5: 0-1000) | N/A (categorical) |
| **Value Check** | No | Yes ("on"/"off" only) |
| **Rate of Change** | Yes (max 100/min for PM2.5) | No (state changes are instant) |
| **Cross-field** | Yes (PM10 >= PM2.5) | No (single field) |
| **Freshness** | Max 2 hours | Max 5 minutes |
| **Gap Detection** | Expected: 1-2 min gaps | Expected: hours between events |

### State Events DQ Rationale

1. **Value Check (reject on invalid)**
   - State must be exactly "on" or "off"
   - Invalid values indicate parsing error or Home Assistant misconfiguration
   - Reject rather than flag because invalid state is meaningless

2. **Freshness Check (5 minutes)**
   - MQTT latency should be <100ms
   - If message is >5 min old, something is wrong with broker/network
   - Flag but don't reject (data still usable)

3. **Gap Detection (36 hours)**
   - State events are inherently sparse
   - A window can stay closed for days with no events
   - 36-hour threshold only flags potential sensor/connection issues
   - Matches pipeline health thresholds in SCOPE.md

---

## Deduplication Strategy

### Why Deduplication is Needed

MQTT QoS 1 guarantees at-least-once delivery, which means:
1. On broker reconnection, messages may be redelivered
2. Network hiccups can cause duplicate publishes
3. Home Assistant may republish retained messages

### Deduplication Configuration

```yaml
deduplication:
  enabled: true
  key_columns: [event_time, ndp_id]
  strategy: upsert
  dedupe_window: "1 minute"
```

| Aspect | Configuration | Rationale |
|--------|---------------|-----------|
| **Key Columns** | `event_time, ndp_id` | Matches primary key |
| **Strategy** | `upsert` | Later message wins (same timestamp) |
| **Dedupe Window** | 1 minute | Duplicates arrive within seconds |

### Edge Case: Rapid State Changes

If a door opens and closes within the same second:
- Both events have different `event_time` values (microsecond precision)
- No deduplication occurs (correct behavior)
- Both events are stored

---

## Transform Implementations

### trim_lowercase

```rust
fn transform_trim_lowercase(value: &str) -> String {
    value.trim().to_lowercase()
}
```

Handles edge cases:
- `" ON "` -> `"on"`
- `"Off\n"` -> `"off"`

### extract_entity_id

```rust
fn transform_extract_entity_id(topic: &str) -> Option<String> {
    let re = Regex::new(r"homeassistant/(?P<domain>[^/]+)/(?P<entity>[^/]+)/state")?;
    let caps = re.captures(topic)?;
    Some(format!("{}.{}", &caps["domain"], &caps["entity"]))
}
```

Example:
- `"homeassistant/binary_sensor/door_backslider/state"` -> `"binary_sensor.door_backslider"`

---

## Incremental Processing

### Watermark Strategy

```yaml
incremental:
  enabled: true
  watermark_column: event_time
  lag_interval: 5 minutes
```

The ETL job:
1. Tracks last processed `event_time` as watermark
2. Queries Bronze Parquet for records after watermark - 5 minutes
3. Processes and upserts to Silver
4. Updates watermark on success

### Recovery from Failure

If ETL fails mid-batch:
- Next run starts from watermark (not failed batch end)
- 5-minute lag ensures late arrivals are caught
- Upsert strategy handles duplicates gracefully

---

## Entity Dimension Integration

The 3 initial sensors should be added to `data/dimensions/entity_context.csv`:

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```

The `category` column enables:
- Dashboard filtering by door vs window
- Future correlation analysis (dp-014)
- Proper labeling in Grafana

---

## File Locations

| File | Purpose |
|------|---------|
| `config/base/streams/home-assistant-state/config.yaml` | Stream configuration |
| `deploy/timescaledb/init/002_state_events_schema.sql` | Silver DDL |
| `data/dimensions/entity_context.csv` | Entity metadata |

---

## Related Documents

- `SILVER_SCHEMA.md` - DDL specification
- `SCOPE.md` - Feature requirements
- `config/base/streams/air-quality/config.yaml` - Pattern reference
