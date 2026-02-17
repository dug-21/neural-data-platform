# dp-023: Architecture Decisions

## Summary

Six architectural decisions for the Text Field Pipeline feature. Each ADR was produced after consulting the actual codebase -- file paths and line numbers reference the code as of 2026-02-17.

## ADR-001: JSONB Coercion Strategy

### Context

`coerce_to_type()` in `core/src/silver/transform.rs` (line 584) matches column types to coerce `serde_json::Value` variants into the correct form for Silver output. It has explicit branches for `"double_precision" | "real"`, `"integer" | "bigint" | "smallint"`, `"text" | "varchar"`, and `"boolean"`. There is no explicit `"jsonb"` branch.

The default wildcard arm `_ => Ok(value.clone())` (line 656) catches `"jsonb"` and passes the value through unchanged. This accidentally works but has three problems:
1. It also catches typos (e.g., `"doubleprecision"`) without error
2. There are no unit tests for jsonb coercion
3. The intent is not documented -- a future developer might not know jsonb is supported

### Decision

Add an explicit `"jsonb"` match arm to `coerce_to_type()`:

```rust
"jsonb" => match value {
    Value::Object(_) | Value::Array(_) => Ok(value.clone()),
    Value::String(s) => {
        // Validate that the string is valid JSON
        serde_json::from_str::<Value>(s)
            .map_err(|_| TransformError::TypeConversion {
                field: field_name.to_string(),
                expected: "jsonb (valid JSON string)".to_string(),
                actual: "invalid JSON string".to_string(),
            })
    }
    Value::Null => Ok(Value::Null),
    // Numbers and booleans are valid JSON primitives
    Value::Number(_) | Value::Bool(_) => Ok(value.clone()),
},
```

This handles five cases:
- `Value::Object` / `Value::Array`: pass through (already structured JSON)
- `Value::String`: validate as JSON and parse (handles pre-serialized JSON strings from upstream)
- `Value::Null`: pass through
- `Value::Number` / `Value::Bool`: pass through (valid JSON primitives)

### Consequences

- **Positive**: Explicit, tested, documented behavior for jsonb
- **Positive**: Pre-serialized JSON strings are validated, preventing corrupt data in Silver
- **Negative**: Adds ~15 lines to coerce_to_type, but this matches the pattern of other branches
- **Tradeoff**: Parsing pre-serialized strings adds CPU cost, but jsonb fields are rare and the parse validates correctness

## ADR-002: TimescaleOutput Text/JSONB Parameter Binding

### Context

`TimescaleOutput` in `core/src/silver/outputs/timescale.rs` writes Silver records to TimescaleDB. The current flow:

1. `build_upsert_query()` (line 159) generates `INSERT INTO ... VALUES ($1, $2, ...)` with generic `$N` placeholders for all columns
2. `write()` (line 272) builds params as `Vec<String>` by calling `value.to_string().trim_matches('"')` on each field (line 325)
3. `build_raw_query()` (line 451) substitutes each `$N` with `'<escaped_value>'` (single-quoted string)

For **text** columns: `Value::String("Partly Cloudy")` -> `.to_string()` = `"\"Partly Cloudy\""` -> `.trim_matches('"')` = `"Partly Cloudy"` -> SQL: `'Partly Cloudy'`. This works correctly because PostgreSQL accepts quoted strings for TEXT columns.

For **jsonb** columns: `Value::Object({"key": "val"})` -> `.to_string()` = `"{\"key\":\"val\"}"` -> `.trim_matches('"')` = `"{\"key\":\"val\"}"` -> SQL: `'{"key":"val"}'`. PostgreSQL **can** implicitly cast this to JSONB in an INSERT if the column type is JSONB, but this is fragile and depends on the PostgreSQL parsing context.

The current approach uses `conn.execute(&raw_query, &[])` (line 346) -- a raw SQL string with no parameterized types. PostgreSQL infers column types from the table definition, so `'{"key":"val"}'` inserted into a JSONB column should work via implicit cast. However, this is not guaranteed in all PostgreSQL configurations and is a source of subtle bugs.

