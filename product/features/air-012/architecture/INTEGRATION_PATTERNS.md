# Home Assistant Integration Architecture

**Feature**: air-012
**Status**: Proposed
**Date**: 2026-01-19
**Author**: NDP Architect

---

## Executive Summary

This document analyzes integration patterns for ingesting Home Assistant window sensor data into the Neural Data Platform. Window state (open/closed) represents **binary state data** - fundamentally different from the continuous sensor readings currently handled by NDP.

**Key Architectural Decisions:**
1. **ADR-012-001**: Polling vs Event-Driven - Recommends **HTTP Polling** for MVP with WebSocket upgrade path
2. **ADR-012-002**: Data Model - Recommends **Separate Bronze Stream** with event-sourced model
3. **ADR-012-003**: Integration Pattern - Recommends **Extend Existing HTTP Polling** with new parser

---

## Context

### Home Assistant API

Home Assistant exposes device state via REST API:

```
GET http://192.168.52.221:8123/api/states/<entity_id>
Authorization: Bearer $HASS_TOKEN
```

**Response Structure:**
```json
{
  "entity_id": "binary_sensor.office_window",
  "state": "off",
  "attributes": {
    "device_class": "window",
    "friendly_name": "Office Window"
  },
  "last_changed": "2025-12-31T12:31:02.467501+00:00",
  "last_reported": "2025-12-31T12:31:32.486649+00:00",
  "last_updated": "2025-12-31T12:31:02.467501+00:00"
}
```

**Key Characteristics:**
- Binary state: `on` (open) / `off` (closed)
- Multiple timestamps: `last_changed`, `last_reported`, `last_updated`
- Entity-specific attributes (device_class, friendly_name)
- State changes are infrequent (minutes to hours)

### Comparison with Existing Data Sources

| Aspect | Air Quality (MQTT) | Weather (HTTP) | Window State (New) |
|--------|-------------------|----------------|-------------------|
| Update Frequency | Every 2 min | Every 10 min | On state change |
| Data Type | Continuous (float) | Continuous (float) | Binary (on/off) |
| Protocol | Push (MQTT) | Poll (HTTP) | Poll or Push |
| Source | AirGradient | OpenWeatherMap | Home Assistant |
| Latency Requirement | Minutes | 10+ minutes | Seconds (for ML) |

---

## ADR-012-001: Polling vs Event-Driven

### Status

**Accepted** (2026-01-19)

### Context

Home Assistant offers multiple integration methods:

1. **HTTP Polling**: GET `/api/states/<entity_id>` at intervals
2. **WebSocket API**: `ws://host:8123/api/websocket` for real-time events
3. **MQTT Integration**: Home Assistant can publish state changes to MQTT
4. **Webhooks**: Home Assistant automations can POST to external endpoints

We need to evaluate these options against NDP's requirements and constraints.

### Options Evaluated

#### Option A: HTTP Polling (Recommended for MVP)

**Implementation:**
- Poll `/api/states/<entity_id>` every 30-60 seconds
- Use existing `HttpPollingSource` infrastructure
- Create `HomeAssistantStateParser` implementing `ResponseParser` trait

**Pros:**
- Zero new infrastructure - reuses existing HTTP polling patterns
- Proven reliability in NDP (OpenWeatherMap integration)
- Simple debugging (curl-testable)
- Stateless - no connection management
- Works with existing `ParserRegistry` pattern

**Cons:**
- Latency: 30-60 second delay between state change and detection
- Resource usage: Polls even when state hasn't changed
- Scales linearly with entity count (1 request per entity)

**Latency Analysis:**
For window-to-air-quality correlation in ML models:
- Air quality changes happen over minutes (CO2 buildup)
- 30-60 second window state latency is acceptable for:
  - Ventilation correlation analysis
  - Predictive model training
  - Dashboard visualization
- NOT acceptable for: Real-time alerting, instant automation

**Resource Impact (Pi 5):**
```
Entities: 10 windows
Poll interval: 30 seconds
Requests/minute: 20
Bandwidth: ~2KB/request = 40KB/min = 2.4MB/hour
CPU: Negligible (JSON parsing)
```

#### Option B: WebSocket Event Stream (Future Enhancement)

**Implementation:**
- Connect to `ws://192.168.52.221:8123/api/websocket`
- Subscribe to state_changed events for specific entities
- New `WebSocketSource` implementing `Source` trait

**Authentication Flow:**
```json
// 1. Connect
// 2. Receive auth_required
// 3. Send auth with access token
{"type": "auth", "access_token": "..."}
// 4. Receive auth_ok
// 5. Subscribe to events
{"type": "subscribe_events", "event_type": "state_changed"}
```

