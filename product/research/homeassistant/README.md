# Home Assistant Integration Research

**Date**: 2025-12-30
**Status**: Research Complete
**Related Feature**: AIR-008 (Home Events)

---

## Executive Summary

This research package addresses the key questions about integrating NDP with Home Assistant:

1. **MQTT Broker Sharing** → Use shared broker with namespace separation
2. **Device Pattern Consistency** → Single pattern handles all devices (AirGradient, Matter/Thread)
3. **InfluxDB vs NDP Analytics** → Complementary systems, use both for different purposes
4. **NDP as HA Addon** → Not recommended; keep as separate platform

---

## Key Decisions

### Decision 1: MQTT Architecture

**Recommendation: Shared Mosquitto Broker**

```
                    Mosquitto (NDP Pi)
                           |
    +----------+-----------+-----------+----------+
    |          |           |           |          |
AirGradient   Home      Matter      Future     NDP App
 Sensors    Assistant   Devices    Sensors
```

**Topic Namespace Design:**
- `ndp/sensors/...` - NDP-specific sensor data
- `homeassistant/...` - HA entity states (via MQTT Statestream)
- `home/...` - Home automation commands/events
- `shared/...` - Cross-platform data

**Why shared broker:**
- Pi 5 resource efficiency (single broker vs two)
- Native MQTT fan-out for multi-subscriber
- Simpler operational model
- Both systems need same sensor data

See: [mqtt-patterns.md](./mqtt-patterns.md)

---

### Decision 2: Unified Device Pattern

**Recommendation: MQTT Statestream for all Home Assistant devices**

Regardless of device protocol (AirGradient MQTT, Matter/Thread, Zigbee, Z-Wave), the pattern is:

```
Device → Home Assistant → MQTT Statestream → NDP MqttSource → Bronze Layer
```

**Configuration (HA side):**
```yaml
# Home Assistant configuration.yaml
mqtt_statestream:
  base_topic: homeassistant
  publish_attributes: true
  publish_timestamps: true
  include:
    entity_globs:
      - sensor.airgradient_*
      - binary_sensor.*_window*
      - binary_sensor.*_door*
      - sensor.*_temperature*
      - sensor.*_humidity*
```

**Configuration (NDP side):**
```yaml
# config/base/streams/home-events/config.yaml
sources:
  - type: mqtt
    topic_patterns:
      - "homeassistant/+/+/state"
    parser: home_assistant_mqtt
```

This provides:
- **Protocol agnostic** - Works for Matter, Zigbee, Z-Wave, WiFi devices
- **Single integration point** - All devices via HA's MQTT Statestream
- **No per-device config** - Add devices to HA, they automatically flow to NDP

See: [matter-thread-integration.md](./matter-thread-integration.md)

---

### Decision 3: Database Strategy

**Recommendation: Use Both (Parallel Ingestion)**

| System | Purpose | Data |
|--------|---------|------|
| **InfluxDB (HA)** | Real-time dashboards, simple alerts | Last 7-14 days |
| **NDP Bronze (Parquet)** | Raw storage, source of truth | All historical data |
| **NDP Silver (DuckDB)** | Cross-stream analytics, correlations | On-demand queries |
| **NDP Gold (TimescaleDB)** | ML features, predictions | Aggregated features |

**Data Flow:**
```
Home Assistant Entity
       |
       +----> InfluxDB ------> Grafana (real-time)
       |                       "What's the current temp?"
       |
       +----> NDP Bronze ----> Silver ----> Gold ----> ML Predictions
                              "Correlate PM2.5 with window state"
```

**Why both:**
- InfluxDB excels at real-time visualization (sub-ms queries)
- NDP excels at analytics (SQL JOINs, window functions, ML)
- Different retention profiles (InfluxDB: days, NDP: years)
- HA's InfluxDB addon is zero-config for basic dashboards

See: [database-comparison.md](./database-comparison.md)

---

### Decision 4: NDP as HA Addon

**Recommendation: Do NOT package NDP as HA addon**