### Decision

Modify `build_upsert_query()` to emit type-cast placeholders for jsonb columns:

```rust
// In build_upsert_query(), when iterating field_names:
for name in &field_names {
    columns.push(name.clone());
    // Look up field mapping type from etl_config
    let field_type = etl_config.field_mappings.iter()
        .find(|m| m.target_column == *name)
        .map(|m| m.column_type.as_str())
        .unwrap_or("text");

    if field_type == "jsonb" {
        placeholders.push(format!("${}::jsonb", param_index));
    } else {
        placeholders.push(format!("${}", param_index));
    }
    param_index += 1;
}
```

This produces `INSERT INTO ... VALUES ($1, $2, $3::jsonb, $4)` -- the `::jsonb` cast makes the intent explicit and avoids relying on PostgreSQL implicit casting.

For text columns, no cast is needed -- PostgreSQL handles `'string value'` -> TEXT natively.

### Consequences

- **Positive**: Explicit JSONB casting eliminates implicit-cast ambiguity
- **Positive**: Works with any PostgreSQL `standard_conforming_strings` setting
- **Positive**: No changes to `build_raw_query()` needed -- the cast is in the SQL template
- **Negative**: `build_upsert_query()` now needs access to field mapping types, adding a lookup per field
- **Tradeoff**: The field_mapping lookup is O(N*M) where N = fields and M = mappings. For typical stream configs (5-15 fields), this is negligible

## ADR-003: Gold Text View Pattern

### Context

Gold layer uses continuous aggregates (CAs) for numeric data, generated by `ContinuousAggregateGenerator` in `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs`. CAs require aggregate functions (`AVG`, `MIN`, `MAX`, etc.) that are undefined for text columns. Text cannot be aggregated into CAs.

The aligned view generator (`aligned_view.rs`) produces MATERIALIZED VIEWs for cross-stream correlation -- also numeric-only, using JOINs on time buckets.

Gold needs a different mechanism for text: a VIEW that surfaces the latest text value per entity per field, queryable by Grafana and intelligence apps.

### Decision

Create a new generator `TextViewGenerator` in `crates/ndp-lib/src/gold/generators/text_view.rs` that produces per-domain VIEWs:

```sql
CREATE OR REPLACE VIEW gold.{domain_id}_text AS
SELECT DISTINCT ON (source_stream, field_name)
    t.observation_time AS time,
    t.source_stream,
    t.field_name,
    t.value
FROM (
    SELECT observation_time, 'nws_forecast_hourly' AS source_stream,
           'short_forecast' AS field_name, short_forecast AS value
    FROM silver.nws_forecast_hourly
    WHERE short_forecast IS NOT NULL
    UNION ALL
    SELECT observation_time, 'nws_forecast_hourly' AS source_stream,
           'detailed_forecast' AS field_name, detailed_forecast AS value
    FROM silver.nws_forecast_hourly
    WHERE detailed_forecast IS NOT NULL
    -- ... additional text fields from other streams in the domain
) t
ORDER BY t.source_stream, t.field_name, t.observation_time DESC;
```

Key design choices:

1. **VIEW, not MATERIALIZED VIEW**: Always fresh, no refresh orchestration, no storage overhead. Suitable for the Pi's resource constraints. If performance becomes an issue, upgrade to MATERIALIZED VIEW later.

2. **Per-domain, not per-stream**: One `gold.{domain_id}_text` view per domain, matching the aligned view pattern. A domain with 3 text-bearing streams gets one unified text view.

3. **DISTINCT ON for latest value**: Returns the most recent text value per stream per field. This is the natural query pattern for text -- "what is the current forecast?" not "what was the average forecast?"

4. **Config-driven**: The generator reads domain config to find streams with text/jsonb field mappings, then builds the UNION ALL query. No hardcoded stream or field names.

5. **Unpivoted schema**: The view uses `(source_stream, field_name, value)` columns rather than one column per text field. This normalizes across streams and avoids schema changes when new text fields are added.

### Consequences

