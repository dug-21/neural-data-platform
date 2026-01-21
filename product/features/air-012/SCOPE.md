# air-012: Home Assistant Integration

## Home Assistant API Access

**Verified working API access to query device state:**

```bash
curl -X GET -H "Authorization: Bearer $HASS_TOKEN" http://192.168.52.221:8123/api/states/<entity_id>
```

**Example - Query RPi Power Status:**
```bash
curl -X GET -H "Authorization: Bearer $HASS_TOKEN" http://192.168.52.221:8123/api/states/binary_sensor.rpi_power_status
```

**Response format:**
```json
{
  "entity_id": "binary_sensor.rpi_power_status",
  "state": "off",
  "attributes": {
    "device_class": "problem",
    "icon": "mdi:raspberry-pi",
    "friendly_name": "RPi Power status"
  },
  "last_changed": "2025-12-31T12:31:02.467501+00:00",
  "last_reported": "2025-12-31T12:31:32.486649+00:00",
  "last_updated": "2025-12-31T12:31:02.467501+00:00",
  "context": {
    "id": "01KDT67SA30AQYHP9CY2BYPVZV",
    "parent_id": null,
    "user_id": null
  }
}
```

**Configuration:**
- Home Assistant IP: `192.168.52.221`
- Port: `8123` (HTTP, not HTTPS)
- Authentication: Long-lived access token via `$HASS_TOKEN` environment variable
- API Endpoint: `/api/states/<entity_id>`

---

## Scope

### Part 1: Home Assistant Window Sensor Integration

Integrate window open/closed state from Home Assistant binary sensors to correlate with air quality observations for ventilation optimization.

**Data Flow:**
- Source: Home Assistant REST API (`/api/states/<entity_id>`)
- Protocol: HTTP polling (30s interval)
- Authentication: Bearer token (`$HASS_TOKEN`)
- Bronze: Raw JSON response stored in Parquet
- Silver: `silver.state_events` table

**Key Fields from HA Response:**
| Field | Use |
|-------|-----|
| `entity_id` | Stored as `source_entity_id` for traceability |
| `state` | "on" (open) / "off" (closed) |
| `last_changed` | Event timestamp (ISO 8601) |
| `attributes.device_class` | "window", "door", etc. |
| `attributes.friendly_name` | Human-readable name |

**Identity Pattern:**
- `ndp_id` assigned in endpoint config (e.g., `window_office`)
- HA's `entity_id` stored as `source_entity_id` for traceability
- Consistent with all other NDP streams

**What's NOT in scope for Part 1:**
- WebSocket real-time events (future upgrade path)
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
    event_time        TIMESTAMPTZ NOT NULL,
    ingestion_time    TIMESTAMPTZ NOT NULL,
    ndp_id            TEXT NOT NULL,        -- NDP standard identity
    source_entity_id  TEXT,                 -- Original source identifier (for traceability)
    category          TEXT NOT NULL,        -- 'window', 'sentiment', etc.
    state             TEXT NOT NULL,        -- Normalized state value
    previous_state    TEXT,
    dq_flags          TEXT[],
    PRIMARY KEY (event_time, ndp_id)
)
```

**Stream Config Pattern:**
- `stream_type: state_events` triggers event-specific ETL behavior
- Deduplication on `(event_time, entity_id)` - natural event-sourcing
- Timestamp from source (e.g., `last_changed`), not poll time

---

### Part 3: Platform Capability - CSV Dimension Loader

Add platform capability to load context/enrichment data from CSV files into Silver dimension tables.

**Rationale:**
- State events need correlation context (which window affects which AQ sensor?)
- CSV is universally accessible (Excel, Sheets, git-versioned)
- Separates "what happened" (events) from "what it means" (context)

**Example CSV (`config/dimensions/entity_context.csv`):**
```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
window_office,window,Office Window,/home/office,aq_airgradient_1,east
window_bedroom,window,Bedroom Window,/home/bedroom,aq_airgradient_2,west
```

**Dimension Config (`config/base/dimensions/entity_context.yaml`):**
```yaml
dimension_id: entity-context
source:
  type: csv
  path: config/dimensions/entity_context.csv
target:
  table: silver.entity_context
  primary_key: [ndp_id]
load:
  strategy: truncate_and_load
```

**Load Trigger Options:**
- On deploy (`deploy.sh sync`)
- CLI command (`ndp dimension load <id>`)
- Scheduled (cron)

---

## Acceptance Criteria

### Part 1: Home Assistant Integration
- [ ] Stream config created for `home-assistant-state`
- [ ] HTTP polling retrieves window state from HA API
- [ ] Bronze stores raw HA JSON response
- [ ] Silver ETL uses `ndp_id` from config, extracts `state`, `event_time` (from `last_changed`)
- [ ] `source_entity_id` stored for traceability to HA
- [ ] Deduplication prevents duplicate events when state unchanged
- [ ] Timestamp stored as `TIMESTAMPTZ` using `iso8601` transform
- [ ] At least one window sensor integrated and producing data

### Part 2: State Events Pattern
- [ ] `stream_type: state_events` recognized in config
- [ ] `silver.state_events` table created with generic schema
- [ ] `category` field supports filtering by domain
- [ ] Documentation: pattern documented for future state event sources

### Part 3: CSV Dimension Loader
- [ ] Dimension YAML config schema defined
- [ ] CSV loader implemented (reads CSV, loads to Silver table)
- [ ] `silver.entity_context` table created
- [ ] Entity context CSV created with window mappings
- [ ] Load integrated into deploy workflow or CLI
- [ ] Join view created: `gold.events_with_context`

### Integration
- [ ] Query works: "Air quality readings when office window is open"
- [ ] Point-in-time correctness: state at observation time, not current state
- [ ] Pipeline Health dashboard updated with state events freshness

---

## Architecture Documents

Created during analysis (see `architecture/` directory):
- `INTEGRATION_PATTERNS.md` - Polling vs WebSocket ADRs
- `DATA_MODEL.md` - Silver/Gold schema design
- `FEATURE_ENGINEERING.md` - ML feature definitions
- `AIR_QUALITY_DOMAIN.md` - Ventilation thresholds for Florida
- `RECOMMENDATIONS_SUMMARY.md` - Consolidated decisions
- `DRAFT_STREAM_CONFIG.yaml` - Working stream configuration

---

## Out of Scope (Future Features)

- Real-time WebSocket integration with HA
- Automated window recommendations/alerts
- ML model for ventilation prediction
- Cross-ventilation pattern detection
- Multi-floor air flow modeling
