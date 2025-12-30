# DP-003: MQTT Multi-Subscription User Stories

## Overview

This document defines user stories for the MQTT multi-subscription feature, organized by actor and priority.

---

## Actors

| Actor | Description |
|-------|-------------|
| **Platform Operator** | Manages NDP configuration and deployment |
| **Data Engineer** | Configures data streams and monitors ingestion |
| **Dashboard User** | Consumes data through Grafana dashboards |
| **System** | Automated processes and integrations |

---

## Epic: Multi-Stream MQTT Ingestion

As a **Platform Operator**, I want to ingest data from multiple MQTT topic patterns through a single broker connection, so that I can add new data sources without code changes or service restarts.

---

## User Stories

### US-001: Configure Multiple Subscriptions

**Priority**: High
**Story Points**: 5
**Sprint**: 1

```
As a Data Engineer
I want to configure multiple MQTT subscriptions in a single config file
So that I can route different topic patterns to different streams
```

**Acceptance Criteria**:
- [ ] Can define `subscriptions` array in YAML config
- [ ] Each subscription specifies `stream_id` and `topic_pattern`
- [ ] Configuration validates for duplicate stream_ids
- [ ] Configuration validates topic pattern syntax

**Example Config**:
```yaml
sources:
  - type: mqtt
    params:
      subscriptions:
        - stream_id: air-quality
          topic_pattern: "airgradient/readings/+"
        - stream_id: homeassistant
          topic_pattern: "homeassistant/+/+/state"
```

---

### US-002: Backward Compatible Configuration

**Priority**: High
**Story Points**: 3
**Sprint**: 1

```
As a Platform Operator
I want existing single topic_pattern configs to continue working
So that I don't need to immediately migrate all configurations
```

**Acceptance Criteria**:
- [ ] Legacy `topic_pattern` field still works
- [ ] System auto-creates single subscription from legacy format
- [ ] No changes required to existing air-quality config
- [ ] Existing tests continue to pass

**Migration Path**:
```yaml
# OLD (still works)
params:
  topic_pattern: "airgradient/readings/+"

# NEW (recommended)
params:
  subscriptions:
    - stream_id: air-quality
      topic_pattern: "airgradient/readings/+"
```

---

### US-003: Route Messages by Topic Pattern

**Priority**: High
**Story Points**: 5
**Sprint**: 1

```
As the System
I want to match incoming MQTT messages to the correct subscription
So that data is stored in the appropriate stream partition
```

**Acceptance Criteria**:
- [ ] Messages matching pattern X go to stream X
- [ ] First-match routing when patterns overlap
- [ ] Unmatched messages logged but not stored
- [ ] Routing adds stream_id tag to TimeSeriesPoint

**Routing Examples**:
| Topic | Pattern Match | Stream |
|-------|---------------|--------|
| `airgradient/readings/abc123` | `airgradient/readings/+` | air-quality |
| `homeassistant/sensor/temp/state` | `homeassistant/+/+/state` | homeassistant |
| `unknown/topic` | (none) | (logged, dropped) |

---

### US-004: Per-Subscription Parser Configuration

**Priority**: Medium
**Story Points**: 3
**Sprint**: 1

```
As a Data Engineer
I want to specify different parser settings for each subscription
So that I can handle different JSON payload formats
```

**Acceptance Criteria**:
- [ ] Each subscription can define its own parser config
- [ ] Parser config includes location_id_field
- [ ] Parser config includes skip_fields list
- [ ] Default parser used if none specified

**Example**:
```yaml
subscriptions:
  - stream_id: air-quality
    topic_pattern: "airgradient/readings/+"
    parser:
      location_id_field: serialno
      skip_fields: [serialno, firmware, model]
  - stream_id: homeassistant
    topic_pattern: "homeassistant/+/+/state"
    parser:
      location_id_field: entity_id
```

---

### US-005: Single Broker Connection

**Priority**: High
**Story Points**: 2
**Sprint**: 1

```
As a Platform Operator
I want multiple subscriptions to share one broker connection
So that I minimize network and broker resource usage
```

**Acceptance Criteria**:
- [ ] Only one TCP connection per MqttSource
- [ ] All subscriptions use shared connection
- [ ] Connection health reflects all subscriptions
- [ ] Disconnection affects all subscriptions equally

---

### US-006: Automatic Reconnection with Re-subscribe

**Priority**: High
**Story Points**: 3
**Sprint**: 1

```
As the System
I want to automatically reconnect and re-subscribe after connection loss
So that data ingestion resumes without manual intervention
```

**Acceptance Criteria**:
- [ ] Reconnection uses exponential backoff
- [ ] All subscriptions re-subscribed after reconnect
- [ ] QoS 1 ensures no message loss during brief outages
- [ ] Reconnection logged at WARN level

---

### US-007: HomeAssistant Stream Enablement

**Priority**: High
**Story Points**: 2
**Sprint**: 1

```
As a Data Engineer
I want to enable the HomeAssistant stream configuration
So that I can ingest Home Assistant state data alongside air-quality
```

**Acceptance Criteria**:
- [ ] HomeAssistant config loads without errors
- [ ] HA data written to `data/bronze/homeassistant/` partition
- [ ] Air-quality data continues in `data/bronze/air-quality/`
- [ ] Both streams visible in Grafana data source

**Current Blocker**: HomeAssistant stream fails because MqttSource only supports one topic pattern. This feature unblocks it.

