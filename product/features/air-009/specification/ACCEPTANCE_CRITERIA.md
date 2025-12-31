# AIR-009: Source Identity and Context Configuration - Acceptance Criteria

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

This document defines testable acceptance criteria for each requirement in AIR-009. Each criterion follows the Given/When/Then format for clarity and testability.

---

## 1. Stream Configuration Acceptance Criteria

### AC-001: ndp_id Field in MQTT Stream Configuration

**Requirement:** REQ-001

```gherkin
Scenario: ndp_id field is defined in MQTT source configuration
  Given a stream configuration file for "air-quality"
  When the configuration is loaded
  Then the source MUST contain an "ndp_id" field
  And the "ndp_id" value MUST be a non-empty string
  And the "ndp_id" value MUST match pattern "^[a-z0-9-]+$"
```

```gherkin
Scenario: ndp_id is at source level, not nested
  Given a stream configuration with sources
  When parsing the YAML structure
  Then "ndp_id" MUST be a direct child of the source object
  And "ndp_id" MUST NOT be inside the "context" block
```

---

### AC-002: Context Block in Source Configuration

**Requirement:** REQ-002

```gherkin
Scenario: Context block is optional but supported
  Given a stream configuration with a source
  When the source includes a "context" block
  Then the configuration MUST parse successfully
  And the context object MUST be accessible

Scenario: Source without context block is valid
  Given a stream configuration with a source
  When the source does NOT include a "context" block
  Then the configuration MUST parse successfully
  And the context MUST default to an empty object
```

---

### AC-003: Location Context Schema Validation

**Requirement:** REQ-003

```gherkin
Scenario: Location coordinates are preserved as tuple
  Given a context with location.coordinates: [29.958, -81.308]
  When the context is processed
  Then location.coordinates MUST remain as a 2-element array
  And the first element MUST be the latitude (29.958)
  And the second element MUST be the longitude (-81.308)

Scenario: Location type is validated as enum
  Given a context with location.type: "indoor"
  When the context is processed
  Then location.type MUST be accepted

  Given a context with location.type: "outdoor"
  When the context is processed
  Then location.type MUST be accepted

Scenario: Location path supports hierarchical structure
  Given a context with location.path: "home/upstairs/office"
  When the context is processed
  Then location.path MUST be stored as "home/upstairs/office"
  And the path MUST support arbitrary depth
```

---

### AC-004: Dynamic Context Keys

**Requirement:** REQ-004

```gherkin
Scenario: Arbitrary string keys are accepted
  Given a context with custom field "device_type": "airgradient"
  When the context is processed
  Then the field MUST be accessible as context.device_type
  And the value MUST be "airgradient"

Scenario: Tags array is preserved
  Given a context with tags: ["primary", "calibrated"]
  When the context is processed
  Then tags MUST be an array with 2 elements
  And the values MUST be "primary" and "calibrated"

Scenario: Nested custom objects are supported
  Given a context with custom.nested.field: "value"
  When the context is processed
  Then the nested structure MUST be preserved
```

---

### AC-005: All Active Streams Updated

**Requirement:** REQ-005

```gherkin
Scenario: air-quality stream has ndp_id and context
  Given the stream configuration at "config/base/streams/air-quality/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field
  And each source MAY have a "context" block

Scenario: nws-observations stream has ndp_id and context
  Given the stream configuration at "config/base/streams/nws-observations/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field

Scenario: nws-forecast-hourly stream has ndp_id and context
  Given the stream configuration at "config/base/streams/nws-forecast-hourly/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field

Scenario: nws-gridpoints-forecast stream has ndp_id and context
  Given the stream configuration at "config/base/streams/nws-gridpoints-forecast/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field

Scenario: outdoor-air-quality stream has ndp_id and context
  Given the stream configuration at "config/base/streams/outdoor-air-quality/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field

Scenario: outdoor-weather stream has ndp_id and context
  Given the stream configuration at "config/base/streams/outdoor-weather/config.yaml"
  When the configuration is inspected
  Then each source MUST have an "ndp_id" field
```

---

## 2. Configuration Synchronization Acceptance Criteria

### AC-006: ndp_id Synced to etcd

**Requirement:** REQ-006

```gherkin
Scenario: ndp_id is visible in etcd after sync
  Given a stream configuration with ndp_id: "airgradient-office-001"
  When the ConfigSyncService synchronizes to etcd
  Then the key "/streams/air-quality/sources/0/ndp_id" MUST exist
  And the value MUST be "airgradient-office-001"
```

---

### AC-007: Context Synced to etcd with Proper Structure

**Requirement:** REQ-007

```gherkin
Scenario: Location coordinates synced to etcd
  Given a context with location.coordinates: [29.958, -81.308]
  When the ConfigSyncService synchronizes to etcd
  Then the key "/streams/{stream}/sources/0/context/location/coordinates" MUST exist
  And the value MUST be "[29.958, -81.308]" or equivalent JSON

Scenario: Location type synced to etcd
  Given a context with location.type: "indoor"
  When the ConfigSyncService synchronizes to etcd
  Then the key "/streams/{stream}/sources/0/context/location/type" MUST exist
  And the value MUST be "indoor"

Scenario: Dynamic context keys synced to etcd
  Given a context with device_type: "airgradient"
  When the ConfigSyncService synchronizes to etcd
  Then the key "/streams/{stream}/sources/0/context/device_type" MUST exist
  And the value MUST be "airgradient"
```

