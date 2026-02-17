# dp-023: Text Field Pipeline (Bronze through Gold)

## Vision

The NDP pipeline today is numeric-only. `RawDataPoint.raw_payload` stores arbitrary JSON (so text enters Bronze), but Silver ETL only supports `double_precision` field mappings, and Gold continuous aggregates can only compute numeric aggregates (`AVG`, `MIN`, `MAX`). Text fields like NWS forecast narratives stop at Bronze and are never queryable.

This feature adds non-numeric type support as a **generic capability**: `text` and `jsonb` field types through Bronze → Silver → Gold. NWS forecast is the validation case; the design must support future text-bearing streams (e.g., syslog in a separate domain) without architectural changes. The goal is plumbing, not intelligence — once text reaches Gold, fe-005 handles embedding it.

## Tracking

- Feature: dp-023
- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/37
- Predecessor: dp-020 (declarative deployment), ops-002 (config-driven generators)
- Successor: fe-005 (event embeddings consume text from Gold)
- Version target: v1.2.x

## Current State

### What works

- **Bronze**: `RawDataPoint.raw_payload` is `serde_json::Value` — text fields already survive in JSON. The `nws-forecast-hourly` parser already extracts `shortForecast` as a string field.
- **Stream config**: `fields[]` already supports `"type": "string"` (see `nws-forecast-hourly/config.json` line 67).
- **StreamType::Forecast** exists in `core/src/types/stream.rs`.
- **Silver DDL generator** (`deploy/pi/ddl-generator.sh`): `map_type()` already handles `text`, `jsonb`, `varchar`, `boolean`, `text[]`. Silver CREATE TABLE generation needs zero changes for non-numeric column types.
- **Silver transform** (`core/src/silver/transform.rs`): `coerce_to_type()` already handles `"text"` and `"varchar"` coercion (line 637). Extracts from `serde_json::Value`, returns `Value::String`.
- **Type enums**: `SilverFieldType` already has `Text`, `Jsonb`, `Boolean`, `Varchar`, `TextArray` variants. `FieldType` already has `String`, `Bool`, `Json`. Zero enum changes needed.

### What's missing

- **Silver transform `coerce_to_type()`**: Missing `"jsonb"` branch — only `"text"` and `"varchar"` are handled. JSON values need to pass through as `Value::Object` or `Value::Array`.
- **Silver output `timescale.rs`**: The INSERT parameter binding in `TimescaleOutput` needs verification that it handles text/jsonb `Value` types correctly when writing to PostgreSQL.
- **Gold**: No mechanism to surface text in Gold. CAs require aggregate functions (text can't be AVG'd).
- **NWS detailedForecast**: The valuable multi-sentence narrative isn't captured in the parser config — only `shortForecast` (brief categorical text like "Partly Cloudy").
- **NWS forecast config**: No `silver_etl` section, no `stream_type` field.

### Architecture Clarification: Active vs Deprecated Silver Paths

**IMPORTANT**: There are two Silver code paths in the repository. Only one is active.

**ACTIVE — Silver Subscriber (event-driven, streaming)**:
- `core/src/subscribers/silver.rs` — `SilverSubscriber` subscribes to the internal EventBus
- `core/src/silver/transform.rs` — `transform_to_silver()`, `apply_field_mapping()`, `coerce_to_type()`
- `core/src/silver/outputs/timescale.rs` — `TimescaleOutput` writes to TimescaleDB via bb8 connection pool
- Data flow: EventBus broadcast → SilverSubscriber → transform → TimescaleDB INSERT

**DEPRECATED — Silver ETL (batch, DuckDB-based)**:
- `apps/silver-etl/` — entire crate is the old batch ETL
- Uses DuckDB to read Bronze Parquet files and batch-load into Silver
- Files: `sql_gen.rs`, `schema_gen.rs`, `dq.rs`, `etl.rs`, `daemon.rs`
- **NOT USED in production**. Superseded by the streaming subscriber.
- References to DuckDB functions (`json_extract_string`, `json_extract`), PIVOT SQL, and batch SQL generation are all part of this deprecated path.

