# BUG: Bronze Layer Stores Parsed Data Instead of Raw Payloads

**Feature**: dp-004 (Bronze Layer Raw Storage)
**Status**: Architecture Analysis Complete
**Created**: 2026-01-01
**Author**: NDP Architect

---

## Problem Statement

The Bronze layer currently stores parsed `TimeSeriesPoint` data re-serialized as JSON instead of the original raw API payloads. This violates the Bronze/Silver/Gold data lake architecture pattern where:

- **Bronze**: Raw, untransformed data exactly as received from sources
- **Silver**: Cleaned, validated, and structured data (future: TimescaleDB)
- **Gold**: Aggregated, feature-engineered data for ML/analytics

---

## Current State (BROKEN)

### Data Flow Diagram

```
                                 CURRENT (INCORRECT)
                                 ==================

    External API                 Source                    Parser                   Storage
    ============                 ======                    ======                   =======

 +-----------------+        +----------------+        +----------------+        +----------------+
 | HTTP Endpoint   | -----> | HttpPolling    | -----> | FlatJsonParser | -----> | RawDataPoint   |
 | (raw JSON)      |  GET   | Source         |  JSON  | (transforms!)  |  TSP   | (fake "raw")   |
 +-----------------+        +----------------+        +----------------+        +----------------+
         |                         |                        |                         |
         |                         |                        |                         |
         v                         v                        v                         v
  {                          Response              TimeSeriesPoint             RawDataPoint.raw_payload
    "pm25": 12.5,            body as              {                           {
    "pm10": 8.3,             JSON Value             timestamp: ...,             "value": 12.5,        <-- WRONG!
    "temp": 22.4,                |                  location_id: "abc",         "location_id": "abc",
    "wifi": -45,                 |                  value: 12.5,                "tags": {"metric":...}
    "serialno": "abc123",        |                  tags: {metric:"pm25"},    }
    "firmware": "v2.1"           |                }
  }                              |
                                 v
                          SourceManager.run_*_source()
                          ---------------------------
                          match source.fetch().await {  <-- Calls Source::fetch() which PARSES!
                              Ok(points) => {
                                  for point in points {
                                      let raw_point = RawDataPoint::new(
                                          &source_id,
                                          serde_json::json!({
                                              "value": point.value,       <-- Re-serializing parsed data
                                              "location_id": point.location_id,
                                              "tags": point.tags,
                                          }),
                                      );
                                  }
                              }
                          }
```

### Code Path (source_manager.rs)

The bug exists in three places with nearly identical code:

1. **run_http_polling_source()** (lines 444-468)
2. **run_mqtt_source()** (lines 781-806)
3. **run_generic_http_polling_source()** (lines 857-886)

All three call `source.fetch().await` which:
1. Makes HTTP/MQTT request
2. **Parses JSON through the Parser trait** - data transformation happens here
3. Returns `Vec<TimeSeriesPoint>` - already parsed/transformed
4. Re-serializes to JSON for `RawDataPoint.raw_payload` - loses original structure

### What Gets Lost

| Original Payload | Currently Stored | Lost Data |
|------------------|------------------|-----------|
| `{"pm25": 12.5, "temp": 22.4, "serialno": "abc"}` | `{"value": 12.5, "location_id": "abc", "tags": {...}}` | `temp`, structure, field names |
| `{"properties": {"temperature": {"value": 22}}}` | `{"value": 22.0, ...}` | Nested structure, metadata |
| `{"list": [{"main": {"aqi": 3}}]}` | Single flattened point | Array structure, indices |

---

## Target State (CORRECT)

### Data Flow Diagram

