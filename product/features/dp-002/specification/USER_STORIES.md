# DP-002: User Stories

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2025-12-30
**Status**: Draft

---

## Overview

This document defines user stories for DP-002: Online Data Dictionary & HomeAssistant Stream Preparation. Stories are organized by persona and include acceptance criteria for each.

---

## Personas

### Platform Operator

**Name**: Alex (Platform Operator)
**Role**: Deploys and maintains the Neural Data Platform on Raspberry Pi
**Goals**: Keep the system running, deploy updates, ensure data integrity
**Pain Points**: Complex deployment steps, unclear state after updates, resource constraints

### Data Analyst

**Name**: Dana (Data Analyst)
**Role**: Queries data for insights, builds dashboards, identifies patterns
**Goals**: Understand available data, write accurate queries, explore correlations
**Pain Points**: Undocumented schemas, inconsistent field naming, missing metadata

### Dashboard Viewer

**Name**: Casey (Dashboard Viewer)
**Role**: Views Grafana dashboards to monitor home environment
**Goals**: See data quality status, identify issues, understand coverage
**Pain Points**: Unclear data sources, missing data without explanation, stale dashboards

### Future Developer

**Name**: Morgan (Future Developer)
**Role**: Adds new streams, sources, and schemas to the platform
**Goals**: Follow patterns, avoid breaking changes, understand conventions
**Pain Points**: Incomplete documentation, inconsistent examples, unclear relationships

---

## User Stories by Persona

### Platform Operator Stories

#### US-O1: Remove Unused Service

**As a** platform operator
**I want to** remove the unused DuckDB container from the deployment
**So that** I can reduce resource usage and simplify the stack

**Acceptance Criteria:**
- [ ] DuckDB service removed from docker-compose.yml
- [ ] DuckDB volume removed from volumes section
- [ ] Grafana continues to query Parquet files successfully
- [ ] No error logs related to DuckDB after removal
- [ ] Memory usage reduced by ~512MB after removal

**Story Points**: 3
**Priority**: High
**Related Requirements**: REQ-1.1, REQ-1.2, REQ-1.3, REQ-1.4

---

#### US-O2: Deploy TimescaleDB

**As a** platform operator
**I want to** deploy TimescaleDB as part of the standard stack
**So that** I have a Silver Layer database for analytics and data dictionary

**Acceptance Criteria:**
- [ ] TimescaleDB service added to docker-compose.yml
- [ ] Service starts successfully with `docker compose up -d`
- [ ] Health check passes within 60 seconds
- [ ] Data persists across container restarts
- [ ] Memory usage stays under 512MB limit
- [ ] PostgreSQL port 5432 accessible from Grafana container

**Story Points**: 5
**Priority**: High
**Related Requirements**: REQ-2.1 through REQ-2.6

---

#### US-O3: Sync Data Dictionary

**As a** platform operator
**I want to** synchronize stream configurations to the data dictionary with a single command
**So that** the dictionary stays current without manual database updates

**Acceptance Criteria:**
- [ ] `./deploy.sh sync-dictionary` command available
- [ ] Command reads all stream configs from etcd
- [ ] Command upserts entity_schemas to TimescaleDB
- [ ] Command reports number of schemas added/updated/deleted
- [ ] Running command twice produces no changes (idempotent)
- [ ] Invalid schemas logged as warnings, not errors

**Story Points**: 8
**Priority**: High
**Related Requirements**: REQ-6.1 through REQ-6.6

---

#### US-O4: Monitor Stack Health

**As a** platform operator
**I want to** see data dictionary health in Grafana
**So that** I know if schema definitions are complete and current

**Acceptance Criteria:**
- [ ] Dashboard accessible at /d/homeassistant-dq
- [ ] Shows total streams with entity_schemas
- [ ] Shows total attributes in dictionary
- [ ] Auto-refreshes without manual intervention
- [ ] Loads within 5 seconds

**Story Points**: 5
**Priority**: Medium
**Related Requirements**: REQ-7.1, REQ-7.2

---

### Data Analyst Stories

#### US-A1: Query Data Dictionary

**As a** data analyst
**I want to** query the data dictionary to understand available data
**So that** I can write accurate queries and explore correlations

**Acceptance Criteria:**
- [ ] Single SQL query returns all attributes across all streams
- [ ] Results include stream_id, schema_name, attribute, type, unit, description
- [ ] Query completes in < 100ms
- [ ] Can filter by stream_id or schema_name
- [ ] Can search by attribute name pattern

**Example Query:**
```sql
SELECT * FROM v_data_dictionary WHERE stream_id = 'air-quality';
```

**Story Points**: 5
**Priority**: High
**Related Requirements**: REQ-4.1 through REQ-4.4

---

#### US-A2: Find Schemas by Device Class

**As a** data analyst
**I want to** find all schemas for a specific device class (e.g., air_quality)
**So that** I can understand related sensors across streams