**Silver DDL generation** (CREATE TABLE, hypertable, indexes, policies):
- `deploy/pi/ddl-generator.sh` — Bash script sourced by `deploy.sh`
- Reads stream config JSON, uses `map_type()` for type mapping
- Called during `deploy.sh` Phase 4 (Silver Tables) via `handle_silver_table()`
- This is NOT in `apps/silver-etl/src/schema_gen.rs` (that's the deprecated batch path)

**Data dictionary sync**:
- `deploy/pi/deploy.sh` — dictionary sync function reads `silver_etl.field_mappings` from stream configs
- Populates `data_dictionary.silver_tables`, `silver_columns`, `silver_lineage`, `silver_dq_rules`
- Already reads `type` field from each mapping and inserts it directly — may already handle text/jsonb

Planning agents MUST target the active paths listed above. Any references to `apps/silver-etl/src/` are targeting deprecated code and are incorrect.

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| T-01 | Silver subscriber non-numeric types | Add `"jsonb"` coercion to `coerce_to_type()` in `core/src/silver/transform.rs`. Verify `TimescaleOutput` INSERT binding handles text/jsonb values. The subscriber already handles `"text"` — `"jsonb"` is the gap. |
| T-02 | Silver DDL generator | Already works — `deploy/pi/ddl-generator.sh` `map_type()` maps text/jsonb correctly. Verify no changes needed. |
| T-03 | NWS forecast mixed stream | Add `silver_etl` config to `nws-forecast-hourly` with BOTH numeric fields (temperature, windSpeed) AND text fields (shortForecast, detailedForecast). Validates mixed-type stream capability. |
| T-04 | Silver validation | `ndp validate` accepts `text` and `jsonb` field types in silver_etl configs |
| T-05 | Gold text view | Per-domain VIEW (not materialized) over Silver text columns using `DISTINCT ON` for latest value. Config-driven via Gold DDL generator. |
| T-06 | Data dictionary | `data_dictionary.stream_fields` entries for text/jsonb columns with correct type metadata |

## Key Design Decisions

### DECIDED: Two non-numeric types — `text` and `jsonb`

- **`text`**: For simple string fields (forecast narratives, log messages). Maps to PostgreSQL `TEXT`.
- **`jsonb`**: For structured non-numeric data (parsed syslog, nested metadata). Maps to PostgreSQL `JSONB`.
- Starting with both avoids a breaking migration when `jsonb` is needed later.

### DECIDED: Mixed streams — numeric + text in one stream

The NWS forecast stream will carry both numeric fields (temperature, wind) and text fields (forecast narratives) in a single stream config with one `silver_etl` section. Silver ETL handles mixed types in one pass. If we can do mixed, we can trivially do text-only or numeric-only.

### DECIDED: How text reaches Gold — per-domain VIEW (Option C)

Continuous aggregates CANNOT include text columns. The aligned materialized view stays numeric-only.

Text reaches Gold via a **separate per-domain VIEW** (not materialized) that queries Silver directly:

```sql
CREATE VIEW gold.indoor_air_quality_text AS
SELECT DISTINCT ON (stream_id, field_name)
    time, stream_id, field_name, value
FROM silver.nws_forecast_hourly  -- joined across all text-bearing streams in domain
ORDER BY stream_id, field_name, time DESC;
```

Why a VIEW (not MATERIALIZED VIEW):
- Always current — no refresh orchestration needed
- Intelligence app queries it when waking on existing `gold_refresh` NOTIFY
- If performance becomes an issue, upgrade to MATERIALIZED VIEW later

Why per-domain (not per-stream):
- One Gold text view per domain, regardless of how many streams have text
- Matches aligned view pattern (one per domain)
- Syslog would be a different domain with its own text view

### DECIDED: No new NOTIFY triggers

fe-005's intelligence app reads the Gold text view when it wakes on the existing `gold_refresh` NOTIFY. This preserves the architectural pattern: **intelligence reads Gold only**. No Silver NOTIFY, no new trigger infrastructure.

### DECIDED: Retention is not dp-023's concern

- Silver hypertables: text columns follow the stream's existing retention policy (TimescaleDB `add_retention_policy` on the hypertable). No special text retention needed.
- Gold text view: regular VIEW, no retention applies (it's a live query).
- Gold embeddings (`gold.event_embeddings`): fe-005/fe-006 territory.

### DECIDED: NWS forecast as validation case, not the only consumer

dp-023 is a generic capability. NWS forecast validates the design. Future consumers (syslog, alerts) would be new streams in new domains using the same `text`/`jsonb` field types and Gold text view pattern. No dp-023 changes needed for new consumers.

### What "text field" means in config

```json
{
  "silver_etl": {
    "field_mappings": [
      {
        "source_path": "temperature",
        "target_column": "temperature_f",
        "type": "double_precision",
        "description": "Forecast temperature"
      },
      {
        "source_path": "shortForecast",
        "target_column": "short_forecast",
        "type": "text",
        "description": "Brief forecast description",
        "nullable": true
      },
      {
        "source_path": "detailedForecast",
        "target_column": "detailed_forecast",
        "type": "text",
        "description": "Multi-sentence forecast narrative",
        "nullable": true
      }
    ]
  }
}
```

No range checks, no DQ rules beyond nullability — text/jsonb fields are pass-through. Preprocessing is fe-005's responsibility (OPEN — see below).

### DECIDED: dp-023 passes text through without transformation

Research consensus (Ferraro et al. 2023, Hidayatullah 2022): classical NLP preprocessing (stopword removal, stemming, lemmatization) hurts transformer embeddings. Domain-specific preprocessing (log template extraction, weather code decoding) helps but belongs in the embedding layer, not the data pipeline.

- **NWS forecast**: Already clean natural English from the API. No preprocessing needed.
- **Future syslog**: Heavy preprocessing needed (Drain3/drain-rs template extraction), but that's fe-005's EventEmbedder responsibility, configured per stream type.
- **dp-023**: Pass-through only. No text transformation, no NLP, no templating.

See fe-005 SCOPE.md Q1 for the full preprocessing decision space.

## Constraints

- Must NOT break existing numeric-only streams — non-numeric type support is additive
- Silver subscriber must handle mixed rows (some fields numeric, some text/jsonb) in the same stream
- Text/jsonb columns must be queryable via SQL (Grafana compatible)
- No text processing / NLP / embedding in this feature — that's fe-005
- NWS forecast hourly stream currently has NO `silver_etl` section — this feature adds it
- `nws-forecast-hourly` needs `stream_type: "forecast"` added to its config (currently missing)
- Silver data path is the event-driven subscriber (`core/src/subscribers/silver.rs`), NOT the batch ETL (`apps/silver-etl/`)
- Silver DDL is generated by `deploy/pi/ddl-generator.sh`, NOT by `apps/silver-etl/src/schema_gen.rs`
- `apps/silver-etl/` is deprecated — do not reference or modify it

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| TEXT field mapping accepted | `ndp validate` passes with `"type": "text"` in field_mappings |
| JSONB field mapping accepted | `ndp validate` passes with `"type": "jsonb"` in field_mappings |
| Silver table has TEXT column | `\d silver.nws_forecast_hourly` shows `detailed_forecast TEXT` |
| Silver table has numeric + text | Same hypertable contains both `temperature_f DOUBLE PRECISION` and `short_forecast TEXT` |
| NWS text ingested | `SELECT detailed_forecast FROM silver.nws_forecast_hourly LIMIT 5` returns text |
| Gold text view exists | `SELECT * FROM gold.indoor_air_quality_text LIMIT 5` returns latest text per field |
| Gold text view is config-driven | Gold DDL generator produces the view from stream config, not hardcoded |
| Existing numeric streams unaffected | All current Silver ETL configs continue working |
| Data dictionary updated | Text/jsonb fields appear in stream_fields with type metadata |
| Grafana queryable | Text columns visible in Grafana SQL explorer |

## Out of Scope

- Text embedding / MiniLM / EventEmbedder (fe-005)
- Text feature extraction tables (fe-005)
- Template caching (fe-005)
- Composite embeddings (fe-006)
- Text preprocessing / templating / filtering — deferred decision
- Syslog domain — future consumer, not dp-023 deliverable
- Retention configuration — handled by existing Silver policies
- Any NLP or text processing — this is plumbing only

## Release

v1.2.x — Non-numeric field types flow through the full pipeline. Prerequisite for fe-005.
