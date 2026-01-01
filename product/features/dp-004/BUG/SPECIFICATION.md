# DP-004 BUG: Bronze Layer Stores Parsed Data Instead of Raw API Responses

## Status

Open

## Date

2026-01-01

## Problem Statement

The Bronze layer is storing **parsed TimeSeriesPoints** instead of **raw API responses**, violating the fundamental principle established in ADR-001 (Bronze Raw JSON Schema). This defeats the core purpose of the Bronze layer as an immutable, replayable source of truth.

### Current (Broken) Behavior

The data flow currently operates as follows:

1. **HTTP source fetches raw JSON** from external APIs (NWS, OpenWeatherMap, etc.)
2. **Parser immediately extracts metrics** from the raw JSON, creating multiple `TimeSeriesPoint` objects (one per metric: temperature, dewpoint, wind_speed, etc.)
3. **SourceManager receives TimeSeriesPoints** via `source.fetch()`
4. **Each TimeSeriesPoint is re-serialized** into a synthetic JSON object for `raw_payload`:
   ```rust
   let raw_point = RawDataPoint::new(
       &source_id,
       serde_json::json!({
           "value": point.value,
           "location_id": point.location_id,
           "tags": point.tags,
       }),
   )
   ```

### Example of WRONG Data Being Stored

What the Bronze layer currently stores in `raw_payload`:

```json
{"location_id":"KSGJ","tags":{"metric":"dewpoint","source":"nws"},"value":-1.0}
{"location_id":"KSGJ","tags":{"metric":"temperature","source":"nws"},"value":15.5}
{"location_id":"KSGJ","tags":{"metric":"wind_speed","source":"nws"},"value":10.2}
```

This is a **fabricated JSON structure** created by serializing parsed TimeSeriesPoint fields.

### What SHOULD Be Stored

The actual raw API response from the source:

```json
{
  "properties": {
    "temperature": {"value": 15.5, "unitCode": "degC"},
    "dewpoint": {"value": -1.0, "unitCode": "degC"},
    "windSpeed": {"value": 10.2, "unitCode": "km_h-1"},
    "barometricPressure": {"value": 101325, "unitCode": "Pa"},
    "relativeHumidity": {"value": 65, "unitCode": "percent"}
  },
  "geometry": {"type": "Point", "coordinates": [-122.0, 37.5]},
  "@type": "wx:ObservationStation"
}
```

### Impact

1. **Data Loss**: Original field names, units, nested structure, and metadata are lost forever
2. **No Replay Capability**: Cannot reprocess data with improved parsing logic
3. **Audit Failure**: Cannot verify what the source API actually returned
4. **Schema Violation**: Breaks the contract defined in ADR-001
5. **Storage Inefficiency**: Multiple rows per API call instead of one row

---

## Requirements

### REQ-1: Capture Raw HTTP Response Body

Sources MUST capture the raw HTTP response body before any parsing occurs.

**Rationale**: The raw response is the source of truth. Once parsed, original data is irrecoverable.

### REQ-2: Store One Row Per API Call

Each API call MUST result in exactly one `RawDataPoint` stored in Bronze layer.

**Rationale**: ADR-001 specifies "wide format" storage - one row per message, not one row per extracted metric.

### REQ-3: Preserve Exact Response Content

The `raw_payload` field MUST contain the exact JSON (or text) returned by the source API, without transformation.

**Rationale**: This enables replay, debugging, and future re-parsing with improved logic.

### REQ-4: Parser Role Change

Parsers MUST NOT be invoked during Bronze layer ingestion. Their role shifts to:
- Bronze: Extract only metadata (timestamp, source_id) - NO metric extraction
- Silver: Extract metrics from raw_payload during ETL transformation

**Rationale**: Separation of concerns - Bronze stores raw data, Silver performs transformations.

### REQ-5: Maintain Backward Compatibility

The fix MUST NOT break existing MQTT ingestion or other source types that already work correctly.

**Rationale**: MQTT sources may already be storing raw payloads correctly. The fix should be targeted.

---

## Acceptance Criteria

These criteria follow London-style TDD (behavior verification) principles.

### AC-1: Raw Response Capture

**Given** an HTTP polling source configured for NWS weather API
**When** the source fetches data from the API
**Then** the raw HTTP response body is captured before parsing

**Verification**: Mock the HTTP client to return a known JSON response. Verify the captured response matches exactly.

### AC-2: Single Row Per API Call

**Given** an API response containing 5 metrics (temperature, dewpoint, wind_speed, pressure, humidity)
**When** the response is ingested into Bronze layer
**Then** exactly 1 row is written to Parquet (not 5 rows)

**Verification**: Count rows written to storage mock after a single API call.

### AC-3: Exact Payload Preservation

**Given** an API response with nested JSON structure:
```json
{"properties":{"temp":{"value":20.5,"unit":"C"}}}
```
**When** stored in Bronze layer
**Then** `raw_payload` contains the exact JSON string (whitespace-normalized)

**Verification**: Compare stored `raw_payload` to original response using JSON equality.

### AC-4: Source Metadata Correctness

**Given** an HTTP source with:
- `source_id`: "outdoor-weather-Http"
- `ndp_id`: "owm-home"
- `context`: `{"provider": "openweathermap"}`

