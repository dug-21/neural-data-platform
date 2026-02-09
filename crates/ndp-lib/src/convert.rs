//! Conversion from config types to dictionary sync types.
//!
//! Bridges `config::StreamConfig` (deserialized from JSON) to
//! `dictionary::types::StreamDictionaryEntry` (consumed by `sync_dictionary`).

use crate::config::{
    BronzeField, DomainConfig, SilverEtlConfig, SilverFieldMapping, SourceConfig, StreamConfig,
};
use crate::dictionary::types::*;
use crate::domain::types::*;

/// Convert a `StreamConfig` to a `StreamDictionaryEntry`.
///
/// This is the missing link between config loading and dictionary sync.
pub fn stream_config_to_dictionary_entry(config: &StreamConfig) -> StreamDictionaryEntry {
    StreamDictionaryEntry {
        stream_id: config.stream_id.clone(),
        description: if config.description.is_empty() {
            None
        } else {
            Some(config.description.clone())
        },
        version: config.version.clone(),
        enabled: config.enabled,
        retention_days: config.retention_days.unwrap_or(90),
        fields: config.fields.iter().map(convert_field).collect(),
        sources: config.sources.iter().map(convert_source).collect(),
        entity_schemas: convert_entity_schemas(&config.entity_schemas),
        silver_etl: config.silver_etl.as_ref().and_then(convert_silver_etl),
    }
}

fn convert_field(f: &BronzeField) -> FieldEntry {
    let (validation_min, validation_max) = extract_range(&f.range);
    FieldEntry {
        name: f.name.clone(),
        field_type: f.field_type.clone(),
        nullable: f.nullable,
        unit: f.unit.clone(),
        description: f.description.clone(),
        validation_min,
        validation_max,
    }
}

/// Extract min/max from the `range` field: `[min, max]`.
fn extract_range(range: &Option<Vec<serde_json::Value>>) -> (Option<f64>, Option<f64>) {
    match range {
        Some(v) if v.len() >= 2 => {
            let min = v[0].as_f64();
            let max = v[1].as_f64();
            (min, max)
        }
        _ => (None, None),
    }
}

fn convert_source(s: &SourceConfig) -> SourceEntry {
    // Build a source_id: prefer ndp_id, fall back to source_type
    let source_id = s.ndp_id.clone().unwrap_or_else(|| s.source_type.clone());

    // Parser type from the nested parser object
    let parser_type = s
        .parser
        .as_ref()
        .and_then(|p| p.get("parser_type"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Config blob: merge the extra fields into a JSON object
    let config = serde_json::Value::Object(s.extra.clone());

    SourceEntry {
        source_id,
        source_type: s.source_type.clone(),
        enabled: s.enabled,
        config,
        parser_type,
    }
}

/// Convert entity_schemas from raw JSON Value to typed Vec.
///
/// Entity schemas in the config are optional and can have varied shapes.
/// We do best-effort extraction; if the JSON doesn't match, we skip.
fn convert_entity_schemas(value: &Option<serde_json::Value>) -> Vec<EntitySchemaEntry> {
    let schemas = match value {
        Some(serde_json::Value::Array(arr)) => arr,
        Some(serde_json::Value::Object(map)) => {
            // Some configs use {"schema_name": {...}} object format
            return map
                .iter()
                .map(|(name, v)| {
                    let description = v
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());
                    let device_class = v
                        .get("device_class")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());
                    let attributes = v
                        .get("attributes")
                        .and_then(|a| a.as_array())
                        .map(|arr| arr.iter().filter_map(parse_attribute).collect())
                        .unwrap_or_default();
                    EntitySchemaEntry {
                        schema_name: name.clone(),
                        description,
                        device_class,
                        attributes,
                    }
                })
                .collect();
        }
        _ => return Vec::new(),
    };

    schemas
        .iter()
        .filter_map(|v| {
            let schema_name = v.get("schema_name")?.as_str()?.to_string();
            let description = v
                .get("description")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let device_class = v
                .get("device_class")
                .and_then(|d| d.as_str())
                .map(|s| s.to_string());
            let attributes = v
                .get("attributes")
                .and_then(|a| a.as_array())
                .map(|arr| arr.iter().filter_map(parse_attribute).collect())
                .unwrap_or_default();
            Some(EntitySchemaEntry {
                schema_name,
                description,
                device_class,
                attributes,
            })
        })
        .collect()
}

