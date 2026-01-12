# BUG-001 Detailed File Changes

## Overview

This document specifies exact file changes needed to fix the DP-007 pre-transform
config schema mismatch.

---

## File 1: YAML Config

**Path**: `config/base/streams/nws-gridpoints-forecast/config.yaml`

**Action**: Replace lines 540-546

**Before**:
```yaml
  # Pre-transform configuration (DP-007)
  # Parses raw_payload through ColumnOrientedParser to flatten columnar arrays
  pre_transform:
    enabled: true
    parser_type: column_oriented
    # Reference to parser config in sources section
    parser_config_ref: sources[0].parser
```

**After**:
```yaml
  # Pre-transform configuration (DP-007)
  # Flattens columnar array data into individual rows before DuckDB processing
  # Uses ArrayExplosion transform type with typed enum config
  pre_transform:
    transform_type:
      type: array_explosion
      metrics_base_path: properties
      timestamp_field: validTime
      value_field: value
      values_path: values
      metrics:
        # Temperature Suite (NWS units: Celsius)
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
        # Wind Suite (NWS units: km/h for speed, degrees for direction)
        - metric_path: windSpeed
          target_column: wind_speed
          type: double_precision
        - metric_path: windGust
          target_column: wind_gust
          type: double_precision
        - metric_path: windDirection
          target_column: wind_direction
          type: double_precision
        # Moisture Suite (NWS units: percent)
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

**Note**: Also update the `field_mappings` section (lines 567+) to use the
simplified `source_path` values that match the `target_column` names from
the pre-transform metrics (since after pre-transform, the data is already
in a flat structure with `metric_name` and `value` columns).

---

## File 2: Pre-Transform Module

**Path**: `apps/silver-etl/src/pre_transform.rs`

**Action**: Add `build_parser_from_config` function after line 262

**Add**:
```rust
// =============================================================================
// Parser Factory from Config (DP-007 Integration)
// =============================================================================

/// Build a ColumnOrientedParser from PreTransformConfig
///
/// Converts the enum-based PreTransformConfig into a working parser instance.
/// Currently supports ArrayExplosion; future variants can be added to the match.
///
/// # Arguments
///
/// * `config` - PreTransformConfig from silver_etl config
///
/// # Returns
///
/// Configured ColumnOrientedParser on success.
///
/// # Errors
///
/// Returns error if parser cannot be created from config.
pub fn build_parser_from_config(
    config: &neural_core::config::PreTransformConfig,
) -> Result<ColumnOrientedParser, PreTransformError> {
    use neural_core::config::PreTransformType;

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
                unit_conversions: std::collections::HashMap::new(),
            };

            let parser_config = ParserConfig {
                parser_type: ParserType::ColumnOriented,
                location_id_field: "id".to_string(),
                default_location_id: Some("unknown".to_string()),
                skip_fields: vec![],
                field_mappings: None,
                default_tags: std::collections::HashMap::new(),
                array_config: None,
                column_config: Some(column_config),
            };

            ColumnOrientedParser::from_config(parser_config)
                .map_err(|e| PreTransformError::Config(e.to_string()))
        }
    }
}
```

**Add imports** at top of file:
```rust
use neural_core::parsers::{ColumnMapping, ParserType, TimestampFormat};
```

---

## File 3: ETL Runner

**Path**: `apps/silver-etl/src/etl.rs`

**Action 1**: Add import at top (around line 35)

```rust
use crate::pre_transform::{apply_pre_transform, build_parser_from_config};
```

**Action 2**: Replace lines 451-458 (TODO block)

**Before**:
```rust
        // Check if pre-transform is enabled
        // TODO (dp-007): When pre_transform.rs is implemented, call apply_pre_transform here
        // to populate the pre_transformed temp table before SQL generation
        let use_pre_transform = config.pre_transform.is_some();
        if use_pre_transform {
            info!(stream_id = %stream_id, "Pre-transform enabled - will use pre_transformed temp table");
            // Future: self.apply_pre_transform_if_needed(&config, stream_id, bronze_path)?;
        }
```

**After**:
```rust
        // Apply pre-transform if enabled (DP-007)
        let use_pre_transform = config.pre_transform.is_some();
        if let Some(ref pre_transform_config) = config.pre_transform {
            info!(stream_id = %stream_id, "Applying pre-transform stage");

            // Extract raw data from Bronze Parquet
            let raw_data = self.extract_bronze_raw_data(
                stream_id,
                bronze_path,
                watermark_before,
            )?;

            if !raw_data.raw_payloads.is_empty() {
                // Build parser from config
                let parser = build_parser_from_config(pre_transform_config)
                    .map_err(|e| EtlError::Config(format!("Pre-transform parser: {}", e)))?;

                // Apply pre-transform to populate temp table
                apply_pre_transform(
                    &self.conn,
                    &parser,
                    &raw_data.raw_payloads,
                    &raw_data.timestamps,
                    &raw_data.ndp_ids,
                ).map_err(|e| EtlError::SqlExecution(format!("Pre-transform: {}", e)))?;

                info!(
                    stream_id = %stream_id,
                    rows = raw_data.raw_payloads.len(),
                    "Pre-transform completed"
                );
            } else {
                debug!(stream_id = %stream_id, "No data to pre-transform");
            }
        }