```
                                 TARGET (CORRECT)
                                 ================

    External API                 Source                    Storage
    ============                 ======                    =======

 +-----------------+        +----------------+        +----------------+
 | HTTP Endpoint   | -----> | HttpPolling    | -----> | RawDataPoint   |
 | (raw JSON)      |  GET   | Source         |  JSON  | (TRUE raw!)    |
 +-----------------+        +----------------+        +----------------+
         |                         |                         |
         |                         |                         |
         v                         v                         v
  {                          RawDataPoint              Stored EXACTLY
    "pm25": 12.5,            {                         as received
    "pm10": 8.3,               source_id: "air-quality-Http",
    "temp": 22.4,              ndp_id: "airgradient-001",
    "wifi": -45,               context: {...},
    "serialno": "abc123",      raw_payload: {           <-- VERBATIM COPY
    "firmware": "v2.1"           "pm25": 12.5,
  }                              "pm10": 8.3,
                                 "temp": 22.4,
                                 "wifi": -45,
                                 "serialno": "abc123",
                                 "firmware": "v2.1"
                               }
                             }


                          SourceManager.run_*_source() (FIXED)
                          ------------------------------------
                          match source.fetch_raw().await {  <-- Uses RawSource::fetch_raw()
                              Ok(raw_point) => {            <-- Returns RawDataPoint directly
                                  ingestion_sender.send(raw_point).await?;  // No transformation!
                              }
                          }


                                 FUTURE: Silver Layer ETL
                                 ========================

                          Bronze Storage              Silver ETL               TimescaleDB
                          ==============              ==========               ===========

                     +--------------------+      +----------------+      +------------------+
                     | RawDataPoint       | ---> | Parser         | ---> | TimeSeriesPoint  |
                     | (raw_payload JSON) |      | (transforms)   |      | (structured)     |
                     +--------------------+      +----------------+      +------------------+
                              |                        |                        |
                              v                        v                        v
                       Parquet file             FlatJsonParser           Hypertable row
                       (append-only)            NwsParser                (queryable)
                                                OpenWeatherParser
```

---

## Component Changes

### 1. SourceManager (apps/air-quality-app/src/coordinator/source_manager.rs)

**Current State**: Calls `Source::fetch()` then manually constructs `RawDataPoint`

**Required Changes**:

```rust
// BEFORE (BROKEN):
async fn run_http_polling_source(...) {
    match source.fetch().await {  // <-- Returns parsed TimeSeriesPoint!
        Ok(points) => {
            for point in points {
                let raw_point = RawDataPoint::new(
                    &source_id,
                    serde_json::json!({
                        "value": point.value,  // Re-serializing PARSED data
                        "location_id": point.location_id,
                        "tags": point.tags,
                    }),
                );
                ingestion_sender.send(raw_point).await?;
            }
        }
    }
}

// AFTER (CORRECT):
async fn run_http_polling_source(...) {
    match source.fetch_raw_batch().await {  // <-- Returns Vec<RawDataPoint> with TRUE raw!
        Ok(raw_points) => {
            for raw_point in raw_points {
                ingestion_sender.send(raw_point).await?;  // Already has raw_payload
            }
        }
    }
}
```

**Files to Modify**:
- `apps/air-quality-app/src/coordinator/source_manager.rs`
  - `run_http_polling_source()` - lines 386-474
  - `run_mqtt_source()` - lines 727-811
  - `run_generic_http_polling_source()` - lines 813-887

### 2. Source Implementations (core/src/sources/)

**Current State**: `RawSource` trait exists but implementations return HTTP body as-is

**Verification Needed**: Confirm `fetch_raw()` implementations preserve raw JSON

| Source | File | RawSource Impl | Status |
|--------|------|----------------|--------|
| HttpPollingSource | http_poll.rs:596-649 | Implemented | Verify preserves raw |
| GenericHttpPollingSource | http_poll.rs | Needs impl | Missing |
| MqttSource | (TBD) | Needs impl | Missing |

### 3. Parser Role Change

**Current Role**: Parse during ingestion (Bronze layer)
**Target Role**: Parse during Silver layer ETL only

| Parser | Current Usage | Target Usage |
|--------|---------------|--------------|
| FlatJsonParser | Bronze ingestion | Silver ETL only |
| NwsParser | Bronze ingestion | Silver ETL only |
| OpenWeatherParser | Bronze ingestion | Silver ETL only |

