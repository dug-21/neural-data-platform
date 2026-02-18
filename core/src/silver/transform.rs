//! Silver layer streaming transform.
//!
//! This module implements the core transformation logic from Bronze (RawDataPoint)
//! to Silver (SilverRecord) using SilverEtlConfig-driven field mappings.
//!
//! Supports both simple transforms and pre-transform (array explosion) for
//! columnar data sources like NWS forecasts.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;
use std::collections::HashMap;

use crate::config::{
    PreTransformType, SilverEtlConfig, SilverFieldMapping, TimestampTransform, TransformConfig,
};
use crate::parsers::{ColumnMapping, ColumnOrientedConfig, ColumnOrientedParser, ParserConfig};
use crate::traits::TimeSeriesPoint;
use crate::types::RawDataPoint;
use crate::Parser;

use super::types::{DqResult, SilverRecord, TransformError};

// =============================================================================
// Main Transform Function
// =============================================================================

/// Transform a Bronze RawDataPoint to a Silver SilverRecord.
pub fn transform_to_silver(
    raw: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<SilverRecord, TransformError> {
    let stream_id = extract_stream_id(&raw.source_id);
    let timestamp = transform_timestamp(raw, &config.timestamp)?;
    let mut record = SilverRecord::new(stream_id, timestamp);

    if let Some(ref ndp_id) = raw.ndp_id {
        record = record.with_device_id(ndp_id.clone());
    }

    // Extract valid_timestamp if configured (e.g., for forecasts)
    // Column name comes from config at write time, not stored in record
    if let Some(ref valid_ts_config) = config.valid_timestamp {
        if let Ok(valid_ts) = extract_valid_timestamp(raw, valid_ts_config) {
            record = record.with_valid_timestamp(valid_ts);
        }
    }

    for identity_field in &config.identity_fields {
        if let Some(value) = extract_json_path(&identity_field.source, raw) {
            record
                .identity_fields
                .insert(identity_field.target.clone(), value);
        }
    }

    for mapping in &config.field_mappings {
        match apply_field_mapping(raw, mapping) {
            Ok(Some(value)) => {
                record.fields.insert(mapping.target_column.clone(), value);
            }
            Ok(None) => {
                if mapping.nullable {
                    record
                        .fields
                        .insert(mapping.target_column.clone(), Value::Null);
                }
            }
            Err(e) => {
                tracing::warn!(field = %mapping.target_column, error = %e, "Field mapping failed");
                if mapping.nullable {
                    record
                        .fields
                        .insert(mapping.target_column.clone(), Value::Null);
                }
            }
        }
    }

    record.dq_result = DqResult::passed();
    Ok(record)
}

// =============================================================================
// Pre-Transform Function (Array Explosion)
// =============================================================================

/// Transform a Bronze RawDataPoint to multiple Silver records using pre-transform.
///
/// This is used for columnar data sources (e.g., NWS forecasts) where a single
/// Bronze payload contains arrays of values for multiple timestamps.
///
/// Uses ColumnOrientedParser (like batch silver-etl) to explode arrays,
/// then pivots the narrow results into wide SilverRecords.
///
/// Returns Vec<SilverRecord> - one per valid_time with all metrics as fields.
pub fn transform_with_pre_transform(
    raw: &RawDataPoint,
    config: &SilverEtlConfig,
) -> Result<Vec<SilverRecord>, TransformError> {
    let pre_transform_config = config
        .pre_transform
        .as_ref()
        .ok_or_else(|| TransformError::ConfigError("pre_transform not configured".to_string()))?;

    // Build ColumnOrientedParser from config (matches batch silver-etl exactly)
    let parser = build_column_oriented_parser(pre_transform_config)?;

    // Parse payload -> narrow points (one per metric/validTime)
    let points = parser
        .parse(&raw.raw_payload, raw.timestamp)
        .map_err(|e| TransformError::ConfigError(format!("Parser error: {}", e)))?;

    if points.is_empty() {
        return Ok(vec![]);
    }

    // Pivot narrow points to wide SilverRecords (group by valid_time)
    let records = pivot_to_silver_records(raw, config, points)?;

    Ok(records)
}

/// Build ColumnOrientedParser from PreTransformConfig.
///
/// This matches the batch silver-etl `build_parser_from_config()` function exactly.
fn build_column_oriented_parser(
    config: &crate::config::PreTransformConfig,
) -> Result<ColumnOrientedParser, TransformError> {
    use crate::parsers::{ParserType, TimestampFormat};

    match &config.transform_type {
        PreTransformType::ArrayExplosion(explosion) => {
            // Convert MetricExplosionMapping to ColumnMapping
            let columns: Vec<ColumnMapping> = explosion
                .metrics
                .iter()
                .map(|m| ColumnMapping {
                    metric_path: m.metric_path.clone(),
                    field_name: m.target_column.clone(),
                    values_path: Some(explosion.values_path.clone()),
                    timestamp_path: Some(explosion.timestamp_field.clone()),
                    value_path: Some(explosion.value_field.clone()),
                })
                .collect();

            let column_config = ColumnOrientedConfig {
                metrics_base_path: explosion.metrics_base_path.clone(),
                columns,
                timestamp_format: TimestampFormat::Iso8601Duration,
                unit_conversions: HashMap::new(),
            };

            // Build ParserConfig for ColumnOrientedParser (matches batch silver-etl)
            let base_config = ParserConfig {
                parser_type: ParserType::ColumnOriented,
                location_id_field: "location".to_string(),
                default_location_id: Some("unknown".to_string()),
                skip_fields: vec![],
                field_mappings: None,
                default_tags: HashMap::new(),
                array_config: None,
                column_config: Some(column_config),
            };

            ColumnOrientedParser::from_config(base_config)
                .map_err(|e| TransformError::ConfigError(format!("Parser creation failed: {}", e)))
        }
    }
}

/// Pivot narrow TimeSeriesPoints to wide SilverRecords.
///
/// Groups points by valid_time, then creates one SilverRecord per valid_time
/// with all metrics as fields. This matches the PIVOT SQL in batch silver-etl.
///
/// Column name mapping (matches batch silver-etl):
/// 1. pre_transform.metrics[].target_column = metric_name in ColumnOrientedParser output
/// 2. field_mappings[].source_path matches metric_name
/// 3. field_mappings[].target_column = final Silver column name
fn pivot_to_silver_records(
    raw: &RawDataPoint,
    config: &SilverEtlConfig,
    points: Vec<TimeSeriesPoint>,
) -> Result<Vec<SilverRecord>, TransformError> {
    let stream_id = extract_stream_id(&raw.source_id);

    // Build metric_name -> target_column mapping from field_mappings
    // This mirrors how batch silver-etl's MetricMapping works
    let metric_to_column: HashMap<String, String> = config
        .field_mappings
        .iter()
        .map(|m| {
            // source_path might be like "raw_payload.temperature" or just "temperature"
            // Extract the final part as the metric name
            let metric_name = m
                .source_path
                .rsplit('.')
                .next()
                .unwrap_or(&m.source_path)
                .to_string();
            (metric_name, m.target_column.clone())
        })
        .collect();

    // Group points by valid_time
    // Key: valid_time as timestamp, Value: map of metric_name -> value
    let mut grouped: HashMap<i64, HashMap<String, f64>> = HashMap::new();

    for point in points {
        // Extract valid_time from tags (set by ColumnOrientedParser)
        let valid_time_ts: i64 = point
            .tags
            .get("forecast_valid_time")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Extract metric name from tags
        let metric_name = point
            .tags
            .get("metric")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        // Add to grouped map
        grouped
            .entry(valid_time_ts)
            .or_default()
            .insert(metric_name, point.value);
    }

    // Convert grouped data to SilverRecords
    let mut records = Vec::with_capacity(grouped.len());

    for (valid_time_ts, metrics) in grouped {
        // Create valid_time DateTime
        let valid_time = DateTime::from_timestamp(valid_time_ts, 0).ok_or_else(|| {
            TransformError::InvalidTimestamp {
                field: "valid_time".to_string(),
                value: valid_time_ts.to_string(),
                reason: "Invalid Unix timestamp".to_string(),
            }
        })?;

        // Create SilverRecord with issue_time as primary timestamp
        let mut record =
            SilverRecord::new(&stream_id, raw.timestamp).with_valid_timestamp(valid_time);

        // Set device_id/ndp_id
        if let Some(ref ndp_id) = raw.ndp_id {
            record = record.with_device_id(ndp_id.clone());
        }

        // Add all metrics as fields with column name mapping
        // metric_name (from pre-transform) -> target_column (from field_mappings)
        for (metric_name, value) in metrics {
            // Look up final column name from field_mappings
            // If no mapping found, use metric_name as-is (shouldn't happen with proper config)
            let column_name = metric_to_column
                .get(&metric_name)
                .cloned()
                .unwrap_or(metric_name);

            record.fields.insert(
                column_name,
                Value::Number(
                    serde_json::Number::from_f64(value)
                        .unwrap_or_else(|| serde_json::Number::from(0)),
                ),
            );
        }

        record.dq_result = DqResult::passed();
        records.push(record);
    }

    Ok(records)
}

// =============================================================================
// Helper Functions
// =============================================================================

pub fn extract_stream_id(source_id: &str) -> String {
    // Source types that can be suffixed to stream_id to form source_id
    // e.g., "outdoor-weather-HttpPoll" -> "outdoor-weather"
    let source_types = ["HttpPoll", "Http", "Mqtt", "File"];
    for suffix in source_types {
        if let Some(stripped) = source_id.strip_suffix(&format!("-{}", suffix)) {
            return stripped.to_string();
        }
    }
    source_id.to_string()
}

fn transform_timestamp(
    raw: &RawDataPoint,
    timestamp_config: &crate::config::TimestampMapping,
) -> Result<DateTime<Utc>, TransformError> {
    let ts_value = get_timestamp_value(raw, &timestamp_config.source_field)?;
    apply_timestamp_transform(
        &ts_value,
        &timestamp_config.transform,
        &timestamp_config.source_field,
    )
}

fn get_timestamp_value(raw: &RawDataPoint, source_field: &str) -> Result<Value, TransformError> {
    match source_field {
        "timestamp" => {
            let micros = raw.timestamp.timestamp_micros();
            Ok(Value::Number(serde_json::Number::from(micros)))
        }
        path => extract_json_path(path, raw).ok_or_else(|| TransformError::MissingField {
            field: "timestamp".to_string(),
            path: path.to_string(),
        }),
    }
}

fn apply_timestamp_transform(
    value: &Value,
    transform: &TimestampTransform,
    field_name: &str,
) -> Result<DateTime<Utc>, TransformError> {
    match transform {
        TimestampTransform::MicrosecondsToTimestamp => {
            let micros = value
                .as_i64()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Expected integer microseconds".to_string(),
                })?;
            Utc.timestamp_micros(micros)
                .single()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Invalid microseconds value".to_string(),
                })
        }
        TimestampTransform::UnixSeconds => {
            let secs = value
                .as_i64()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Expected integer seconds".to_string(),
                })?;
            Utc.timestamp_opt(secs, 0)
                .single()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Invalid Unix timestamp".to_string(),
                })
        }
        TimestampTransform::Iso8601 => {
            let s = value
                .as_str()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Expected ISO 8601 string".to_string(),
                })?;
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: s.to_string(),
                    reason: e.to_string(),
                })
        }
        TimestampTransform::NwsDuration => {
            let s = value
                .as_str()
                .ok_or_else(|| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: value.to_string(),
                    reason: "Expected NWS duration string".to_string(),
                })?;
            let timestamp_part = s.split('/').next().unwrap_or(s);
            DateTime::parse_from_rfc3339(timestamp_part)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| TransformError::InvalidTimestamp {
                    field: field_name.to_string(),
                    value: s.to_string(),
                    reason: format!("NWS duration parse error: {}", e),
                })
        }
    }
}

