# BUG-001 Implementation Plan

## Overview

Fix the DP-007 pre-transform feature by:
1. Updating YAML config to match Rust structs
2. Wiring up the integration in `etl.rs`
3. Removing alternative/dead code paths
4. Updating tests
5. Correcting the AgentDB pattern

## Phase 1: Config Alignment

### 1.1 Update YAML Config

**File**: `config/base/streams/nws-gridpoints-forecast/config.yaml`

**Before** (lines 542-546):
```yaml
pre_transform:
  enabled: true
  parser_type: column_oriented
  parser_config_ref: sources[0].parser
```

**After**:
```yaml
pre_transform:
  transform_type:
    type: array_explosion
    metrics_base_path: properties
    timestamp_field: validTime
    value_field: value
    values_path: values
    metrics:
      # Temperature Suite
      - metric_path: temperature
        target_column: temperature
        type: double_precision
      - metric_path: dewpoint
        target_column: dewpoint
        type: double_precision
      - metric_path: apparentTemperature
        target_column: apparent_temperature
        type: double_precision
      - metric_path: heatIndex
        target_column: heat_index
        type: double_precision
      - metric_path: windChill
        target_column: wind_chill
        type: double_precision
      # Wind Suite
      - metric_path: windSpeed
        target_column: wind_speed
        type: double_precision
      - metric_path: windGust
        target_column: wind_gust
        type: double_precision
      - metric_path: windDirection
        target_column: wind_direction
        type: double_precision
      # Moisture Suite
      - metric_path: relativeHumidity
        target_column: relative_humidity
        type: double_precision
      - metric_path: skyCover
        target_column: sky_cover
        type: double_precision
      # Precipitation Suite
      - metric_path: probabilityOfPrecipitation
        target_column: precip_probability
        type: double_precision
      - metric_path: quantitativePrecipitation
        target_column: precip_amount
        type: double_precision
```

### 1.2 Verify Rust Structs Are Complete

**File**: `core/src/config/silver_etl.rs`

Existing structs are correct:
- `PreTransformConfig` with `transform_type: PreTransformType`
- `PreTransformType` enum with `ArrayExplosion(ArrayExplosionConfig)`
- `ArrayExplosionConfig` with all needed fields

**No changes needed** - structs are correct.

---

## Phase 2: Wire Up Integration

### 2.1 Add Pre-Transform Execution to EtlRunner

**File**: `apps/silver-etl/src/etl.rs`

**Location**: Lines 451-458 (currently has TODO)

**Changes**:

1. Add import at top:
```rust
use crate::pre_transform::{apply_pre_transform, build_parser_from_config, PreTransformError};
```

2. Replace TODO block with actual implementation:
```rust
// Apply pre-transform if enabled
if let Some(ref pre_transform_config) = config.pre_transform {
    info!(stream_id = %stream_id, "Applying pre-transform stage");

    // Extract raw data from Bronze Parquet
    let raw_data = self.extract_bronze_raw_data(stream_id, bronze_path, watermark_before)?;

    if !raw_data.raw_payloads.is_empty() {
        // Build parser from config and apply pre-transform
        let parser = build_parser_from_config(pre_transform_config)
            .map_err(|e| EtlError::Config(format!("Pre-transform parser error: {}", e)))?;

        apply_pre_transform(
            &self.conn,
            &parser,
            &raw_data.raw_payloads,
            &raw_data.timestamps,
            &raw_data.ndp_ids,
        ).map_err(|e| EtlError::SqlExecution(format!("Pre-transform failed: {}", e)))?;
    }
}

let use_pre_transform = config.pre_transform.is_some();
```

3. Add new method `extract_bronze_raw_data`:
```rust
/// Extract raw data from Bronze Parquet for pre-transformation
fn extract_bronze_raw_data(
    &self,
    stream_id: &str,
    bronze_path: &str,
    watermark: Option<DateTime<Utc>>,
) -> Result<BronzeRawData, EtlError> {
    let parquet_glob = format!("{}/{}/**/*.parquet", bronze_path, stream_id);

    // Build query with optional watermark filter
    let mut sql = format!(
        "SELECT timestamp, ndp_id, raw_payload FROM read_parquet('{}')",
        parquet_glob
    );

    if let Some(wm) = watermark {
        let wm_micros = wm.timestamp_micros();
        sql.push_str(&format!(" WHERE timestamp > {}", wm_micros));
    }

    let mut stmt = self.conn.prepare(&sql)?;
    let mut raw_data = BronzeRawData::default();

    let rows = stmt.query_map([], |row| {
        let ts: i64 = row.get(0)?;
        let ndp_id: Option<String> = row.get(1)?;
        let payload_str: String = row.get(2)?;
        Ok((ts, ndp_id, payload_str))
    })?;

    for row in rows {
        let (ts, ndp_id, payload_str) = row?;
        raw_data.timestamps.push(ts);
        raw_data.ndp_ids.push(ndp_id);
        raw_data.raw_payloads.push(
            serde_json::from_str(&payload_str).unwrap_or(serde_json::Value::Null)
        );
    }

    Ok(raw_data)
}
```

