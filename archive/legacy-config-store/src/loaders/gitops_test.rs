/// TDD Tests for GitOpsLoader - Written FIRST before implementation
/// Following London TDD style with mocks

#[cfg(test)]
mod gitops_loader_tests {
    use super::super::gitops::*;
    use crate::types::{ConfigValue, ConfigError};
    use std::path::PathBuf;
    use std::collections::HashMap;
    use tempfile::TempDir;
    use std::fs;
    
    fn create_test_config_structure() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path();
        
        // Create base configs
        let base_dir = base_path.join("base");
        fs::create_dir_all(&base_dir).unwrap();
        
        // Create base service config
        let base_service_dir = base_dir.join("neural-trading");
        fs::create_dir_all(&base_service_dir).unwrap();
        
        let base_config = r#"
version: "1.0.0"
service: neural-trading
configuration:
  trading:
    capital: 100000
    mode: paper
    risk_limits:
      position_size_pct: 5.0
      daily_loss_pct: 2.0
"#;
        fs::write(base_service_dir.join("config.yaml"), base_config).unwrap();
        
        // Create overlays
        let overlay_dir = base_path.join("overlays").join("dev");
        fs::create_dir_all(&overlay_dir).unwrap();
        
        let overlay_service_dir = overlay_dir.join("neural-trading");
        fs::create_dir_all(&overlay_service_dir).unwrap();
        
        let overlay_config = r#"
configuration:
  trading:
    mode: test
    risk_limits:
      daily_loss_pct: 5.0
  monitoring:
    enabled: true
"#;
        fs::write(overlay_service_dir.join("config.yaml"), overlay_config).unwrap();
        
        // Create schemas
        let schema_dir = base_path.join("schemas");
        fs::create_dir_all(&schema_dir).unwrap();
        
