# DP-002: Online Data Dictionary & HomeAssistant Stream Preparation

**Created**: 2025-12-30
**Status**: Scoping
**Phase**: Data Platform (DP)
**Priority**: High

---

## Overview

Establish TimescaleDB as the Silver Layer foundation with an online, queryable data dictionary that enables configuration-driven schema management. This feature standardizes on `entity_schemas` as THE data dictionary definition mechanism across all streams, preparing the platform for HomeAssistant integration.

---

## Objectives

1. **Simplify deployment** by removing unused DuckDB container
2. **Establish TimescaleDB** as the Silver Layer database and data dictionary store
3. **Standardize on entity_schemas** as the unified data dictionary format for all streams
4. **Enable GitOps-driven schema management** with config → data dictionary sync
5. **Prepare HomeAssistant stream** configuration with AirGradient as test entity
6. **Create data quality dashboard** for identifying undefined/incomplete schemas

---

## Key Design Decision: Unified Data Dictionary Model

**Decision**: All streams use `entity_schemas` as THE data dictionary definition.

| Concept | Purpose | Used By |
|---------|---------|---------|
| `fields` | Bronze Parquet column schema | Ingestion (technical, unchanged) |
| `entity_schemas` | Data dictionary entries | Data dictionary, DQ dashboard, documentation |

**Rationale**:
- Single normalized data dictionary interface for all consumers
- Queries don't need to know if stream is single-source or multi-source
- Consistent config model across all streams
- `fields` remains for ingestion; `entity_schemas` added for documentation

**Approach for existing streams**: ADD `entity_schemas` to existing config files without modifying existing `fields` definitions. This preserves ingestion stability while enabling the data dictionary.

---

## Scope

### In Scope

#### 1. Remove DuckDB Container

- Remove `duckdb` service from `deploy/pi/docker-compose.yml`
- Retain Grafana DuckDB plugin for direct Parquet queries (Bronze layer access)
- Update deployment documentation
- Verify no runtime dependencies on the container

#### 2. Instantiate TimescaleDB

- Add TimescaleDB container to deployment configuration
- Configure for Raspberry Pi resource constraints
- Establish as Silver Layer foundation
- Create initial schema for data dictionary tables

#### 3. Add Entity Schemas to All Existing Streams

Add `entity_schemas` definitions to all 6 active streams (existing `fields` untouched):

| Stream | Schema Name | Description |
|--------|-------------|-------------|
| `air-quality` | airgradient | AirGradient indoor sensors |
| `outdoor-weather` | nws-weather | NWS current conditions |
| `outdoor-air-quality` | airnow | AirNow outdoor AQI |
| `nws-observations` | nws-observations | NWS station observations |
| `nws-forecast-hourly` | nws-hourly | NWS hourly forecasts |
| `nws-gridpoints-forecast` | nws-gridpoints | NWS gridpoint forecasts |

For each stream:
- Create `entity_schemas` section in existing `config.yaml`
- Define `schema_name`, `description`, and `attributes` array
- Document attribute `name`, `type`, `unit`, `description`
- Leave existing `fields` section unchanged (ingestion stability)

#### 4. Online Data Dictionary (Normalized)

Design queryable, normalized schema registry in TimescaleDB:

```
┌──────────────────────────────────────────────────────────────────────────┐
│                         data_dictionary                                   │
│  stream_id | schema_name | attribute | type | unit | description         │
├──────────────────────────────────────────────────────────────────────────┤
│  air-quality     │ airgradient              │ pm25  │ f64    │ µg/m³ │...│
│  outdoor-weather │ nws-weather              │ temp  │ f64    │ °F    │...│
│  homeassistant   │ sensor.airgradient_*     │ pm02  │ f64    │ µg/m³ │...│
│  homeassistant   │ binary_sensor.*_window*  │ state │ string │       │...│
└──────────────────────────────────────────────────────────────────────────┘
```

- One unified table/view for all data dictionary queries
- Works identically for streams with single schema or multiple entity patterns
- Support queries like:
  - "What streams are defined?"
  - "What attributes exist for stream X?"
  - "What attributes are expected for `sensor.airgradient_*`?"
  - "What schemas exist for device_class `window`?"

#### 5. HomeAssistant Stream Configuration

- Create `config/base/streams/homeassistant/config.yaml`
- Define Bronze layer schema (generic: entity_id, state, attributes JSON)
- Create initial entity_schema for AirGradient devices:
  - Schema name: `sensor.airgradient_*`
  - Device class: `air_quality`
  - Expected attributes: pm02, pm10, atmp, rhum, rco2, tvoc
- Use existing AirGradient data (via HA Statestream) as validation source

#### 6. Extend Deploy Script for Data Dictionary Sync

- **Extend existing `deploy/pi/deploy.sh`** - do NOT create separate sync process
- Add new command: `./deploy.sh sync-dictionary` (or integrate into existing `sync` command)
- Sync mechanism: etcd config → TimescaleDB data dictionary
- Single code path for all streams (entity_schemas → data_dictionary table)
- Support operations:
  - **Add**: New stream/entity_schema appears in config → insert to dictionary
  - **Update**: Schema definition changes → update dictionary
  - **Delete**: Schema removed from config → remove from dictionary
- Maintain consistency with existing deploy script patterns and conventions
- Source validation: Confirm etcd is the operational config source

