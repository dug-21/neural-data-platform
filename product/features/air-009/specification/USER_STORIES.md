# AIR-009: Source Identity and Context Configuration - User Stories

## Document Information

| Field | Value |
|-------|-------|
| Feature | AIR-009 |
| Version | 1.2.0 |
| Status | Draft |
| Last Updated | 2025-12-31 |
| Author | NDP Specification Agent |
| Amendment | ADR-002-AMENDMENT-002 (Simple Blob) |

---

## Overview

This document captures user stories for the source identity and context configuration feature. Stories are organized by persona and include acceptance criteria references.

---

## Personas

| Persona | Description |
|---------|-------------|
| **Data Engineer** | Configures streams, maintains pipelines, queries data |
| **Platform Operator** | Deploys, monitors, and troubleshoots the NDP system |
| **Data Analyst** | Queries data for insights, builds dashboards |
| **Home Automation User** | End user with sensors providing data to NDP |

---

## Epic: Source Identity

### US-001: Stable Source Identification

**As a** Data Engineer,
**I want** each data source to have a stable, unique identifier (`ndp_id`),
**So that** I can track all data from a source regardless of configuration changes.

**Acceptance Criteria:**
- AC-001: ndp_id field defined in configuration
- AC-006: ndp_id synced to etcd
- AC-009: ndp_id in SourceConfig struct

**Priority:** High

**Story Points:** 3

---

### US-002: Query Data by Source

**As a** Data Analyst,
**I want** to query all historical data from a specific source using its ndp_id,
**So that** I can analyze trends for that source over time.

**Acceptance Criteria:**
- AC-013: ndp_id column in TimescaleDB
- AC-015: Index on ndp_id
- AC-016: ETL maps ndp_id correctly

**Example Query:**
```sql
SELECT time, pm25, temperature
FROM sensor_readings
WHERE ndp_id = 'airgradient-office-001'
  AND time > NOW() - INTERVAL '30 days'
ORDER BY time;
```

**Priority:** High

**Story Points:** 5

---

### US-003: Immutable Source Identity

**As a** Platform Operator,
**I want** ndp_id to never change once assigned,
**So that** I can rely on data lineage even when sources are reconfigured.

**Acceptance Criteria:**
- ndp_id format enforced (lowercase alphanumeric with hyphens)
- Documentation states immutability requirement
- Migration tools warn against ndp_id changes

**Priority:** Medium

**Story Points:** 2

---

## Epic: Context Configuration

### US-004: Location Context for Indoor Sensors

**As a** Home Automation User,
**I want** to configure my sensor's location (room, floor, building),
**So that** my data is automatically tagged with where it was recorded.

**Acceptance Criteria:**
- AC-003: Location context schema
- AC-011: Context stored as JSON blob

**Example Configuration:**
```yaml
context:
  location:
    type: indoor
    path: home/upstairs/office
    coordinates: [29.958, -81.308]
```

**Priority:** High

**Story Points:** 3

---

### US-005: Device Metadata in Context

**As a** Data Engineer,
**I want** to include device type and model in the context,
**So that** I can filter and group data by device characteristics.

**Acceptance Criteria:**
- AC-004: Dynamic context keys
- AC-010: Context attached to records

**Example Query:**
```sql
SELECT ndp_id, AVG(pm25)
FROM sensor_readings
WHERE context->>'device_type' = 'airgradient'
GROUP BY ndp_id;
```

**Priority:** Medium

**Story Points:** 3

---

### US-006: Point-in-Time Context Accuracy

**As a** Data Analyst,
**I want** historical records to retain the context values at write time,
**So that** I can see where a sensor was located when it recorded a reading.

**Acceptance Criteria:**
- AC-012: Context written to Bronze layer
- AC-014: Context JSONB in Silver layer

**Scenario:**
1. Sensor configured in "office" room
2. Sensor records data for 30 days
3. Sensor moved to "bedroom" room
4. Sensor records more data
5. Query shows "office" context for first 30 days, "bedroom" for later data

**Priority:** High

**Story Points:** 5

---

### US-007: Optional Context Fields

**As a** Data Engineer,
**I want** all context fields to be optional,
**So that** I can configure only the metadata relevant to each source.

**Acceptance Criteria:**
- AC-002: Context block is optional
- AC-NFR-002: Backward compatibility

**Example:** NWS weather station only needs:
```yaml
context:
  location:
    coordinates: [29.959, -81.339]
    type: outdoor
  station_id: KSGJ
```

**Priority:** Medium

**Story Points:** 2

---

## Epic: Configuration Management

### US-008: etcd Configuration Visibility

**As a** Platform Operator,
**I want** ndp_id and context to be visible in etcd,
**So that** I can verify configuration is correctly synchronized.

**Acceptance Criteria:**
- AC-006: ndp_id synced to etcd
- AC-007: Context synced with proper structure

**Verification Command:**
```bash
etcdctl get --prefix /streams/air-quality/sources/0/
```

**Priority:** Medium

**Story Points:** 3

---

### US-009: Configuration Round-Trip Integrity

**As a** Platform Operator,
**I want** configuration to survive the YAML -> etcd -> app round-trip without data loss,
**So that** I can trust GitOps-based deployments.

**Acceptance Criteria:**
- AC-008: Round-trip configuration integrity

**Priority:** High

**Story Points:** 3

---

### US-010: Update All Existing Streams