| Factor | Addon | Separate Platform |
|--------|-------|-------------------|
| Architectural fit | 2/10 | 9/10 |
| Resource management | 3/10 | 9/10 |
| ML/Analytics support | 2/10 | 10/10 |
| User experience | 8/10 | 6/10 |

**Why not addon:**
- HA addons are single-container; NDP has 5 services
- S6 overlay not designed for multiple primary applications
- Memory constraints (1.7GB combined doesn't fit addon model)
- Addon limits future extensibility for ML workloads

**Integration instead:**
- NDP connects to HA via WebSocket API (real-time events)
- HA publishes to shared MQTT broker (sensor states)
- Consider HACS integration component for UX (future)

See: [addon-architecture.md](./addon-architecture.md)

---

## Implementation Roadmap

### Phase 1: MQTT Statestream (Quick Start)

1. **Configure HA MQTT to point to NDP's Mosquitto**
   ```yaml
   # HA configuration.yaml
   mqtt:
     broker: <NDP_PI_IP>
     port: 1883
   ```

2. **Enable MQTT Statestream for relevant entities**
   ```yaml
   mqtt_statestream:
     base_topic: homeassistant
     publish_attributes: true
     include:
       domains: [sensor, binary_sensor]
   ```

3. **Create `HomeAssistantMqttParser` in NDP**
   - Parse `homeassistant/{domain}/{entity}/state` topics
   - Extract attributes from separate topic messages
   - Convert to `TimeSeriesPoint`

### Phase 2: Matter/Thread Sensors

1. **Set up Thread Border Router** (HA Yellow, SkyConnect, or external)
2. **Pair Matter devices** (e.g., Aqara P2 window sensors)
3. **Entities automatically flow via MQTT Statestream** (no additional config)
4. **Add home-events stream** with event-state-hybrid schema

### Phase 3: Silver Layer Analytics

1. **Deploy DuckDB views** for cross-stream correlation
2. **Create window_state feature** for ML
3. **Build correlation dashboard** (air quality vs window state)

### Phase 4: Production Hardening

1. **Enable MQTT authentication** (per-service credentials)
2. **Configure ACLs** for topic isolation
3. **Optional: Add TLS** for encrypted connections

---

## Research Documents

| Document | Content |
|----------|---------|
| [mqtt-patterns.md](./mqtt-patterns.md) | MQTT broker sharing, namespace design, multi-subscriber patterns |
| [database-comparison.md](./database-comparison.md) | InfluxDB vs DuckDB vs TimescaleDB analysis |
| [addon-architecture.md](./addon-architecture.md) | HA addon feasibility, separate platform recommendation |
| [matter-thread-integration.md](./matter-thread-integration.md) | Matter/Thread protocol, window sensor integration |

---

## Quick Reference

### Network Setup (Same Pi)

```
Raspberry Pi 5 (8GB)
├── Home Assistant OS
│   ├── HA Core
│   ├── Thread Border Router
│   └── MQTT Statestream → port 1883
│
└── Docker Compose (NDP)
    ├── Mosquitto ← receives from HA
    ├── air-quality-app
    ├── etcd
    ├── DuckDB
    └── Grafana
```

### Memory Budget

| Service | Memory |
|---------|--------|
| Home Assistant | ~800MB |
| InfluxDB (optional) | ~500MB |
| NDP Stack | ~1.7GB |
| **Total** | ~3GB (fits in 8GB Pi) |

### Configuration Files to Modify

| System | File | Change |
|--------|------|--------|
| HA | configuration.yaml | Add mqtt: and mqtt_statestream: |
| NDP | deploy/pi/.env | Add MQTT_BROKER_URL |
| NDP | config/base/streams/home-events/config.yaml | New stream definition |

---

## Summary

The recommended architecture:

1. **Share NDP's Mosquitto broker** with Home Assistant
2. **Use MQTT Statestream** as the unified device pattern
3. **Keep InfluxDB for real-time dashboards**, NDP for analytics
4. **Run NDP as separate Docker Compose stack**, not HA addon
5. **Matter/Thread devices** flow through HA → MQTT → NDP automatically
