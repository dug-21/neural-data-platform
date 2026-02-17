# dp-023: platform-core Pseudocode

## Component: core/src/silver/transform.rs

### Change: Add jsonb branch to coerce_to_type()

**File**: `core/src/silver/transform.rs`
**Location**: Line 584, within the `match column_type` block
**Insert after**: The `"boolean"` branch (line 653)

```rust
// Pseudocode for the new jsonb branch
fn coerce_to_type(value, column_type, field_name) -> Result<Value, TransformError>:
    match column_type:
        // ... existing branches for double_precision, integer, text, boolean ...

        "jsonb" =>
            match value:
                Value::Object(_) | Value::Array(_):
                    // Structured JSON -- pass through as-is
                    return Ok(value.clone())

                Value::String(s):
                    // Pre-serialized JSON string -- validate and parse
                    match serde_json::from_str(s):
                        Ok(parsed) => return Ok(parsed)
                        Err(_) => return Err(TypeConversion {
                            field: field_name,
                            expected: "jsonb (valid JSON string)",
                            actual: "invalid JSON string"
                        })

                Value::Null:
                    return Ok(Value::Null)

                Value::Number(_) | Value::Bool(_):
                    // JSON primitives are valid JSONB
                    return Ok(value.clone())

        // ... existing wildcard branch ...
```

**Rationale**: The wildcard `_ => Ok(value.clone())` currently handles jsonb implicitly, but an explicit branch provides validation for string inputs and documents the intent.

## Component: core/src/silver/outputs/timescale.rs

### Change 1: Type-aware placeholders in build_upsert_query()

**File**: `core/src/silver/outputs/timescale.rs`
**Location**: Line 196-202, where field_names are iterated to build placeholders

```rust
// Current code (line 196-202):
//   let field_names: Vec<String> = record.fields.keys().cloned().collect();
//   for name in &field_names {
//       columns.push(name.clone());
//       placeholders.push(format!("${}", param_index));
//       param_index += 1;
//   }

// New pseudocode:
fn build_upsert_query(self, record, etl_config) -> (String, Vec<String>):
    // ... existing columns/placeholders setup for timestamp, identity, valid_timestamp ...

    // 5. Data fields with type-aware placeholders
    let field_names = record.fields.keys().cloned().collect()
    for name in field_names:
        columns.push(name.clone())

        // Look up the column type from etl_config
        let col_type = etl_config.field_mappings.iter()
            .find(|m| m.target_column == name)
            .map(|m| m.column_type.as_str())
            .unwrap_or("text")

        // JSONB columns need explicit cast
        if col_type == "jsonb":
            placeholders.push(format!("${}::jsonb", param_index))
        else:
            placeholders.push(format!("${}", param_index))

        param_index += 1

    // ... rest of query building unchanged ...
```

**Note**: The `etl_config.field_mappings` is already available in scope (passed as parameter). The lookup is O(N*M) but N and M are small (5-15 fields).

### Change 2: No changes to build_raw_query()

`build_raw_query()` (line 451) substitutes `$N` with `'<value>'`. With the `::jsonb` cast in the template, the substitution produces `'{"key":"val"}'::jsonb` which is correct PostgreSQL syntax. No changes needed to `build_raw_query()`.

### Change 3: No changes to write()

The `write()` method (line 272) builds params by calling `value.to_string().trim_matches('"')` for each field. For text values, this produces the raw string. For jsonb values (Value::Object), `.to_string()` produces the JSON string `{"key":"val"}`, and `.trim_matches('"')` is a no-op because the string doesn't start/end with `"`. The param is correct.

**One edge case**: If `coerce_to_type("jsonb")` receives a `Value::String` containing pre-serialized JSON, it parses it into a `Value::Object`. Then `.to_string()` produces `{"key":"val"}` correctly.

## Summary of Changes

| File | Change | Lines Affected |
|------|--------|---------------|
| `core/src/silver/transform.rs` | Add `"jsonb"` match arm to `coerce_to_type()` | Insert ~15 lines after line 653 |
| `core/src/silver/outputs/timescale.rs` | Add type-aware placeholders in `build_upsert_query()` | Modify lines 196-202 (~10 lines changed) |
| `core/src/silver/outputs/timescale.rs` | No change to `build_raw_query()` or `write()` | 0 lines |
