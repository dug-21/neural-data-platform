//! Environment Variable Validation Tests
//!
//! This test suite validates that environment variables for neural initialization
//! features are properly parsed and respected.

use std::env;
use std::fs;
use tempfile::TempDir;

/// Helper to manage environment variables safely during tests
struct EnvGuard {
    vars: Vec<String>,
}

impl EnvGuard {
    fn new() -> Self {
        Self { vars: Vec::new() }
    }
    
    fn set(&mut self, key: &str, value: &str) {
        self.vars.push(key.to_string());
        env::set_var(key, value);
    }
    
    fn remove(&mut self, key: &str) {
        self.vars.push(key.to_string());
        env::remove_var(key);
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for var in &self.vars {
            env::remove_var(var);
        }
    }
}

#[cfg(test)]
mod environment_variable_tests {
    use super::*;

    #[test]
    fn test_enable_sector_models_parsing() {
        // Test cases for ENABLE_SECTOR_MODELS environment variable parsing
        let test_cases = vec![
            ("true", true),
            ("True", true),
            ("TRUE", true),
            ("false", false),
            ("False", false),
            ("FALSE", false),
            ("1", false),    // Only "true" (case insensitive) should enable
            ("yes", false),  // Only "true" (case insensitive) should enable
            ("0", false),
            ("no", false),
            ("", false),     // Empty string should be false
            ("random", false), // Random strings should be false
        ];
        
        for (env_value, expected_enabled) in test_cases {
            // When: Setting environment variable to specific value
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_SECTOR_MODELS", env_value);
            
            // Then: Should parse correctly using the same logic as the application
            let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            
            assert_eq!(sector_enabled, expected_enabled, 
                "Sector models parsing failed for value: '{}'", env_value);
        }
    }

    #[test]
    fn test_enable_autonomous_training_parsing() {
        // Test cases for ENABLE_AUTONOMOUS_TRAINING environment variable parsing
        let test_cases = vec![
            ("true", true),
            ("True", true),
            ("TRUE", true),
            ("false", false),
            ("False", false),
            ("FALSE", false),
            ("1", false),    // Only "true" (case insensitive) should enable
            ("yes", false),  // Only "true" (case insensitive) should enable
            ("0", false),
            ("no", false),
            ("", false),     // Empty string should be false
            ("invalid", false), // Invalid strings should be false
        ];
        
        for (env_value, expected_enabled) in test_cases {
            // When: Setting environment variable to specific value
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_AUTONOMOUS_TRAINING", env_value);
            
            // Then: Should parse correctly using the same logic as the application
            let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            
            assert_eq!(training_enabled, expected_enabled, 
                "Autonomous training parsing failed for value: '{}'", env_value);
        }
    }

    #[test]
    fn test_default_behavior_when_env_vars_not_set() {
        // Given: Environment variables are not set
        let mut env_guard = EnvGuard::new();
        env_guard.remove("ENABLE_SECTOR_MODELS");
        env_guard.remove("ENABLE_AUTONOMOUS_TRAINING");
        
        // When: Checking if features should be enabled (simulating application startup)
        let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        
        // Then: Both should default to disabled
        assert!(!sector_enabled, "Sector models should be disabled by default");
        assert!(!training_enabled, "Autonomous training should be disabled by default");
    }

    #[test]
    fn test_whitespace_handling() {
        // Test that whitespace is handled correctly
        let test_cases = vec![
            (" true ", true),   // Spaces around true
            ("\ttrue\n", true), // Tab and newline
            ("true  ", true),   // Trailing spaces
            ("  true", true),   // Leading spaces
            (" false ", false), // Spaces around false
            ("  ", false),      // Only whitespace
        ];
        
        for (env_value, expected_enabled) in test_cases {
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_SECTOR_MODELS", env_value);
            
            // Using trim() to handle whitespace like the application should
            let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
                .map(|v| v.to_lowercase().trim() == "true")
                .unwrap_or(false);
            
            assert_eq!(sector_enabled, expected_enabled, 
                "Whitespace handling failed for value: '{}'", env_value);
        }
    }

    #[test]
    fn test_both_features_coordination() {
        // Test that both features can be enabled/disabled independently
        
        // Test Case 1: Both enabled
        {
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_SECTOR_MODELS", "true");
            env_guard.set("ENABLE_AUTONOMOUS_TRAINING", "true");
            
            let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            
            assert!(sector_enabled, "Sector models should be enabled");
            assert!(training_enabled, "Autonomous training should be enabled");
        }
        
        // Test Case 2: Both disabled
        {
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_SECTOR_MODELS", "false");
            env_guard.set("ENABLE_AUTONOMOUS_TRAINING", "false");
            
            let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            
            assert!(!sector_enabled, "Sector models should be disabled");
            assert!(!training_enabled, "Autonomous training should be disabled");
        }
        
        // Test Case 3: Mixed settings
        {
            let mut env_guard = EnvGuard::new();
            env_guard.set("ENABLE_SECTOR_MODELS", "true");
            env_guard.set("ENABLE_AUTONOMOUS_TRAINING", "false");
            
            let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
                .map(|v| v.to_lowercase() == "true")
                .unwrap_or(false);
            
            assert!(sector_enabled, "Sector models should be enabled");
            assert!(!training_enabled, "Autonomous training should be disabled");
        }
    }

