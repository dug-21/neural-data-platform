# AIR-011: Eliminate Duplicative Parser Processing

## System Requirements Specification

**Version:** 1.0.0
**Date:** 2026-01-01
**Author:** Specification Agent (SPARC)

---

## 1. Introduction

### 1.1 Purpose

This document specifies the requirements for removing parser invocations from the ingestion path of the Neural Data Platform. Parsers are currently executed during HTTP and MQTT polling but their output is never consumed because the Bronze layer stores raw JSON, not parsed TimeSeriesPoints.

### 1.2 Scope

- Remove parser execution from HTTP and MQTT source polling
- Eliminate the double-polling problem causing Pi lockups
- Preserve parser code for future Silver layer ETL use
- Maintain backward compatibility for stream configurations

### 1.3 Definitions

| Term | Definition |
|------|------------|
| **Parser** | Component that transforms raw JSON into `Vec<TimeSeriesPoint>` |
| **Source** | HTTP or MQTT polling component that fetches data from endpoints |
| **Bronze Layer** | Raw data storage tier using Parquet files |
| **Double Polling** | Bug where two concurrent polling loops run per source |
| **TimeSeriesPoint** | Parsed metric structure with timestamp, value, location_id, tags |
| **RawDataPoint** | Unprocessed JSON payload with metadata for Bronze storage |

---

## 2. Parser Inventory

### 2.1 Parser Types

| Parser Type | File | Line Count | Description |
|-------------|------|------------|-------------|
| `FlatJsonParser` | `/core/src/parsers/flat_json.rs` | 375 | Extracts all numeric fields from flat JSON |
| `JsonPathParser` | `/core/src/parsers/json_path.rs` | 366 | Extracts fields using JSON path expressions |
| `ArrayIteratorParser` | `/core/src/parsers/array_iterator.rs` | 1279 | Iterates arrays producing multiple points per element |
| `ColumnOrientedParser` | `/core/src/parsers/column_oriented.rs` | 1001 | Handles column-oriented data (NWS gridpoints) |

### 2.2 Parser Module Structure

```
/core/src/parsers/
    mod.rs              # Module exports
    traits.rs           # Parser trait definition (lines 1-146)
    config.rs           # ParserConfig, ParserType enums (lines 1-198)
    factory.rs          # create_parser_from_config() factory (lines 1-162)
    flat_json.rs        # FlatJsonParser implementation
    json_path.rs        # JsonPathParser implementation
    array_iterator.rs   # ArrayIteratorParser implementation
    column_oriented.rs  # ColumnOrientedParser implementation
```

### 2.3 Legacy ResponseParser Types

These are separate from the core parsers and used by GenericHttpPollingSource:

| Parser | File | Description |
|--------|------|-------------|
| `WeatherParser` | `/core/src/sources/parsers/weather.rs` | OpenWeatherMap current weather |
| `AirPollutionParser` | `/core/src/sources/parsers/air_pollution.rs` | OpenWeatherMap air pollution |

---

## 3. Parser Usage Analysis

### 3.1 Current Import Locations

| File | Imports | Line Numbers |
|------|---------|--------------|
| `apps/air-quality-app/src/coordinator/source_manager.rs` | `create_parser_from_config, ParserConfig, ParserType` | 6 |
| `apps/air-quality-app/src/ingestion/mqtt_handler.rs` | `create_parser_from_config, ParserConfig, ParserType` | 9 |
| `core/src/sources/http_poll.rs` | `ParseContext, Parser` | 23 |
| `core/src/sources/mqtt/mod.rs` | `ParseContext, Parser` | 30 |
| `core/src/coordinator/source_manager.rs` | `Parser, create_parser_from_config, ParserConfig, ParserType` | 6-7 |

### 3.2 Parser Creation Sites

#### 3.2.1 source_manager.rs (apps/air-quality-app)

**Lines 414-434 (run_http_polling_source):**
```rust
let parser_config = ParserConfig {
    parser_type: ParserType::FlatJson,
    location_id_field: "serialno".to_string(),
    default_location_id: None,
    skip_fields: vec!["serialno", "wifi", "boot", "firmware", "model", "ledMode", "bootCount"],
    ...
};
let parser = create_parser_from_config(parser_config).map_err(...)?;
let mut source = HttpPollingSource::with_raw_config(config, parser, ...);
```

