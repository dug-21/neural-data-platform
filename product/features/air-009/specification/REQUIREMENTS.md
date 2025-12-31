# AIR-009: Source Identity and Context Configuration - Requirements

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

## 1. Introduction

### 1.1 Purpose

This document defines the functional and non-functional requirements for implementing source identity (`ndp_id`) and context configuration across the Neural Data Platform (NDP) stack.

### 1.2 Scope

The requirements cover:
- Stream configuration updates for all active streams
- Configuration synchronization to etcd
- Ingestion pipeline modifications
- Data dictionary updates for TimescaleDB Silver layer

### 1.3 References

- SCOPE.md: AIR-009 feature scope definition
- Sample configs: `config/samples/mqtt_stream.yaml`, `config/samples/http_stream.yaml`
- Amazon event envelope pattern (inspiration for context separation)

---

## 2. Functional Requirements

### 2.1 Stream Configuration

#### REQ-001: ndp_id Field Definition

**Description:** Each source in a stream configuration MUST support an `ndp_id` field.

**Details:**
- Field name: `ndp_id`
- Location: Source level (not nested in context)
- Format: Lowercase alphanumeric with hyphens
- Constraints: Required for all sources, immutable once assigned

**Rationale:** Provides a stable identifier for linking all data from a source across time, regardless of metadata changes.

---

#### REQ-002: Context Block Definition

**Description:** Each source in a stream configuration MUST support an optional `context` block.

**Details:**
- Field name: `context`
- Location: Source level, sibling to `ndp_id`
- Structure: Nested YAML object with dynamic keys
- Constraints: All fields within context are optional

**Rationale:** Enables mutable attributes to be denormalized at write time, providing point-in-time accuracy for historical records.

---

#### REQ-003: Location Context Schema

**Description:** The `context` block MUST support a standardized `location` sub-schema.

**Details:**
```yaml
location:
  coordinates: [latitude, longitude]  # Tuple, not flattened
  type: indoor | outdoor              # Optional enum
  path: hierarchical/path/string      # Optional, flexible depth
```

**Constraints:**
- `coordinates` MUST remain as tuple/array (not flattened)
- `path` uses forward slashes for hierarchy

**Rationale:** Standardizes location representation while preserving geospatial query capability.

---

#### REQ-004: Dynamic Context Keys

**Description:** The `context` block MUST support arbitrary domain-specific keys beyond the standardized location schema.

**Details:**
- Keys can be any valid YAML identifier
- Values can be strings, numbers, arrays, or nested objects
- Examples: `device_type`, `model`, `tags`, `station_id`

**Rationale:** Provides flexibility for different stream types without requiring schema changes.

---

#### REQ-005: Stream Configuration Updates

**Description:** All active streams MUST be updated to include `ndp_id` and `context` fields.

**Affected Streams:**
| Stream ID | Source Type |
|-----------|-------------|
| `air-quality` | MQTT |
| `nws-observations` | HTTP |
| `nws-forecast-hourly` | HTTP |
| `nws-gridpoints-forecast` | HTTP |
| `outdoor-air-quality` | HTTP |
| `outdoor-weather` | HTTP |

**Rationale:** Ensures consistent source identity across all data pipelines.

---

### 2.2 Configuration Synchronization

#### REQ-006: etcd Sync for ndp_id

**Description:** The ConfigSyncService MUST synchronize `ndp_id` fields to etcd.

**Expected Key Pattern:**
```
/streams/{stream_id}/sources/{index}/ndp_id
```

**Rationale:** Enables runtime configuration access and service discovery.

---

#### REQ-007: etcd Sync for Context

**Description:** The ConfigSyncService MUST synchronize nested `context` fields to etcd with proper key structure.

**Expected Key Patterns:**
```
/streams/{stream_id}/sources/{index}/context/location/coordinates
/streams/{stream_id}/sources/{index}/context/location/type
/streams/{stream_id}/sources/{index}/context/location/path
/streams/{stream_id}/sources/{index}/context/{dynamic_key}
```

**Rationale:** Preserves nested structure in etcd for structured access.

---

#### REQ-008: Round-Trip Configuration Integrity

**Description:** Configuration MUST maintain integrity through the full sync cycle: YAML -> etcd -> application config.

