# dp-023: Task Decomposition

## Wave Structure

Tasks are organized into 3 implementation waves based on dependencies. Each wave can be parallelized internally but must complete before the next wave starts.

## Wave 1: Core Silver Changes

Foundation work: make the Silver subscriber correctly handle text and jsonb values end-to-end.

### Task W1-01: Add explicit jsonb branch to coerce_to_type()

- **Files**: `core/src/silver/transform.rs`
- **Description**: Add `"jsonb"` match arm to `coerce_to_type()` (line 584). Accept `Value::Object` and `Value::Array` (pass through as-is), `Value::String` (validate as JSON, pass through), `Value::Null`. The existing wildcard `_ => Ok(value.clone())` accidentally handles this but is implicit and untested.
- **Dependencies**: None
- **Complexity**: Low
- **AC mapping**: AC-03, AC-04, AC-05
- **Tests**: Unit tests for jsonb coercion -- object, array, pre-serialized string, null, invalid types

### Task W1-02: Fix TimescaleOutput JSONB parameter binding

- **Files**: `core/src/silver/outputs/timescale.rs`
- **Description**: The `build_upsert_query()` generates `$N` placeholders for all data fields uniformly. `build_raw_query()` wraps all non-null params in single quotes. For JSONB columns, PostgreSQL needs a `::jsonb` cast on the placeholder (e.g., `$4::jsonb` or `'{"key":"val"}'::jsonb`). Two options: (a) modify `build_upsert_query()` to emit type-aware placeholders, or (b) modify `build_raw_query()` to detect JSON values. The `build_upsert_query` approach is cleaner because it can read the field mapping type from `etl_config`.
- **Dependencies**: None (parallel with W1-01)
- **Complexity**: Medium
- **AC mapping**: AC-03, AC-04, AC-05
- **Tests**: Integration test with mock TimescaleDB verifying JSONB INSERT syntax

### Task W1-03: Verify text value flow through TimescaleOutput

- **Files**: `core/src/silver/outputs/timescale.rs`
- **Description**: Verify that text values (`Value::String`) flow correctly through `write()` -> `build_raw_query()`. The current `value.to_string().trim_matches('"')` extracts the inner string, then `build_raw_query` wraps in single quotes. This should work but needs explicit test coverage.
- **Dependencies**: None (parallel with W1-01, W1-02)
- **Complexity**: Low
- **Tests**: Unit test asserting text values produce correct SQL

## Wave 2: Configuration + DDL + Gold

Build on the Silver foundation to handle NWS forecast config and Gold text views.

### Task W2-01: Add silver_etl section to NWS forecast config

- **Files**: `config/base/streams/nws-forecast-hourly/config.json`
- **Description**: Add `stream_type: "forecast"` and `silver_etl` section with field_mappings for both numeric (temperature_f, dewpoint_c, relative_humidity, wind_speed_mph, wind_direction_deg, probability_of_precipitation, forecast_issue_time) and text (short_forecast, detailed_forecast) fields. Add `detailedForecast` to parser element_mappings.
- **Dependencies**: W1-01, W1-02
- **Complexity**: Medium
- **AC mapping**: AC-03, AC-04, AC-05
- **Tests**: `ndp validate` passes on updated config

### Task W2-02: Verify DDL generator handles text/jsonb

- **Files**: `deploy/pi/ddl-generator.sh`
- **Description**: Verify `map_type()` and `generate_silver_ddl()` correctly produce TEXT/JSONB columns. `map_type()` already handles these types (line 47-61). Verify `generate_silver_ddl()` does not assume all data columns are numeric (e.g., no DEFAULT 0 or NOT NULL constraints that would break text).
- **Dependencies**: W2-01 (need config to test against)
- **Complexity**: Low
- **AC mapping**: AC-03, AC-08
- **Tests**: Run DDL generator on NWS forecast config, verify output DDL

### Task W2-03: Create Gold text view generator

