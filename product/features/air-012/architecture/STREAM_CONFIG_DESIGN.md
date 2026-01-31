# Stream Configuration Design: home-assistant-state

## Overview

This document defines the stream configuration for ingesting Home Assistant binary sensor state events via MQTT. The configuration follows established NDP patterns from the `air-quality` stream.

## Source: MQTT (Not HTTP Polling)

**Key Decision:** Use MQTT subscription instead of HTTP polling.

| Approach | Status | Rationale |
|----------|--------|-----------|
| HTTP Polling | Superseded | Original design; requires custom parser, polling overhead |
| **MQTT** | **Selected** | Native HA integration, push-based, existing adapter works |

Home Assistant publishes state changes to MQTT automatically. The broker is already accessible:

```bash
mosquitto_sub -h 192.168.52.103 -p 1883 -t "homeassistant/binary_sensor/+/state" -v
```

## Configuration Structure

### File Location

```
config/base/streams/home-assistant-state/config.yaml
```

### Proposed Configuration

```yaml
# Home Assistant State Events Stream Configuration
# GitOps managed - synced to etcd at /streams/home-assistant-state/*

stream_id: "home-assistant-state"
description: "Home Assistant binary sensor state events (doors, windows)"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

# Simple schema - just the state event
fields:
  ndp_id:
    type: "string"
    description: "NDP entity identifier (derived from topic)"
    nullable: false
  source_entity_id:
    type: "string"
    description: "Home Assistant entity ID for traceability"
    nullable: true
  state:
    type: "string"
    description: "Entity state (on/off)"
    nullable: false

# MQTT Source Configuration
sources:
  - type: mqtt
    enabled: true
    ndp_id: "ha_state_events"
    context:
      source_type:
        provider: home_assistant
        purpose: binary_sensor_state
      location:
        coordinates: [29.95838, -81.30878]
        type: indoor
        path: /home
    broker_url: "192.168.52.103"
    port: 1883
    client_id: "ndp-ha-state"
    topic_pattern: "homeassistant/binary_sensor/+/state"
    qos: 1
    reconnect_delay_secs: 1
    max_reconnect_delay_secs: 30
    buffer_capacity: 100
    parser:
      parser_type: ha_binary_state
      ndp_id_from_topic: true      # Extract ndp_id from topic path
      topic_segment: 2             # homeassistant/binary_sensor/{entity}/state
      prefix_strip: "door_"        # Optional: strip prefix for cleaner ndp_id

# Storage configuration (smaller than air-quality due to sparse events)
storage:
  batch_size: 10
  batch_timeout_secs: 60
  buffer_capacity: 100

# Silver ETL Configuration
silver_etl:
  enabled: true
  target_table: silver.state_events
  description: "State change events (simple event log)"
  grain: "One row per state event"

  timestamp:
    source_field: timestamp
    target_field: event_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload
      target_column: state
      type: text
      description: "State value (on/off)"
      nullable: false

    - source_path: topic
      target_column: source_entity_id
      type: text
      description: "Original HA entity from topic"
      nullable: true
      transform: extract_entity_from_topic

  dq_rules:
    - rule: null_check
      field: ndp_id
      action: reject

    - rule: enum_check
      field: state
      allowed_values: ["on", "off"]
      action: flag
      message: "unexpected_state_value"

  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true

  deduplication:
    enabled: true
    key_columns: [event_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: event_time
    lag_interval: 1 minute
```

## Topic-to-ndp_id Mapping

MQTT topic structure:

```
homeassistant/binary_sensor/{entity_name}/state
```

Mapping strategy:

| Topic | Extracted ndp_id |
|-------|------------------|
| `homeassistant/binary_sensor/door_backslider/state` | `door_backslider` |
| `homeassistant/binary_sensor/door_officewindow/state` | `door_officewindow` |
| `homeassistant/binary_sensor/door_dinettewindow/state` | `door_dinettewindow` |

The `ndp_id` preserves Home Assistant's naming for traceability. The `entity_context` dimension provides friendly names and categorization.

## Message Format

Home Assistant publishes simple text payloads:

```
homeassistant/binary_sensor/door_backslider/state off
homeassistant/binary_sensor/door_backslider/state on
```

| Field | Source | Notes |
|-------|--------|-------|
| `event_time` | Ingestion timestamp | When NDP received message |
| `ndp_id` | Topic segment 2 | `door_backslider` |
| `source_entity_id` | Full topic | For debugging |
| `state` | Payload | `on` or `off` |

**Note on Timestamps:** Using ingestion time (when NDP receives). MQTT latency is typically <100ms, acceptable for event correlation.

## Parser Requirements

The `ha_binary_state` parser needs to:

1. Extract `ndp_id` from topic path segment
2. Store raw payload as `state` (simple text: "on" or "off")
3. Capture full topic as `source_entity_id`
4. Generate ingestion timestamp

### Parser Options

**Option A: New Parser Type**

Create `HaBinaryStateParser` specifically for this use case.

**Option B: Extend Existing Parsers**

The existing `flat_json` parser may work with configuration tweaks, but the payload is plain text, not JSON.

**Recommendation:** New lightweight parser (`ha_binary_state`) given the simple payload format.

## Comparison with air-quality Stream

| Aspect | air-quality | home-assistant-state |
|--------|-------------|----------------------|
| Source | MQTT | MQTT |
| Payload | JSON object | Plain text |
| Frequency | ~1 msg/min | Sparse (events only) |
| Fields | 7+ measurements | 1 state value |
| Parser | flat_json | ha_binary_state |
| Batch size | 100 | 10 |
| Timeout | 5 sec | 60 sec |

## Dimension Table Update

Add to `data/dimensions/entity_context.csv`:

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,home/living_room,aq_airgradient_1,south
door_officewindow,window,Office Window,home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,home/dining,aq_airgradient_1,west
```

**Note:** Home Assistant categorizes these as `binary_sensor.door_*` regardless of whether they monitor doors or windows. The `category` in entity_context provides the accurate classification.

## Pipeline Health Configuration

State events are sparse (fire only on change). Configure custom freshness thresholds:

| Status | Threshold | Rationale |
|--------|-----------|-----------|
| Fresh | < 18 hours | Normal operation |
| Stale | 18-36 hours | Worth monitoring |
| Critical | > 36 hours | Likely sensor/connection issue |

Implementation in pipeline health query:

```sql
-- Stream-specific threshold override
SELECT
    'home-assistant-state' AS stream_id,
    MAX(event_time) AS last_event,
    CASE
        WHEN MAX(event_time) > NOW() - INTERVAL '18 hours' THEN 'fresh'
        WHEN MAX(event_time) > NOW() - INTERVAL '36 hours' THEN 'stale'
        ELSE 'critical'
    END AS status
FROM silver.state_events;
```

## Implementation Checklist

### Bronze Layer

- [ ] Create directory `config/base/streams/home-assistant-state/`
- [ ] Write `config.yaml` based on template above
- [ ] Implement `HaBinaryStateParser` (or configure existing parser)
- [ ] Register parser in parser registry
- [ ] Test MQTT subscription with actual broker

### Silver Layer

- [ ] Create `silver.state_events` table (see ADR-001)
- [ ] Configure ETL in stream config
- [ ] Verify data lands correctly

### Dimension

- [ ] Update `entity_context.csv` with 3 sensors
- [ ] Run `./deploy.sh sync-dimensions`

### Pipeline Health

- [ ] Add stream-specific threshold configuration
- [ ] Update dashboard query
- [ ] Verify no false alarms

## Open Questions

1. **Parser registration** - Does `ha_binary_state` need new Rust code, or can config achieve the same?
2. **Topic filtering** - Should we filter to only the 3 configured sensors, or accept all binary_sensor topics?
3. **Authentication** - MQTT broker currently has no auth. Document security considerations.

## References

- `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml` - Reference pattern
- `/workspaces/neural-data-platform/core/src/sources/mqtt/mod.rs` - MQTT source implementation
- `/workspaces/neural-data-platform/product/features/air-012/SCOPE.md` - Feature scope
- ADR-001-simple-event-log.md - Silver schema decision

---

**Last Updated:** 2026-01-30