**Pros:**
- Near-instant state change detection (<1 second)
- Efficient: Only receive actual changes
- Single connection for all entities
- Scales to 100+ entities without additional requests

**Cons:**
- New source type required (`WebSocketSource`)
- Connection management complexity (reconnection, heartbeats)
- State reconciliation on reconnection
- More complex testing (mock WebSocket server)
- Additional dependencies (tungstenite, tokio-tungstenite)

**When to Upgrade:**
- Real-time alerting requirements
- Entity count > 50
- ML models requiring sub-second latency

#### Option C: MQTT Bridge (Rejected)

**Implementation:**
- Configure Home Assistant MQTT integration
- Publish state changes to `homeassistant/binary_sensor/+/state`
- Use existing `MqttSource` with new parser

**Pros:**
- Reuses existing MQTT infrastructure
- Push-based (real-time)
- Decoupled from Home Assistant API changes

**Cons:**
- Requires Home Assistant configuration changes
- MQTT integration setup complexity
- Different message format than REST API
- Debugging harder (message broker in middle)
- Couples NDP to Home Assistant's MQTT implementation

**Verdict:** Rejected - External configuration dependency, maintenance burden.

#### Option D: Webhooks (Rejected)

**Implementation:**
- Configure Home Assistant automation to POST on state change
- NDP exposes webhook endpoint
- Use existing `WebhookHandler` infrastructure

**Pros:**
- Push-based (real-time)
- Minimal NDP changes (webhook exists)

**Cons:**
- Requires Home Assistant automation setup per entity
- Firewall/network configuration (NDP must be reachable)
- No initial state on NDP startup (must poll once)
- Webhook failures not visible in Home Assistant UI
- Configuration scattered (NDP + Home Assistant)

**Verdict:** Rejected - Operational complexity, configuration scattered across systems.

### Decision

**Implement HTTP Polling for MVP, design for WebSocket upgrade.**

Rationale:
1. Reuses proven infrastructure (HttpPollingSource, ParserRegistry)
2. 30-60 second latency acceptable for air quality correlation use case
3. Simple operations (stateless, curl-testable)
4. Clean upgrade path to WebSocket when real-time requirements emerge

**Migration Path:**
```
MVP (air-012):     HTTP Polling → 30-60s latency
Enhancement:       WebSocket → <1s latency (optional)
Future:            Auto-select based on latency requirements
```

### Consequences

**Positive:**
- Fast implementation (days vs weeks)
- Leverages existing patterns and tests
- Simple debugging and monitoring
- No new dependencies

**Negative:**
- 30-60 second latency for state changes
- Polls continuously even when state stable
- Must implement WebSocket source for real-time requirements

---

## ADR-012-002: Data Model

### Status

**Accepted** (2026-01-19)

### Context

Window state data differs fundamentally from existing NDP streams:

| Aspect | Continuous Sensors | Binary State |
|--------|-------------------|--------------|
| Value type | Float (pm25: 15.3) | Enum (on/off) |
| Change frequency | Continuous | Discrete events |
| Meaningful queries | AVG, percentiles | Duration, count, transitions |
| Time semantics | Sample at timestamp | State valid until next change |

Two modeling approaches exist:
1. **Event-Sourced**: Store each state change as an event
2. **Snapshot**: Store current state periodically

### Options Evaluated

#### Option A: Event-Sourced Model (Recommended)

**Storage Pattern:**
```
timestamp                    | entity_id           | state | prev_state | duration_secs
2025-01-15 10:00:00+00:00   | office_window       | on    | off        | 3600
2025-01-15 11:00:00+00:00   | office_window       | off   | on         | NULL
```

**Characteristics:**
- Only store when state changes (efficient)
- Duration calculated as time until next event
- Enables: "How long was window open?" queries
- Natural fit for `state_changed` event model

**Query Examples:**
```sql
-- Total open time per window today
SELECT entity_id,
       SUM(CASE WHEN state = 'on' THEN duration_secs END) as open_seconds
FROM window_events
WHERE timestamp > NOW() - INTERVAL '24 hours'
GROUP BY entity_id;

-- Windows currently open
SELECT entity_id, timestamp
FROM window_events w1
WHERE state = 'on'
  AND NOT EXISTS (
    SELECT 1 FROM window_events w2
    WHERE w2.entity_id = w1.entity_id
      AND w2.timestamp > w1.timestamp
  );
```