- **Positive**: Text is queryable in Gold via standard SQL
- **Positive**: VIEW avoids storage overhead on Pi
- **Positive**: Config-driven -- new text streams automatically appear in the view
- **Positive**: Unpivoted schema is Grafana-friendly (can filter by field_name)
- **Negative**: VIEW performance depends on Silver table size. For large tables, an index on `(observation_time DESC)` is needed (already exists from hypertable)
- **Negative**: DISTINCT ON returns only the latest row per field, not historical text. Historical text requires querying Silver directly.
- **Tradeoff**: Per-domain granularity means all text from all streams in a domain appears in one view. This is a feature (unified query point) but could be noisy for domains with many text-bearing streams.

## ADR-004: NWS Forecast Mixed-Stream Configuration

### Context

The NWS forecast hourly config (`config/base/streams/nws-forecast-hourly/config.json`) currently has:
- Stream fields (Bronze): temperature, dewpoint, relative_humidity, wind_speed, wind_direction, short_forecast, probability_of_precipitation, forecast_issue_time
- Parser element_mappings: Maps NWS API fields to metric names
- **No `silver_etl` section** -- Silver subscriber cannot process this stream
- **No `stream_type` field** -- Gold generators cannot classify this stream

The parser already extracts `shortForecast` but does NOT extract `detailedForecast` (the multi-sentence narrative).

### Decision

Add to `config/base/streams/nws-forecast-hourly/config.json`:

1. **`stream_type: "forecast"`** at the top level (after `stream_id`)

2. **`detailedForecast`** element mapping in the parser:
```json
{
    "path": "detailedForecast",
    "metric_name": "detailed_forecast",
    "optional": true
}
```

3. **`silver_etl` section** with mixed numeric + text field_mappings:
```json
{
    "silver_etl": {
        "enabled": true,
        "target_table": "silver.nws_forecast_hourly",
        "timestamp": {
            "source_field": "timestamp",
            "target_field": "observation_time",
            "transform": "iso8601"
        },
        "identity_fields": [
            {
                "source": "ndp_id",
                "target": "ndp_id"
            }
        ],
        "deduplication": {
            "enabled": true,
            "strategy": "upsert",
            "key_columns": ["observation_time", "ndp_id"]
        },
        "field_mappings": [
            {
                "source_path": "temperature",
                "target_column": "temperature_f",
                "type": "double_precision",
                "description": "Forecast temperature in Fahrenheit"
            },
            {
                "source_path": "dewpoint",
                "target_column": "dewpoint_c",
                "type": "double_precision",
                "description": "Forecast dew point in Celsius",
                "nullable": true
            },
            {
                "source_path": "relative_humidity",
                "target_column": "relative_humidity",
                "type": "double_precision",
                "description": "Forecast relative humidity percentage",
                "nullable": true
            },
            {
                "source_path": "wind_speed",
                "target_column": "wind_speed_mph",
                "type": "double_precision",
                "description": "Forecast wind speed in mph",
                "nullable": true
            },
            {
                "source_path": "wind_direction",
                "target_column": "wind_direction_deg",
                "type": "double_precision",
                "description": "Forecast wind direction in degrees",
                "nullable": true
            },
            {
                "source_path": "probability_of_precipitation",
                "target_column": "probability_of_precipitation",
                "type": "double_precision",
                "description": "Precipitation probability percentage",
                "nullable": true
            },
            {
                "source_path": "forecast_issue_time",
                "target_column": "forecast_issue_time",
                "type": "double_precision",
                "description": "Forecast issue timestamp as epoch seconds",
                "nullable": true
            },
            {
                "source_path": "short_forecast",
                "target_column": "short_forecast",
                "type": "text",
                "description": "Brief forecast description (e.g., Partly Cloudy)",
                "nullable": true
            },
            {
                "source_path": "detailed_forecast",
                "target_column": "detailed_forecast",
                "type": "text",
                "description": "Multi-sentence forecast narrative",
                "nullable": true
            }
        ],
        "dq_rules": [],
        "dq_output": {
            "enabled": false
        }
    }
}
```