fn parse_attribute(v: &serde_json::Value) -> Option<EntitySchemaAttribute> {
    let name = v.get("name")?.as_str()?.to_string();
    let attribute_type = v
        .get("type")
        .or_else(|| v.get("attribute_type"))
        .and_then(|t| t.as_str())
        .unwrap_or("text")
        .to_string();
    Some(EntitySchemaAttribute {
        name,
        attribute_type,
        unit: v
            .get("unit")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string()),
        description: v
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string()),
        nullable: v.get("nullable").and_then(|n| n.as_bool()).unwrap_or(true),
        range_min: v
            .get("range")
            .and_then(|r| r.get(0))
            .and_then(|v| v.as_f64()),
        range_max: v
            .get("range")
            .and_then(|r| r.get(1))
            .and_then(|v| v.as_f64()),
    })
}

/// Convert SilverEtlConfig to SilverEtlEntry.
///
/// Returns None if target_table is missing (invalid config).
fn convert_silver_etl(etl: &SilverEtlConfig) -> Option<SilverEtlEntry> {
    let target_table = etl.target_table.as_ref()?;

    let timestamp_column = etl
        .timestamp
        .as_ref()
        .and_then(|t| t.target_field.clone())
        .unwrap_or_else(|| "observation_time".to_string());

    let field_mappings: Vec<crate::dictionary::types::SilverFieldMapping> = etl
        .field_mappings
        .iter()
        .filter_map(convert_silver_field_mapping)
        .collect();

    let dq_rules: Vec<SilverTableDqRule> = etl
        .dq_rules
        .iter()
        .filter_map(convert_table_dq_rule)
        .collect();

    Some(SilverEtlEntry {
        enabled: etl.enabled,
        target_table: target_table.clone(),
        description: etl.description.clone(),
        grain: etl.grain.clone(),
        timestamp_column,
        field_mappings,
        dq_rules,
    })
}

fn convert_silver_field_mapping(
    m: &SilverFieldMapping,
) -> Option<crate::dictionary::types::SilverFieldMapping> {
    let source_path = m.source_path.clone().unwrap_or_default();
    let target_column = m.target_column.clone()?;
    let data_type = m.column_type.clone().unwrap_or_else(|| "text".to_string());

    // Determine transform type from the transform field
    let transform_type = m.transform.as_ref().and_then(|t| {
        if let Some(s) = t.as_str() {
            Some(s.to_string())
        } else {
            t.get("type")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
    });

    let dq_rules: Vec<SilverColumnDqRule> = m
        .dq_rules
        .iter()
        .filter_map(convert_column_dq_rule)
        .collect();

    Some(crate::dictionary::types::SilverFieldMapping {
        source_path,
        target_column,
        data_type,
        unit: m.unit.clone(),
        description: m.description.clone(),
        nullable: m.nullable.unwrap_or(true),
        transform_type,
        dq_rules,
    })
}

/// Convert a column-level DQ rule from JSON.
///
/// Config format: `{"rule": "range_check", "min": 0, "max": 1000, "action": "flag"}`
fn convert_column_dq_rule(v: &serde_json::Value) -> Option<SilverColumnDqRule> {
    let rule_name = v.get("rule")?.as_str()?.to_string();
    let action = v
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("flag")
        .to_string();

    // Build params: everything except "rule" and "action"
    let mut params = serde_json::Map::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if k != "rule" && k != "action" {
                params.insert(k.clone(), val.clone());
            }
        }
    }

    Some(SilverColumnDqRule {
        rule_name,
        params: serde_json::Value::Object(params),
        action,
    })
}

/// Convert a table-level DQ rule from JSON.
///
/// Config format varies by rule type:
/// - cross_field_check: `{"rule":"cross_field_check","name":"...","expression":"...","action":"flag"}`
/// - freshness_check: `{"rule":"freshness_check","field":"...","max_age":"...","action":"flag"}`
/// - rate_of_change: `{"rule":"rate_of_change","field":"...","max_change_per_minute":100,"action":"flag"}`
fn convert_table_dq_rule(v: &serde_json::Value) -> Option<SilverTableDqRule> {
    let rule_type = v.get("rule")?.as_str()?.to_string();

    // Derive rule_name: use explicit "name" field, or "{rule_type}_{field}"
    let rule_name = v
        .get("name")
        .and_then(|n| n.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            let field = v.get("field").and_then(|f| f.as_str()).unwrap_or("table");
            format!("{}_{}", rule_type, field)
        });

    let action = v
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("flag")
        .to_string();

    // Build params: everything except "rule", "action", "name"
    let mut params = serde_json::Map::new();
    if let Some(obj) = v.as_object() {
        for (k, val) in obj {
            if k != "rule" && k != "action" && k != "name" {
                params.insert(k.clone(), val.clone());
            }
        }
    }

    Some(SilverTableDqRule {
        rule_type,
        rule_name,
        params: serde_json::Value::Object(params),
        action,
    })
}