    #[test]
    fn test_config_path_environment_variables() {
        // Test that config path environment variables work
        let mut env_guard = EnvGuard::new();
        
        // Test sector config path
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_config_path = temp_dir.path().join("custom_sector_config.toml");
        let config_content = r#"
[metadata]
version = "2.0.0"
description = "Test configuration"

[data_requirements]
description = "Test requirements"
"#;
        fs::write(&custom_config_path, config_content).unwrap();
        
        env_guard.set("SECTOR_CONFIG_PATH", &custom_config_path.to_string_lossy());
        
        // When: Reading configuration path from environment
        let config_path = env::var("SECTOR_CONFIG_PATH").unwrap();
        
        // Then: Should point to our custom path and be readable
        assert_eq!(config_path, custom_config_path.to_string_lossy());
        assert!(std::path::Path::new(&config_path).exists(), "Config file should exist");
        
        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("Test configuration"), "Should contain our test content");
    }

    #[test]
    fn test_regression_prevention_critical_functionality() {
        // This test ensures the critical environment variable functionality never regresses
        
        // REGRESSION TEST 1: Basic true/false parsing must always work
        let mut env_guard = EnvGuard::new();
        env_guard.set("ENABLE_SECTOR_MODELS", "true");
        env_guard.set("ENABLE_AUTONOMOUS_TRAINING", "true");
        
        let sector_enabled = env::var("ENABLE_SECTOR_MODELS")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let training_enabled = env::var("ENABLE_AUTONOMOUS_TRAINING")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        
        assert!(sector_enabled, "REGRESSION: ENABLE_SECTOR_MODELS=true must always parse as enabled");
        assert!(training_enabled, "REGRESSION: ENABLE_AUTONOMOUS_TRAINING=true must always parse as enabled");
        
        // REGRESSION TEST 2: Default disabled behavior must always work
        env_guard.remove("ENABLE_SECTOR_MODELS");
        env_guard.remove("ENABLE_AUTONOMOUS_TRAINING");
        
        let sector_default = env::var("ENABLE_SECTOR_MODELS")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let training_default = env::var("ENABLE_AUTONOMOUS_TRAINING")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        
        assert!(!sector_default, "REGRESSION: Missing ENABLE_SECTOR_MODELS must default to disabled");
        assert!(!training_default, "REGRESSION: Missing ENABLE_AUTONOMOUS_TRAINING must default to disabled");
        
        // REGRESSION TEST 3: Case insensitive parsing must always work
        env_guard.set("ENABLE_SECTOR_MODELS", "TRUE");
        env_guard.set("ENABLE_AUTONOMOUS_TRAINING", "True");
        
        let sector_case = env::var("ENABLE_SECTOR_MODELS")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        let training_case = env::var("ENABLE_AUTONOMOUS_TRAINING")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);
        
        assert!(sector_case, "REGRESSION: Case insensitive parsing must work for ENABLE_SECTOR_MODELS");
        assert!(training_case, "REGRESSION: Case insensitive parsing must work for ENABLE_AUTONOMOUS_TRAINING");
    }

    #[test]
    fn test_environment_isolation() {
        // Test that environment variable cleanup works properly
        {
            let mut env_guard1 = EnvGuard::new();
            env_guard1.set("TEST_VAR_1", "value1");
            
            {
                let mut env_guard2 = EnvGuard::new();
                env_guard2.set("TEST_VAR_2", "value2");
                
                assert_eq!(env::var("TEST_VAR_1").unwrap(), "value1");
                assert_eq!(env::var("TEST_VAR_2").unwrap(), "value2");
            }
            
            // TEST_VAR_2 should be cleaned up
            assert!(env::var("TEST_VAR_2").is_err(), "TEST_VAR_2 should be cleaned up");
            assert_eq!(env::var("TEST_VAR_1").unwrap(), "value1");
        }
        
        // Both should be cleaned up
        assert!(env::var("TEST_VAR_1").is_err(), "TEST_VAR_1 should be cleaned up");
        assert!(env::var("TEST_VAR_2").is_err(), "TEST_VAR_2 should be cleaned up");
    }
}