**Validation:**
- Values read from etcd MUST match original YAML values
- Nested structures MUST be correctly reconstructed
- Array values (e.g., coordinates) MUST preserve order

**Rationale:** Ensures configuration consistency across deployment methods.

---

### 2.3 Ingestion Pipeline

#### REQ-009: SourceConfig Struct Update

**Description:** The Rust `SourceConfig` struct MUST be updated to include `ndp_id` and `context` fields.

**Details:**
```rust
pub struct SourceConfig {
    // ... existing fields
    pub ndp_id: String,
    pub context: Option<HashMap<String, serde_json::Value>>,
}
```

**Rationale:** Enables parsed configuration to include identity and context data.

---

#### REQ-010: Context Attachment to Records

**Description:** Parsers MUST attach `ndp_id` and `context` JSON blob to every parsed record.

**Details:**
- `ndp_id` added as top-level field
- `context` serialized as JSON string and attached to record

**Rationale:** Ensures context travels with every data point as a simple blob. No flattening, no promoted fields - just the raw JSON.

---

#### REQ-011: Simple Context Blob Storage

**Description:** The ingestion pipeline MUST store context as a single JSON blob. No flattening. No promoted fields.

**Schema:**
| Column | Type | Description |
|--------|------|-------------|
| `ndp_id` | STRING | Stable source identifier |
| `context` | STRING (JSON) | Complete context as JSON blob |

**Example Output:**
```json
{
  "ndp_id": "airgradient-office-001",
  "context": "{\"location\":{\"coordinates\":[29.958,-81.308],\"type\":\"indoor\",\"path\":\"home/upstairs/office\"},\"device_type\":\"airgradient\",\"model\":\"ONE-V9\",\"tags\":[\"primary\",\"calibrated\"]}"
}
```

**Rationale:** Maximum simplicity - one column, one format. Query context using JSONB operators in Silver layer. See ADR-002-AMENDMENT-002.

---

#### REQ-012: Bronze Layer Writer Update

**Description:** The Bronze layer writer MUST include `ndp_id` and `context` columns in Parquet files.

**Details:**
- `ndp_id` as STRING column
- `context` as STRING column containing complete JSON blob

**Rationale:** Bronze layer stores context as-is. No transformation, no flattening.

---

### 2.4 Data Dictionary (Silver Layer)

#### REQ-013: ndp_id Column in TimescaleDB

**Description:** TimescaleDB tables MUST include an `ndp_id` column.

**Details:**
```sql
ALTER TABLE sensor_readings ADD COLUMN ndp_id TEXT NOT NULL;
```

**Rationale:** Enables efficient queries by source identity.

---

#### REQ-014: Context Storage in TimescaleDB

**Description:** TimescaleDB tables MUST support context storage using JSONB.

**Details:**
```sql
ALTER TABLE sensor_readings ADD COLUMN context JSONB;
```

**Rationale:** JSONB provides flexible, queryable storage for dynamic context fields.

---

#### REQ-015: ndp_id Index Creation

**Description:** An index MUST be created on the `ndp_id` column for efficient querying.

**Details:**
```sql
CREATE INDEX idx_readings_ndp_id ON sensor_readings(ndp_id);
```

**Rationale:** Ensures performant queries filtering by source identity.

---

#### REQ-016: ETL Pipeline Update

**Description:** The Bronze -> Silver ETL pipeline MUST map `ndp_id` and parse `context` JSON to JSONB.

**Details:**
- Extract `ndp_id` from Bronze records (direct copy)
- Parse `context` JSON string to JSONB for the Silver `context` column
- Create GIN index on `context` JSONB column for efficient queries
- Handle missing optional context gracefully (null)

**Query Examples:**
```sql
-- Query by nested location type
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- Query by device type
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- Query coordinates
SELECT * FROM sensor_readings
WHERE (context->'location'->'coordinates'->>0)::float > 29.0;
```

**Rationale:** Simple JSON blob in Bronze becomes JSONB in Silver. All context queries use JSONB operators. See ADR-002-AMENDMENT-002.

---

## 3. Non-Functional Requirements

### 3.1 Performance

#### NFR-001: Ingestion Latency

**Description:** Adding `ndp_id` and context to records MUST NOT increase ingestion latency by more than 5%.