---

### US-008: Health Status Per Subscription

**Priority**: Medium
**Story Points**: 3
**Sprint**: 2

```
As a Platform Operator
I want to see health status for each MQTT subscription
So that I can identify which streams are receiving data
```

**Acceptance Criteria**:
- [ ] Health endpoint shows per-subscription status
- [ ] Status includes: healthy/unhealthy, message_count, last_message_at
- [ ] Subscription with no recent messages marked as stale
- [ ] Overall MqttSource health aggregates subscription health

**Health Response Example**:
```json
{
  "source_id": "air-quality-Mqtt",
  "healthy": true,
  "subscriptions": [
    {
      "stream_id": "air-quality",
      "status": "healthy",
      "message_count": 1523,
      "last_message_at": "2025-12-30T12:34:56Z"
    },
    {
      "stream_id": "homeassistant",
      "status": "healthy",
      "message_count": 456,
      "last_message_at": "2025-12-30T12:34:55Z"
    }
  ]
}
```

---

### US-009: Configuration Validation

**Priority**: Medium
**Story Points**: 2
**Sprint**: 1

```
As a Data Engineer
I want clear error messages when configuration is invalid
So that I can quickly fix misconfigurations
```

**Acceptance Criteria**:
- [ ] Duplicate stream_id detected at load time
- [ ] Invalid topic pattern syntax detected
- [ ] Missing required fields detected
- [ ] Error messages include file path and line reference

**Error Examples**:
```
ConfigError: Duplicate stream_id "air-quality" in subscriptions
  at: config/base/streams/air-quality/config.yaml

ConfigError: Invalid topic pattern "airgradient/readings/[invalid"
  at: subscriptions[0].topic_pattern
  hint: MQTT wildcards are + (single level) and # (multi-level)
```

---

### US-010: Structured Logging with Stream Context

**Priority**: Low
**Story Points**: 2
**Sprint**: 2

```
As a Platform Operator
I want all MQTT logs to include stream context
So that I can filter logs by stream when debugging
```

**Acceptance Criteria**:
- [ ] All logs include stream_id field
- [ ] All logs include source=mqtt field
- [ ] Connection events logged at INFO
- [ ] Parse errors logged at WARN with payload snippet

**Log Examples**:
```
INFO mqtt: Connected to broker broker_url="mosquitto" port=1883
INFO mqtt: Subscribed to topic stream_id="air-quality" topic="airgradient/readings/+"
DEBUG mqtt: Message received stream_id="air-quality" topic="airgradient/readings/abc123"
WARN mqtt: Parse error stream_id="air-quality" error="invalid JSON" payload_preview="not valid..."
```

---

## Story Map

```
                        Sprint 1                    Sprint 2
                    ┌────────────────────┐     ┌────────────────┐
High Priority       │ US-001: Multi-sub  │     │                │
                    │ US-002: Backward   │     │                │
                    │ US-003: Routing    │     │                │
                    │ US-005: Single conn│     │                │
                    │ US-006: Reconnect  │     │                │
                    │ US-007: HA stream  │     │                │
                    └────────────────────┘     └────────────────┘
                    ┌────────────────────┐     ┌────────────────┐
Medium Priority     │ US-004: Parser cfg │     │ US-008: Health │
                    │ US-009: Validation │     │                │
                    └────────────────────┘     └────────────────┘
                    ┌────────────────────┐     ┌────────────────┐
Low Priority        │                    │     │ US-010: Logging│
                    └────────────────────┘     └────────────────┘
```

---

## Dependencies

```
US-001 ─┬─> US-003 ─> US-007
        │
        └─> US-004

US-005 ─> US-006

US-002 (independent, but validates US-001)

US-009 (validates US-001, US-004)

US-008, US-010 (Sprint 2, no blockers)
```

---

## Story Sizing Reference

| Points | Complexity | Example |
|--------|------------|---------|
| 1 | Trivial | Config field addition |
| 2 | Simple | Single module change |
| 3 | Moderate | Cross-module changes |
| 5 | Complex | New subsystem or major refactor |
| 8 | Very Complex | Architecture change |

**Total Sprint 1**: 25 points
**Total Sprint 2**: 5 points

---

## Definition of Ready

A user story is ready for development when:

1. [ ] Acceptance criteria are testable
2. [ ] Dependencies are identified and unblocked
3. [ ] Technical approach is understood
4. [ ] Story fits in a single sprint
5. [ ] No open questions blocking implementation

---

## Traceability Matrix

| User Story | Requirements | Acceptance Criteria |
|------------|--------------|---------------------|
| US-001 | FR-2.1.1, FR-2.1.2, FR-2.1.3 | AC-1.1 |
| US-002 | FR-2.1.5, NFR-3.4.1 | AC-1.2 |
| US-003 | FR-2.2.1, FR-2.2.2, FR-2.2.3 | AC-2.1 |
| US-004 | FR-2.1.4, FR-2.3.1 | AC-1.3 |
| US-005 | FR-2.4.1 | AC-4.1 |
| US-006 | FR-2.4.2, FR-2.4.3 | AC-4.2 |
| US-007 | NFR-3.4.2 | AC-7.1, AC-7.2 |
| US-008 | NFR-3.5.1 | AC-6.1 |
| US-009 | NFR-3.3.2 | AC-1.1 |
| US-010 | NFR-3.5.3, NFR-3.5.4 | AC-6.2 |
