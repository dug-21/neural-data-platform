/// TDD Tests for SchemaValidator - Written FIRST before implementation
/// London TDD style

#[cfg(test)]
mod schema_validator_tests {
    use super::super::schema::*;
    use crate::types::{ConfigValue, ConfigError};
    use std::collections::HashMap;
    use serde_json::json;
    
    fn create_test_schema() -> String {
        json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "required": ["version", "service", "configuration"],
            "properties": {
                "version": {
                    "type": "string",
                    "pattern": "^\\d+\\.\\d+\\.\\d+$"
                },
                "service": {
                    "type": "string",
                    "enum": ["neural-trading", "data-ingestion", "config-store"]
                },
                "configuration": {
                    "type": "object",
                    "properties": {
                        "port": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 65535
                        },
                        "host": {
                            "type": "string"
                        },
                        "enabled": {
                            "type": "boolean"
                        },
                        "limits": {
                            "type": "object",
                            "properties": {
                                "max_connections": {
                                    "type": "integer",
                                    "minimum": 1
                                },
                                "timeout": {
                                    "type": "number",
                                    "minimum": 0
                                }
                            }
                        }
                    }
                }
            }
        }).to_string()
    }
    
    #[test]
    fn test_validator_creation() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema.clone());
        
        assert!(validator.is_valid_schema());
        assert_eq!(validator.schema_string(), &schema);
    }
    
    #[test]
    fn test_invalid_schema_detection() {
        let invalid_schema = "{ not valid json schema }";
        let validator = SchemaValidator::new(invalid_schema.to_string());
        
        assert!(!validator.is_valid_schema());
    }
    
    #[tokio::test]
    async fn test_validate_valid_config() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create valid config
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let mut configuration = HashMap::new();
        configuration.insert("port".to_string(), ConfigValue::Integer(8080));
        configuration.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
        configuration.insert("enabled".to_string(), ConfigValue::Boolean(true));
        
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_validate_missing_required_field() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create config missing required "version" field
        let mut config_map = HashMap::new();
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let configuration = HashMap::new();
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            let error_str = errors.join(" ");
            assert!(error_str.contains("version") || error_str.contains("required"));
        } else {
            panic!("Expected ValidationFailed error");
        }
    }
    
    #[tokio::test]
    async fn test_validate_invalid_pattern() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create config with invalid version pattern
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("invalid_version".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let configuration = HashMap::new();
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            let error_str = errors.join(" ");
            assert!(error_str.contains("pattern") || error_str.contains("version"));
        }
    }
    
    #[tokio::test]
    async fn test_validate_enum_constraint() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create config with invalid service enum value
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("invalid-service".to_string()));
        
        let configuration = HashMap::new();
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            let error_str = errors.join(" ");
            assert!(error_str.contains("enum") || error_str.contains("service"));
        }
    }
    
    #[tokio::test]
    async fn test_validate_number_constraints() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Test port out of range
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let mut configuration = HashMap::new();
        configuration.insert("port".to_string(), ConfigValue::Integer(70000)); // Out of range
        
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            let error_str = errors.join(" ");
            assert!(error_str.contains("maximum") || error_str.contains("port"));
        }
    }
    
    #[tokio::test]
    async fn test_validate_nested_objects() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create config with nested limits object
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let mut limits = HashMap::new();
        limits.insert("max_connections".to_string(), ConfigValue::Integer(100));
        limits.insert("timeout".to_string(), ConfigValue::Float(30.5));
        
        let mut configuration = HashMap::new();
        configuration.insert("limits".to_string(), ConfigValue::Object(limits));
        
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_validate_type_mismatch() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create config with type mismatch (string instead of boolean)
        let mut config_map = HashMap::new();
        config_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        config_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        
        let mut configuration = HashMap::new();
        configuration.insert("enabled".to_string(), ConfigValue::String("true".to_string())); // Should be boolean
        
        config_map.insert("configuration".to_string(), ConfigValue::Object(configuration));
        
        let config = ConfigValue::Object(config_map);
        
        let result = validator.validate(&config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            let error_str = errors.join(" ");
            assert!(error_str.contains("type") || error_str.contains("boolean"));
        }
    }
    
    #[test]
    fn test_config_to_json_conversion() {
        // Test conversion of ConfigValue to JSON for validation
        let mut config_map = HashMap::new();
        config_map.insert("string".to_string(), ConfigValue::String("value".to_string()));
        config_map.insert("integer".to_string(), ConfigValue::Integer(42));
        config_map.insert("float".to_string(), ConfigValue::Float(3.14));
        config_map.insert("boolean".to_string(), ConfigValue::Boolean(true));
        config_map.insert("null".to_string(), ConfigValue::Null);
        
        let mut nested = HashMap::new();
        nested.insert("nested_key".to_string(), ConfigValue::String("nested_value".to_string()));
        config_map.insert("object".to_string(), ConfigValue::Object(nested));
        
        let array = vec![
            ConfigValue::Integer(1),
            ConfigValue::Integer(2),
            ConfigValue::Integer(3)
        ];
        config_map.insert("array".to_string(), ConfigValue::Array(array));
        
        let config = ConfigValue::Object(config_map);
        
        let json = SchemaValidator::config_to_json(&config).unwrap();
        
        // Verify JSON structure
        assert_eq!(json["string"], "value");
        assert_eq!(json["integer"], 42);
        assert_eq!(json["float"], 3.14);
        assert_eq!(json["boolean"], true);
        assert_eq!(json["null"], serde_json::Value::Null);
        assert_eq!(json["object"]["nested_key"], "nested_value");
        assert_eq!(json["array"][0], 1);
        assert_eq!(json["array"][1], 2);
        assert_eq!(json["array"][2], 3);
    }
    
    #[tokio::test]
    async fn test_load_schema_from_file() {
        use tempfile::TempDir;
        use std::fs;
        
        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("test.schema.json");
        
        let schema = create_test_schema();
        fs::write(&schema_path, &schema).unwrap();
        
        let validator = SchemaValidator::from_file(&schema_path).await.unwrap();
        assert!(validator.is_valid_schema());
        assert_eq!(validator.schema_string(), &schema);
    }
    
    #[tokio::test]
    async fn test_batch_validation() {
        let schema = create_test_schema();
        let validator = SchemaValidator::new(schema);
        
        // Create multiple configs
        let mut configs = Vec::new();
        
        // Valid config
        let mut valid_map = HashMap::new();
        valid_map.insert("version".to_string(), ConfigValue::String("1.0.0".to_string()));
        valid_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        valid_map.insert("configuration".to_string(), ConfigValue::Object(HashMap::new()));
        configs.push(("valid", ConfigValue::Object(valid_map)));
        
        // Invalid config
        let mut invalid_map = HashMap::new();
        invalid_map.insert("service".to_string(), ConfigValue::String("neural-trading".to_string()));
        configs.push(("invalid", ConfigValue::Object(invalid_map)));
        
        // Validate batch
        let results = validator.validate_batch(configs).await;
        
        assert_eq!(results.len(), 2);
        assert!(results[0].1.is_ok());
        assert!(results[1].1.is_err());
    }
}