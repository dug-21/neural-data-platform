# dp-023: ndp-validate Test Plan

## Validation Tests

### Test: validate_text_field_mapping

```bash
# Run ndp validate on NWS forecast config with text field_mappings
ndp validate --config config/base/streams/nws-forecast-hourly/config.json
# Expected: PASS (exit code 0)
```

### Test: validate_jsonb_field_mapping

```bash
# Create a test config with jsonb field_mapping
# Run ndp validate
# Expected: PASS (exit code 0)
```

### Test: validate_existing_configs_unchanged

```bash
# Run ndp validate on all existing stream configs
for config in config/base/streams/*/config.json; do
    ndp validate --config "$config"
done
# Expected: All PASS (same results as before dp-023)
```

### Test: validate_mixed_type_config

```bash
# Validate config with both numeric and text field_mappings in same silver_etl
ndp validate --config config/base/streams/nws-forecast-hourly/config.json
# Expected: PASS -- mixed types are valid
```

## Unit Tests (if schema changes needed)

### Test: schema_accepts_text_type

```rust
// If embedded schema is updated, test that "text" is accepted in field_mappings[].type
let config = json!({
    "silver_etl": {
        "field_mappings": [{
            "source_path": "forecast",
            "target_column": "forecast_text",
            "type": "text"
        }]
    }
});
let errors = validator.validate_value(&config);
assert!(errors.is_empty());
```

### Test: schema_accepts_jsonb_type

```rust
let config = json!({
    "silver_etl": {
        "field_mappings": [{
            "source_path": "metadata",
            "target_column": "metadata_json",
            "type": "jsonb"
        }]
    }
});
let errors = validator.validate_value(&config);
assert!(errors.is_empty());
```

## Summary

| Test | Type | AC Mapping | Priority |
|------|------|-----------|----------|
| validate_text_field_mapping | CLI | AC-01 | High |
| validate_jsonb_field_mapping | CLI | AC-02 | High |
| validate_existing_configs_unchanged | Regression | AC-08 | Critical |
| validate_mixed_type_config | CLI | AC-01, AC-02 | High |
| schema_accepts_text_type | Unit | AC-01 | High (if schema changes) |
| schema_accepts_jsonb_type | Unit | AC-02 | High (if schema changes) |