fn extract_valid_timestamp(
    raw: &RawDataPoint,
    config: &crate::config::ValidTimestampMapping,
) -> Result<DateTime<Utc>, TransformError> {
    use crate::config::ValidTimestampSource;

    let value = match &config.source {
        ValidTimestampSource::ArrayExplosion => {
            extract_json_path("_valid_time", raw).ok_or_else(|| TransformError::MissingField {
                field: config.target_field.clone(),
                path: "_valid_time (from array explosion)".to_string(),
            })?
        }
        ValidTimestampSource::Field(field_source) => extract_json_path(&field_source.path, raw)
            .ok_or_else(|| TransformError::MissingField {
                field: config.target_field.clone(),
                path: field_source.path.clone(),
            })?,
    };

    apply_timestamp_transform(&value, &config.transform, &config.target_field)
}

fn apply_field_mapping(
    raw: &RawDataPoint,
    mapping: &SilverFieldMapping,
) -> Result<Option<Value>, TransformError> {
    let value = match extract_json_path(&mapping.source_path, raw) {
        Some(v) => v,
        None => return Ok(None),
    };

    let transformed = if let Some(ref transform) = mapping.transform {
        apply_transform(&value, transform, &mapping.target_column)?
    } else {
        value
    };

    let coerced = coerce_to_type(&transformed, &mapping.column_type, &mapping.target_column)?;
    Ok(Some(coerced))
}

