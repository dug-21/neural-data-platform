# BUG-001 AgentDB Pattern Correction

## Issue

The AgentDB pattern `arch-pre-transform-silver-etl` contains **INCORRECT** information
that directly caused the DP-007 implementation bug.

## Current (WRONG) Pattern

Pattern name: `arch-pre-transform-silver-etl`

Wrong content in AgentDB:
```
CONFIG: pre_transform.enabled, pre_transform.parser_type (column_oriented),
pre_transform.metrics_base_path, pre_transform.columns[]
```

This does NOT match the Rust struct definitions and causes silent deserialization failure.

## Correct Pattern

### YAML Config Format (CORRECT)

```yaml
pre_transform:
  transform_type:
    type: array_explosion
    metrics_base_path: properties
    timestamp_field: validTime      # default
    value_field: value              # default
    values_path: values             # default
    metrics:
      - metric_path: temperature
        target_column: temperature_c
        type: double_precision
      - metric_path: windSpeed
        target_column: wind_speed_kmh
        type: double_precision
```

### Rust Struct Definition (Source of Truth)

```rust
// core/src/config/silver_etl.rs

pub struct PreTransformConfig {
    pub transform_type: PreTransformType,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum PreTransformType {
    ArrayExplosion(ArrayExplosionConfig),
}

pub struct ArrayExplosionConfig {
    pub metrics_base_path: String,
    #[serde(default = "default_valid_time")]
    pub timestamp_field: String,
    #[serde(default = "default_value")]
    pub value_field: String,
    #[serde(default = "default_values")]
    pub values_path: String,
    pub metrics: Vec<MetricExplosionMapping>,
}

pub struct MetricExplosionMapping {
    pub metric_path: String,
    pub target_column: String,
    #[serde(rename = "type")]
    pub column_type: String,
}
```

### Integration Flow

1. `SilverEtlConfig` loaded from YAML/etcd
2. `config.pre_transform` is `Option<PreTransformConfig>`
3. If `Some`, `etl.rs` calls:
   - `build_parser_from_config(&pre_transform_config)` → creates `ColumnOrientedParser`
   - `apply_pre_transform(conn, parser, payloads, timestamps, ndp_ids)` → populates temp table
4. SQL generator uses `FROM pre_transformed` instead of `FROM read_parquet(...)`

### Key Differences from Wrong Pattern

| Wrong Pattern | Correct Pattern |
|---------------|-----------------|
| `enabled: true` | No `enabled` field |
| `parser_type: column_oriented` | `transform_type.type: array_explosion` |
| `parser_config_ref: sources[0].parser` | Inline `metrics[]` array |
| `columns[]` | `metrics[]` |

## Pattern Update Commands

When AgentDB is working correctly, run:

```bash
# Delete old pattern
agentdb skill delete "arch-pre-transform-silver-etl"

# Create correct pattern
agentdb skill create "arch-pre-transform-silver-etl" \
  "Pre-Transform Pattern DP-007 CORRECTED. Config uses tagged enum: \
   pre_transform.transform_type.type=array_explosion with ArrayExplosionConfig \
   (metrics_base_path, metrics[]). Rust: PreTransformConfig has transform_type: \
   PreTransformType enum. Integration: etl.rs calls build_parser_from_config \
   then apply_pre_transform. SQL uses FROM pre_transformed. \
   WARNING: Do NOT use enabled/parser_type/parser_config_ref - causes silent \
   deserialization failure. Tags: dp-007, silver, etl, pre-transform."
```

## Record Reflexion Episode

To record this bug for future learning:

```bash
agentdb reflexion store "dp-007-bug-001" \
  "Investigated pre-transform config mismatch between YAML and Rust structs" \
  0.0 false \
  "Pattern arch-pre-transform-silver-etl documented WRONG config format. \
   YAML had enabled/parser_type/parser_config_ref but Rust expected \
   transform_type with tagged enum. Pattern needs UPDATE to correct format: \
   transform_type.type=array_explosion with metrics[] array."
```

## Verification

After pattern update, verify with:

```bash
agentdb skill search "pre-transform silver" 5
```

Should show the CORRECTED pattern with:
- `transform_type.type: array_explosion`
- `metrics[]` array
- WARNING about wrong format
