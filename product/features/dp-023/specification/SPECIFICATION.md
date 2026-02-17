# dp-023: Text Field Pipeline (Bronze through Gold) -- Specification

## Objective

Add non-numeric type support (`text` and `jsonb`) as a generic capability through the full Bronze-Silver-Gold pipeline. NWS forecast is the validation case; the design must support future text-bearing streams without architectural changes. This is plumbing, not intelligence -- once text reaches Gold, fe-005 handles embedding it.

## System Context

### Current Pipeline State

The NDP pipeline processes data through three layers:
- **Bronze**: `RawDataPoint.raw_payload` stores arbitrary JSON. Text fields already survive in Bronze.
- **Silver**: Streaming subscriber transforms Bronze to Silver via field mappings. Currently handles numeric types fully; `text` and `varchar` coercion exists, but `jsonb` is missing an explicit branch.
- **Gold**: Continuous aggregates compute numeric aggregates only. No mechanism exists for text in Gold.

### Active Code Paths (CRITICAL)

| Component | Active Path | Deprecated Path (DO NOT USE) |
|-----------|------------|------------------------------|
| Silver transform | `core/src/subscribers/silver.rs` -> `core/src/silver/transform.rs` -> `core/src/silver/outputs/timescale.rs` | `apps/silver-etl/` (batch ETL) |
| Silver DDL | `deploy/pi/ddl-generator.sh` | `apps/silver-etl/src/schema_gen.rs` |
| Silver types | `SilverFieldType` enum (already has Text, Jsonb, Boolean, Varchar, TextArray) | N/A |

### What Already Works

1. **`SilverFieldType`** already has `Text`, `Jsonb`, `Boolean`, `Varchar`, `TextArray` -- zero enum changes needed.
2. **`coerce_to_type()`** in `transform.rs` line 637 handles `"text" | "varchar"` -- strings, numbers, bools, nulls all convert correctly.
3. **`map_type()`** in `ddl-generator.sh` line 47-61 handles `string|text` -> `TEXT`, `json|jsonb` -> `JSONB`, `varchar` -> `VARCHAR`.
4. **Data dictionary sync** in `deploy.sh` line 679 handles `jsonb` type mapping.
5. **NWS forecast parser** already extracts `shortForecast` as a string field.

### What Is Missing

1. **`coerce_to_type()` jsonb branch**: No explicit `"jsonb"` match arm. Falls through to default wildcard `_ => Ok(value.clone())`, which works but is implicit and untested.
2. **`TimescaleOutput` JSONB casting**: `build_raw_query()` wraps all non-null params in single quotes. For JSONB columns, PostgreSQL needs `'{"key":"val"}'::jsonb` cast -- raw string insertion may fail or produce text instead of jsonb.
3. **Gold text views**: No generator exists for text-specific Gold views. CAs cannot aggregate text.
4. **NWS forecast config**: Missing `silver_etl` section and `stream_type` field.
5. **NWS `detailedForecast`**: Not captured in parser config.
6. **Validation**: `ndp validate` behavior with text/jsonb field_mappings is unverified.

## Functional Requirements

### FR-01: JSONB Coercion in Silver Transform
Add explicit `"jsonb"` branch to `coerce_to_type()` in `core/src/silver/transform.rs`. Accept `Value::Object`, `Value::Array`, `Value::String` (already-serialized JSON), and `Value::Null`. Serialize objects/arrays to JSON string for consistent downstream handling.

### FR-02: TimescaleOutput Text/JSONB Parameter Binding
Verify and fix `build_raw_query()` and `write()` in `core/src/silver/outputs/timescale.rs` to correctly handle text and jsonb values. For JSONB columns, the INSERT must use `::jsonb` cast. For text columns, standard string quoting suffices.

### FR-03: NWS Forecast Mixed-Stream Configuration
Add `silver_etl` section to `config/base/streams/nws-forecast-hourly/config.json` with both numeric fields (`temperature_f`, `dewpoint_c`, etc.) and text fields (`short_forecast`, `detailed_forecast`). Add `stream_type: "forecast"` field.

### FR-04: NWS Forecast Parser Extension
Add `detailedForecast` to the parser element_mappings in the NWS forecast hourly config.

### FR-05: Gold Text View Generator
Create a new Gold DDL generator in `crates/ndp-lib/src/gold/generators/` that produces per-domain VIEWs over Silver text columns. Uses `DISTINCT ON` for latest value per entity per field. Config-driven from domain configuration.

### FR-06: Silver Validation for Text/JSONB
Verify `ndp validate` accepts `"type": "text"` and `"type": "jsonb"` in field_mappings without errors. Update validation schema if needed.

### FR-07: Data Dictionary Text Type Metadata
Ensure text/jsonb fields appear in `data_dictionary.silver_columns` with correct `data_type`, `unit` (NULL for text), and `description`. The dictionary sync already maps these types (deploy.sh line 679).

## Non-Functional Requirements

### NFR-01: Backward Compatibility
All existing numeric-only stream configs must continue working without modification. No changes to existing Silver tables, Gold CAs, or deployment scripts.

### NFR-02: Config-Driven
Gold text views must be generated from domain configuration, not hardcoded SQL. The generator must support any domain with text-bearing streams.

### NFR-03: No Text Processing
dp-023 passes text through without transformation. No NLP, no templating, no embedding. That is fe-005's responsibility.

### NFR-04: Resource Efficiency
Gold text uses a VIEW (not MATERIALIZED VIEW) to avoid storage overhead and refresh orchestration on the Pi.

## Acceptance Criteria Mapping

| AC | Description | Requirement | Deliverable |
|----|-------------|-------------|-------------|
| AC-01 | TEXT field mapping accepted | FR-06 | T-04 |
| AC-02 | JSONB field mapping accepted | FR-06 | T-04 |
| AC-03 | Silver table has TEXT column | FR-03, FR-02 | T-01, T-03 |
| AC-04 | Silver table has numeric + text | FR-03, FR-02 | T-01, T-03 |
| AC-05 | NWS text ingested | FR-01, FR-02, FR-03 | T-01, T-03 |
| AC-06 | Gold text view exists | FR-05 | T-05 |
| AC-07 | Gold text view is config-driven | FR-05 | T-05 |
| AC-08 | Existing numeric streams unaffected | NFR-01 | All |
| AC-09 | Data dictionary updated | FR-07 | T-06 |
| AC-10 | Grafana queryable | FR-05 | T-05 |

## Constraints

- ARM64 (Raspberry Pi 5) -- all dependencies must compile for aarch64
- Config-driven -- no hardcoded DDL, view names, or type mappings
- No DuckDB, no Polars -- use TimescaleDB
- `apps/silver-etl/` is deprecated -- do not reference or modify
- No new NOTIFY triggers -- intelligence reads Gold text view on existing `gold_refresh`
- Version target: v1.2.x
- No text processing/NLP/embedding (fe-005 territory)

## Out of Scope

- Text embedding / MiniLM / EventEmbedder (fe-005)
- Text feature extraction tables (fe-005)
- Template caching (fe-005)
- Composite embeddings (fe-006)
- Text preprocessing / templating / filtering
- Syslog domain -- future consumer
- Retention configuration -- handled by existing Silver policies
- Any NLP or text processing