- **Files**: `crates/ndp-lib/src/gold/generators/text_view.rs`, `crates/ndp-lib/src/gold/generators/mod.rs`, `crates/ndp-lib/src/gold/mod.rs`
- **Description**: New generator `TextViewGenerator` that produces per-domain VIEWs over Silver text columns. Scans domain streams for text/jsonb field mappings, generates a UNION ALL query across text-bearing streams, uses `DISTINCT ON (stream_id, field_name)` for latest value. VIEW (not MATERIALIZED VIEW) for simplicity and freshness.
- **Dependencies**: W1-01, W1-02 (Silver must handle text before Gold views make sense)
- **Complexity**: High
- **AC mapping**: AC-06, AC-07, AC-10
- **Tests**: Unit test generating SQL from mock config, verify VIEW syntax

### Task W2-04: Wire Gold text view into deploy.sh

- **Files**: `deploy/pi/deploy.sh`
- **Description**: Add Gold text view generation to deploy.sh Phase 6 (Gold DDL). Call `ndp gold text-view --domain <id>` or integrate TextViewGenerator output into the deployment flow.
- **Dependencies**: W2-03
- **Complexity**: Low
- **AC mapping**: AC-06, AC-07

## Wave 3: Validation + Data Dictionary + Integration

Final validation, data dictionary, and end-to-end integration testing.

### Task W3-01: Validate ndp-validate accepts text/jsonb

- **Files**: `tools/ndp-validate/src/schema.rs`, `config/schemas/stream.schema.json` (if schema exists)
- **Description**: Run `ndp validate` against the updated NWS forecast config. If it rejects text/jsonb types, update the validation schema. Verify DQ rule validation skips range_check for non-numeric types.
- **Dependencies**: W2-01
- **Complexity**: Low
- **AC mapping**: AC-01, AC-02
- **Tests**: `ndp validate` passes on text/jsonb field_mappings

### Task W3-02: Verify data dictionary sync

- **Files**: `deploy/pi/deploy.sh` (dictionary sync function)
- **Description**: Verify the Bash dictionary sync correctly populates `data_dictionary.silver_columns` with TEXT/JSONB types for text/jsonb field_mappings. The sync at line 679 already maps `jsonb` -> `JSONB`. Verify `text` -> `TEXT` mapping exists (line 676). Verify lineage entries are created.
- **Dependencies**: W2-01
- **Complexity**: Low
- **AC mapping**: AC-09
- **Tests**: Run dictionary sync on NWS forecast config, query silver_columns

### Task W3-03: Integration test -- full pipeline

- **Files**: New test file or existing integration test suite
- **Description**: End-to-end test: NWS forecast config -> DDL generation -> Silver subscriber with text/jsonb -> TimescaleDB INSERT -> Gold text view query. Verify AC-03 through AC-06.
- **Dependencies**: W1-01, W1-02, W2-01, W2-03
- **Complexity**: High
- **AC mapping**: AC-03, AC-04, AC-05, AC-06

### Task W3-04: Existing stream regression test

- **Files**: Existing test suite
- **Description**: Run all existing Silver ETL tests and Gold DDL tests to verify no regressions. Run `ndp validate` on all existing stream configs.
- **Dependencies**: All Wave 1 + Wave 2 tasks
- **Complexity**: Low
- **AC mapping**: AC-08

## Dependency Graph

```
W1-01 (jsonb coercion) ──┐
W1-02 (JSONB binding) ───┤──> W2-01 (NWS config) ──> W2-02 (DDL verify) ──> W3-01 (validate)
W1-03 (text verify) ─────┘                                                    W3-02 (dictionary)
                          ├──> W2-03 (Gold text gen) ──> W2-04 (deploy.sh) ──> W3-03 (integration)
                          └──────────────────────────────────────────────────> W3-04 (regression)
```

## Estimated Effort

| Wave | Tasks | Complexity | Estimated Effort |
|------|-------|-----------|------------------|
| Wave 1 | W1-01, W1-02, W1-03 | Low-Medium | 2-3 hours |
| Wave 2 | W2-01, W2-02, W2-03, W2-04 | Medium-High | 4-6 hours |
| Wave 3 | W3-01, W3-02, W3-03, W3-04 | Low-High | 3-4 hours |
| **Total** | **11 tasks** | | **9-13 hours** |