**Lines 749-772 (run_mqtt_source):**
```rust
let parser_config = ParserConfig {
    parser_type: ParserType::FlatJson,
    location_id_field: "serialno".to_string(),
    ...
};
let parser = create_parser_from_config(parser_config).map_err(...)?;
let mut source = MqttSource::with_raw_config(config, parser, ...);
```

**Lines 821-846 (run_generic_http_polling_source):**
```rust
let parser = create_parser_from_config(parser_config).map_err(...)?;
let mut source = GenericHttpPollingSource::with_raw_config(config, parser, ...);
```

### 3.3 Parser Invocation Sites

#### 3.3.1 HttpPollingSource (http_poll.rs)

**Parsing happens in `poll_sensor()` (lines 430-459):**
```rust
async fn poll_sensor(&self, sensor: &SensorConfig) -> CoreResult<Vec<TimeSeriesPoint>> {
    // ... HTTP request ...
    let parse_context = ParseContext::new(self.ndp_id.clone(), self.context.clone());
    self.parser.parse_with_context(&json, timestamp, &parse_context)  // PARSER INVOKED
}
```

**Called from `poll_all_sensors()` (lines 461-495):**
```rust
async fn poll_all_sensors(&self) -> CoreResult<()> {
    for sensor in &self.config.sensors {
        match self.poll_sensor(sensor).await {  // Calls parser
            Ok(points) => {
                for point in points {
                    self.sender.send(point).await  // Sent to internal channel (NEVER CONSUMED)
                }
            }
            ...
        }
    }
}
```

**Called from `polling_loop()` (lines 497-512):**
```rust
async fn polling_loop(&self) -> CoreResult<()> {
    while *self.is_running.lock().await {
        interval.tick().await;
        self.poll_all_sensors().await  // Triggers parsing
    }
}
```

**Started by `source.start()` (lines 756-791):**
```rust
pub async fn start(&mut self) -> CoreResult<()> {
    *self.is_running.lock().await = true;
    tokio::spawn(async move {
        source_clone.polling_loop().await  // SPAWNS BACKGROUND PARSING LOOP
    });
    self.poll_all_sensors().await?;  // Initial poll with parsing
}
```

#### 3.3.2 MqttSource (mqtt/mod.rs)

**Parsing happens in `process_events()` (lines 346-377):**
```rust
let parse_context = ParseContext::new(ndp_id.clone(), context.clone());
match parser.parse_with_context(&json, timestamp, &parse_context) {  // PARSER INVOKED
    Ok(mut points) => {
        let mut cache = cached_points.lock().await;
        cache.extend(points);  // Cached but never consumed
    }
    ...
}
```

---

## 4. Problem Statement: Double Polling

### 4.1 The Bug

When a source is started, TWO concurrent polling mechanisms run:

1. **Internal polling_loop (Parsing Path):**
   - `source.start()` -> `polling_loop()` -> `poll_all_sensors()` -> `poll_sensor()`
   - Parses JSON into `Vec<TimeSeriesPoint>`
   - Sends to internal `mpsc::Sender<TimeSeriesPoint>` channel
   - **Channel is NEVER consumed** (receiver exists but `fetch()` never called)

2. **External fetch_raw_batch (Storage Path):**
   - SourceManager loop calls `source.fetch_raw_batch()` every 1 second
   - Fetches raw JSON without parsing
   - Returns `Vec<RawDataPoint>` for Bronze layer storage
   - **This is the ACTUAL ingestion path**

### 4.2 Code Evidence

**source_manager.rs lines 452-476 (run_http_polling_source):**
```rust
// Start the source (spawns polling_loop with parsing)
source.start().await.map_err(...)?;

// Poll loop - fetch data and send to ingestion channel
let mut interval = tokio::time::interval(Duration::from_secs(1));

loop {
    tokio::select! {
        _ = cancel_token.cancelled() => { break; }
        _ = interval.tick() => {
            // DP-004: Fetch raw data points directly (NO PARSING)
            match source.fetch_raw_batch().await {
                Ok(raw_points) => {
                    for raw_point in raw_points {
                        ingestion_sender.send(raw_point).await  // Actual storage path
                    }
                }
                ...
            }
        }
    }
}
```