fn extract_json_path(path: &str, raw: &RawDataPoint) -> Option<Value> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "raw_payload" => {
            let mut current = &raw.raw_payload;
            for part in &parts[1..] {
                current = navigate_json_part(current, part)?;
            }
            Some(current.clone())
        }
        "context" => {
            let context = raw.context.as_ref()?;
            let mut current = context;
            for part in &parts[1..] {
                current = navigate_json_part(current, part)?;
            }
            Some(current.clone())
        }
        "ndp_id" => raw.ndp_id.clone().map(Value::String),
        "timestamp" => Some(Value::Number(serde_json::Number::from(
            raw.timestamp.timestamp_micros(),
        ))),
        "source_id" => Some(Value::String(raw.source_id.clone())),
        _ => raw.raw_payload.get(parts[0]).cloned(),
    }
}

/// Navigate a single path part, handling both field access and array indexing.
/// Supports: "field", "field[0]", "field[0][1]" (nested arrays)
fn navigate_json_part<'a>(current: &'a Value, part: &str) -> Option<&'a Value> {
    if let Some(bracket_start) = part.find('[') {
        // Has array index(es): "list[0]" or "list[0][1]"
        let field = &part[..bracket_start];
        let mut result = if field.is_empty() {
            current
        } else {
            current.get(field)?
        };

        // Extract all indices like [0], [1], etc.
        let indices_part = &part[bracket_start..];
        for cap in indices_part.split(']') {
            if let Some(idx_str) = cap.strip_prefix('[') {
                if !idx_str.is_empty() {
                    let idx: usize = idx_str.parse().ok()?;
                    result = result.get(idx)?;
                }
            }
        }
        Some(result)
    } else {
        // Simple field access
        current.get(part)
    }
}