---

### AC-008: Round-Trip Configuration Integrity

**Requirement:** REQ-008

```gherkin
Scenario: Configuration survives round-trip through etcd
  Given an original YAML configuration with:
    | Field | Value |
    | ndp_id | airgradient-office-001 |
    | context.location.coordinates | [29.958, -81.308] |
    | context.location.type | indoor |
    | context.device_type | airgradient |
  When the configuration is synced to etcd
  And the application reads configuration from etcd
  Then all values MUST match the original YAML
  And coordinates array order MUST be preserved
  And nested structure MUST be correctly reconstructed
```

---

## 3. Ingestion Pipeline Acceptance Criteria

### AC-009: SourceConfig Struct Contains New Fields

**Requirement:** REQ-009

```gherkin
Scenario: SourceConfig struct includes ndp_id
  Given the Rust SourceConfig struct definition
  When the struct is inspected
  Then it MUST contain a field "ndp_id" of type String

Scenario: SourceConfig struct includes context
  Given the Rust SourceConfig struct definition
  When the struct is inspected
  Then it MUST contain a field "context" of type Option<HashMap<String, Value>>
```

---

### AC-010: Parser Attaches Context to Records

**Requirement:** REQ-010

```gherkin
Scenario: ndp_id attached to parsed MQTT record
  Given an MQTT message from source with ndp_id: "airgradient-office-001"
  When the message is parsed
  Then the output record MUST contain field "ndp_id"
  And the value MUST be "airgradient-office-001"

Scenario: Context blob attached to parsed HTTP record
  Given an HTTP response from source with context.device_type: "nws-station"
  When the response is parsed
  Then the output record MUST contain "context" field as JSON blob
```

---

### AC-011: Simple Context Blob Storage

**Requirement:** REQ-011

```gherkin
Scenario: Context stored as JSON blob
  Given a context with:
    | Field | Value |
    | location.type | indoor |
    | location.path | home/upstairs/office |
    | location.coordinates | [29.958, -81.308] |
    | device_type | airgradient |
    | tags | ["primary", "calibrated"] |
  When the context is processed
  Then output MUST contain column "context"
  And "context" MUST be a valid JSON string
  And "context" MUST contain all original fields with nested structure preserved

Scenario: No flattening or promoted fields
  Given a context with location.type: "indoor"
  When the context is processed
  Then output MUST NOT contain column "ctx_location_type"
  And output MUST NOT contain column "ctx_location_path"
  And output MUST NOT contain column "ctx_location_coordinates"
  And output MUST contain column "context" with the complete JSON blob

Scenario: Empty context is valid
  Given a source without a context block
  When the record is processed
  Then "context" column MUST be null or empty JSON object
  And processing MUST NOT fail

Scenario: Nested structure preserved in blob
  Given a context with:
    | Field | Value |
    | location.coordinates | [29.958, -81.308] |
    | calibration.sensor_a.offset | 0.5 |
  When the context is processed
  Then "context" MUST preserve the nested structure
  And JSON path "$.location.coordinates[0]" MUST equal 29.958
  And JSON path "$.calibration.sensor_a.offset" MUST equal 0.5
```

---

### AC-012: Bronze Layer Contains ndp_id and Context Blob

**Requirement:** REQ-012

```gherkin
Scenario: Parquet file includes ndp_id column
  Given ingested records with ndp_id values
  When the Bronze layer writer creates a Parquet file
  Then the schema MUST include column "ndp_id"
  And the column type MUST be string/utf8

Scenario: Parquet file includes context column as JSON string
  Given ingested records with context
  When the Bronze layer writer creates a Parquet file
  Then the schema MUST include column "context"
  And the column type MUST be string/utf8
  And values MUST be valid JSON strings

Scenario: No promoted context columns in schema
  Given ingested records with context containing location fields
  When the Bronze layer writer creates a Parquet file
  Then the schema MUST NOT include column "ctx_location_type"
  And the schema MUST NOT include column "ctx_location_path"
  And the schema MUST NOT include column "ctx_location_coordinates"
  And the schema MUST NOT include column "context_raw"

Scenario: Context blob contains complete original structure
  Given ingested record with context containing device_type, model, tags, and location
  When inspecting the Bronze Parquet file
  Then "context" MUST contain all original fields
  And parsing "context" as JSON MUST reproduce the original nested structure
```

---

## 4. Data Dictionary (Silver Layer) Acceptance Criteria

### AC-013: ndp_id Column Exists in TimescaleDB

**Requirement:** REQ-013

```gherkin
Scenario: ndp_id column added to sensor_readings table
  Given the TimescaleDB sensor_readings table
  When the schema is queried
  Then column "ndp_id" MUST exist
  And the column type MUST be TEXT
  And the column MUST be NOT NULL
```

