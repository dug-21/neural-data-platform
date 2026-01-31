# air-012: Home Assistant Integration (Barebones)

## Overview

Get window/door state events flowing from Home Assistant into NDP's Bronze and Silver layers. This is the foundation for future correlation analysis with air quality observations.

**Decision (2026-01-30):** Minimal scope focused on data flow. SCD semantics and point-in-time queries deferred to dp-014 (Config-Driven Gold Layer).

---

## Home Assistant MQTT Integration

**Verified working MQTT access:**

```bash
mosquitto_sub -h 192.168.52.103 -p 1883 -t "homeassistant/binary_sensor/door_backslider/state" -v
```

**Message format:**
```
homeassistant/binary_sensor/door_backslider/state off
homeassistant/binary_sensor/door_backslider/state on
```

**Configuration:**
- MQTT Broker: `192.168.52.103`
- Port: `1883`
- Authentication: None (internal network)
- Topic Pattern: `homeassistant/binary_sensor/+/state`

**Note on Timestamps:** Using ingestion time (when NDP receives the message). MQTT latency is typically <100ms, acceptable for event correlation.

---

## Scope

### Deliverables

| Layer | Deliverable | Description |
|-------|-------------|-------------|
| **Bronze** | Stream config | `home-assistant-state` with MQTT source |
| **Dimension** | Entity metadata | Add 3 sensors to `entity_context.csv` |
| **Silver** | Event table | Simple `state_events` (event log, no SCD) |
| **Dashboard** | Pipeline health | Freshness tracking with sparse-data thresholds |

### Initial Sensors (3)

| ndp_id | MQTT Topic | Description |
|--------|------------|-------------|
| `door_backslider` | `homeassistant/binary_sensor/door_backslider/state` | Back door - Slider |
| `door_officewindow` | `homeassistant/binary_sensor/door_officewindow/state` | Office window |
| `door_dinettewindow` | `homeassistant/binary_sensor/door_dinettewindow/state` | Dinette window |

**Note:** Home Assistant categorizes these as `binary_sensor.door_*` regardless of whether they monitor doors or windows. We preserve this naming for traceability.

---

## Data Flow

```
Home Assistant MQTT
        │
        ▼
   MQTT Source (topic: homeassistant/binary_sensor/+/state)
        │
        ▼
   Bronze (Parquet)
   - raw_payload: "on" or "off"
   - topic metadata
   - ingestion timestamp
        │
        ▼
   Silver (TimescaleDB)
   - state_events table
   - Simple event log
        │
        ▼
   Dashboard
   - Pipeline health freshness
```

---

## Silver Schema (Simple Event Log)

```sql
CREATE TABLE silver.state_events (
    event_time       TIMESTAMPTZ NOT NULL,  -- Ingestion time
    ndp_id           TEXT NOT NULL,         -- 'door_backslider'
    source_entity_id TEXT,                  -- 'binary_sensor.door_backslider'
    state            TEXT NOT NULL,         -- 'on' (open) / 'off' (closed)
    dq_flags         TEXT[],
    PRIMARY KEY (event_time, ndp_id)
);

SELECT create_hypertable('silver.state_events', 'event_time');
```

**Design Decisions:**
- **No `category` column** - JOIN with `entity_context` dimension for metadata
- **No `previous_state`** - Compute via window functions if needed
- **No SCD semantics** - Deferred to dp-014 (Gold layer materialized view)

---

## Dimension Table Update

Add to existing `data/dimensions/entity_context.csv`:

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
door_backslider,door,Back Door Slider,/home/living,aq_airgradient_1,south
door_officewindow,window,Office Window,/home/office,aq_airgradient_1,east
door_dinettewindow,window,Dinette Window,/home/dining,aq_airgradient_1,west
```

---

## Pipeline Health: Sparse Data Thresholds

State events are sparse (only fire on change). A window staying closed for 24 hours = no messages for 24 hours. Standard freshness thresholds would false-alarm.

| Status | Threshold | Rationale |
|--------|-----------|-----------|
| 🟢 Fresh | < 18 hours | Normal operation |
| 🟡 Stale | 18-36 hours | Worth monitoring |
| 🔴 Critical | > 36 hours | Likely sensor/connection issue |

**Implementation:** Update pipeline health query to use stream-specific thresholds for `state_events`.

---

## Acceptance Criteria

### Bronze Layer
- [ ] Stream config `home-assistant-state` created in `config/base/streams/`
- [ ] MQTT source connects to broker at `192.168.52.103:1883`
- [ ] Topic pattern `homeassistant/binary_sensor/+/state` subscribed
- [ ] Raw payload ("on"/"off") stored in Parquet with topic metadata
- [ ] `ndp_id` extracted/assigned for each sensor

### Silver Layer
- [ ] `silver.state_events` table created (hypertable)
- [ ] ETL extracts `state` from payload
- [ ] `source_entity_id` extracted from topic path
- [ ] `event_time` uses ingestion timestamp

### Dimension
- [ ] 3 sensors added to `entity_context.csv`
- [ ] `./deploy.sh sync-dimensions` loads updated dimension

### Pipeline Health
- [ ] State events freshness visible in dashboard
- [ ] Sparse-data thresholds applied (18/36 hours)
- [ ] No false alarms when windows stay closed overnight

---

## Out of Scope (Deferred)

| Item | Deferred To | Rationale |
|------|-------------|-----------|
| SCD semantics (valid_from/valid_to) | dp-014 | Config-driven Gold layer |
| Point-in-time queries | dp-014 | Requires Gold materialized view |
| Previous state tracking | dp-014 | Compute in Gold layer |
| Correlation with air quality | dp-014+ | Needs SCD for accurate point-in-time |
| Generic `stream_type: state_events` | Future | Over-engineering for MVP |
| Source timestamp (`last_changed` topic) | Future | Ingestion time sufficient |
| Non-binary sensors | Future | Different schema needs |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| MQTT source adapter | ✅ Ready | Fully implemented in `core/src/sources/mqtt/` |
| Bronze Parquet storage | ✅ Ready | Working for air-quality stream |
| Dimension tables (dp-013) | ✅ Ready | `entity_context` config exists |
| Pipeline health dashboard | ✅ Ready | Needs threshold customization |

---

## Related Features

| Feature | Relationship |
|---------|--------------|
| dp-013 | Provides dimension table infrastructure |
| dp-014 | Will add SCD Gold layer for this data |
| ml-??? | Future unsupervised learning will consume Gold features |

---

## Architecture Notes

**Why simple event log instead of SCD in Silver?**

1. **Separation of concerns** - Silver stores facts, Gold computes features
2. **Flexibility** - Can iterate on SCD logic in view without migrating data
3. **Incremental approach** - Get data flowing first, optimize later
4. **Config-driven Gold** - dp-014 will establish the pattern properly

**Future Gold Layer (dp-014):**
```sql
-- Computed SCD semantics
CREATE MATERIALIZED VIEW gold.state_periods AS
SELECT
    ndp_id,
    state,
    event_time AS valid_from,
    LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time) AS valid_to
FROM silver.state_events;
```

This enables efficient point-in-time queries for ML feature engineering.