**Pros:**
- Storage efficient (only changes stored)
- Rich temporal queries (duration, transitions)
- Matches Home Assistant's `state_changed` semantics
- Enables window open/close analytics

**Cons:**
- Requires state tracking in parser
- "Current state" queries need latest-event pattern
- Duration calculation requires window functions

#### Option B: Snapshot Model (Rejected)

**Storage Pattern:**
```
timestamp                    | entity_id           | state
2025-01-15 10:00:00+00:00   | office_window       | on
2025-01-15 10:00:30+00:00   | office_window       | on
2025-01-15 10:01:00+00:00   | office_window       | on
...
2025-01-15 11:00:00+00:00   | office_window       | off
```

**Characteristics:**
- Store state at each poll interval (every 30s)
- Simple: same model as continuous sensors
- Redundant data when state unchanged

**Pros:**
- Matches existing continuous sensor pattern
- Simple parser (no state tracking)
- Easy time-based queries

**Cons:**
- Storage bloat: 30s polls = 2880 rows/day/entity (vs ~10-20 events)
- Loses precise state change timestamps
- Duration queries require row counting
- No semantic meaning to repeated identical states

**Verdict:** Rejected - Storage inefficient, loses temporal precision.

#### Option C: Hybrid (Rejected)

Store both snapshots and events in separate tables.

**Verdict:** Rejected - Complexity without benefit; event model sufficient.

### Decision

**Implement Event-Sourced Model** with:
1. Store only state changes (from Home Assistant `last_changed` timestamp)
2. Track previous state for transition analysis
3. Compute duration in Silver layer views

### Stream Configuration

**Recommended Stream ID:** `home-assistant-state` (generic for all HA binary sensors)

**Bronze Schema:**
```yaml
stream_id: home-assistant-state
description: "Home Assistant binary sensor state changes"
version: "1.0.0"
fields:
  - name: entity_id
    type: string
    description: "Home Assistant entity identifier"
    nullable: false
  - name: state
    type: string
    description: "Current state (on/off/unknown)"
    nullable: false
  - name: device_class
    type: string
    description: "Device class (window, door, motion, etc.)"
    nullable: true
  - name: friendly_name
    type: string
    description: "Human-readable name"
    nullable: true
  - name: last_changed
    type: string
    description: "ISO8601 timestamp of last state change"
    nullable: false
  - name: last_updated
    type: string
    description: "ISO8601 timestamp of last update"
    nullable: false
```

**Silver Schema (TimescaleDB):**
```sql
CREATE TABLE home_assistant_state (
    time            TIMESTAMPTZ NOT NULL,
    entity_id       TEXT NOT NULL,
    state           TEXT NOT NULL,
    device_class    TEXT,
    friendly_name   TEXT,
    -- Derived columns for analytics
    prev_state      TEXT,
    state_duration_ms BIGINT
);
SELECT create_hypertable('home_assistant_state', 'time');

-- Index for entity-specific queries
CREATE INDEX idx_ha_state_entity ON home_assistant_state (entity_id, time DESC);
```

### Consequences

**Positive:**
- Storage efficient (events only)
- Rich temporal analytics (duration, transitions)
- Generic model supports all binary sensors (windows, doors, motion)
- Aligned with Home Assistant's event model

**Negative:**
- Parser must track state to detect changes
- "Current state" queries slightly more complex
- Need to handle first poll (unknown previous state)

---

## ADR-012-003: Integration Pattern

### Status

**Accepted** (2026-01-19)

### Context

NDP supports multiple source types:
- `MqttSource`: Push-based MQTT subscriptions
- `HttpPollingSource`: Pull-based HTTP with pluggable parsers
- `WebhookHandler`: Push-based HTTP callbacks

We need to decide how Home Assistant integration fits into this architecture.

### Options Evaluated

#### Option A: Extend HTTP Polling with New Parser (Recommended)

**Implementation:**
1. Create `HomeAssistantStateParser` implementing `ResponseParser` trait
2. Register in `ParserRegistry::with_default_parsers()`
3. Configure via stream YAML with multiple endpoints (one per entity)

**Parser Design:**
```rust
pub struct HomeAssistantStateParser {
    // Track last known state per entity for change detection
    last_states: Arc<Mutex<HashMap<String, String>>>,
}

impl ResponseParser for HomeAssistantStateParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let response: HassStateResponse = serde_json::from_str(response_body)?;

        // Check if state changed since last poll
        let mut last_states = self.last_states.lock().unwrap();
        let prev_state = last_states.get(&response.entity_id).cloned();

        if prev_state.as_deref() == Some(&response.state) {
            // No change - return empty (event-sourced model)
            return Ok(vec![]);
        }

        // State changed - record event
        last_states.insert(response.entity_id.clone(), response.state.clone());

        // Create TimeSeriesPoint with state data
        Ok(vec![create_state_point(&response, prev_state, timestamp)])
    }

    fn name(&self) -> &'static str {
        "home_assistant_state"
    }
}
```