**When** a response is ingested
**Then** the RawDataPoint contains:
- `source_id` = "outdoor-weather-Http"
- `ndp_id` = "owm-home"
- `context` = `{"provider": "openweathermap"}`
- `raw_payload` = exact API response

**Verification**: Inspect RawDataPoint fields via mock sender capture.

### AC-5: No Parser Invocation in Bronze Path

**Given** an HTTP polling source with a registered parser
**When** data is ingested into Bronze layer
**Then** the parser's `parse()` method is NOT called

**Verification**: Use a mock parser that fails if `parse()` is invoked during Bronze ingestion.

### AC-6: MQTT Sources Unaffected

**Given** an existing MQTT source configuration
**When** the bug fix is deployed
**Then** MQTT ingestion behavior remains unchanged

**Verification**: Existing MQTT integration tests continue to pass.

---

## Scope

### In Scope

| Item | Description |
|------|-------------|
| `source_manager.rs` | Fix the conversion logic that creates RawDataPoint from TimeSeriesPoint |
| `http_poll.rs` | Potentially modify to return raw response alongside or instead of parsed points |
| Integration with RawSource trait | Ensure HTTP sources implement RawSource for raw data access |
| Unit tests | New tests verifying correct raw storage behavior |
| Integration tests | End-to-end tests with mock HTTP servers |

### Out of Scope

| Item | Rationale |
|------|-----------|
| Silver layer ETL | Separate feature (dp-002/dp-005) |
| Migration of existing Parquet files | Separate migration task |
| MQTT source changes | MQTT may already work correctly |
| Grafana dashboard updates | Deferred until Silver layer is ready |
| Parser refactoring | Parsers will be used in Silver ETL, not removed |

---

## Affected Components

### Primary Files (Must Change)

| File | Location | Issue |
|------|----------|-------|
| `source_manager.rs` | `apps/air-quality-app/src/coordinator/source_manager.rs` | Lines 447-471, 859-876: Creates RawDataPoint from parsed TimeSeriesPoint instead of raw response |

### Secondary Files (May Need Changes)

| File | Location | Issue |
|------|----------|-------|
| `http_poll.rs` | `core/src/sources/http_poll.rs` | May need to expose raw response body, not just parsed points |
| `traits.rs` | `core/src/traits.rs` | RawSource trait may need enhancement |
| `generic_http.rs` | `core/src/sources/generic_http.rs` | Similar pattern to http_poll.rs |

### Files for Reference (No Changes Expected)

| File | Purpose |
|------|---------|
| `raw_data_point.rs` | `core/src/types/raw_data_point.rs` - RawDataPoint struct is correct |
| `parquet.rs` | `core/src/storage/parquet.rs` - Storage logic is correct |
| `parsers/*.rs` | Parser implementations - will be used in Silver layer |

---

## Technical Analysis

### Root Cause

The `SourceManager::run_http_polling_source_loop` and `run_generic_http_polling_source_loop` methods:

1. Call `source.fetch()` which returns `Vec<TimeSeriesPoint>` (already parsed)
2. Iterate over each TimeSeriesPoint
3. Construct a **new JSON object** from TimeSeriesPoint fields
4. Store this fabricated JSON as `raw_payload`

```rust
// Current BROKEN code (source_manager.rs:447-458)
match source.fetch().await {
    Ok(points) => {
        for point in points {
            let raw_point = RawDataPoint::new(
                &source_id,
                serde_json::json!({
                    "value": point.value,          // WRONG: This is parsed data
                    "location_id": point.location_id,
                    "tags": point.tags,
                }),
            )
            // ...
        }
    }
}
```

### Correct Architecture

The HTTP source should:
1. Capture raw HTTP response body as `String` or `Value`
2. Create ONE `RawDataPoint` per API call with the raw response
3. NOT invoke parsers during ingestion
4. Parsers are invoked later in Silver layer ETL

```rust
// Correct approach (pseudocode)
match source.fetch_raw().await {
    Ok(raw_responses) => {
        for (timestamp, raw_json) in raw_responses {
            let raw_point = RawDataPoint::new(&source_id, raw_json)
                .with_timestamp(timestamp)
                .with_ndp_id_opt(ndp_id.clone())
                .with_context_opt(context.clone());
            ingestion_sender.send(raw_point).await?;
        }
    }
}
```

---

## Test Strategy

### Unit Tests

1. **Mock HTTP Client**: Return known JSON responses
2. **Mock Storage Sender**: Capture RawDataPoints for inspection
3. **Verify**: raw_payload contains exact response, not fabricated JSON

### Integration Tests

1. **Mock HTTP Server**: Spin up local server returning realistic API responses
2. **Real Storage Path**: Write to in-memory Parquet or temp files
3. **Verify**: Read back Parquet and confirm raw_payload contents

### Regression Tests

1. **MQTT Tests**: Ensure existing MQTT tests pass unchanged
2. **End-to-End**: Full pipeline test with mocked external APIs

---

## References

- [ADR-001: Bronze Raw JSON Schema](/workspaces/neural-data-platform/product/features/dp-004/architecture/ADR-001-bronze-raw-json-schema.md)
- [DP-004 SCOPE](/workspaces/neural-data-platform/product/features/dp-004/SCOPE.md)
- [Platform Architecture Overview](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