fn apply_transform(
    value: &Value,
    transform: &TransformConfig,
    field_name: &str,
) -> Result<Value, TransformError> {
    match transform {
        TransformConfig::UnitConversion { formula, .. } => {
            let num = value
                .as_f64()
                .ok_or_else(|| TransformError::TypeConversion {
                    field: field_name.to_string(),
                    expected: "number".to_string(),
                    actual: type_name(value),
                })?;
            let converted = formula.apply(num);
            Ok(Value::Number(
                serde_json::Number::from_f64(converted)
                    .unwrap_or_else(|| serde_json::Number::from(0)),
            ))
        }
        TransformConfig::Expression { expression } => {
            if expression.contains("value") {
                if let Some(num) = value.as_f64() {
                    return Ok(Value::Number(
                        serde_json::Number::from_f64(num)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    ));
                }
            }
            Ok(value.clone())
        }
        TransformConfig::Lookup { table } => {
            let key = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return Ok(value.clone()),
            };
            Ok(table
                .get(&key)
                .map(|v| Value::String(v.clone()))
                .unwrap_or_else(|| value.clone()))
        }
        TransformConfig::JsonExtract { path } => {
            let parts: Vec<&str> = path.trim_start_matches("$.").split('.').collect();
            let mut current = value;
            for part in parts {
                if let Some(idx_start) = part.find('[') {
                    let field = &part[..idx_start];
                    let idx_str = &part[idx_start + 1..part.len() - 1];
                    current = current
                        .get(field)
                        .ok_or_else(|| TransformError::JsonPathError {
                            path: path.to_string(),
                            reason: format!("Field '{}' not found", field),
                        })?;
                    let idx: usize =
                        idx_str.parse().map_err(|_| TransformError::JsonPathError {
                            path: path.to_string(),
                            reason: format!("Invalid array index: {}", idx_str),
                        })?;
                    current = current
                        .get(idx)
                        .ok_or_else(|| TransformError::JsonPathError {
                            path: path.to_string(),
                            reason: format!("Array index {} out of bounds", idx),
                        })?;
                } else {
                    current = current
                        .get(part)
                        .ok_or_else(|| TransformError::JsonPathError {
                            path: path.to_string(),
                            reason: format!("Field '{}' not found", part),
                        })?;
                }
            }
            Ok(current.clone())
        }
        TransformConfig::Timestamp { format } => {
            let ts = apply_timestamp_transform(value, format, field_name)?;
            Ok(Value::String(ts.to_rfc3339()))
        }
        TransformConfig::Computed { expression, .. } => Err(TransformError::ExpressionError {
            field: field_name.to_string(),
            reason: format!("Computed fields not yet supported: {}", expression),
        }),
    }
}