### 2.2 Add Parser Builder from Config

**File**: `apps/silver-etl/src/pre_transform.rs`

Add new function:
```rust
/// Build ColumnOrientedParser from PreTransformConfig
pub fn build_parser_from_config(
    config: &PreTransformConfig,
) -> Result<ColumnOrientedParser, PreTransformError> {
    match &config.transform_type {
        PreTransformType::ArrayExplosion(explosion_config) => {
            // Convert ArrayExplosionConfig to ColumnOrientedConfig
            let columns: Vec<ColumnMapping> = explosion_config
                .metrics
                .iter()
                .map(|m| ColumnMapping {
                    metric_path: m.metric_path.clone(),
                    field_name: m.target_column.clone(),
                    values_path: Some(explosion_config.values_path.clone()),
                    timestamp_path: Some(explosion_config.timestamp_field.clone()),
                    value_path: Some(explosion_config.value_field.clone()),
                })
                .collect();

            let column_config = ColumnOrientedConfig {
                metrics_base_path: explosion_config.metrics_base_path.clone(),
                columns,
                timestamp_format: TimestampFormat::Iso8601Duration,
                unit_conversions: HashMap::new(),
            };

            let parser_config = ParserConfig {
                parser_type: ParserType::ColumnOriented,
                location_id_field: "id".to_string(),
                default_location_id: Some("unknown".to_string()),
                skip_fields: vec![],
                field_mappings: None,
                default_tags: HashMap::new(),
                array_config: None,
                column_config: Some(column_config),
            };

            ColumnOrientedParser::from_config(parser_config)
                .map_err(|e| PreTransformError::Config(e.to_string()))
        }
    }
}
```

---

## Phase 3: Update SQL Generation

### 3.1 Update SQL Generator for Pre-Transform Source

**File**: `apps/silver-etl/src/sql_gen.rs`

The SQL generator already handles `use_pre_transform` flag in `etl.rs`.
Verify it correctly generates:
- `FROM pre_transformed` (not `FROM read_parquet`)
- Proper column references for pivoted data

### 3.2 Update Field Mapping for Pre-Transform Mode

When pre-transform is enabled, field mappings should reference the `metric_name`
and `value` columns from the flattened table, not JSON paths.

Add to `SqlGenerator`:
```rust
/// Generate select expression for pre-transform mode
///
/// In pre-transform mode, data is already flattened into (metric_name, value) rows.
/// This generates PIVOT-style aggregation to create typed columns.
pub fn generate_pivot_select(&self, config: &SilverEtlConfig) -> String {
    let mut exprs = Vec::new();

    // Add timestamps
    exprs.push("issue_time".to_string());
    exprs.push("valid_time".to_string());
    exprs.push("ndp_id".to_string());
    exprs.push("location_id".to_string());

    // Add pivoted metric columns
    if let Some(ref pre_transform) = config.pre_transform {
        if let PreTransformType::ArrayExplosion(ref explosion) = pre_transform.transform_type {
            for metric in &explosion.metrics {
                exprs.push(format!(
                    "MAX(CASE WHEN metric_name = '{}' THEN value END) AS {}",
                    metric.target_column, metric.target_column
                ));
            }
        }
    }

    exprs.join(",\n    ")
}
```

---

## Phase 4: Remove Dead Code / Alternative Approaches

### 4.1 Files to Clean Up

| File | Action |
|------|--------|
| YAML config `pre_transform` section | Replace with correct format |
| `etl.rs` TODO comment | Replace with implementation |

### 4.2 Remove Unused Code Paths

There is no dead code to remove - the `pre_transform.rs` implementation is
correct and will be used. The issue was it was never called.

---

## Phase 5: Test Updates

### 5.1 Unit Tests in `core/src/config/silver_etl.rs`

Existing tests (26-31) already test the correct format. Verify they pass:
- `test_parse_pre_transform_config_array_explosion`
- `test_parse_array_explosion_config_defaults`
- `test_parse_metric_explosion_mapping`
- `test_silver_etl_config_with_pre_transform`
- `test_silver_etl_config_without_pre_transform`
- `test_pre_transform_serialization_round_trip`

### 5.2 Integration Tests in `apps/silver-etl/src/etl.rs`