#### 7. Data Quality Dashboard (Grafana)

- Dashboard: "HomeAssistant Data Quality"
- Panels:
  - **Schema Coverage Summary**: Known vs Unknown entities count
  - **Unknown Entities**: Entities not matching any entity_schema pattern
  - **Incomplete Schemas**: Entities with missing/extra attributes vs definition
  - **Raw Event Browser**: View actual data for debugging
  - **Attribute Heatmap**: What attributes are actually present per device_class
- Dynamic: Dashboard queries data dictionary, auto-updates when schemas change
- No dashboard JSON edits required for schema changes

#### 8. Update Procedure Documentation

Update `docs/procedures/` to reflect the new entity_schemas model:

- **HOW_TO_ADD_NEW_STREAM.md**
  - Add section on entity_schemas (required for data dictionary)
  - Document relationship: `fields` (ingestion) vs `entity_schemas` (data dictionary)
  - Include entity_schema YAML examples
  - Update config.yaml examples to show both sections

- **HOW_TO_ADD_NEW_SOURCE.md**
  - Clarify source vs entity_schema distinction
  - Update terminology if needed
  - Add cross-reference to entity_schemas documentation

- Ensure documentation is accurate and complete after all changes
- Documentation should enable future contributors to add streams/schemas correctly

---

### Out of Scope

- Silver layer data aggregation/transformation views
- Continuous aggregates or materialized views
- ML feature engineering tables
- TimescaleDB compression policies
- Production hardening (HA, backups, monitoring)
- Full HomeAssistant integration (MQTT Statestream setup)
- Other HomeAssistant entity schemas beyond AirGradient (initial test only)
- Enterprise-grade sync automation (webhook triggers, etc.)
- Removing `fields` duplication from existing streams (accept duplication for stability)

---

## Success Criteria

1. **DuckDB container removed** from deployment without breaking existing functionality
2. **TimescaleDB operational** on Pi deployment with data dictionary tables
3. **All 6 existing streams** have entity_schemas added to config
4. **Data dictionary queryable** via single normalized interface
5. **HomeAssistant stream config** created with AirGradient entity_schema
6. **Deploy script extended** with data dictionary sync command
7. **Grafana dashboard** displays schema coverage and identifies test "unknown" entities
8. **Procedure docs updated** and accurate for new entity_schemas model

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| Existing Parquet Bronze layer | ✅ Ready | All 6 streams operational |
| Grafana DuckDB plugin | ✅ Ready | Direct Parquet queries working |
| etcd configuration store | ✅ Ready | Stream configs synced |
| AirGradient sensor data | ✅ Ready | Test data source available |
| HomeAssistant research | ✅ Complete | See `product/research/homeassistant/` |

---

## Technical Context

### Current Architecture (Pre-DP-002)

```
Sensors → Sources → Bronze (Parquet) → Grafana (via DuckDB plugin)
                                     ↗
             DuckDB Container (unused)

Config: fields only (no entity_schemas)
```

### Target Architecture (Post-DP-002)

```
Sensors → Sources → Bronze (Parquet) → Grafana (via DuckDB plugin)
                                              ↓
Config (etcd) → Sync Script → TimescaleDB (Data Dictionary)
  - entity_schemas                            ↓
  - (fields unchanged)       Grafana (DQ Dashboard queries dictionary)

Config: fields (ingestion) + entity_schemas (data dictionary)
```

### Streams to Update

| Stream | Current Config | Add Entity Schema |
|--------|----------------|-------------------|
| air-quality | fields ✅ | entity_schemas: airgradient |
| outdoor-weather | fields ✅ | entity_schemas: nws-weather |
| outdoor-air-quality | fields ✅ | entity_schemas: airnow |
| nws-observations | fields ✅ | entity_schemas: nws-observations |
| nws-forecast-hourly | fields ✅ | entity_schemas: nws-hourly |
| nws-gridpoints-forecast | fields ✅ | entity_schemas: nws-gridpoints |
| homeassistant | NEW | entity_schemas: sensor.airgradient_* |

### Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Data dictionary source | entity_schemas (unified) | One query interface for all streams |
| Existing config approach | Add entity_schemas, keep fields | Ingestion stability |
| Bronze schema (HA) | Generic (entity_id, state, attributes JSON) | Capture everything, type in dictionary |
| Silver layer DB | TimescaleDB | Continuous aggregates, compression, ML-ready |
| Dashboard approach | Schema-driven dynamic | No edits for schema changes |

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| TimescaleDB memory on Pi | High | Configure conservative limits, monitor |
| Sync script complexity | Medium | MVP manual trigger, defer automation |
| DuckDB removal breaks queries | Medium | Verify plugin independence first |
| Entity schema definition errors | Low | Validate against existing Bronze data |

---

## References

- [HomeAssistant Integration Research](../../research/homeassistant/README.md)
- [Database Comparison Analysis](../../research/homeassistant/database-comparison.md)
- [MQTT Patterns](../../research/homeassistant/mqtt-patterns.md)
- [Platform Architecture Overview](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

---

## Next Steps

1. **Specification Phase**: Detail technical requirements for each scope item
2. **Architecture Phase**: ADRs for TimescaleDB schema, entity_schema format, sync mechanism
3. **Implementation**: Iterative delivery per scope item

---

*This document defines the boundaries for DP-002. Detailed specifications will follow in the specification phase.*
