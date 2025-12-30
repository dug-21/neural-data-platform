# Matter/Thread Device Integration for Neural Data Platform

**Feature**: AIR-008 Home Events (Extended for Matter/Thread Sensors)
**Date**: 2025-12-30
**Author**: Research Agent
**Status**: Analysis Complete

---

## Executive Summary

This document provides a comprehensive analysis of integrating Matter/Thread window sensors (specifically the Aqara P2 Door and Window Sensor) into the Neural Data Platform via Home Assistant. The analysis covers protocol fundamentals, data flow architecture, and integration with NDP's existing event-state-hybrid pattern.

### Key Findings

1. **Matter/Thread devices connect to HA through Thread Border Router** - HA can serve as both Matter controller and Thread Border Router
2. **Data is exposed via MQTT Statestream or WebSocket API** - Both options integrate with existing NDP patterns
3. **Window sensors report binary states** - `on`/`off` (or `open`/`closed`) with timestamps and attributes
4. **Event-state-hybrid pattern applies** - Store events in Bronze, materialize state views in Silver

---

## 1. Matter/Thread Protocol Overview

### 1.1 What is Matter?

[Matter](https://www.home-assistant.io/integrations/matter/) is an open-source smart home standard that defines how devices communicate over local networks. Key characteristics:

| Aspect | Description |
|--------|-------------|
| **Protocol Layer** | Application-level control protocol over IPv6 |
| **Transport** | Wi-Fi, Ethernet, or Thread (low-power mesh) |
| **Local-First** | No cloud dependency for basic operation |
| **Interoperability** | Works across Apple, Google, Amazon, Samsung ecosystems |

**Critical Distinction**: Matter is NOT a radio protocol - it runs on top of existing network infrastructure (Wi-Fi or Thread).

### 1.2 What is Thread?

[Thread](https://www.home-assistant.io/integrations/thread/) is a low-power mesh networking protocol designed for IoT devices:

| Characteristic | Value |
|----------------|-------|
| **Power** | Low-power (ideal for battery devices) |
| **Bandwidth** | Low (switches, sensors - not cameras) |
| **Topology** | Mesh networking with multiple border routers |
| **Protocol** | IPv6-based |
| **Range** | Extended via mesh (devices relay for each other) |

### 1.3 How They Work Together

```
+------------------+     +------------------+     +------------------+
|  Window Sensor   |     |  Thread Border   |     |  Home Assistant  |
|  (Aqara P2)      |     |  Router          |     |  (Matter Ctrl)   |
|                  |     |                  |     |                  |
|  Thread Radio    |---->|  Thread <-> IP   |---->|  Matter Server   |
|  Matter Device   |     |  Translation     |     |  WebSocket API   |
+------------------+     +------------------+     +------------------+
                                                          |
                                                          v
                                                  +------------------+
                                                  |  NDP Platform    |
                                                  |  (via MQTT or    |
                                                  |   WebSocket)     |
                                                  +------------------+
```

---

## 2. Aqara P2 Door and Window Sensor

### 2.1 Device Specifications

The [Aqara Door and Window Sensor P2](https://www.aqara.com/us/product/door-and-window-sensor-p2/) is a Matter-native Thread sensor:

| Specification | Value |
|---------------|-------|
| **Protocol** | Matter over Thread (native) |
| **Firmware** | Matter 1.4 (latest update) |
| **Battery** | CR123 1400mAh |
| **Dimensions** | 77 x 22 x 22 mm |
| **Detection** | Hall sensor (magnetic) |
| **Gap Range** | 1-22 mm (configurable sensitivity) |
| **Local Automation** | Yes (no cloud required) |

### 2.2 Matter Cluster Implementation

The sensor implements the **BooleanState** cluster:

```
Matter Cluster: BooleanState
  Attribute: StateValue (boolean)
    - true = Contact detected (door/window CLOSED)
    - false = No contact (door/window OPEN)
```

**Important**: Matter's BooleanState is **inverted** from Home Assistant's convention:

| Matter BooleanState.StateValue | Home Assistant Entity State | Meaning |
|--------------------------------|-----------------------------|---------|
| `true` | `off` | Door/Window **Closed** |
| `false` | `on` | Door/Window **Open** |

This inversion is handled automatically by HA's Matter integration.

### 2.3 Home Assistant Entity Creation

When paired, HA creates a `binary_sensor` entity:

```yaml
entity_id: binary_sensor.aqara_p2_window_living_room
state: "on"  # or "off"
attributes:
  device_class: window  # or door, depending on configuration
  friendly_name: "Living Room Window"
  battery_level: 95
  last_changed: "2025-12-30T10:15:30.123456+00:00"
  last_updated: "2025-12-30T10:15:30.123456+00:00"
```

---

## 3. Home Assistant Thread Border Router Setup

### 3.1 Compatible Border Router Hardware

Home Assistant supports Thread Border Router functionality with:

| Device | Type | Notes |
|--------|------|-------|
| **Home Assistant Yellow** | Built-in | Native Thread support |
| **Home Assistant SkyConnect** | USB Dongle | Zigbee + Thread multiprotocol |
| **Home Assistant Connect ZBT-2** | USB Dongle | Thread support |
| **Apple HomePod/Apple TV 4K** | External | Can share credentials with HA |
| **Google Nest Hub (2nd gen)** | External | Thread Border Router |
| **Samsung SmartThings Station** | External | Thread Border Router |

### 3.2 Setup Process

1. **Install OpenThread Border Router Add-on**:
   - HA automatically creates a Thread network named `ha-thread-xxxx`
   - Or import existing Thread network credentials from iOS/Android

2. **Pair Matter Device**:
   - Use QR code or manual pairing code
   - Device uses Bluetooth for initial commissioning
   - Then communicates via Thread mesh

3. **Verify Connectivity**:
   - Device appears in HA's Matter integration
   - Entity state updates in real-time

### 3.3 Network Architecture

```
                    +-------------------------+
                    |     Home Network        |
                    |      (Wi-Fi/LAN)        |
                    +-------------------------+
                              |
          +-------------------+-------------------+
          |                   |                   |
   +------+------+    +-------+-------+   +------+------+
   | HA Yellow   |    | Apple TV 4K   |   | Nest Hub    |
   | (Primary    |    | (Secondary    |   | (Secondary  |
   | Border      |    | Border        |   | Border      |
   | Router)     |    | Router)       |   | Router)     |
   +------+------+    +-------+-------+   +------+------+
          |                   |                   |
          +-------------------+-------------------+
                              |
                    +-------------------------+
                    |    Thread Mesh Network   |
                    |    (IPv6 over 802.15.4)  |
                    +-------------------------+
                              |
          +-------------------+-------------------+
          |                   |                   |
   +------+------+    +-------+-------+   +------+------+
   | Aqara P2    |    | Aqara P2      |   | Eve Door    |
   | Window 1    |    | Window 2      |   | & Window    |
   +-------------+    +---------------+   +-------------+
```

**Key Benefit**: Multiple border routers provide redundancy and extended coverage.

---

## 4. Data Flow: Matter Device to NDP

### 4.1 Option A: MQTT Statestream (Recommended)

[MQTT Statestream](https://www.home-assistant.io/integrations/mqtt_statestream/) publishes all entity state changes to an MQTT broker:

```yaml
# Home Assistant configuration.yaml
mqtt_statestream:
  base_topic: homeassistant
  publish_attributes: true
  publish_timestamps: true
  include:
    entity_globs:
      - binary_sensor.aqara_*
      - binary_sensor.*_window*
      - binary_sensor.*_door*
```

**MQTT Topic Structure**:
```
homeassistant/binary_sensor/aqara_p2_window_living_room/state
  -> "on" or "off"

homeassistant/binary_sensor/aqara_p2_window_living_room/battery_level
  -> "95"

homeassistant/binary_sensor/aqara_p2_window_living_room/last_changed
  -> "2025-12-30T10:15:30.123456+00:00"
```

**Integration with NDP**:
```
+------------------+     +------------------+     +------------------+
|  Home Assistant  |     |  MQTT Broker     |     |  NDP             |
|                  |     |  (Mosquitto)     |     |  MqttSource      |
|  Matter Server   |---->|                  |---->|  (Existing)      |
|  MQTT Statestream|     |                  |     |                  |
+------------------+     +------------------+     +--------+---------+
                                                          |
                                                          v
                                              +------------------------+
                                              | HomeAssistantParser    |
                                              | (New - converts MQTT   |
                                              |  to TimeSeriesPoint)   |
                                              +------------------------+
```

### 4.2 Option B: WebSocket API (Already Designed)

The existing `HomeAssistantSource` design in `/workspaces/neural-data-platform/product/research/dp-analysis/home-assistant-integration.md` uses the WebSocket API:

```rust
// WebSocket subscription
{
  "id": 1,
  "type": "subscribe_events",
  "event_type": "state_changed"
}

// Event received
{
  "id": 1,
  "type": "event",
  "event": {
    "event_type": "state_changed",
    "data": {
      "entity_id": "binary_sensor.aqara_p2_window_living_room",
      "new_state": {
        "state": "on",
        "last_changed": "2025-12-30T10:15:30.123456+00:00",
        "attributes": {
          "device_class": "window",
          "battery_level": 95
        }
      },
      "old_state": {
        "state": "off",
        "last_changed": "2025-12-30T08:00:00.000000+00:00"
      }
    }
  }
}
```

### 4.3 Recommendation

| Criteria | MQTT Statestream | WebSocket API |
|----------|-----------------|---------------|
| **Existing NDP Support** | MqttSource exists | New source needed |
| **Latency** | Near real-time | Real-time |
| **Complexity** | Low (config only) | Medium (new code) |
| **Filtering** | Topic-based | Entity filter logic |
| **Attributes** | Optional publish | Always included |
| **Reconnection** | MQTT handles | Custom logic needed |

**Recommendation**: Start with **MQTT Statestream** for simplicity, as NDP already has MqttSource. Create a `HomeAssistantMqttParser` to convert the topic/payload format.

---

## 5. Schema Design for Window Events

### 5.1 Event-State-Hybrid Pattern Application

Following the architecture in `/workspaces/neural-data-platform/product/research/dp-analysis/data-architecture-analysis.md`:

```
+------------------+     +------------------+     +------------------+
| Matter Sensor    |     | Bronze Layer     |     | Silver Layer     |
| (via HA MQTT)    |     | (Parquet)        |     | (TimescaleDB)    |
|                  |     |                  |     |                  |
| state_changed    |---->| Raw events with  |---->| State views      |
| events           |     | old/new state    |     | Duration calcs   |
+------------------+     +------------------+     +------------------+
```

### 5.2 Bronze Layer Schema (Parquet)

```yaml
stream_id: home-events
description: "Home automation events from Matter/Thread sensors"
version: "1.0.0"
partitioning_strategy: daily

fields:
  - name: event_id
    type: String
    nullable: false
    description: "Unique event identifier (UUID)"

  - name: timestamp
    type: Int
    nullable: false
    unit: "epoch_ms"
    description: "Event timestamp (last_changed from HA)"

  - name: event_type
    type: String
    nullable: false
    description: "Event type (state_changed)"

  - name: entity_id
    type: String
    nullable: false
    description: "Home Assistant entity ID (e.g., binary_sensor.aqara_p2_window_living_room)"

  - name: entity_domain
    type: String
    nullable: false
    description: "Entity domain (binary_sensor, sensor, etc.)"

  - name: device_class
    type: String
    nullable: true
    description: "Device class (window, door, motion, etc.)"

  - name: new_state
    type: String
    nullable: false
    description: "New state value (on, off)"

  - name: old_state
    type: String
    nullable: true
    description: "Previous state value"

  - name: state_changed
    type: Bool
    nullable: false
    description: "True if state value changed (vs attribute-only update)"

  - name: attributes
    type: Json
    nullable: true
    description: "Entity attributes (battery_level, friendly_name, etc.)"

  - name: protocol
    type: String
    nullable: false
    description: "Device protocol (matter_thread, zigbee, zwave, etc.)"

  - name: manufacturer
    type: String
    nullable: true
    description: "Device manufacturer (Aqara, Eve, etc.)"
```

### 5.3 Silver Layer Views (TimescaleDB)

```sql
-- Main hypertable for home events
CREATE TABLE home_events (
    time TIMESTAMPTZ NOT NULL,
    entity_id TEXT NOT NULL,
    domain TEXT NOT NULL,
    device_class TEXT,
    new_state TEXT NOT NULL,
    old_state TEXT,
    state_changed BOOLEAN NOT NULL,
    battery_level INTEGER,
    protocol TEXT,
    manufacturer TEXT,
    attributes JSONB,
    CONSTRAINT home_events_pkey PRIMARY KEY (time, entity_id)
);

SELECT create_hypertable('home_events', 'time');

-- Index for common queries
CREATE INDEX idx_home_events_entity ON home_events (entity_id, time DESC);
CREATE INDEX idx_home_events_device_class ON home_events (device_class, time DESC);

-- Current state view (most recent state per entity)
CREATE VIEW current_entity_state AS
SELECT DISTINCT ON (entity_id)
    entity_id,
    device_class,
    new_state AS current_state,
    time AS last_updated,
    battery_level
FROM home_events
ORDER BY entity_id, time DESC;

-- State transitions with duration
CREATE VIEW state_transitions AS
SELECT
    entity_id,
    device_class,
    old_state,
    new_state,
    time AS transition_time,
    LEAD(time) OVER (PARTITION BY entity_id ORDER BY time) AS next_transition,
    LEAD(time) OVER (PARTITION BY entity_id ORDER BY time) - time AS duration
FROM home_events
WHERE state_changed = true
ORDER BY entity_id, time;

-- Window open duration per hour (for correlation with air quality)
CREATE MATERIALIZED VIEW window_open_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    entity_id,
    device_class,
    AVG(CASE WHEN new_state = 'on' THEN 1.0 ELSE 0.0 END) AS open_ratio,
    COUNT(*) FILTER (WHERE new_state = 'on') AS times_opened,
    COUNT(*) FILTER (WHERE new_state = 'off') AS times_closed
FROM home_events
WHERE device_class IN ('window', 'door')
GROUP BY time_bucket('1 hour', time), entity_id, device_class
WITH NO DATA;

-- Refresh policy
SELECT add_continuous_aggregate_policy('window_open_hourly',
    start_offset => INTERVAL '2 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '15 minutes');
```

### 5.4 Point-in-Time State Query

For ML features that need "was window open at time T?":

```sql
-- Function to get entity state at any point in time
CREATE OR REPLACE FUNCTION entity_state_at_time(
    p_entity_id TEXT,
    p_timestamp TIMESTAMPTZ
) RETURNS TEXT AS $$
    SELECT new_state
    FROM home_events
    WHERE entity_id = p_entity_id
      AND time <= p_timestamp
    ORDER BY time DESC
    LIMIT 1;
$$ LANGUAGE SQL;

-- Usage: Correlate air quality with window state
SELECT
    aq.time,
    aq.pm25,
    aq.temperature,
    entity_state_at_time('binary_sensor.aqara_p2_window_living_room', aq.time) AS window_state
FROM air_quality_silver aq
WHERE aq.time >= '2025-12-01';
```

---

## 6. Integration with Existing NDP Patterns

### 6.1 NDP Domain Adapter Pattern

Following ADR-001 (Channel Ownership), the integration follows hexagonal architecture:

```rust
// Option A: Use existing MqttSource with new parser
let mqtt_config = MqttConfig {
    broker_url: "mqtt://homeassistant.local:1883",
    topics: vec!["homeassistant/binary_sensor/+/state"],
    client_id: "ndp-home-events",
};

let parser = HomeAssistantMqttParser::new(HomeAssistantMqttConfig {
    state_topic_pattern: "homeassistant/{domain}/{object_id}/state",
    attribute_topics: vec!["battery_level", "device_class"],
    entity_filters: vec!["*_window*", "*_door*"],
});

let source = MqttSource::new(mqtt_config, Box::new(parser));

// Option B: New HomeAssistantSource (WebSocket-based)
let ha_config = HomeAssistantConfig {
    websocket_url: "ws://homeassistant.local:8123/api/websocket",
    access_token: std::env::var("HASS_ACCESS_TOKEN")?,
    entity_filters: vec!["binary_sensor.*window*", "binary_sensor.*door*"],
};

let source = HomeAssistantSource::new(ha_config);
```

### 6.2 Stream Configuration

```yaml
# config/base/streams/home-events/config.yaml
stream_id: home-events
description: "Home automation events from Matter/Thread sensors via Home Assistant"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

sources:
  - type: mqtt
    enabled: true
    broker_url: "${MQTT_BROKER_URL}"
    topics:
      - "homeassistant/binary_sensor/+/state"
      - "homeassistant/binary_sensor/+/last_changed"
    parser:
      parser_type: home_assistant_mqtt
      topic_pattern: "homeassistant/{domain}/{entity}/+"
      default_tags:
        source: home_assistant
        protocol: matter_thread
        stream_id: home-events

storage:
  batch_size: 50
  batch_timeout_secs: 30
  buffer_capacity: 500
```

### 6.3 Parser Implementation (Pseudo-code)

```rust
pub struct HomeAssistantMqttParser {
    topic_pattern: Regex,
    entity_filters: Vec<String>,
}

impl Parser for HomeAssistantMqttParser {
    fn parse(&self, message: &MqttMessage, timestamp: DateTime<Utc>) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract entity info from topic
        // homeassistant/binary_sensor/aqara_p2_window_living_room/state
        let captures = self.topic_pattern.captures(&message.topic)?;
        let domain = captures.get(1)?.as_str();  // binary_sensor
        let entity = captures.get(2)?.as_str();  // aqara_p2_window_living_room
        let topic_type = captures.get(3)?.as_str();  // state

        // Check entity filter
        if !self.matches_filter(&entity) {
            return Ok(vec![]);
        }

        let entity_id = format!("{}.{}", domain, entity);

        // Convert state to numeric value for TimeSeriesPoint
        let value = match message.payload.to_lowercase().as_str() {
            "on" | "open" | "true" => 1.0,
            "off" | "closed" | "false" => 0.0,
            _ => return Err(CoreError::Parse("Unknown state value")),
        };

        let mut tags = HashMap::new();
        tags.insert("stream_id".to_string(), "home-events".to_string());
        tags.insert("entity_id".to_string(), entity_id.clone());
        tags.insert("domain".to_string(), domain.to_string());
        tags.insert("metric".to_string(), format!("{}_{}", domain, entity));
        tags.insert("state_string".to_string(), message.payload.clone());
        tags.insert("protocol".to_string(), "matter_thread".to_string());

        Ok(vec![TimeSeriesPoint {
            timestamp,
            location_id: entity_id,
            value,
            tags,
        }])
    }
}
```

---

## 7. Implementation Recommendations

### 7.1 Phase 1: MQTT Statestream (Quick Win)

1. **Configure Home Assistant MQTT Statestream**
   - Enable for binary_sensor entities
   - Publish attributes and timestamps

2. **Create HomeAssistantMqttParser**
   - Parse topic structure
   - Convert state to numeric values
   - Extract entity metadata

3. **Test with Existing MqttSource**
   - Verify data flows to Parquet
   - Check entity filtering

### 7.2 Phase 2: Enhanced Integration

1. **Implement HomeAssistantSource (WebSocket)**
   - Real-time event subscription
   - Full state change context (old_state, new_state)
   - Context ID for causality tracking

2. **Add Silver Layer Views**
   - Current state materialization
   - Duration calculations
   - Correlation with air quality

### 7.3 Phase 3: ML Features

1. **Cross-Stream Correlation**
   - Window state + temperature/humidity
   - Ventilation score features

2. **Predictive Features**
   - Window open patterns by time of day
   - Correlation with outdoor air quality

---

## 8. References

### Home Assistant Documentation
- [Matter Integration](https://www.home-assistant.io/integrations/matter/)
- [Thread Integration](https://www.home-assistant.io/integrations/thread/)
- [MQTT Statestream](https://www.home-assistant.io/integrations/mqtt_statestream/)
- [Binary Sensor](https://www.home-assistant.io/integrations/binary_sensor/)

### Aqara P2 Sensor
- [Product Page](https://www.aqara.com/us/product/door-and-window-sensor-p2/)
- [Amazon Listing](https://www.amazon.com/Aqara-Requires-Contact-Automation-Supports/dp/B0BTL8B72D)
- [Matter 1.4 Firmware Update](https://www.matteralpha.com/news/aqara-releases-matter-1-4-firmware-for-p2-thread-sensor-duo)
- [Product Review](https://www.matteralpha.com/review/aqara-p2-door-window-sensor-review)

### Home Assistant Updates (2025)
- [HA 2025.12 Matter Enhancements](https://www.matteralpha.com/news/home-assistant-2025-12-adds-enhancements-to-matter-sensor-doorlock-and-covering)
- [HA 2025.11 Matter Updates](https://www.matteralpha.com/news/home-assistant-2025-11-a-simpler-smarter-home-with-matter)
- [State of Matter Blog](https://www.home-assistant.io/blog/2023/02/08/state-of-matter-and-thread/)

### NDP Patterns
- Event-State-Hybrid Pattern: `/workspaces/neural-data-platform/product/research/dp-analysis/data-architecture-analysis.md`
- Home Assistant Integration Design: `/workspaces/neural-data-platform/product/research/dp-analysis/home-assistant-integration.md`

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-12-30 | Initial research and analysis |