        let schema = r#"{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "required": ["version", "service", "configuration"],
  "properties": {
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "service": {
      "type": "string"
    },
    "configuration": {
      "type": "object"
    }
  }
}"#;
        fs::write(schema_dir.join("neural-trading.json"), schema).unwrap();
        
        temp_dir
    }
    
    #[test]
    fn test_gitops_loader_creation() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        assert_eq!(loader.environment(), "dev");
        assert_eq!(loader.base_path(), temp_dir.path());
    }
    
    #[tokio::test]
    async fn test_load_yaml_file() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let yaml_path = temp_dir.path()
            .join("base")
            .join("neural-trading")
            .join("config.yaml");
        
        let result = loader.load_yaml_file(&yaml_path).await;
        assert!(result.is_ok());
        
        let config = result.unwrap();
        if let ConfigValue::Object(map) = config {
            assert!(map.contains_key("version"));
            assert!(map.contains_key("service"));
            assert!(map.contains_key("configuration"));
        } else {
            panic!("Expected Object, got {:?}", config);
        }
    }
    
    #[tokio::test]
    async fn test_load_base_configs() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let configs = loader.load_base_configs().await.unwrap();
        assert!(configs.contains_key("neural-trading"));
        
        let neural_config = &configs["neural-trading"];
        if let ConfigValue::Object(map) = neural_config {
            assert!(map.contains_key("configuration"));
        } else {
            panic!("Expected Object for neural-trading config");
        }
    }
    
    #[tokio::test]
    async fn test_load_overlay_configs() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let configs = loader.load_overlay_configs().await.unwrap();
        assert!(configs.contains_key("neural-trading"));
        
        let overlay = &configs["neural-trading"];
        if let ConfigValue::Object(map) = overlay {
            assert!(map.contains_key("configuration"));
            
            // Check overlay specific values
            if let Some(ConfigValue::Object(config)) = map.get("configuration") {
                if let Some(ConfigValue::Object(trading)) = config.get("trading") {
                    if let Some(ConfigValue::String(mode)) = trading.get("mode") {
                        assert_eq!(mode, "test"); // Overlay value
                    }
                }
                assert!(config.contains_key("monitoring")); // Only in overlay
            }
        } else {
            panic!("Expected Object for overlay config");
        }
    }
    
    #[tokio::test]
    async fn test_merge_configs() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        // Create base config
        let mut base_map = HashMap::new();
        let mut base_trading = HashMap::new();
        base_trading.insert("capital".to_string(), ConfigValue::Integer(100000));
        base_trading.insert("mode".to_string(), ConfigValue::String("paper".to_string()));
        
        let mut base_risk = HashMap::new();
        base_risk.insert("position_size_pct".to_string(), ConfigValue::Float(5.0));
        base_risk.insert("daily_loss_pct".to_string(), ConfigValue::Float(2.0));
        base_trading.insert("risk_limits".to_string(), ConfigValue::Object(base_risk));
        
        let mut base_config = HashMap::new();
        base_config.insert("trading".to_string(), ConfigValue::Object(base_trading));
        base_map.insert("configuration".to_string(), ConfigValue::Object(base_config));
        
        let base = ConfigValue::Object(base_map);
        
        // Create overlay config
        let mut overlay_map = HashMap::new();
        let mut overlay_trading = HashMap::new();
        overlay_trading.insert("mode".to_string(), ConfigValue::String("test".to_string()));
        
        let mut overlay_risk = HashMap::new();
        overlay_risk.insert("daily_loss_pct".to_string(), ConfigValue::Float(5.0));
        overlay_trading.insert("risk_limits".to_string(), ConfigValue::Object(overlay_risk));
        
        let mut overlay_config = HashMap::new();
        overlay_config.insert("trading".to_string(), ConfigValue::Object(overlay_trading));
        
        let mut monitoring = HashMap::new();
        monitoring.insert("enabled".to_string(), ConfigValue::Boolean(true));
        overlay_config.insert("monitoring".to_string(), ConfigValue::Object(monitoring));
        
        overlay_map.insert("configuration".to_string(), ConfigValue::Object(overlay_config));
        
        let overlay = ConfigValue::Object(overlay_map);
        
        // Test merge
        let merged = loader.merge_configs(&base, &overlay).unwrap();
        
        if let ConfigValue::Object(map) = merged {
            if let Some(ConfigValue::Object(config)) = map.get("configuration") {
                // Check merged trading config
                if let Some(ConfigValue::Object(trading)) = config.get("trading") {
                    // Base value preserved
                    if let Some(ConfigValue::Integer(capital)) = trading.get("capital") {
                        assert_eq!(*capital, 100000);
                    }
                    // Overlay value overrides
                    if let Some(ConfigValue::String(mode)) = trading.get("mode") {
                        assert_eq!(mode, "test");
                    }
                    // Nested merge
                    if let Some(ConfigValue::Object(risk)) = trading.get("risk_limits") {
                        // Base value preserved
                        if let Some(ConfigValue::Float(pos_size)) = risk.get("position_size_pct") {
                            assert_eq!(*pos_size, 5.0);
                        }
                        // Overlay value overrides
                        if let Some(ConfigValue::Float(daily_loss)) = risk.get("daily_loss_pct") {
                            assert_eq!(*daily_loss, 5.0);
                        }
                    }
                }
                // New overlay value added
                assert!(config.contains_key("monitoring"));
            }
        } else {
            panic!("Expected Object from merge");
        }
    }
    
    #[tokio::test]
    async fn test_load_all_configs() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let configs = loader.load_all_configs().await.unwrap();
        assert!(configs.contains_key("neural-trading"));
        
        let config = &configs["neural-trading"];
        if let ConfigValue::Object(map) = config {
            // Should have merged values
            assert!(map.contains_key("version")); // From base
            assert!(map.contains_key("service")); // From base
            assert!(map.contains_key("configuration"));
            
            if let Some(ConfigValue::Object(config)) = map.get("configuration") {
                if let Some(ConfigValue::Object(trading)) = config.get("trading") {
                    // Check overlay override
                    if let Some(ConfigValue::String(mode)) = trading.get("mode") {
                        assert_eq!(mode, "test");
                    }
                    // Check base preserved
                    assert!(trading.contains_key("capital"));
                }
                // Check overlay addition
                assert!(config.contains_key("monitoring"));
            }
        }
    }
    
    #[tokio::test]
    async fn test_validate_with_schema() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let configs = loader.load_all_configs().await.unwrap();
        let neural_config = &configs["neural-trading"];
        
        // Should validate successfully
        let result = loader.validate_config("neural-trading", neural_config).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_invalid_config_fails_validation() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        // Create invalid config (missing required fields)
        let mut invalid = HashMap::new();
        invalid.insert("service".to_string(), ConfigValue::String("test".to_string()));
        let invalid_config = ConfigValue::Object(invalid);
        
        let result = loader.validate_config("neural-trading", &invalid_config).await;
        assert!(result.is_err());
        
        if let Err(ConfigError::ValidationFailed(errors)) = result {
            assert!(!errors.is_empty());
            // Should contain error about missing required fields
            let error_str = errors.join(", ");
            assert!(error_str.contains("version") || error_str.contains("required"));
        } else {
            panic!("Expected ValidationFailed error");
        }
    }
    
    #[tokio::test]
    async fn test_build_config_tree() {
        let temp_dir = create_test_config_structure();
        let loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        
        let tree = loader.build_config_tree().await.unwrap();
        
        // Check tree structure
        assert!(tree.contains_key("/neural-trading"));
        assert!(tree.contains_key("/neural-trading/version"));
        assert!(tree.contains_key("/neural-trading/service"));
        assert!(tree.contains_key("/neural-trading/configuration"));
        assert!(tree.contains_key("/neural-trading/configuration/trading"));
        assert!(tree.contains_key("/neural-trading/configuration/trading/capital"));
        assert!(tree.contains_key("/neural-trading/configuration/trading/mode"));
        assert!(tree.contains_key("/neural-trading/configuration/monitoring"));
        assert!(tree.contains_key("/neural-trading/configuration/monitoring/enabled"));
        
        // Verify values
        if let Some(ConfigValue::String(mode)) = tree.get("/neural-trading/configuration/trading/mode") {
            assert_eq!(mode, "test"); // Overlay value
        }
        if let Some(ConfigValue::Integer(capital)) = tree.get("/neural-trading/configuration/trading/capital") {
            assert_eq!(*capital, 100000); // Base value
        }
        if let Some(ConfigValue::Boolean(enabled)) = tree.get("/neural-trading/configuration/monitoring/enabled") {
            assert_eq!(*enabled, true); // Overlay addition
        }
    }
    
    #[test]
    fn test_environment_specific_loading() {
        let temp_dir = create_test_config_structure();
        
        // Test dev environment
        let dev_loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "dev".to_string(),
        );
        assert_eq!(dev_loader.environment(), "dev");
        
        // Test prod environment (should handle missing overlay gracefully)
        let prod_loader = GitOpsLoader::new(
            temp_dir.path().to_path_buf(),
            "prod".to_string(),
        );
        assert_eq!(prod_loader.environment(), "prod");
    }
}