# air-012: Status Tracker

**Feature**: Home Assistant Integration (Barebones)
**Current Phase**: Refinement Complete - Ready for Deployment
**Started**: 2026-01-29
**Last Updated**: 2026-01-31

---

## Current Status: Implementation Complete (Pending Deployment)

**2026-01-31 Implementation Completed:**

All code and configuration artifacts created:
- ✅ Stream config with dynamic ndp_id extraction
- ✅ MQTT plain text wrapping (non-JSON → wrapped JSON)
- ✅ Silver DDL with hypertable, compression, retention
- ✅ Dimension table entries (3 sensors)
- ✅ Pipeline health queries with sparse-data thresholds
- ✅ Unit tests for ndp_id extraction (5 new tests)
- ✅ Pattern recorded in AgentDB (ID: 95)

**Ready for Pi deployment and integration testing.**

---

## Phase Status

| Phase | Status | Started | Completed |
|-------|--------|---------|-----------|
| Scope Definition | **Complete** | 2026-01-29 | 2026-01-30 |
| Specification (SPARC-S) | **Complete** | 2026-01-30 | 2026-01-30 |
| Pseudocode (SPARC-P) | Skipped | - | - |
| Architecture (SPARC-A) | **Complete** | 2026-01-30 | 2026-01-30 |
| Refinement (SPARC-R) | **Complete** | 2026-01-31 | 2026-01-31 |
| Completion (SPARC-C) | In Progress | 2026-01-31 | - |

**Note:** Pseudocode skipped - this is config-driven work, not algorithm design.

---

## Documentation Summary

### New Documents (Created 2026-01-30)

| Document | Location | Description |
|----------|----------|-------------|
| SPECIFICATION.md | `specification/` | 7 FRs, 3 NFRs, interface contracts |
| VALIDATION.md | `specification/` | Test scenarios, manual validation |
| ADR-001-simple-event-log.md | `architecture/` | Decision: simple event log vs SCD |
| STREAM_CONFIG_DESIGN.md | `architecture/` | Proposed stream config YAML |
| SILVER_SCHEMA.md | `architecture/` | DDL, hypertable, indexes, retention |
| SILVER_ETL_CONFIG.md | `architecture/` | silver_etl YAML, DQ rules |
| TEST_STRATEGY.md | `refinement/` | Test pyramid, mock strategy |
| ACCEPTANCE_TESTS.md | `refinement/` | 14 test scenarios with scripts |
| ALIGNMENT_REPORT.md | `.` | Audit of existing docs |

### Existing Documents (Status per Alignment Report)

| Document | Status | Action |
|----------|--------|--------|
| INTEGRATION_PATTERNS.md | Superseded | Keep as reference (HTTP approach) |
| DATA_MODEL.md | Partially superseded | Silver sections replaced by new docs |
| FEATURE_ENGINEERING.md | Deferred | Move to dp-014/fe-001 |
| AIR_QUALITY_DOMAIN.md | Valid | Domain reference |
| RECOMMENDATIONS_SUMMARY.md | Partially superseded | HTTP recommendations obsolete |
| DRAFT_STREAM_CONFIG.yaml | Superseded | Replace with STREAM_CONFIG_DESIGN.md |

---

## Key Design Decisions

| Decision | Choice | ADR/Implementation |
|----------|--------|-----|
| Schema complexity | Simple event log | ADR-001 |
| SCD semantics | Deferred to dp-014 Gold layer | ADR-001 |
| Timestamp source | Ingestion time (not HA event time) | SPECIFICATION.md |
| Freshness thresholds | 18hr/36hr (sparse-aware) | SILVER_ETL_CONFIG.md |
| Deduplication | Upsert with 1-min window | SILVER_ETL_CONFIG.md |
| Test approach | Integration-heavy (70%) | TEST_STRATEGY.md |
| Plain text handling | Wrap as JSON at MQTT source | `{"_raw_text": "on", "_topic": "..."}` |
| ndp_id extraction | Dynamic from topic segment | `ndp_id_topic_segment: 2` config |

---

## Identified Gaps/Risks

| Gap | Risk | Status |
|-----|------|--------|
| Plain-text MQTT parser | May need new parser code | ✅ **Resolved** - MQTT wraps text as JSON |
| Topic wildcard filtering | Unknown sensors create orphan events | Accept all, dimension JOIN filters |
| No point-in-time queries | Cannot correlate until dp-014 | Ad-hoc window function query |
| MQTT no auth | Low (internal network) | Document as tech debt |
| Per-stream health thresholds | Dashboard may use global threshold | ✅ **Resolved** - Unified health query supports per-stream |
| Static vs dynamic ndp_id | Event streams need per-device ndp_id | ✅ **Resolved** - Dynamic extraction from topic |