### 4.3 Impact

| Issue | Impact |
|-------|--------|
| Wasted CPU | Parsing ~100KB JSON into 1000+ TimeSeriesPoints per poll |
| Memory Accumulation | Parsed points accumulate in unbounded channel |
| Pi Lockup | After hours, memory pressure causes system lockup |
| Duplicate HTTP Requests | Two loops may issue separate HTTP requests |

---

## 5. Functional Requirements

### 5.1 FR-001: Remove Parser from HttpPollingSource

**Description:** HttpPollingSource SHALL NOT invoke any parser during the `fetch_raw_batch()` or `start()` code paths.

**Rationale:** Bronze layer stores raw JSON; parsing is unnecessary for ingestion.

**Acceptance Criteria:**
- [ ] `HttpPollingSource::new()` does not require a parser parameter
- [ ] `poll_sensor()` does not call `parser.parse_with_context()`
- [ ] `polling_loop()` is removed or made no-op
- [ ] `start()` does not spawn a background polling task
- [ ] `fetch_raw_batch()` remains functional for raw JSON retrieval

### 5.2 FR-002: Remove Parser from MqttSource

**Description:** MqttSource SHALL NOT invoke any parser during message processing.

**Rationale:** Raw MQTT payloads stored directly to Bronze layer.

**Acceptance Criteria:**
- [ ] `MqttSource::new()` does not require a parser parameter
- [ ] `process_events()` does not call `parser.parse_with_context()`
- [ ] `cached_points` (parsed) is removed; only `cached_raw_points` used
- [ ] `fetch_raw_batch()` returns raw JSON from MQTT messages

### 5.3 FR-003: Remove Parser from GenericHttpPollingSource

**Description:** GenericHttpPollingSource SHALL NOT invoke any parser.

**Rationale:** Same as FR-001; external API responses stored raw.

**Acceptance Criteria:**
- [ ] Constructor does not require parser parameter
- [ ] No `parser.parse()` calls in any method
- [ ] `fetch_raw_batch()` returns raw JSON responses

### 5.4 FR-004: Eliminate Double Polling

**Description:** Each source SHALL have exactly one polling mechanism.

**Rationale:** Prevent duplicate HTTP/MQTT requests and resource waste.

**Acceptance Criteria:**
- [ ] `source.start()` does not spawn background polling loop
- [ ] Only SourceManager's external loop calls `fetch_raw_batch()`
- [ ] HTTP requests occur at exactly the configured poll interval

### 5.5 FR-005: Remove Parser Creation from SourceManager

**Description:** SourceManager SHALL NOT create parser instances.

**Rationale:** Parsers not needed for Bronze layer ingestion.

**Acceptance Criteria:**
- [ ] `run_http_polling_source()` does not call `create_parser_from_config()`
- [ ] `run_mqtt_source()` does not call `create_parser_from_config()`
- [ ] `run_generic_http_polling_source()` does not call `create_parser_from_config()`
- [ ] ParserConfig parsing removed from `parse_generic_http_polling_config()`

### 5.6 FR-006: Preserve Parser Code for Future ETL

**Description:** Parser modules SHALL remain intact but unused in ingestion path.

**Rationale:** Silver layer ETL will need parsers for data transformation.

**Acceptance Criteria:**
- [ ] All files in `/core/src/parsers/` unchanged
- [ ] Parser tests continue to pass
- [ ] `create_parser_from_config()` factory function preserved
- [ ] Parser traits and types remain exported from `neural_core`

---

## 6. Non-Functional Requirements

### 6.1 NFR-001: Memory Stability

**Description:** Memory usage SHALL remain stable over extended operation.

**Measurement:**
- Memory usage does not grow unboundedly
- No accumulation in internal channels

**Target:** < 50MB variance over 24 hours of operation

### 6.2 NFR-002: Pi Stability