**Acceptance Criteria:**
- [ ] Can query by device_class field
- [ ] HomeAssistant schemas include device_class
- [ ] Results show all attributes for matching schemas
- [ ] Works for device_class values: air_quality, temperature, humidity, window, door

**Example Query:**
```sql
SELECT * FROM v_data_dictionary WHERE device_class = 'air_quality';
```

**Story Points**: 3
**Priority**: Medium
**Related Requirements**: REQ-4.4, REQ-4.5

---

#### US-A3: Match HomeAssistant Entities

**As a** data analyst
**I want to** match actual HomeAssistant entity IDs to schema patterns
**So that** I can validate incoming data against expected attributes

**Acceptance Criteria:**
- [ ] Schema names support wildcard patterns (e.g., `sensor.airgradient_*`)
- [ ] Can determine which schema applies to `sensor.airgradient_abc123_pm25`
- [ ] Function or view supports pattern matching
- [ ] Returns expected attributes for matched schema

**Story Points**: 5
**Priority**: Medium
**Related Requirements**: REQ-4.5, REQ-5.3

---

#### US-A4: Understand Stream Schema

**As a** data analyst
**I want to** see all attributes defined for a specific stream
**So that** I know what fields are available for analysis

**Acceptance Criteria:**
- [ ] Query returns all schemas for a stream
- [ ] Each attribute includes type, unit, and description
- [ ] Shows which attributes are nullable
- [ ] Results ordered by schema_name, then attribute_name

**Example Query:**
```sql
SELECT schema_name, attribute_name, attribute_type, unit, description
FROM v_data_dictionary
WHERE stream_id = 'nws-observations'
ORDER BY schema_name, attribute_name;
```

**Story Points**: 3
**Priority**: High
**Related Requirements**: REQ-4.3, REQ-4.4

---

### Dashboard Viewer Stories

#### US-V1: View Schema Coverage

**As a** dashboard viewer
**I want to** see overall schema coverage at a glance
**So that** I know if data is properly documented

**Acceptance Criteria:**
- [ ] Panel shows percentage of streams with entity_schemas
- [ ] Panel shows total schemas across all streams
- [ ] Panel shows total attributes in dictionary
- [ ] Visual indicator (green/yellow/red) based on coverage level
- [ ] Updates automatically when schemas change

**Story Points**: 3
**Priority**: Medium
**Related Requirements**: REQ-7.2

---

#### US-V2: Identify Unknown Entities

**As a** dashboard viewer
**I want to** see entities that don't match any defined schema
**So that** I can request schema definitions for new devices

**Acceptance Criteria:**
- [ ] Panel lists entity IDs not matching known patterns
- [ ] Shows entity domain (sensor, binary_sensor, etc.)
- [ ] Shows last seen timestamp
- [ ] Sortable by entity_id or last_seen
- [ ] Empty state shows "All entities have matching schemas"

**Story Points**: 5
**Priority**: Medium
**Related Requirements**: REQ-7.3

---

#### US-V3: Review Incomplete Schemas

**As a** dashboard viewer
**I want to** see schemas with missing or extra attributes
**So that** I can identify documentation gaps

**Acceptance Criteria:**
- [ ] Panel shows schemas where actual data differs from definition
- [ ] Lists missing attributes (defined but not seen)
- [ ] Lists extra attributes (seen but not defined)
- [ ] Shows deviation count per schema
- [ ] Clickable to show full comparison

**Story Points**: 8
**Priority**: Medium
**Related Requirements**: REQ-7.4

---

#### US-V4: Browse Raw Events

**As a** dashboard viewer
**I want to** browse recent raw events for debugging
**So that** I can see actual data structure for troubleshooting

**Acceptance Criteria:**
- [ ] Panel shows recent HomeAssistant events
- [ ] Displays entity_id, state, and attributes JSON
- [ ] Filterable by entity pattern
- [ ] Shows last 100 events by default
- [ ] Timestamps in local timezone

**Story Points**: 5
**Priority**: Low
**Related Requirements**: REQ-7.1

---

### Future Developer Stories

#### US-D1: Add New Stream with Schema

**As a** future developer
**I want to** add a new stream with proper entity_schema documentation
**So that** my stream is included in the data dictionary

**Acceptance Criteria:**
- [ ] Documentation explains when to add entity_schemas
- [ ] Entity schema YAML format is fully specified
- [ ] At least 3 complete examples provided
- [ ] Validation errors are clear and actionable
- [ ] Can verify schema in dictionary after sync

**Story Points**: 5
**Priority**: High
**Related Requirements**: REQ-8.1, REQ-8.3

---

#### US-D2: Understand Fields vs Entity Schemas

**As a** future developer
**I want to** understand the relationship between `fields` and `entity_schemas`
**So that** I know when to use each and avoid duplication issues

**Acceptance Criteria:**
- [ ] Documentation explains purpose of each section
- [ ] Clear guidance on when duplication is acceptable
- [ ] Examples show both sections in same config
- [ ] Explains why existing `fields` shouldn't be modified