**Configuration:**
```yaml
# config/base/streams/home-assistant-state/config.yaml
stream_id: home-assistant-state
description: "Home Assistant binary sensor state changes"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: entity_id
    type: string
    nullable: false
  - name: state
    type: string
    nullable: false
  - name: device_class
    type: string
    nullable: true
  - name: friendly_name
    type: string
    nullable: true
  - name: last_changed
    type: string
    nullable: false

sources:
  - type: http_poll
    enabled: true
    ndp_id: home-assistant-integration
    poll_interval_secs: 30
    timeout_secs: 10
    auth_method: bearer
    auth_value: "${HASS_TOKEN}"
    parser_name: home_assistant_state
    endpoints:
      - endpoint_id: office-window
        url: "http://192.168.52.221:8123/api/states/binary_sensor.office_window"
        location_id: office
      - endpoint_id: bedroom-window
        url: "http://192.168.52.221:8123/api/states/binary_sensor.bedroom_window"
        location_id: bedroom
      # Add more entities as needed
```

**Pros:**
- Minimal new code (parser only)
- Uses existing infrastructure (HttpPollingSource, ParserRegistry)
- Consistent with OpenWeatherMap pattern
- Config-driven entity management
- Testable in isolation

**Cons:**
- One HTTP request per entity (N requests for N windows)
- Parser holds state (needs careful lifecycle management)

#### Option B: New Source Type (Future WebSocket)

**Implementation:**
Create `HomeAssistantSource` that handles both REST and WebSocket modes.

```rust
pub struct HomeAssistantSource {
    mode: HassMode,  // Poll or WebSocket
    base_url: String,
    token: String,
    entities: Vec<String>,
}

pub enum HassMode {
    Poll { interval: Duration },
    WebSocket,
}
```

**Pros:**
- Optimal protocol selection
- Single source for all entities
- Future-proof

**Cons:**
- Significant new code
- Different pattern than existing sources
- WebSocket complexity deferred anyway

**Verdict:** Keep as future enhancement; not needed for MVP.

#### Option C: Bulk Entity Fetch (Alternative)

**Implementation:**
Use `/api/states` (all entities) or template endpoint to fetch multiple entities.

```
GET http://192.168.52.221:8123/api/states
```

**Pros:**
- Single request for all entities
- Efficient for large entity counts

**Cons:**
- Returns ALL entities (thousands) - filter required
- No native filter parameter in HA API
- Large payload parsing
- Still requires polling

**Verdict:** Consider if entity count exceeds 20; HTTP overhead becomes significant.

### Decision

**Implement Option A: Extend HTTP Polling with New Parser**

Rationale:
1. Minimal new code - leverage existing infrastructure
2. Consistent with established patterns (OpenWeatherMap)
3. Clear upgrade path to WebSocket source
4. Testable and debuggable

### Implementation Checklist

1. **Parser Implementation:**
   - [ ] `HomeAssistantStateParser` struct with state tracking
   - [ ] `ResponseParser` trait implementation
   - [ ] Unit tests with mock responses
   - [ ] Register in `ParserRegistry::with_default_parsers()`

2. **Configuration:**
   - [ ] Stream config YAML (`config/base/streams/home-assistant-state/config.yaml`)
   - [ ] Environment variable documentation (`HASS_TOKEN`)
   - [ ] Entity discovery guide

3. **Testing:**
   - [ ] Parser unit tests (state change detection, no-change handling)
   - [ ] Integration test with mock Home Assistant
   - [ ] End-to-end test on Pi with real Home Assistant

4. **Silver Layer:**
   - [ ] TimescaleDB schema for state events
   - [ ] DuckDB views for Bronze analytics
   - [ ] Grafana dashboard panel

### Consequences

**Positive:**
- Rapid implementation (days)
- Proven infrastructure
- Easy to debug and monitor
- Config-driven entity management

**Negative:**
- One request per entity (acceptable for <20 entities)
- 30-60 second latency (acceptable for correlation use case)
- Parser holds state (requires careful shutdown handling)

---

## Entity Discovery and Management

### Manual vs Automatic Discovery

**Recommendation: Manual Configuration (MVP)**