fn coerce_to_type(
    value: &Value,
    column_type: &str,
    field_name: &str,
) -> Result<Value, TransformError> {
    match column_type {
        "double_precision" | "real" => match value {
            Value::Number(n) => Ok(Value::Number(n.clone())),
            Value::String(s) => s
                .parse::<f64>()
                .map(|f| {
                    Value::Number(
                        serde_json::Number::from_f64(f)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                })
                .map_err(|_| TransformError::TypeConversion {
                    field: field_name.to_string(),
                    expected: column_type.to_string(),
                    actual: "unparseable string".to_string(),
                }),
            Value::Null => Ok(Value::Null),
            _ => Err(TransformError::TypeConversion {
                field: field_name.to_string(),
                expected: column_type.to_string(),
                actual: type_name(value),
            }),
        },
        "integer" | "bigint" | "smallint" => match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(Value::Number(serde_json::Number::from(i)))
                } else if let Some(f) = n.as_f64() {
                    Ok(Value::Number(serde_json::Number::from(f as i64)))
                } else {
                    Ok(Value::Number(n.clone()))
                }
            }
            Value::String(s) => s
                .parse::<i64>()
                .map(|i| Value::Number(serde_json::Number::from(i)))
                .map_err(|_| TransformError::TypeConversion {
                    field: field_name.to_string(),
                    expected: column_type.to_string(),
                    actual: "unparseable string".to_string(),
                }),
            Value::Null => Ok(Value::Null),
            _ => Err(TransformError::TypeConversion {
                field: field_name.to_string(),
                expected: column_type.to_string(),
                actual: type_name(value),
            }),
        },
        "text" | "varchar" => match value {
            Value::String(s) => Ok(Value::String(s.clone())),
            Value::Number(n) => Ok(Value::String(n.to_string())),
            Value::Bool(b) => Ok(Value::String(b.to_string())),
            Value::Null => Ok(Value::Null),
            _ => Ok(Value::String(value.to_string())),
        },
        "boolean" => match value {
            Value::Bool(b) => Ok(Value::Bool(*b)),
            Value::String(s) => {
                let lower = s.to_lowercase();
                Ok(Value::Bool(
                    lower == "true" || lower == "1" || lower == "yes",
                ))
            }
            Value::Number(n) => Ok(Value::Bool(n.as_i64().map(|i| i != 0).unwrap_or(false))),
            Value::Null => Ok(Value::Null),
            _ => Err(TransformError::TypeConversion {
                field: field_name.to_string(),
                expected: "boolean".to_string(),
                actual: type_name(value),
            }),
        },
        "jsonb" => match value {
            Value::Object(_) | Value::Array(_) => Ok(value.clone()),
            Value::String(s) => {
                serde_json::from_str::<Value>(s).map_err(|_| TransformError::TypeConversion {
                    field: field_name.to_string(),
                    expected: "jsonb (valid JSON string)".to_string(),
                    actual: "invalid JSON string".to_string(),
                })
            }
            Value::Null => Ok(Value::Null),
            Value::Number(_) | Value::Bool(_) => Ok(value.clone()),
        },
        _ => Ok(value.clone()),
    }
}