Existing tests (12-14) check pre-transform flag:
- `test_pre_transform_not_applied_when_disabled`
- `test_pre_transform_applied_when_enabled`
- `test_sql_uses_temp_table_after_pre_transform`

**Add new tests**:
```rust
#[test]
fn test_extract_bronze_raw_data() {
    // Test that raw data extraction works from Parquet
}

#[test]
fn test_pre_transform_end_to_end() {
    // Test complete flow: Bronze Parquet -> pre_transform -> flattened table
}

#[test]
fn test_pre_transform_with_watermark_filter() {
    // Test that watermark filtering works during extraction
}
```

### 5.3 Integration Tests in `apps/silver-etl/src/pre_transform.rs`

Existing tests are comprehensive. **Add**:
```rust
#[test]
fn test_build_parser_from_config() {
    // Test that build_parser_from_config correctly creates parser
}

#[test]
fn test_build_parser_from_config_with_all_metrics() {
    // Test with full NWS metrics list
}
```

### 5.4 Config Loading Test

**File**: `apps/silver-etl/src/config.rs`

**Add test**:
```rust
#[tokio::test]
async fn test_load_config_with_pre_transform() {
    // Test that pre_transform section deserializes correctly
    let config_content = r#"
stream_id: nws-gridpoints-forecast
silver_etl:
  enabled: true
  target_table: silver.weather_forecasts
  pre_transform:
    transform_type:
      type: array_explosion
      metrics_base_path: properties
      metrics:
        - metric_path: temperature
          target_column: temperature_c
          type: double_precision
  timestamp:
    source_field: timestamp
    target_field: issue_time
    transform: microseconds_to_timestamp
  field_mappings: []
"#;
    // ... test deserialization succeeds
}
```

---

## Phase 6: Update AgentDB Pattern

### 6.1 Delete Incorrect Pattern

```bash
agentdb skill delete "arch-pre-transform-silver-etl"
```

### 6.2 Create Correct Pattern

```bash
agentdb skill create "arch-pre-transform-silver-etl" \
  "Pre-Transform Integration Pattern - DP-007 (CORRECTED). Enables config-driven
pre-transformation of columnar array data in Silver ETL using tagged enum approach.
CONFIG FORMAT: pre_transform.transform_type with tagged enum (type: array_explosion).
ArrayExplosionConfig fields: metrics_base_path, timestamp_field (default: validTime),
value_field (default: value), values_path (default: values), metrics[] with metric_path,
target_column, type. INTEGRATION: etl.rs calls build_parser_from_config() then
apply_pre_transform() to populate pre_transformed temp table before SQL generation.
SQL uses FROM pre_transformed with PIVOT aggregation. EXTENSIBILITY: Add new
PreTransformType variants (JsonFlatten, ColumnOriented) for other data structures.
Tags: dp-007, silver, etl, pre-transform, array-explosion, tagged-enum."
```

---

## Phase 7: Verification Checklist

### 7.1 Unit Test Verification
- [ ] `cargo test -p neural-core config::silver_etl` - All 39 tests pass
- [ ] `cargo test -p silver-etl pre_transform` - All tests pass
- [ ] `cargo test -p silver-etl etl` - All tests pass
- [ ] `cargo test -p silver-etl config` - All tests pass

### 7.2 Integration Verification
- [ ] Config loads from YAML without errors
- [ ] Config loads from etcd without errors
- [ ] Pre-transform creates `pre_transformed` table with correct schema
- [ ] SQL generation uses `FROM pre_transformed`
- [ ] End-to-end: NWS data flows to Silver layer

### 7.3 Pattern Verification
- [ ] Old incorrect pattern deleted from AgentDB
- [ ] New correct pattern saved to AgentDB
- [ ] `agentdb skill search "pre-transform"` returns correct pattern

---

## Implementation Order

1. **Phase 6.1**: Delete incorrect AgentDB pattern (prevent future confusion)
2. **Phase 1.1**: Update YAML config (align with Rust structs)
3. **Phase 2.2**: Add `build_parser_from_config` to pre_transform.rs
4. **Phase 2.1**: Wire up integration in etl.rs
5. **Phase 5**: Run all tests, add new tests
6. **Phase 6.2**: Save correct pattern to AgentDB
7. **Phase 7**: Verification checklist

---

## Estimated Scope

| Phase | Files Changed | Lines Changed |
|-------|---------------|---------------|
| 1 | 1 | ~50 |
| 2 | 2 | ~100 |
| 3 | 1 | ~30 |
| 4 | 0 | 0 |
| 5 | 3 | ~150 |
| 6 | AgentDB | N/A |
| **Total** | **7 files** | **~330 lines** |