```

**Action 3**: Add new method after `run_etl` (around line 530)

```rust
    /// Extract raw data from Bronze Parquet for pre-transformation
    ///
    /// Reads raw_payload, timestamp, and ndp_id from Bronze Parquet files
    /// for processing through the pre-transform stage.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Stream identifier
    /// * `bronze_path` - Path to Bronze data
    /// * `watermark` - Optional watermark for incremental filtering
    fn extract_bronze_raw_data(
        &self,
        stream_id: &str,
        bronze_path: &str,
        watermark: Option<DateTime<Utc>>,
    ) -> Result<BronzeRawData, EtlError> {
        let parquet_glob = format!("{}/{}/**/*.parquet", bronze_path, stream_id);

        debug!(
            stream_id = %stream_id,
            parquet_glob = %parquet_glob,
            "Extracting Bronze raw data for pre-transform"
        );

        // Build query with optional watermark filter
        let sql = if let Some(wm) = watermark {
            let wm_micros = wm.timestamp_micros();
            format!(
                "SELECT timestamp, ndp_id, raw_payload FROM read_parquet('{}') WHERE timestamp > {}",
                parquet_glob, wm_micros
            )
        } else {
            format!(
                "SELECT timestamp, ndp_id, raw_payload FROM read_parquet('{}')",
                parquet_glob
            )
        };

        let mut stmt = self.conn.prepare(&sql).map_err(|e| {
            EtlError::ParquetResolution {
                stream_id: stream_id.to_string(),
                message: e.to_string(),
            }
        })?;

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

        debug!(
            stream_id = %stream_id,
            rows = raw_data.timestamps.len(),
            "Extracted Bronze raw data"
        );

        Ok(raw_data)
    }
```

---

## File 4: ETL Lib Exports

**Path**: `apps/silver-etl/src/lib.rs`

**Action**: Update export (around line 46)

**Before**:
```rust
pub use pre_transform::{
    apply_pre_transform, build_parser, create_temp_table, get_pre_transformed_count,
    query_pre_transformed, PreTransformError, PreTransformResult, PreTransformedRow,
};
```

**After**:
```rust
pub use pre_transform::{
    apply_pre_transform, build_parser, build_parser_from_config, create_temp_table,
    get_pre_transformed_count, query_pre_transformed, PreTransformError, PreTransformResult,
    PreTransformedRow,
};
```

---

## File 5: No Changes Needed

**Path**: `core/src/config/silver_etl.rs`

The Rust struct definitions are already correct. The existing structs match
the target YAML format:

- `PreTransformConfig` with `transform_type: PreTransformType`
- `PreTransformType::ArrayExplosion(ArrayExplosionConfig)`
- `ArrayExplosionConfig` with all required fields

**No changes needed** - this is the source of truth.

---

## File 6: SQL Generator (Optional Enhancement)

**Path**: `apps/silver-etl/src/sql_gen.rs`

The current SQL generator handles `use_pre_transform` by changing the FROM clause.
For full integration, consider adding a pivot query helper, but this may not be
strictly necessary if field_mappings are updated correctly.

**Potential addition** (if pivot is needed):
```rust
/// Generate PIVOT-style SELECT for pre-transformed data
///
/// Converts (metric_name, value) rows into typed columns via MAX(CASE WHEN...)
pub fn generate_pivot_select(config: &SilverEtlConfig) -> String {
    // Implementation as shown in implementation plan
}
```

---

## Summary of Changes

| File | Action | Scope |
|------|--------|-------|
| `config/.../nws-gridpoints-forecast/config.yaml` | Replace pre_transform section | ~50 lines |
| `apps/silver-etl/src/pre_transform.rs` | Add `build_parser_from_config` | ~60 lines |
| `apps/silver-etl/src/etl.rs` | Wire up integration | ~80 lines |
| `apps/silver-etl/src/lib.rs` | Update exports | 1 line |
| `core/src/config/silver_etl.rs` | No changes | 0 lines |
| `apps/silver-etl/src/sql_gen.rs` | Optional pivot helper | ~30 lines |

**Total**: ~220 lines of changes across 4-5 files
