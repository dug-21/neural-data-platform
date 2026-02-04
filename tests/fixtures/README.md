# Test Fixtures for Phase A Validation

This directory contains test fixtures for validating Phase A (Gold Layer Foundation) implementations.

## Directory Structure

```
tests/fixtures/
  configs/
    valid/              # Valid configuration examples that should pass validation
    invalid/            # Invalid configurations that should fail with specific errors
  expected-ddl/         # Expected SQL DDL output for DDL generators
```

## Valid Configuration Fixtures

| File | Description | Tests |
|------|-------------|-------|
| `minimal_gold_etl.json` | Minimal valid gold_etl section | Basic schema validation |
| `full_gold_etl.json` | Complete gold_etl with all features | Full feature validation |
| `disabled_gold_etl.json` | gold_etl.enabled = false | Disabled state |
| `minimal_domain.yaml` | Minimal domain config | Basic domain structure |
| `full_domain.yaml` | Complete domain with objectives/constraints | All domain features |
| `objectives_only.json` | Objectives section only | Objective validation |
| `all_join_strategies.yaml` | Examples of all join strategies | Join strategy enum |
| `all_condition_operators.json` | All valid condition operators | Condition enum |

## Invalid Configuration Fixtures

| File | Expected Error | Error Code |
|------|----------------|------------|
| `unknown_metric.json` | Invalid metric "average" | ENUM_VIOLATION |
| `bad_granularity.json` | Invalid format "hourly" | PATTERN_MISMATCH |
| `nonexistent_field.json` | Field not in stream | INVALID_GOLD_FIELD (400) |
| `domain_missing_primary.yaml` | No stream with role: primary | CONSTRAINT_VIOLATION |
| `invalid_join_strategy.yaml` | Invalid strategy "outer" | ENUM_VIOLATION |
| `invalid_condition.json` | Invalid condition "less_than" | ENUM_VIOLATION |
| `between_single_value.json` | Between needs array threshold | TYPE_MISMATCH |
| `duplicate_alias.yaml` | Duplicate stream alias | DUPLICATE_NAME |
| `missing_view_name.yaml` | Missing required view_name | MISSING_REQUIRED |
| `invalid_null_handling.yaml` | Invalid null_handling "drop" | ENUM_VIOLATION |
| `enabled_no_aggregates.json` | enabled=true without aggregates | MISSING_REQUIRED |
| `invalid_time_window.json` | Invalid time format "10pm" | PATTERN_MISMATCH |

## Expected DDL Fixtures

| File | Description | Generator |
|------|-------------|-----------|
| `air-quality-hourly.sql` | Hourly continuous aggregate | generate_continuous_aggregate |
| `air-quality-daily.sql` | Daily continuous aggregate | generate_continuous_aggregate |
| `indoor-air-quality-aligned.sql` | Aligned view with FULL OUTER JOIN | generate_aligned_view |
| `air-quality-lag-features.sql` | Lag feature views | generate_lag_features |
| `air-quality-rolling-features.sql` | Rolling statistics views | generate_rolling_features |
| `air-quality-trend-features.sql` | Trend feature views | generate_trend_features |

## Usage in Tests

### Schema Validation Tests

```rust
#[test]
fn test_valid_configs_pass() {
    let valid_dir = "tests/fixtures/configs/valid";
    for entry in std::fs::read_dir(valid_dir).unwrap() {
        let path = entry.unwrap().path();
        let result = validate_config(&path);
        assert!(result.is_ok(), "Valid config should pass: {:?}", path);
    }
}

#[test]
fn test_unknown_metric_fails() {
    let config = load_config("tests/fixtures/configs/invalid/unknown_metric.json");
    let result = validate_gold_etl_schema(&config);

    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert!(errors.iter().any(|e| e.code == ErrorCode::EnumViolation));
}
```

### DDL Generator Tests

```rust
#[test]
fn test_hourly_aggregate_ddl() {
    let config = load_stream_config("config/base/streams/air-quality/config.json");
    let expected = include_str!("../fixtures/expected-ddl/air-quality-hourly.sql");

    let generated = generate_continuous_aggregate(&config, "1 hour").unwrap();

    // Compare key elements (exact match may differ in whitespace)
    assert!(generated.contains("CREATE MATERIALIZED VIEW gold.air_quality_hourly"));
    assert!(generated.contains("time_bucket('1 hour'"));
    assert!(generated.contains("AVG(pm25) AS pm25_mean"));
}
```

## Adding New Fixtures

1. **Valid configs**: Add to `configs/valid/` with descriptive name
2. **Invalid configs**: Add to `configs/invalid/` with error type in name
3. **Expected DDL**: Add to `expected-ddl/` matching the generator output format

## Schema Files

The JSON schemas used for validation are located at:
- `config/schemas/gold-etl.schema.json`
- `config/schemas/alignment.schema.json`
- `config/schemas/objectives.schema.json`
- `config/schemas/domain.schema.json` (references alignment and objectives)

## Related Documentation

- `product/features/fe-001/phase-a/specification/` - Phase A specifications
- `product/features/fe-001/phase-a/refinement/TDD-GUIDE.md` - TDD guide
- `product/features/fe-001/phase-a/refinement/TEST-PLAN.md` - Test plan
