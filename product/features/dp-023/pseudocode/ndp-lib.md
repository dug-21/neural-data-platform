# dp-023: ndp-lib Pseudocode (Gold Text View Generator)

## New File: crates/ndp-lib/src/gold/generators/text_view.rs

### Purpose

Generate per-domain VIEWs over Silver text columns. The VIEW uses DISTINCT ON to return the latest text value per stream per field.

### Data Structures

```rust
/// Configuration for a single text field discovered in a stream
struct TextFieldInfo {
    stream_id: String,          // e.g., "nws-forecast-hourly"
    silver_table: String,       // e.g., "silver.nws_forecast_hourly"
    column_name: String,        // e.g., "short_forecast"
    field_type: String,         // "text" or "jsonb"
    timestamp_column: String,   // e.g., "observation_time"
}

/// Generator for Gold text views
pub struct TextViewGenerator<L: ConfigLoader> {
    config_loader: L,
}
```

### Algorithm: discover_text_fields()

```rust
fn discover_text_fields(domain_config: &DomainConfig) -> Vec<TextFieldInfo>:
    let mut text_fields = Vec::new()

    // Iterate all streams referenced by this domain
    for stream_ref in domain_config.streams:
        let stream_config = config_loader.load_stream(stream_ref.stream_id)

        // Skip streams without silver_etl
        let silver_etl = match stream_config.silver_etl:
            Some(etl) => etl
            None => continue

        let timestamp_col = silver_etl.timestamp.target_field  // default: "observation_time"
        let silver_table = silver_etl.target_table

        // Find text/jsonb field mappings
        for mapping in silver_etl.field_mappings:
            if mapping.column_type in ["text", "varchar", "jsonb", "text[]"]:
                text_fields.push(TextFieldInfo {
                    stream_id: stream_config.stream_id,
                    silver_table: silver_table,
                    column_name: mapping.target_column,
                    field_type: mapping.column_type,
                    timestamp_column: timestamp_col,
                })

    return text_fields
```

### Algorithm: generate()

```rust
fn generate(domain_id: &str, action: Action) -> Result<String>:
    let domain_config = config_loader.load_domain(domain_id)?
    let text_fields = discover_text_fields(&domain_config)

    if text_fields.is_empty():
        return Ok("-- No text fields found for domain {domain_id}\n")

    let view_name = format!("gold.{}_text", domain_id.replace("-", "_"))

    // Build UNION ALL subqueries, one per text field
    let mut subqueries = Vec::new()
    for field in text_fields:
        let cast = if field.field_type == "jsonb":
            format!("{}::text", field.column_name)  // Cast JSONB to text for uniform view schema
        else:
            field.column_name.clone()

        subqueries.push(format!(
            "SELECT {ts} AS time, '{stream}' AS source_stream, \
             '{col}' AS field_name, {value_expr} AS value \
             FROM {table} WHERE {col} IS NOT NULL",
            ts = field.timestamp_column,
            stream = field.stream_id.replace("-", "_"),
            col = field.column_name,
            value_expr = cast,
            table = field.silver_table,
        ))

    let union_query = subqueries.join("\n    UNION ALL\n    ")

    let drop_clause = match action:
        Action::DropCreate => format!("DROP VIEW IF EXISTS {} CASCADE;\n", view_name)
        _ => String::new()

    let sql = format!(
        "{drop}CREATE OR REPLACE VIEW {view} AS\n\
         SELECT DISTINCT ON (source_stream, field_name)\n\
         \    t.time,\n\
         \    t.source_stream,\n\
         \    t.field_name,\n\
         \    t.value\n\
         FROM (\n\
         \    {union}\n\
         ) t\n\
         ORDER BY t.source_stream, t.field_name, t.time DESC;\n\n\
         COMMENT ON VIEW {view} IS 'Latest text field values for domain {domain} (dp-023, config-driven)';\n",
        drop = drop_clause,
        view = view_name,
        union = union_query,
        domain = domain_id,
    )

    return Ok(sql)
```

## Module Registration

### File: crates/ndp-lib/src/gold/generators/mod.rs

Add module declaration and re-export:

```rust
// Add to mod.rs:
pub mod text_view;
pub use text_view::TextViewGenerator;
```

### File: crates/ndp-lib/src/gold/mod.rs

Ensure text_view is accessible from the gold module's public API.

## CLI Integration

### File: tools/ndp-gold-ddl/src/lib.rs (or ndp-cli)

The Gold text view needs to be callable from deployment:

```
ndp gold text-view --domain indoor_air_quality
```

This invokes `TextViewGenerator::new(config_loader).generate("indoor_air_quality", action)` and outputs the SQL.

## Summary of Files

| File | Action | Description |
|------|--------|-------------|
| `crates/ndp-lib/src/gold/generators/text_view.rs` | Create | TextViewGenerator implementation |
| `crates/ndp-lib/src/gold/generators/mod.rs` | Modify | Add `pub mod text_view` and re-export |
| `crates/ndp-lib/src/gold/mod.rs` | Verify | Ensure generators module is re-exported |
| `tools/ndp-gold-ddl/src/lib.rs` | Modify | Add re-export of TextViewGenerator |