// ---------------------------------------------------------------------------
// DomainConfig -> DomainSyncEntry conversion
// ---------------------------------------------------------------------------

/// Convert a `DomainConfig` (parsed from domain.json) to a `DomainSyncEntry` (DB-ready).
pub fn domain_config_to_sync_entry(config: &DomainConfig) -> DomainSyncEntry {
    let config_path = format!("config/domains/{}/domain.json", config.id);

    let streams: Vec<StreamMappingEntry> = config
        .streams
        .iter()
        .map(|s| StreamMappingEntry {
            stream_id: s.stream_id.clone(),
            alias: s.alias.clone(),
            role: s.role.clone(),
        })
        .collect();

    let objectives: Vec<ObjectiveSyncEntry> = config
        .objectives
        .iter()
        .map(|o| ObjectiveSyncEntry {
            objective_id: o.id.clone(),
            description: o.description.clone(),
            target_stream: o.target.stream.clone(),
            target_metric: o.target.metric.clone(),
            condition: o.target.condition.clone(),
            threshold: o.target.threshold,
            threshold_upper: o.target.threshold_upper,
            unit: o.target.unit.clone(),
            priority: o.priority.clone(),
        })
        .collect();

    let constraints: Vec<ConstraintSyncEntry> = config
        .constraints
        .iter()
        .map(|c| ConstraintSyncEntry {
            constraint_id: c.id.clone(),
            description: c.description.clone(),
            constraint_stream: c.stream.clone(),
            constraint_metric: c.metric.clone(),
            condition: c.condition.clone(),
            threshold: c.threshold,
            unit: c.unit.clone(),
        })
        .collect();

    DomainSyncEntry {
        domain_id: config.id.clone(),
        description: config.description.clone(),
        stream_count: config.streams.len() as i32,
        config_path,
        streams,
        objectives,
        constraints,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Load the real air-quality config and convert it.
    #[test]
    fn test_convert_air_quality_config() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        assert_eq!(entry.stream_id, "air-quality");
        assert!(entry.enabled);
        assert_eq!(entry.retention_days, 365);
        assert_eq!(entry.version, "1.1.0");
        assert!(entry.description.is_some());
    }

    #[test]
    fn test_convert_fields_with_range() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        // air-quality has 28 fields
        assert!(!entry.fields.is_empty());

        // pm02 has range [0, 1000]
        let pm02 = entry.fields.iter().find(|f| f.name == "pm02").unwrap();
        assert_eq!(pm02.field_type, "float");
        assert_eq!(pm02.validation_min, Some(0.0));
        assert_eq!(pm02.validation_max, Some(1000.0));
        assert!(!pm02.nullable);
    }

    #[test]
    fn test_convert_sources() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        assert_eq!(entry.sources.len(), 1);
        let source = &entry.sources[0];
        assert_eq!(source.source_id, "aq_airgradient_1");
        assert_eq!(source.source_type, "mqtt");
        assert!(source.enabled);
        assert_eq!(source.parser_type.as_deref(), Some("flat_json"));
    }

    #[test]
    fn test_convert_silver_etl() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        let etl = entry.silver_etl.as_ref().unwrap();
        assert!(etl.enabled);
        assert_eq!(etl.target_table, "silver.air_quality_observations");
        assert_eq!(etl.timestamp_column, "observation_time");

        // 7 field mappings
        assert_eq!(etl.field_mappings.len(), 7);

        let pm25 = etl
            .field_mappings
            .iter()
            .find(|m| m.target_column == "pm25")
            .unwrap();
        assert_eq!(pm25.source_path, "raw_payload.pm02Compensated");
        assert_eq!(pm25.data_type, "double_precision");
        assert!(!pm25.nullable);
    }

    #[test]
    fn test_convert_column_dq_rules() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        let etl = entry.silver_etl.as_ref().unwrap();
        let pm25 = etl
            .field_mappings
            .iter()
            .find(|m| m.target_column == "pm25")
            .unwrap();

        assert_eq!(pm25.dq_rules.len(), 1);
        assert_eq!(pm25.dq_rules[0].rule_name, "range_check");
        assert_eq!(pm25.dq_rules[0].action, "flag");
        assert_eq!(pm25.dq_rules[0].params["min"], 0);
        assert_eq!(pm25.dq_rules[0].params["max"], 1000);
    }

    #[test]
    fn test_convert_table_dq_rules() {
        let content = include_str!("../../../config/base/streams/air-quality/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        let etl = entry.silver_etl.as_ref().unwrap();
        // air-quality has 5 table-level DQ rules
        assert_eq!(etl.dq_rules.len(), 5);

        let cross_field = etl
            .dq_rules
            .iter()
            .find(|r| r.rule_type == "cross_field_check")
            .unwrap();
        assert_eq!(cross_field.rule_name, "pm10_gte_pm25");
        assert_eq!(cross_field.action, "flag");

        let freshness = etl
            .dq_rules
            .iter()
            .find(|r| r.rule_type == "freshness_check")
            .unwrap();
        assert_eq!(freshness.rule_name, "freshness_check_observation_time");
    }

    #[test]
    fn test_convert_outdoor_weather() {
        let content = include_str!("../../../config/base/streams/outdoor-weather/config.json");
        let config: StreamConfig = serde_json::from_str(content).unwrap();
        let entry = stream_config_to_dictionary_entry(&config);

        assert_eq!(entry.stream_id, "outdoor-weather");
        assert!(!entry.sources.is_empty());
        assert_eq!(entry.sources[0].source_type, "http_poll");

        let etl = entry.silver_etl.as_ref().unwrap();
        assert_eq!(etl.target_table, "silver.weather_observations");
    }

    #[test]
    fn test_convert_no_silver_etl() {
        let config = StreamConfig {
            stream_id: "test".to_string(),
            description: String::new(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: None,
            fields: vec![],
            sources: vec![],
            silver_etl: None,
            entity_schemas: None,
        };
        let entry = stream_config_to_dictionary_entry(&config);
        assert!(entry.silver_etl.is_none());
        assert_eq!(entry.retention_days, 90); // default
        assert!(entry.description.is_none()); // empty string -> None
    }

    // -----------------------------------------------------------------------
    // DomainConfig -> DomainSyncEntry conversion tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_convert_real_domain_config() {
        let content = include_str!("../../../config/domains/indoor-air-quality/domain.json");
        let config: crate::config::DomainConfig = serde_json::from_str(content).unwrap();
        let entry = domain_config_to_sync_entry(&config);

        assert_eq!(entry.domain_id, "indoor-air-quality");
        assert_eq!(
            entry.description.as_deref(),
            Some("Maintain healthy indoor air quality")
        );
        assert_eq!(entry.stream_count, 4);
        assert_eq!(
            entry.config_path,
            "config/domains/indoor-air-quality/domain.json"
        );
        assert_eq!(entry.objectives.len(), 6);
        assert!(entry.constraints.is_empty());
    }

    #[test]
    fn test_convert_objective_fields_flattened() {
        let content = include_str!("../../../config/domains/indoor-air-quality/domain.json");
        let config: crate::config::DomainConfig = serde_json::from_str(content).unwrap();
        let entry = domain_config_to_sync_entry(&config);

        // First objective: healthy_co2
        let obj = &entry.objectives[0];
        assert_eq!(obj.objective_id, "healthy_co2");
        assert_eq!(obj.target_stream, "air-quality");
        assert_eq!(obj.target_metric, "co2");
        assert_eq!(obj.condition, "<");
        assert_eq!(obj.threshold, 800.0);
        assert_eq!(obj.unit.as_deref(), Some("ppm"));
        assert_eq!(obj.priority, "high");
        assert!(obj.threshold_upper.is_none());
    }

    #[test]
    fn test_convert_stream_mappings() {
        let content = include_str!("../../../config/domains/indoor-air-quality/domain.json");
        let config: crate::config::DomainConfig = serde_json::from_str(content).unwrap();
        let entry = domain_config_to_sync_entry(&config);

        assert_eq!(entry.streams.len(), 4);

        // Verify each stream mapping
        assert_eq!(entry.streams[0].stream_id, "air-quality");
        assert_eq!(entry.streams[0].alias, "indoor");
        assert_eq!(entry.streams[0].role, "primary");

        assert_eq!(entry.streams[1].stream_id, "outdoor-weather");
        assert_eq!(entry.streams[1].alias, "outdoor");
        assert_eq!(entry.streams[1].role, "context");

        assert_eq!(entry.streams[2].stream_id, "home-assistant-state");
        assert_eq!(entry.streams[2].alias, "state");
        assert_eq!(entry.streams[2].role, "actuator");

        assert_eq!(entry.streams[3].stream_id, "outdoor-air-quality");
        assert_eq!(entry.streams[3].alias, "outdoor_aqi");
        assert_eq!(entry.streams[3].role, "constraint");
    }
}