**As a** Data Engineer,
**I want** all existing active streams to be updated with ndp_id and context,
**So that** the entire platform uses consistent source identification.

**Acceptance Criteria:**
- AC-005: All active streams updated

**Streams to Update:**
- [ ] air-quality (MQTT)
- [ ] nws-observations (HTTP)
- [ ] nws-forecast-hourly (HTTP)
- [ ] nws-gridpoints-forecast (HTTP)
- [ ] outdoor-air-quality (HTTP)
- [ ] outdoor-weather (HTTP)

**Priority:** High

**Story Points:** 5

---

## Epic: Data Pipeline

### US-011: Context in Bronze Layer

**As a** Data Engineer,
**I want** ndp_id and context written as simple columns to Parquet files,
**So that** the Bronze layer stores complete context as a JSON blob without transformation.

**Acceptance Criteria:**
- AC-012: Bronze layer contains ndp_id and context blob

**Bronze Schema:**
- `ndp_id`: STRING
- `context`: STRING (JSON blob)

**Priority:** High

**Story Points:** 3

---

### US-012: Simple Context Blob Storage

**As a** Data Engineer,
**I want** context stored as a single JSON blob with no flattening or promoted fields,
**So that** the system remains simple and all context queries use JSONB operators.

**Acceptance Criteria:**
- AC-011: Simple context blob storage

**Schema:**
| Layer | Column | Type |
|-------|--------|------|
| Bronze | `context` | STRING (JSON) |
| Silver | `context` | JSONB |

**Example Queries (Silver Layer):**
```sql
-- Query by device type
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- Query by nested location type
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- Query coordinates
SELECT * FROM sensor_readings
WHERE (context->'location'->'coordinates'->>0)::float > 29.0;

-- Full context access
SELECT ndp_id, context FROM sensor_readings;
```

**Priority:** High

**Story Points:** 3

---

### US-013: Context in Silver Layer

**As a** Data Analyst,
**I want** context stored as JSONB in TimescaleDB,
**So that** I can query flexible attributes without schema changes.

**Acceptance Criteria:**
- AC-014: Context JSONB column
- AC-016: ETL reconstructs context

**Example Query:**
```sql
SELECT
  ndp_id,
  context->'location'->>'path' as room,
  AVG(temperature) as avg_temp
FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor'
GROUP BY ndp_id, room;
```

**Priority:** High

**Story Points:** 5

---

## Epic: Performance & Reliability

### US-014: Minimal Latency Impact

**As a** Platform Operator,
**I want** adding ndp_id and context to not significantly impact ingestion latency,
**So that** real-time data processing is not degraded.

**Acceptance Criteria:**
- AC-NFR-001: Latency increase under 5%

**Priority:** Medium

**Story Points:** 3

---

### US-015: Efficient ndp_id Queries

**As a** Data Analyst,
**I want** queries filtering by ndp_id to be fast,
**So that** dashboards and reports load quickly.

**Acceptance Criteria:**
- AC-015: ndp_id index created
- Query time < 100ms for 10M records

**Priority:** High

**Story Points:** 3

---

### US-016: Backward Compatible Configuration

**As a** Platform Operator,
**I want** existing configurations without ndp_id/context to continue working,
**So that** I can upgrade incrementally without breaking deployments.

**Acceptance Criteria:**
- AC-NFR-002: Backward compatibility

**Priority:** High

**Story Points:** 3

---

## Story Map

```
                        Source Identity & Context
                                  |
        +-----------+-------------+-------------+-----------+
        |           |             |             |           |
   Identity    Context      Config Mgmt    Pipeline    Performance
        |           |             |             |           |
    US-001      US-004        US-008        US-011      US-014
    US-002      US-005        US-009        US-012      US-015
    US-003      US-006        US-010        US-013      US-016
                US-007
```

---

## Sprint Planning Recommendations

### Sprint 1: Foundation (13 points)
- US-001: Stable Source Identification (3)
- US-004: Location Context for Indoor Sensors (3)
- US-007: Optional Context Fields (2)
- US-010: Update All Existing Streams (5)

### Sprint 2: Pipeline (11 points)
- US-011: Context in Bronze Layer (3)
- US-012: Simple Context Blob Storage (3)
- US-013: Context in Silver Layer (5)

### Sprint 3: Integration & Performance (14 points)
- US-002: Query Data by Source (5)
- US-008: etcd Configuration Visibility (3)
- US-009: Configuration Round-Trip Integrity (3)
- US-015: Efficient ndp_id Queries (3)

### Sprint 4: Polish (10 points)
- US-005: Device Metadata in Context (3)
- US-006: Point-in-Time Context Accuracy (5)
- US-003: Immutable Source Identity (2)

---

## Dependencies

```
US-001 (ndp_id definition)
   |
   +---> US-002 (query by source)
   +---> US-008 (etcd visibility)
   +---> US-011 (Bronze layer)
            |
            +---> US-013 (Silver layer)
                     |
                     +---> US-015 (efficient queries)

US-004 (location context)
   |
   +---> US-006 (point-in-time)
   +---> US-012 (blob storage)
```

---

## Out of Scope (Future Stories)

These stories are explicitly out of scope for AIR-009:

- **US-F01**: Webhook source type support
- **US-F02**: Context validation/schema enforcement
- **US-F03**: UI for managing source context
- **US-F04**: Migration of existing historical records