---

## Implementation Deliverables

| Deliverable | Status | Notes |
|-------------|--------|-------|
| Stream config | ✅ Complete | `config/base/streams/home-assistant-state/config.yaml` |
| Silver DDL | ✅ Complete | `deploy/timescaledb/init/002_state_events_schema.sql` |
| Dimension update | ✅ Complete | 3 sensors in `entity_context.csv` |
| Pipeline health | ✅ Complete | `deploy/grafana/dashboards/queries/state_events_health.sql` |
| MQTT text wrapping | ✅ Complete | `core/src/sources/mqtt/mod.rs` lines 325-343 |
| Dynamic ndp_id | ✅ Complete | `ndp_id_topic_segment` config option |

---

## Acceptance Criteria

### Bronze Layer
- [x] Stream config `home-assistant-state` created
- [ ] MQTT connects to broker `192.168.52.103:1883` *(requires Pi deployment)*
- [x] Topic `homeassistant/binary_sensor/+/state` configured
- [x] Raw payload wrapped for storage: `{"_raw_text": "on", "_topic": "..."}`
- [x] Dynamic ndp_id extracted from topic segment 2

### Silver Layer
- [x] `silver.state_events` table DDL created (hypertable)
- [x] ETL config extracts state from `raw_payload._raw_text`
- [x] `source_entity_id` from ndp_id with `prefix:binary_sensor.` transform
- [x] Ingestion timestamp mapped to `event_time`

### Dimension
- [x] 3 sensors in `entity_context.csv`: door_backslider, door_officewindow, door_dinettewindow
- [ ] `./deploy.sh sync-dimensions` loads data *(requires Pi deployment)*

### Pipeline Health
- [x] State events freshness queries created
- [x] 18hr = fresh, 18-36hr = stale, >36hr = critical thresholds
- [x] Unified health dashboard includes state_events with sparse-aware thresholds

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| MQTT source adapter | ✅ Ready | `core/src/sources/mqtt/` |
| Bronze Parquet storage | ✅ Ready | Working pattern |
| Dimension tables (dp-013) | ✅ Ready | `entity_context` config exists |
| Pipeline health dashboard | ✅ Ready | Verify threshold config |

---

## Related Features

| Feature | Relationship |
|---------|--------------|
| dp-013 | Provides dimension infrastructure ✅ |
| dp-014 | Will add Gold layer SCD (draft scope) |
| ml-??? | Future unsupervised learning |

---

## Next Steps (Completion Phase)

1. ~~**Verify MQTT parser** - Can existing router handle plain text payloads?~~ ✅ Done - MQTT wraps text as JSON
2. ~~**Create stream config** - Follow STREAM_CONFIG_DESIGN.md~~ ✅ Done
3. ~~**Add sensors to dimension** - Update `entity_context.csv`~~ ✅ Done
4. ~~**Create Silver DDL** - Follow SILVER_SCHEMA.md~~ ✅ Done
5. ~~**Configure pipeline health** - Add stream-specific thresholds~~ ✅ Done
6. **Deploy to Pi** - Run `./deploy.sh` to deploy changes
7. **Integration test** - Verify MQTT connectivity and data flow
8. **Validate Silver ETL** - Confirm data appears in `silver.state_events`

---

## Implementation Notes (2026-01-31)

### MQTT Plain Text Handling
- MQTT source wraps non-JSON payloads: `{"_raw_text": "on", "_topic": "homeassistant/..."}`
- No custom parser needed - FlatJsonParser handles wrapped JSON
- Silver ETL extracts state from `raw_payload._raw_text`

### Dynamic ndp_id Extraction
- Added `ndp_id_topic_segment` config option to SubscriptionConfig
- For topic `homeassistant/binary_sensor/door_backslider/state`, segment 2 = `door_backslider`
- Pattern recorded in AgentDB (ID: 95) - `architecture:mqtt-event-ndp-id`

### Event vs Time-Series Data Model
- Time-series (AirGradient): 1 device = 1 ndp_id, multiple metrics → static ndp_id
- Event-oriented (Home Assistant): N devices = N ndp_ids, sparse events → dynamic ndp_id from topic

---

*Status last updated: 2026-01-31 by implementation session*