---

### AC-014: Context JSONB Column Exists

**Requirement:** REQ-014

```gherkin
Scenario: context column added to sensor_readings table
  Given the TimescaleDB sensor_readings table
  When the schema is queried
  Then column "context" MUST exist
  And the column type MUST be JSONB
```

---

### AC-015: ndp_id Index Created

**Requirement:** REQ-015

```gherkin
Scenario: Index exists on ndp_id column
  Given the TimescaleDB sensor_readings table
  When indexes are queried
  Then an index on "ndp_id" MUST exist
  And the index name SHOULD be "idx_readings_ndp_id"
```

---

### AC-016: ETL Maps Context Blob to JSONB

**Requirement:** REQ-016

```gherkin
Scenario: ETL populates ndp_id in Silver layer
  Given a Bronze layer record with ndp_id: "airgradient-office-001"
  When ETL transforms to Silver layer
  Then the Silver record MUST have ndp_id = "airgradient-office-001"

Scenario: ETL parses context JSON to JSONB
  Given a Bronze layer record with context JSON containing:
    | Field | Value |
    | location.type | indoor |
    | location.path | home/upstairs/office |
    | device_type | airgradient |
    | tags | ["primary", "calibrated"] |
  When ETL transforms to Silver layer
  Then the Silver record context MUST be valid JSONB
  And context->'location'->>'type' MUST be "indoor"
  And context->'location'->>'path' MUST be "home/upstairs/office"
  And context->>'device_type' MUST be "airgradient"
  And context->'tags' MUST be a JSON array

Scenario: Query context fields via JSONB operators
  Given Silver layer records with context containing device_type
  When executing: SELECT * FROM sensor_readings WHERE context->>'device_type' = 'airgradient'
  Then only records with that device_type MUST be returned
  And query MUST use GIN index on context column

Scenario: Query nested context fields via JSONB
  Given Silver layer records with context containing location.type
  When executing: SELECT * FROM sensor_readings WHERE context->'location'->>'type' = 'indoor'
  Then only indoor records MUST be returned

Scenario: Query coordinates via JSONB array access
  Given Silver layer records with context containing location.coordinates
  When executing: SELECT * FROM sensor_readings WHERE (context->'location'->'coordinates'->>0)::float > 29.0
  Then only records with latitude > 29.0 MUST be returned

Scenario: Query by ndp_id returns correct records
  Given Silver layer records with various ndp_id values
  When executing: SELECT * FROM sensor_readings WHERE ndp_id = 'airgradient-office-001'
  Then only records with that ndp_id MUST be returned
  And query execution time MUST be under 100ms for up to 10M records
```

---

## 5. Non-Functional Acceptance Criteria

### AC-NFR-001: Ingestion Latency Impact

```gherkin
Scenario: Latency increase is within acceptable bounds
  Given baseline p95 ingestion latency of X milliseconds
  When processing records with ndp_id and context
  Then p95 latency MUST NOT exceed X * 1.05 (5% increase)
```

### AC-NFR-002: Backward Compatibility

```gherkin
Scenario: Old configuration without ndp_id still works
  Given a legacy stream configuration without ndp_id
  When the application loads the configuration
  Then it MUST NOT fail
  And ndp_id SHOULD default to "{stream_id}-source-{index}"

Scenario: Old configuration without context still works
  Given a legacy stream configuration without context
  When the application loads the configuration
  Then it MUST NOT fail
  And context SHOULD default to empty object {}
```

---

## 6. Success Criteria Summary

The feature is complete when ALL of the following are true:

| # | Criterion | Verification Method |
|---|-----------|---------------------|
| 1 | All active streams have `ndp_id` and `context` in config | File inspection |
| 2 | `ndp_id` and `context` visible in etcd after sync | etcd query |
| 3 | Bronze records include `ndp_id` and `context` JSON blob | Parquet inspection |
| 4 | TimescaleDB includes `ndp_id` (TEXT) and `context` (JSONB) | Schema query |
| 5 | Query `SELECT * FROM readings WHERE ndp_id = 'x'` works | SQL execution |
| 6 | Query `SELECT * FROM readings WHERE context->>'device_type' = 'x'` uses GIN index | EXPLAIN ANALYZE |
| 7 | Query `SELECT * FROM readings WHERE context->'location'->>'type' = 'indoor'` works | SQL execution |
| 8 | Sample configs remain valid documentation | Config validation |

---

## 7. Test Plan Reference

Each acceptance criterion should have corresponding tests:

| Criterion | Test Type | Location |
|-----------|-----------|----------|
| AC-001 to AC-005 | Unit Test | `core/tests/config/` |
| AC-006 to AC-008 | Integration Test | `apps/tests/integration/` |
| AC-009 to AC-012 | Unit Test | `core/tests/ingestion/` |
| AC-013 to AC-016 | Integration Test | `apps/tests/integration/` |
| AC-NFR-* | Performance Test | `apps/tests/performance/` |