fn type_name(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(_) => "boolean".to_string(),
        Value::Number(_) => "number".to_string(),
        Value::String(_) => "string".to_string(),
        Value::Array(_) => "array".to_string(),
        Value::Object(_) => "object".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        ConversionFormula, DeduplicationConfig, DqOutputConfig, IdentityField, IncrementalConfig,
        TimestampMapping,
    };
    use chrono::TimeZone;
    use serde_json::json;

    fn test_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![],
            field_mappings: vec![],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        }
    }

    fn test_raw_data_point() -> RawDataPoint {
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        RawDataPoint::new(
            "air-quality-Mqtt",
            json!({
                "pm02": 12.5,
                "rco2": 420,
                "atmp": 21.5,
            }),
        )
        .with_timestamp(ts)
        .with_ndp_id("aq_airgradient_1")
    }

    #[test]
    fn test_extract_stream_id() {
        assert_eq!(extract_stream_id("air-quality-Mqtt"), "air-quality");
        assert_eq!(extract_stream_id("outdoor-weather-Http"), "outdoor-weather");
        assert_eq!(
            extract_stream_id("outdoor-weather-HttpPoll"),
            "outdoor-weather"
        );
        assert_eq!(
            extract_stream_id("nws-observations-HttpPoll"),
            "nws-observations"
        );
    }

    #[test]
    fn test_transform_to_silver_basic() {
        let raw = test_raw_data_point();
        let mut config = test_config();
        config.field_mappings = vec![SilverFieldMapping {
            source_path: "raw_payload.pm02".to_string(),
            target_column: "pm25".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: None,
            dq_rules: vec![],
        }];

        let result = transform_to_silver(&raw, &config).unwrap();

        assert_eq!(result.stream_id, "air-quality");
        assert_eq!(result.device_id, Some("aq_airgradient_1".to_string()));
        assert_eq!(result.fields["pm25"].as_f64(), Some(12.5));
    }

    #[test]
    fn test_transform_with_unit_conversion() {
        let raw = RawDataPoint::new(
            "weather-Http",
            json!({
                "main": { "temp": 300.0 }
            }),
        )
        .with_timestamp(Utc::now());

        let mut config = test_config();
        config.field_mappings = vec![SilverFieldMapping {
            source_path: "raw_payload.main.temp".to_string(),
            target_column: "temperature_c".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::UnitConversion {
                from: "kelvin".to_string(),
                to: "celsius".to_string(),
                formula: ConversionFormula::Linear {
                    scale: 1.0,
                    offset: -273.15,
                },
            }),
            dq_rules: vec![],
        }];

        let result = transform_to_silver(&raw, &config).unwrap();
        let temp_c = result.fields["temperature_c"].as_f64().unwrap();
        assert!((temp_c - 26.85).abs() < 0.01);
    }

    #[test]
    fn test_coerce_to_type() {
        assert_eq!(
            coerce_to_type(&json!(42), "double_precision", "test")
                .unwrap()
                .as_f64(),
            Some(42.0)
        );
        assert_eq!(
            coerce_to_type(&json!(42.7), "integer", "test")
                .unwrap()
                .as_i64(),
            Some(42)
        );
        assert_eq!(
            coerce_to_type(&json!(42), "text", "test").unwrap().as_str(),
            Some("42")
        );
    }

    #[test]
    fn test_coerce_jsonb_object() {
        let input = json!({"temperature": 72, "status": "ok"});
        let result = coerce_to_type(&input, "jsonb", "test").unwrap();
        assert_eq!(result, json!({"temperature": 72, "status": "ok"}));
    }

    #[test]
    fn test_coerce_jsonb_array() {
        let input = json!([1, 2, 3]);
        let result = coerce_to_type(&input, "jsonb", "test").unwrap();
        assert_eq!(result, json!([1, 2, 3]));
    }

    #[test]
    fn test_coerce_jsonb_string_valid() {
        let input = json!("{\"key\": \"val\"}");
        let result = coerce_to_type(&input, "jsonb", "test").unwrap();
        assert_eq!(result, json!({"key": "val"}));
    }

    #[test]
    fn test_coerce_jsonb_string_invalid() {
        let input = json!("not json");
        let result = coerce_to_type(&input, "jsonb", "test");
        assert!(result.is_err());
    }

    #[test]
    fn test_coerce_jsonb_null() {
        let result = coerce_to_type(&Value::Null, "jsonb", "test").unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_coerce_jsonb_number() {
        let input = json!(42);
        let result = coerce_to_type(&input, "jsonb", "test").unwrap();
        assert_eq!(result, json!(42));
    }

    #[test]
    fn test_coerce_jsonb_boolean() {
        let input = json!(true);
        let result = coerce_to_type(&input, "jsonb", "test").unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn test_array_index_extraction() {
        // Test extraction with array indexing like outdoor-air-quality uses
        let raw = RawDataPoint::new(
            "outdoor-air-quality-Http",
            json!({
                "list": [{
                    "main": { "aqi": 2 },
                    "components": {
                        "pm2_5": 12.5,
                        "pm10": 25.0
                    }
                }]
            }),
        )
        .with_timestamp(Utc::now());

        let mut config = test_config();
        config.field_mappings = vec![
            SilverFieldMapping {
                source_path: "raw_payload.list[0].main.aqi".to_string(),
                target_column: "aqi".to_string(),
                column_type: "integer".to_string(),
                nullable: false,
                transform: None,
                dq_rules: vec![],
            },
            SilverFieldMapping {
                source_path: "raw_payload.list[0].components.pm2_5".to_string(),
                target_column: "pm25".to_string(),
                column_type: "double_precision".to_string(),
                nullable: false,
                transform: None,
                dq_rules: vec![],
            },
        ];

        let result = transform_to_silver(&raw, &config).unwrap();

        assert_eq!(result.fields["aqi"].as_i64(), Some(2));
        assert_eq!(result.fields["pm25"].as_f64(), Some(12.5));
    }
}
