//! Silver layer streaming transform.
//!
//! This module implements the core transformation logic from Bronze (RawDataPoint)
//! to Silver (SilverRecord) using SilverEtlConfig-driven field mappings.

use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use crate::config::{SilverEtlConfig, SilverFieldMapping, TimestampTransform, TransformConfig};
use crate::types::RawDataPoint;

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
// Helper Functions
// =============================================================================

fn extract_stream_id(source_id: &str) -> String {
    let source_types = ["Http", "Mqtt", "File"];
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