Home Assistant exposes entity lists via:
- `/api/states` - All entities (requires filtering)
- `/api/template` - Custom queries

For MVP, manual configuration is recommended because:
1. Explicit control over which entities NDP tracks
2. No complex filtering logic
3. Clear documentation of monitored devices

**Future Enhancement: Auto-Discovery**

```yaml
# Future config pattern
sources:
  - type: http_poll
    parser_name: home_assistant_state
    discovery:
      enabled: true
      filter:
        domain: binary_sensor
        device_class: [window, door]
      poll_interval_secs: 3600  # Re-discover hourly
```

---

## Cross-Stream Correlation Patterns

### Window State + Air Quality

The primary use case is correlating window state with indoor air quality:

```sql
-- Air quality change after window opened
WITH window_events AS (
  SELECT
    time,
    entity_id,
    state,
    LEAD(time) OVER (PARTITION BY entity_id ORDER BY time) as next_change
  FROM home_assistant_state
  WHERE device_class = 'window'
    AND time > NOW() - INTERVAL '24 hours'
)
SELECT
  w.entity_id,
  w.time as window_opened,
  aq.time as reading_time,
  aq.pm25,
  aq.co2
FROM window_events w
ASOF JOIN air_quality_readings aq
  ON aq.time >= w.time AND aq.time < COALESCE(w.next_change, NOW())
WHERE w.state = 'on'
ORDER BY w.time, aq.time;
```

### Dashboard Overlay

Grafana annotations can show window state on air quality graphs:

```sql
-- Annotation query for window open periods
SELECT
  time,
  entity_id || ': ' || state as text,
  CASE state WHEN 'on' THEN 'window-open' ELSE 'window-closed' END as tags
FROM home_assistant_state
WHERE device_class = 'window'
  AND $__timeFilter(time)
```

---

## Risk Analysis

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Home Assistant API changes | Low | Medium | Version-specific parser, HA changelog monitoring |
| Token expiration | Medium | High | Token rotation docs, monitoring for 401 errors |
| Network partition (Pi <-> HA) | Medium | Low | Retry logic, staleness detection |
| Entity rename/removal | Medium | Low | Graceful 404 handling, config validation on startup |
| State drift (missed changes) | Low | Medium | Periodic full state reconciliation |

---

## Summary

### Recommended Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                    Home Assistant                            │
│  - REST API: /api/states/<entity_id>                        │
│  - Authentication: Bearer token                              │
└─────────────────────────┬────────────────────────────────────┘
                          │ HTTP Poll (30s)
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                  HttpPollingSource                           │
│  - Existing NDP component                                    │
│  - Multiple endpoints (1 per entity)                         │
│  - Bearer auth via $HASS_TOKEN                               │
└─────────────────────────┬────────────────────────────────────┘
                          │
                          ▼
┌──────────────────────────────────────────────────────────────┐
│              HomeAssistantStateParser (NEW)                  │
│  - Implements ResponseParser trait                           │
│  - Tracks state for change detection                         │
│  - Event-sourced: only emits on state change                │
└─────────────────────────┬────────────────────────────────────┘
                          │ TimeSeriesPoint
                          ▼
┌──────────────────────────────────────────────────────────────┐
│                     Bronze Layer                             │
│  - Stream: home-assistant-state                              │
│  - Parquet: /data/home-assistant-state/YYYY-MM-DD/*.parquet │
└──────────────────────────────────────────────────────────────┘
```

### Key Decisions Summary

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-012-001 | HTTP Polling | Reuses infrastructure, acceptable latency |
| ADR-012-002 | Event-Sourced Model | Storage efficient, rich temporal queries |
| ADR-012-003 | Extend HTTP Polling | Minimal code, proven patterns |

### Next Steps

1. **Implementation**: Create `HomeAssistantStateParser` and register in parser registry
2. **Configuration**: Add stream config YAML and environment documentation
3. **Testing**: Unit tests, integration tests, Pi end-to-end validation
4. **Silver Layer**: TimescaleDB schema and Grafana dashboard

---

## References

- [Home Assistant REST API](https://developers.home-assistant.io/docs/api/rest)
- [Home Assistant WebSocket API](https://developers.home-assistant.io/docs/api/websocket)
- ADR-001-MULTISTREAM-FOUNDATION.md (AIR-004)
- ADR-001-MQTT-SUBSCRIPTIONS.md (DP-003)
- core/src/sources/http_poll.rs (HttpPollingSource)
- core/src/sources/parsers/weather.rs (ResponseParser example)

---

**Document History:**

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-01-19 | Initial architecture analysis |