**No parser changes needed** - parsers remain as-is but are invoked later in the pipeline.

---

## Interface Changes

### Trait Usage (No Changes Required)

The `RawSource` trait already exists with the correct interface:

```rust
// core/src/traits.rs - ALREADY CORRECT
#[async_trait]
pub trait RawSource: Send + Sync {
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint>;
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>>;
}
```

### SourceManager Internal API

```rust
// BEFORE: Creates parser, passes to source
let parser = create_parser_from_config(parser_config)?;
let source = HttpPollingSource::with_context(config, parser, ndp_id, context)?;
// Then calls source.fetch() -> Vec<TimeSeriesPoint>

// AFTER: No parser needed for Bronze ingestion
let source = HttpPollingSource::with_raw_config(config, None, stream_id, ndp_id, context)?;
// Then calls source.fetch_raw_batch() -> Vec<RawDataPoint>
```

---

## Cleanup Opportunities

### Code That Becomes Unused in Bronze Layer

| Component | Location | After Fix |
|-----------|----------|-----------|
| Parser creation in run_*_source | source_manager.rs:397-419 | Remove |
| Parser creation in run_*_source | source_manager.rs:739-760 | Remove |
| Parser creation in run_*_source | source_manager.rs:829-832 | Remove |
| TimeSeriesPoint to RawDataPoint conversion | source_manager.rs:447-458 | Remove |
| TimeSeriesPoint to RawDataPoint conversion | source_manager.rs:783-795 | Remove |
| TimeSeriesPoint to RawDataPoint conversion | source_manager.rs:859-870 | Remove |

### Code That Remains (Moves to Silver Layer)

| Component | Current Location | Future Location |
|-----------|------------------|-----------------|
| create_parser_from_config() | neural_core::parsers | Silver ETL service |
| FlatJsonParser | neural_core::parsers | Silver ETL service |
| NwsParser | neural_core::parsers | Silver ETL service |
| ParserConfig | neural_core::parsers | Silver ETL service |

---

## Migration Notes

### 1. Backward Compatibility

**Risk**: Existing Parquet files contain parsed data, not raw
**Mitigation**:
- No data migration needed for Bronze files (they remain as-is)
- Silver layer queries will need to handle both formats during transition
- Add `schema_version` field to RawDataPoint for format detection

### 2. Schema Detection

```rust
// Proposed schema version field in RawDataPoint
pub struct RawDataPoint {
    // ... existing fields ...

    /// Schema version for format detection
    /// - None or 1: Legacy (parsed data in raw_payload)
    /// - 2: Correct (true raw payload)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u8>,
}
```

### 3. Deployment Strategy

1. **Phase 1**: Deploy fix with schema_version = 2
2. **Phase 2**: New data uses correct format
3. **Phase 3**: Old data remains readable (backward compatible)
4. **Phase 4**: (Optional) Backfill from source if re-ingestion possible

### 4. Testing Strategy

| Test Type | Coverage |
|-----------|----------|
| Unit | Verify fetch_raw() returns exact HTTP response body |
| Unit | Verify raw_payload matches source JSON exactly |
| Integration | End-to-end: HTTP API -> Parquet file -> verify contents |
| Regression | Existing TimeSeriesPoint path still works (for Silver layer) |

---

## ADR Reference

This architecture document supports:

- **ADR-001**: Bronze layer stores exact source payloads
- **DP-004**: Raw data storage implementation
- **Future DP-XXX**: Silver layer ETL pipeline (where parsing belongs)

---

## Decision Summary

| Aspect | Decision |
|--------|----------|
| Fix Location | SourceManager.run_*_source() methods |
| Trait to Use | RawSource::fetch_raw_batch() instead of Source::fetch() |
| Parser Removal | Remove from Bronze ingestion path |
| Data Format | True raw JSON in raw_payload field |
| Migration | Add schema_version, maintain backward compat |
| Timeline | Can be done incrementally per source type |