Key choices:
- Text fields are `nullable: true` because NWS API sometimes omits forecast text
- No DQ rules for text fields (no range_check, no pattern_check for initial release)
- `stream_type: "forecast"` enables Gold generators to classify the stream correctly

### Consequences

- **Positive**: NWS forecast hourly becomes the first mixed numeric+text stream, validating the entire pipeline
- **Positive**: Both shortForecast and detailedForecast are captured
- **Negative**: Adding silver_etl creates a new Silver hypertable -- requires DDL generation during deployment
- **Tradeoff**: No DQ rules for text means no quality gates on text content. This is intentional -- text DQ is deferred to fe-005

## ADR-005: Validation Rule Updates

### Context

`ndp validate` (`tools/ndp-validate/`) validates stream configurations against schemas. The validation includes:
1. JSON schema validation (`schema.rs`) against `config/schemas/stream.schema.json`
2. Semantic validation (`semantic/dq_rules.rs`) for DQ rules -- range_check, enum_check, pattern_check, etc.

Text/jsonb field types may trigger validation issues:
- If the schema's `field_mappings[].type` enum does not include `"text"` or `"jsonb"`, schema validation fails
- DQ rules like `range_check` expect numeric min/max -- they would be nonsensical for text fields
- `pattern_check` could apply to text but is optional

### Decision

1. **Verify schema** (`config/schemas/stream.schema.json` if it exists, or embedded schema): Ensure `field_mappings[].type` enum includes `"text"`, `"jsonb"`, `"varchar"`, `"boolean"`, `"text[]"`. If missing, add them.

2. **DQ rule validation**: No changes needed. The `validate_dq_rules()` function in `semantic/dq_rules.rs` validates rules by type (`range_check`, `enum_check`, etc.) independent of field type. Text fields simply should not have `range_check` rules -- this is a configuration concern, not a validation bug.

3. **Optional enhancement**: Add a warning (not error) if a `range_check` DQ rule references a text/jsonb field. This catches misconfiguration early. Deferred -- not required for dp-023.

### Consequences

- **Positive**: Minimal validation changes -- the schema enum is the only required update
- **Positive**: No new validation logic needed for text fields
- **Negative**: No text-specific validation (e.g., max length, encoding checks). Deferred to future work if needed.

## ADR-006: Data Dictionary Text Type Metadata

### Context

The data dictionary sync in `deploy/pi/deploy.sh` populates `data_dictionary.silver_columns` during deployment. The sync function `_sync_to_data_dictionary_bash()` (line 431) processes `silver_etl.field_mappings` and maps types at line 669-681:

```bash
case "$col_type" in
    double_precision) pg_type="DOUBLE PRECISION" ;;
    ...
    text) pg_type="TEXT" ;;
    ...
    jsonb) pg_type="JSONB" ;;
    *) pg_type="TEXT" ;;
esac
```

Both `text` and `jsonb` are already mapped. The `silver_columns` table (init-script 006) stores `data_type TEXT` which accepts any type string.

For text fields, `unit` should be NULL (text has no unit) and `validation_min`/`validation_max` in the `fields` table should be NULL.

### Decision

No code changes required for data dictionary sync. The existing mapping handles text and jsonb correctly:
- `text` -> `TEXT` (line 676)
- `jsonb` -> `JSONB` (line 679)
- `unit` is set from config; text fields specify no unit, so it becomes NULL (line 687)
- `description` is set from config field description

The only requirement is that the NWS forecast config (ADR-004) includes proper `description` values for text field_mappings, so the dictionary entries are informative.

Verify with a deployment test that `data_dictionary.silver_columns` contains entries for `short_forecast` (TEXT) and `detailed_forecast` (TEXT) after syncing the NWS forecast config.

### Consequences

- **Positive**: Zero code changes for dictionary sync -- existing code handles text/jsonb already
- **Positive**: Lineage entries (`silver_lineage`) will correctly show source_path -> target_column for text fields
- **Negative**: No text-specific metadata (e.g., max observed length, character encoding). Could be added later if needed.
