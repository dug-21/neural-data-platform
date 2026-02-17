# dp-023: ndp-lib Test Plan (Gold Text View Generator)

## Unit Tests: TextViewGenerator

**File**: `crates/ndp-lib/src/gold/generators/text_view.rs` (new test module)

### Test: generate_single_text_field

```rust
// Setup: Domain config with one stream having one text field_mapping
// Expected: VIEW SQL with single SELECT (no UNION ALL)
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("CREATE OR REPLACE VIEW gold.test_domain_text"));
assert!(sql.contains("DISTINCT ON (source_stream, field_name)"));
assert!(sql.contains("short_forecast AS value"));
assert!(sql.contains("WHERE short_forecast IS NOT NULL"));
```

### Test: generate_multiple_text_fields

```rust
// Setup: Domain config with one stream having two text fields
// Expected: VIEW SQL with UNION ALL of two SELECTs
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("UNION ALL"));
// Count occurrences of SELECT
let select_count = sql.matches("SELECT ").count();
assert_eq!(select_count, 3); // outer SELECT + 2 subquery SELECTs
```

### Test: generate_mixed_numeric_text_stream

```rust
// Setup: Stream with both numeric and text field_mappings
// Expected: Only text fields appear in the view (numeric fields excluded)
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(!sql.contains("temperature_f")); // numeric -- should NOT be in text view
assert!(sql.contains("short_forecast"));  // text -- should be in text view
```

### Test: generate_no_text_fields

```rust
// Setup: Domain config with streams having only numeric fields
// Expected: Comment-only output, no VIEW created
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("No text fields found"));
assert!(!sql.contains("CREATE"));
```

### Test: generate_jsonb_field_cast

```rust
// Setup: Domain config with a jsonb field_mapping
// Expected: jsonb column is cast to text in the SELECT
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("::text AS value"));
```

### Test: generate_multiple_streams

```rust
// Setup: Domain config with two streams, each having text fields
// Expected: UNION ALL includes subqueries from both streams
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("nws_forecast_hourly"));
assert!(sql.contains("other_stream"));
assert!(sql.contains("UNION ALL"));
```

### Test: generate_drop_create_action

```rust
// Setup: Action::DropCreate
// Expected: DROP VIEW IF EXISTS before CREATE
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("DROP VIEW IF EXISTS gold.test_domain_text CASCADE"));
```

### Test: generate_view_has_comment

```rust
// Expected: COMMENT ON VIEW for documentation
let sql = generator.generate("test_domain", Action::DropCreate)?;
assert!(sql.contains("COMMENT ON VIEW gold.test_domain_text"));
```

### Test: discover_text_fields_finds_correct_types

```rust
// Setup: Stream config with text, jsonb, varchar, text[], double_precision fields
// Expected: discover returns text, jsonb, varchar, text[] fields only
let fields = generator.discover_text_fields(&domain_config);
let types: Vec<&str> = fields.iter().map(|f| f.field_type.as_str()).collect();
assert!(types.contains(&"text"));
assert!(types.contains(&"jsonb"));
assert!(!types.contains(&"double_precision"));
```

## Regression Tests

### Test: existing_gold_generator_tests_pass

```bash
cargo test -p ndp-lib -- gold
```

All existing Gold generator tests (aligned view, continuous aggregate, events, classification, pgvector) must pass unchanged.

## Summary

| Test | Type | AC Mapping | Priority |
|------|------|-----------|----------|
| generate_single_text_field | Unit | AC-06, AC-07 | High |
| generate_multiple_text_fields | Unit | AC-06 | High |
| generate_mixed_numeric_text_stream | Unit | AC-07 | High |
| generate_no_text_fields | Unit | AC-07 | Medium |
| generate_jsonb_field_cast | Unit | AC-06 | Medium |
| generate_multiple_streams | Unit | AC-06 | Medium |
| generate_drop_create_action | Unit | AC-06 | Low |
| generate_view_has_comment | Unit | AC-06 | Low |
| discover_text_fields_finds_correct_types | Unit | AC-07 | High |
| existing_gold_generator_tests_pass | Regression | AC-08 | Critical |