**Measurement:** p95 latency for record processing.

**Rationale:** Maintains system performance while adding new functionality.

---

#### NFR-002: Query Performance

**Description:** Queries filtering by `ndp_id` MUST execute in less than 100ms for datasets up to 10 million records.

**Measurement:** Query execution time with index utilization.

**Rationale:** Ensures practical usability for dashboards and analytics.

---

### 3.2 Compatibility

#### NFR-003: Backward Compatibility

**Description:** Existing stream configurations without `ndp_id` and `context` MUST continue to function.

**Details:**
- `ndp_id` defaults to stream_id + source index if not specified
- `context` defaults to empty object if not specified
- No breaking changes to existing deployments

**Rationale:** Enables incremental adoption without disruption.

---

#### NFR-004: API Compatibility

**Description:** Existing APIs MUST remain functional; new fields are additive only.

**Rationale:** Prevents breaking integrations during upgrade.

---

### 3.3 Maintainability

#### NFR-005: Configuration Documentation

**Description:** Sample configurations MUST be updated to document new fields.

**Locations:**
- `config/samples/mqtt_stream.yaml`
- `config/samples/http_stream.yaml`

**Rationale:** Ensures developers can correctly configure new streams.

---

#### NFR-006: Schema Documentation

**Description:** Data dictionary MUST document `ndp_id` and context column semantics.

**Rationale:** Enables data consumers to understand and use new fields.

---

### 3.4 Security

#### NFR-007: No Sensitive Data in Context

**Description:** Context fields MUST NOT contain sensitive or personally identifiable information.

**Guidance:**
- No API keys or credentials
- No PII (names, addresses, etc.)
- Location coordinates are acceptable (not PII for sensors)

**Rationale:** Prevents security/privacy issues from denormalized data.

---

## 4. Constraints

### 4.1 Technical Constraints

| ID | Constraint |
|----|------------|
| CON-001 | Must use existing PostgreSQL/TimescaleDB infrastructure |
| CON-002 | Must be compatible with Rust 2021 edition |
| CON-003 | Must deploy to existing Docker/Pi infrastructure |
| CON-004 | Context stored as JSON blob - no flattening, no promoted fields |

### 4.2 Business Constraints

| ID | Constraint |
|----|------------|
| CON-005 | No migration of existing records (new records only) |
| CON-006 | Must not require UI changes (config-driven) |

---

## 5. Assumptions

| ID | Assumption |
|----|------------|
| ASM-001 | ConfigSyncService can handle arbitrary nested YAML structures |
| ASM-002 | etcd key structure supports nested path representation |
| ASM-003 | Parquet schema can accommodate dynamic columns |
| ASM-004 | TimescaleDB JSONB provides sufficient query performance |

---

## 6. Dependencies

| ID | Dependency | Status |
|----|------------|--------|
| DEP-001 | DP-003: MQTT Multi-Subscription Support | Complete |
| DEP-002 | Existing stream configurations | Available |
| DEP-003 | ConfigSyncService | Operational |
| DEP-004 | Bronze/Silver layer infrastructure | Operational |

---

## 7. Traceability Matrix

| Requirement | Scope Section | Acceptance Criteria |
|-------------|---------------|---------------------|
| REQ-001 | 1. Stream Config | AC-001 |
| REQ-002 | 1. Stream Config | AC-002 |
| REQ-003 | 1. Stream Config | AC-003 |
| REQ-004 | 1. Stream Config | AC-004 |
| REQ-005 | 1. Stream Config | AC-005 |
| REQ-006 | 2. Config Sync | AC-006 |
| REQ-007 | 2. Config Sync | AC-007 |
| REQ-008 | 2. Config Sync | AC-008 |
| REQ-009 | 3. Ingestion | AC-009 |
| REQ-010 | 3. Ingestion | AC-010 |
| REQ-011 | 3. Ingestion | AC-011 |
| REQ-012 | 3. Ingestion | AC-012 |
| REQ-013 | 4. Data Dictionary | AC-013 |
| REQ-014 | 4. Data Dictionary | AC-014 |
| REQ-015 | 4. Data Dictionary | AC-015 |
| REQ-016 | 4. Data Dictionary | AC-016 |