**Story Points**: 3
**Priority**: High
**Related Requirements**: REQ-3.1, REQ-3.8, REQ-8.1

---

#### US-D3: Add HomeAssistant Entity Schema

**As a** future developer
**I want to** add an entity_schema for a new HomeAssistant device type
**So that** the data dictionary includes my device's attributes

**Acceptance Criteria:**
- [ ] Pattern matching syntax is documented
- [ ] device_class values are listed
- [ ] Example shows complete HomeAssistant schema
- [ ] Explains relationship to MQTT Statestream topics
- [ ] Can verify with DQ dashboard after sync

**Story Points**: 5
**Priority**: Medium
**Related Requirements**: REQ-5.3, REQ-8.3

---

#### US-D4: Follow Stream Configuration Patterns

**As a** future developer
**I want to** see consistent patterns across all existing stream configs
**So that** I can follow established conventions

**Acceptance Criteria:**
- [ ] All 6 existing streams have entity_schemas
- [ ] Entity schema format is consistent across streams
- [ ] Attribute naming follows snake_case convention
- [ ] Unit formats are standardized
- [ ] Documentation references actual config files

**Story Points**: 5
**Priority**: High
**Related Requirements**: REQ-3.2 through REQ-3.7

---

## Story Map

### Epic 1: Infrastructure Changes

| Priority | Story | Points |
|----------|-------|--------|
| High | US-O1: Remove Unused Service | 3 |
| High | US-O2: Deploy TimescaleDB | 5 |
| | **Total** | **8** |

### Epic 2: Data Dictionary Foundation

| Priority | Story | Points |
|----------|-------|--------|
| High | US-A1: Query Data Dictionary | 5 |
| High | US-A4: Understand Stream Schema | 3 |
| Medium | US-A2: Find Schemas by Device Class | 3 |
| Medium | US-A3: Match HomeAssistant Entities | 5 |
| | **Total** | **16** |

### Epic 3: Configuration & Sync

| Priority | Story | Points |
|----------|-------|--------|
| High | US-O3: Sync Data Dictionary | 8 |
| High | US-D1: Add New Stream with Schema | 5 |
| High | US-D2: Understand Fields vs Entity Schemas | 3 |
| High | US-D4: Follow Stream Configuration Patterns | 5 |
| Medium | US-D3: Add HomeAssistant Entity Schema | 5 |
| | **Total** | **26** |

### Epic 4: Observability

| Priority | Story | Points |
|----------|-------|--------|
| Medium | US-O4: Monitor Stack Health | 5 |
| Medium | US-V1: View Schema Coverage | 3 |
| Medium | US-V2: Identify Unknown Entities | 5 |
| Medium | US-V3: Review Incomplete Schemas | 8 |
| Low | US-V4: Browse Raw Events | 5 |
| | **Total** | **26** |

---

## Story Dependencies

```
US-O1 (Remove DuckDB) ─────────────────────────────────────────────────────────┐
                                                                                │
US-O2 (Deploy TimescaleDB) ───┬───> US-O3 (Sync Dictionary) ───> US-O4 (Health)│
                              │                                                 │
                              └───> US-A1 (Query Dictionary) ───> US-A2, US-A3 │
                                                                                │
US-D4 (Config Patterns) ───┬───> US-D1 (Add Stream with Schema)                 │
                           │                                                    │
                           └───> US-D2 (Fields vs Entity Schemas)               │
                                        │                                       │
                                        └───> US-D3 (Add HA Entity Schema)      │
                                                                                │
US-A1 (Query Dictionary) ───> US-V1 (Coverage) ───> US-V2 (Unknown Entities)   │
                                                                                │
US-V2 (Unknown Entities) ───> US-V3 (Incomplete Schemas)                       │
```

---

## Velocity Assumptions

- **Sprint Duration**: 2 weeks
- **Estimated Velocity**: 20 story points per sprint
- **Total Story Points**: 76

**Estimated Completion**: 4 sprints (8 weeks)

---

## MoSCoW Prioritization

### Must Have (Sprint 1-2)
- US-O1: Remove Unused Service
- US-O2: Deploy TimescaleDB
- US-A1: Query Data Dictionary
- US-O3: Sync Data Dictionary
- US-D4: Follow Stream Configuration Patterns

### Should Have (Sprint 2-3)
- US-A4: Understand Stream Schema
- US-D1: Add New Stream with Schema
- US-D2: Understand Fields vs Entity Schemas
- US-O4: Monitor Stack Health
- US-V1: View Schema Coverage

### Could Have (Sprint 3-4)
- US-A2: Find Schemas by Device Class
- US-A3: Match HomeAssistant Entities
- US-D3: Add HomeAssistant Entity Schema
- US-V2: Identify Unknown Entities
- US-V3: Review Incomplete Schemas

### Won't Have (This Release)
- US-V4: Browse Raw Events (deferred to DP-003)

---

*This document is part of the SPARC Specification phase for DP-002.*
