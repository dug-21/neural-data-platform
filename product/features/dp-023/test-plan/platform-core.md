# dp-023: platform-core Test Plan

## Unit Tests: coerce_to_type()

**File**: `core/src/silver/transform.rs` (in existing `#[cfg(test)] mod tests`)

### Test: coerce_jsonb_object

```rust
// Input: Value::Object({"temperature": 72, "status": "ok"})
// Type: "jsonb"
// Expected: Ok(Value::Object({"temperature": 72, "status": "ok"}))
assert_eq!(
    coerce_to_type(&json!({"temperature": 72, "status": "ok"}), "jsonb", "test").unwrap(),
    json!({"temperature": 72, "status": "ok"})
);
```

### Test: coerce_jsonb_array

```rust
// Input: Value::Array([1, 2, 3])
// Type: "jsonb"
// Expected: Ok(Value::Array([1, 2, 3]))
assert_eq!(
    coerce_to_type(&json!([1, 2, 3]), "jsonb", "test").unwrap(),
    json!([1, 2, 3])
);
```

### Test: coerce_jsonb_string_valid

```rust
// Input: Value::String("{\"key\": \"val\"}")
// Type: "jsonb"
// Expected: Ok(Value::Object({"key": "val"})) -- parsed from string
assert_eq!(
    coerce_to_type(&json!("{\"key\": \"val\"}"), "jsonb", "test").unwrap(),
    json!({"key": "val"})
);
```

### Test: coerce_jsonb_string_invalid

```rust
// Input: Value::String("not json")
// Type: "jsonb"
// Expected: Err(TypeConversion)
assert!(coerce_to_type(&json!("not json"), "jsonb", "test").is_err());
```

### Test: coerce_jsonb_null

```rust
// Input: Value::Null
// Type: "jsonb"
// Expected: Ok(Value::Null)
assert_eq!(
    coerce_to_type(&Value::Null, "jsonb", "test").unwrap(),
    Value::Null
);
```

### Test: coerce_jsonb_number

```rust
// Input: Value::Number(42)
// Type: "jsonb"
// Expected: Ok(Value::Number(42))
assert_eq!(
    coerce_to_type(&json!(42), "jsonb", "test").unwrap(),
    json!(42)
);
```

### Test: coerce_jsonb_boolean

```rust
// Input: Value::Bool(true)
// Type: "jsonb"
// Expected: Ok(Value::Bool(true))
assert_eq!(
    coerce_to_type(&json!(true), "jsonb", "test").unwrap(),
    json!(true)
);
```

## Unit Tests: TimescaleOutput

**File**: `core/src/silver/outputs/timescale.rs` (in existing test module)

### Test: build_upsert_query_jsonb_cast

```rust
// Setup: SilverEtlConfig with a field_mapping of type "jsonb"
// Input: SilverRecord with a jsonb field
// Expected: Query contains "$N::jsonb" placeholder for the jsonb column
let (query, _) = output.build_upsert_query(&record, &config);
assert!(query.contains("::jsonb"), "JSONB column should have ::jsonb cast");
```

### Test: build_upsert_query_text_no_cast

```rust
// Setup: SilverEtlConfig with a field_mapping of type "text"
// Input: SilverRecord with a text field
// Expected: Query contains "$N" without cast for the text column
let (query, _) = output.build_upsert_query(&record, &config);
// Find the text column placeholder -- should NOT have ::
// (Verify by checking the placeholder for the text field specifically)
```

### Test: build_raw_query_text_value

```rust
// Input: template="INSERT INTO t (col) VALUES ($1)", params=["Partly Cloudy"]
// Expected: "INSERT INTO t (col) VALUES ('Partly Cloudy')"
let result = build_raw_query("INSERT INTO t (col) VALUES ($1)", &["Partly Cloudy".to_string()]);
assert_eq!(result, "INSERT INTO t (col) VALUES ('Partly Cloudy')");
```

### Test: build_raw_query_jsonb_value

```rust
// Input: template="INSERT INTO t (col) VALUES ($1::jsonb)", params=["{\"key\":\"val\"}"]
// Expected: "INSERT INTO t (col) VALUES ('{\"key\":\"val\"}'::jsonb)"
let result = build_raw_query(
    "INSERT INTO t (col) VALUES ($1::jsonb)",
    &["{\"key\":\"val\"}".to_string()]
);
assert!(result.contains("::jsonb"));
assert!(result.contains("'"));
```

### Test: build_raw_query_text_with_quotes

```rust
// Input: params=["It's partly cloudy"]
// Expected: SQL escaping: "INSERT INTO t (col) VALUES ('It''s partly cloudy')"
let result = build_raw_query(
    "INSERT INTO t (col) VALUES ($1)",
    &["It's partly cloudy".to_string()]
);
assert!(result.contains("It''s partly cloudy"));
```

## Regression Tests

### Test: existing_numeric_coercion_unchanged

Run the existing `test_coerce_to_type()` test at line 787 -- must still pass with no modifications to its assertions.

### Test: existing_transform_tests_pass

Run `cargo test -p platform-core` -- all 908 existing tests must pass.

## Summary

| Test | Type | AC Mapping | Priority |
|------|------|-----------|----------|
| coerce_jsonb_object | Unit | AC-04 | High |
| coerce_jsonb_array | Unit | AC-04 | High |
| coerce_jsonb_string_valid | Unit | AC-04 | High |
| coerce_jsonb_string_invalid | Unit | AC-04 | High |
| coerce_jsonb_null | Unit | AC-04 | Medium |
| coerce_jsonb_number | Unit | AC-04 | Medium |
| coerce_jsonb_boolean | Unit | AC-04 | Low |
| build_upsert_query_jsonb_cast | Unit | AC-03 | High |
| build_upsert_query_text_no_cast | Unit | AC-03 | Medium |
| build_raw_query_text_value | Unit | AC-05 | High |
| build_raw_query_jsonb_value | Unit | AC-05 | High |
| build_raw_query_text_with_quotes | Unit | AC-05 | Medium |
| existing_numeric_coercion_unchanged | Regression | AC-08 | Critical |
| existing_transform_tests_pass | Regression | AC-08 | Critical |
