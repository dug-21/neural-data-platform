# air-012: Home Assistant Integration

## Home Assistant MQTT Integration

**Verified working MQTT access via Home Assistant's MQTT integration:**

```bash
mosquitto_sub -h 192.168.52.103 -p 1883 -t "homeassistant/binary_sensor/backdoor_door/state" -v
```

**Message format:**
```
homeassistant/binary_sensor/backdoor_door/state off
homeassistant/binary_sensor/backdoor_door/state on
```

Home Assistant publishes state changes to MQTT topics. Each state change produces a single message with the new state value ("on" or "off").

**Configuration:**
- MQTT Broker: `192.168.52.103`
- Port: `1883`
- Authentication: None (internal network)
- Topic Pattern: `homeassistant/binary_sensor/{name}/state`

**Note on Timestamps:** Home Assistant also publishes to `.../last_changed` with ISO 8601 timestamps. For MVP, we use ingestion time (when NDP receives the message) rather than correlating multiple topics. MQTT latency is typically <100ms, making ingestion time acceptable for event correlation.

---

## Scope

### Part 1: Home Assistant Window/Door Sensor Integration

Integrate window and door open/closed state from Home Assistant binary sensors via MQTT to correlate with air quality observations for ventilation optimization.

**Initial Sensors (3):**
| ndp_id | MQTT Topic | Description |
|--------|------------|-------------|
| `door_backslider` | `homeassistant/binary_sensor/door_backslider/state` | Back door - Slider |
| `door_officewindow` | `homeassistant/binary_sensor/door_officewindow/state` | Office window |
| `door_dinettewindow` | `homeassistant/binary_sensor/door_dinettewindow/state` | Dinette window |

**Note on Category:** Home Assistant categorizes these as `binary_sensor.door_*` regardless of whether they monitor doors or windows. We preserve this naming for traceability to the source system.

**Data Flow:**
- Source: Home Assistant MQTT integration
- Protocol: MQTT subscription (event-driven)
- Authentication: None (broker on internal network)
- Bronze: Raw state payload stored in Parquet with topic metadata
- Silver: `silver.state_events` table

**MQTT Payload:**
| Field | Source | Value |
|-------|--------|-------|
| `state` | Message payload | `"on"` (open) / `"off"` (closed) |
| `source_entity_id` | Extracted from topic | `binary_sensor.door_backslider` |
| `event_time` | Ingestion time | `Utc::now()` when NDP receives message |

**Identity Pattern:**
- `ndp_id` assigned in stream config (e.g., `door_backslider`)
- `source_entity_id` extracted from topic path for traceability
- Consistent with all other NDP streams

**Metadata Strategy:**
Since MQTT payload contains only the state value (no attributes), all metadata (friendly_name, category, location, correlates_with) is defined in:
1. Stream config (`context` field)
2. Dimension table (`silver.entity_context`)

**What's NOT in scope for Part 1:**
- Source timestamp correlation (using `last_changed` topic)
- Home Assistant automation triggers
- Non-binary sensors (temperature, etc.)

---

### Part 2: Platform Capability - State Events Stream Type

Generalize window state tracking to a reusable `stream_type: state_events` pattern applicable across domains.

**Use Cases:**
| Domain | State Entity | Observation Stream | Correlation |
|--------|-------------|-------------------|-------------|
| IoT/Home | Window open/closed | Air quality | Ventilation impact |
| Finance | Sentiment +/-/neutral | Price movements | Market reaction |
| Operations | System maintenance/active | Performance metrics | Throughput impact |
| Energy | Peak/off-peak pricing | Consumption | Demand response |

**Generic Silver Schema:**
```sql
silver.state_events (
    event_time        TIMESTAMPTZ NOT NULL,  -- Ingestion time (MQTT is near-real-time)
    ndp_id            TEXT NOT NULL,         -- NDP standard identity
    source_entity_id  TEXT,                  -- Original source identifier (for traceability)
    category          TEXT NOT NULL,         -- 'window', 'door', 'sentiment', etc.
    state             TEXT NOT NULL,         -- Normalized state value ('on'/'off')
    previous_state    TEXT,
    dq_flags          TEXT[],
    PRIMARY KEY (event_time, ndp_id)
)
```

**Stream Config Pattern:**
- `stream_type: state_events` triggers event-specific ETL behavior
- No deduplication needed - MQTT is event-driven (only fires on state change)
- Timestamp is ingestion time (MQTT latency <100ms, acceptable for correlation)

---

## Acceptance Criteria

### Part 1: Home Assistant Integration
- [ ] Stream config created for `home-assistant-state` with MQTT source
- [ ] MQTT subscription connects to broker and receives state messages
- [ ] Topic pattern `homeassistant/binary_sensor/+/state` routes to stream
- [ ] Bronze stores raw state payload with topic metadata
- [ ] Silver ETL extracts `state` from payload, `source_entity_id` from topic
- [ ] `event_time` uses ingestion timestamp (`Utc::now()`)
- [ ] All 3 sensors (door_backslider, door_officewindow, door_dinettewindow) integrated
- [ ] Dimension table contains metadata for each sensor (friendly_name, category, location)

### Part 2: State Events Pattern
- [ ] `stream_type: state_events` recognized in config
- [ ] `silver.state_events` table created with generic schema
- [ ] `category` field supports filtering by domain
- [ ] Documentation: pattern documented for future state event sources

### Integration (requires dp-013 CSV Loader)
- [ ] Query works: "Air quality readings when office window is open"
- [ ] Point-in-time correctness: state at observation time, not current state
- [ ] Pipeline Health dashboard updated with state events freshness

---

## Architecture Documents

Created during analysis (see `architecture/` directory):
- `INTEGRATION_PATTERNS.md` - MQTT vs HTTP polling ADRs
- `DATA_MODEL.md` - Silver/Gold schema design
- `FEATURE_ENGINEERING.md` - ML feature definitions
- `AIR_QUALITY_DOMAIN.md` - Ventilation thresholds for Florida
- `RECOMMENDATIONS_SUMMARY.md` - Consolidated decisions
- `DRAFT_STREAM_CONFIG.yaml` - Working stream configuration

---

## Out of Scope (Future Features)

- Source timestamp correlation (subscribing to `last_changed` topic)
- Automated window recommendations/alerts
- ML model for ventilation prediction
- Cross-ventilation pattern detection
- Multi-floor air flow modeling