**Description:** System SHALL run continuously without lockup.

**Measurement:** Raspberry Pi 4 runs for 24+ hours without intervention.

**Target:** 99.9% uptime (< 1.5 min downtime per day)

### 6.3 NFR-003: Minimal Code Change

**Description:** Changes SHALL minimize disruption to existing code.

**Measurement:**
- No changes to parser module internals
- No changes to Bronze storage format
- No changes to RawDataPoint structure

### 6.4 NFR-004: Test Coverage

**Description:** All modified code paths SHALL have test coverage.

**Measurement:**
- Unit tests for modified source constructors
- Integration tests for raw data retrieval
- No decrease in overall test coverage

### 6.5 NFR-005: Backward Compatibility

**Description:** Stream configurations SHALL continue to work.

**Measurement:**
- YAML configs do not require changes
- Parser configuration in YAML can be ignored
- No breaking changes to public API

---

## 7. Code Paths Requiring Changes

### 7.1 Files to Modify

| File | Changes Required |
|------|------------------|
| `core/src/sources/http_poll.rs` | Remove parser field, simplify start(), remove polling_loop |
| `core/src/sources/mqtt/mod.rs` | Remove parser field, remove parsed cache, simplify process_events |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Remove parser creation, simplify source construction |
| `apps/air-quality-app/src/ingestion/mqtt_handler.rs` | Remove parser creation if used |

### 7.2 Files to NOT Modify

| File | Reason |
|------|--------|
| `core/src/parsers/*.rs` | Preserve for Silver layer ETL |
| `core/src/types/raw_data_point.rs` | Bronze layer format unchanged |
| `config/base/streams/*.yaml` | Backward compatibility |

### 7.3 Estimated Impact

| Metric | Current | After Change |
|--------|---------|--------------|
| Parser invocations per poll | ~4 per source | 0 |
| CPU usage (parsing) | ~5-10% | 0% |
| Memory (cached points) | Growing | Stable |
| HTTP requests per interval | Potentially 2x | 1x |

---

## 8. Acceptance Criteria Checklist

### 8.1 Must Have (P0)

- [ ] `source.start()` does not spawn parsing loop
- [ ] No `parser.parse()` calls in ingestion path
- [ ] `fetch_raw_batch()` works without parser
- [ ] Pi runs 24+ hours without lockup
- [ ] Memory usage stable

### 8.2 Should Have (P1)

- [ ] Parser modules untouched
- [ ] Parser tests pass
- [ ] No YAML config changes required
- [ ] Integration tests updated

### 8.3 Nice to Have (P2)

- [ ] Performance benchmark comparison
- [ ] Documentation updated
- [ ] ADR documenting the change

---

## 9. Constraints

### 9.1 Technical Constraints

| Constraint | Description |
|------------|-------------|
| Rust Edition | 2021 (no async_trait changes) |
| Backward Compatibility | YAML configs must work unchanged |
| Storage Format | RawDataPoint structure unchanged |
| Parser Preservation | Parser code must remain functional |

### 9.2 Business Constraints

| Constraint | Description |
|------------|-------------|
| Scope | Bronze layer only; Silver ETL out of scope |
| Timeline | Must fix Pi lockup before extended testing |
| Risk | Minimal - removing unused code paths |

---

## 10. Dependencies

### 10.1 Upstream Dependencies

| Dependency | Impact |
|------------|--------|
| DP-004 (Bronze Layer) | Provides RawDataPoint and storage |
| ADR-001 (Channel Ownership) | IngestionCoordinator owns mpsc channel |

### 10.2 Downstream Dependencies

| Dependency | Impact |
|------------|--------|
| Future Silver Layer ETL | Will reuse parser code |
| Dashboard Features | No impact (uses Bronze data) |

---

## 11. Validation Checklist

Before implementation is complete:

- [ ] All functional requirements have tests
- [ ] Non-functional requirements are measurable
- [ ] Code paths clearly identified
- [ ] No parser code removed (only disabled in ingestion)
- [ ] Memory stability verified
- [ ] Pi stability verified (24+ hours)
- [ ] Integration tests pass
- [ ] No YAML config changes needed
