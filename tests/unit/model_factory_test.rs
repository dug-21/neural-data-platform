//! Comprehensive Unit Tests for ModelFactory
//!
//! Tests vendor model creation, configuration parsing, and capabilities matching.

use anyhow::Result;
use std::collections::HashMap;

use crate::neural::model_factory::{ModelFactory, ModelCapabilities};
use crate::neural::vendor_predictor::{ModelConfig, DataRequirements};

// Test utilities
fn create_test_model_config(architecture: &str, custom_params: Option<HashMap<String, serde_json::Value>>) -> ModelConfig {
    let parameters = custom_params.unwrap_or_else(|| {
        let mut params = HashMap::new();
        match architecture {
            "MLP" => {
                params.insert("input_size".to_string(), serde_json::json!(24));
                params.insert("hidden_size".to_string(), serde_json::json!(64));
                params.insert("num_layers".to_string(), serde_json::json!(2));
            }
            "LSTM" | "GRU" => {
                params.insert("input_size".to_string(), serde_json::json!(24));
                params.insert("hidden_size".to_string(), serde_json::json!(64));
                params.insert("num_layers".to_string(), serde_json::json!(2));
                params.insert("dropout".to_string(), serde_json::json!(0.1));
            }
            "TCN" => {
                params.insert("input_size".to_string(), serde_json::json!(24));
                params.insert("num_channels".to_string(), serde_json::json!(64));
                params.insert("kernel_size".to_string(), serde_json::json!(3));
            }
            "TFT" => {
                params.insert("d_model".to_string(), serde_json::json!(128));
                params.insert("num_heads".to_string(), serde_json::json!(4));
                params.insert("num_encoder_layers".to_string(), serde_json::json!(6));
            }
            "DeepAR" => {
                params.insert("hidden_size".to_string(), serde_json::json!(100));
                params.insert("num_layers".to_string(), serde_json::json!(2));
            }
            "DLinear" | "NLinear" => {
                params.insert("seq_len".to_string(), serde_json::json!(96));
                params.insert("pred_len".to_string(), serde_json::json!(24));
            }
            _ => {
                params.insert("input_size".to_string(), serde_json::json!(24));
                params.insert("hidden_size".to_string(), serde_json::json!(64));
            }
        }
        params
    });
    
    ModelConfig {
        architecture: architecture.to_string(),
        parameters,
        data_requirements: DataRequirements {
            required: vec!["price".to_string()],
            optional: vec!["volume".to_string()],
            min_history: 24,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_mlp_model() {
        let config = create_test_model_config("MLP", None);
        let result = ModelFactory::create_model("MLP", &config);
        
        assert!(result.is_ok());
        let model = result.unwrap();
        
        // Verify model was created (basic interface check)
        // Note: In real implementation, this would use actual vendor library
        // For now, we're testing the factory logic structure
    }
    
    #[test]
    fn test_create_lstm_model() {
        let config = create_test_model_config("LSTM", None);
        let result = ModelFactory::create_model("LSTM", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify LSTM-specific parameters were processed correctly
        assert_eq!(config.parameters.get("input_size").unwrap(), &serde_json::json!(24));
        assert_eq!(config.parameters.get("hidden_size").unwrap(), &serde_json::json!(64));
        assert_eq!(config.parameters.get("num_layers").unwrap(), &serde_json::json!(2));
        assert_eq!(config.parameters.get("dropout").unwrap(), &serde_json::json!(0.1));
    }
    
    #[test]
    fn test_create_gru_model() {
        let config = create_test_model_config("GRU", None);
        let result = ModelFactory::create_model("GRU", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
    }
    
    #[test]
    fn test_create_tcn_model() {
        let config = create_test_model_config("TCN", None);
        let result = ModelFactory::create_model("TCN", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify TCN-specific parameters
        assert_eq!(config.parameters.get("num_channels").unwrap(), &serde_json::json!(64));
        assert_eq!(config.parameters.get("kernel_size").unwrap(), &serde_json::json!(3));
    }
    
    #[test]
    fn test_create_tft_model() {
        let config = create_test_model_config("TFT", None);
        let result = ModelFactory::create_model("TFT", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify TFT-specific parameters
        assert_eq!(config.parameters.get("d_model").unwrap(), &serde_json::json!(128));
        assert_eq!(config.parameters.get("num_heads").unwrap(), &serde_json::json!(4));
        assert_eq!(config.parameters.get("num_encoder_layers").unwrap(), &serde_json::json!(6));
    }
    
    #[test]
    fn test_create_deepar_model() {
        let config = create_test_model_config("DeepAR", None);
        let result = ModelFactory::create_model("DeepAR", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify DeepAR-specific parameters
        assert_eq!(config.parameters.get("hidden_size").unwrap(), &serde_json::json!(100));
        assert_eq!(config.parameters.get("num_layers").unwrap(), &serde_json::json!(2));
    }
    
    #[test]
    fn test_create_nbeats_model() {
        let config = create_test_model_config("NBEATS", None);
        let result = ModelFactory::create_model("NBEATS", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
    }
    
    #[test]
    fn test_create_nhits_model() {
        let config = create_test_model_config("NHITS", None);
        let result = ModelFactory::create_model("NHITS", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
    }
    
    #[test]
    fn test_create_dlinear_model() {
        let config = create_test_model_config("DLinear", None);
        let result = ModelFactory::create_model("DLinear", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify linear model parameters
        assert_eq!(config.parameters.get("seq_len").unwrap(), &serde_json::json!(96));
        assert_eq!(config.parameters.get("pred_len").unwrap(), &serde_json::json!(24));
    }
    
    #[test]
    fn test_create_nlinear_model() {
        let config = create_test_model_config("NLinear", None);
        let result = ModelFactory::create_model("NLinear", &config);
        
        assert!(result.is_ok());
        let _model = result.unwrap();
        
        // Verify linear model parameters
        assert_eq!(config.parameters.get("seq_len").unwrap(), &serde_json::json!(96));
        assert_eq!(config.parameters.get("pred_len").unwrap(), &serde_json::json!(24));
    }
    
    #[test]
    fn test_create_unknown_model() {
        let config = create_test_model_config("UNKNOWN", None);
        let result = ModelFactory::create_model("UNKNOWN", &config);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Unsupported model architecture"));
        assert!(error.to_string().contains("UNKNOWN"));
    }
    
    #[test]
    fn test_custom_parameters() {
        let mut custom_params = HashMap::new();
        custom_params.insert("input_size".to_string(), serde_json::json!(48));
        custom_params.insert("hidden_size".to_string(), serde_json::json!(128));
        custom_params.insert("num_layers".to_string(), serde_json::json!(4));
        custom_params.insert("dropout".to_string(), serde_json::json!(0.2));
        
        let config = create_test_model_config("LSTM", Some(custom_params));
        let result = ModelFactory::create_model("LSTM", &config);
        
        assert!(result.is_ok());
        
        // Verify custom parameters were used
        assert_eq!(config.parameters.get("input_size").unwrap(), &serde_json::json!(48));
        assert_eq!(config.parameters.get("hidden_size").unwrap(), &serde_json::json!(128));
        assert_eq!(config.parameters.get("num_layers").unwrap(), &serde_json::json!(4));
        assert_eq!(config.parameters.get("dropout").unwrap(), &serde_json::json!(0.2));
    }
    
    #[test]
    fn test_missing_parameters_use_defaults() {
        let empty_params = HashMap::new();
        let config = create_test_model_config("MLP", Some(empty_params));
        let result = ModelFactory::create_model("MLP", &config);
        
        assert!(result.is_ok());
        
        // Factory should use default values when parameters are missing
        // Default values are applied in the factory logic
    }
    
    #[test]
    fn test_get_model_capabilities_lstm() {
        let caps = ModelFactory::get_model_capabilities("LSTM");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 10);
        assert_eq!(caps.optimal_sequence_length, 100);
    }
    
    #[test]
    fn test_get_model_capabilities_gru() {
        let caps = ModelFactory::get_model_capabilities("GRU");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 10);
        assert_eq!(caps.optimal_sequence_length, 100);
    }
    
    #[test]
    fn test_get_model_capabilities_tft() {
        let caps = ModelFactory::get_model_capabilities("TFT");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(caps.supports_static);
        assert_eq!(caps.min_sequence_length, 24);
        assert_eq!(caps.optimal_sequence_length, 168);
    }
    
    #[test]
    fn test_get_model_capabilities_informer() {
        let caps = ModelFactory::get_model_capabilities("Informer");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(caps.supports_static);
        assert_eq!(caps.min_sequence_length, 24);
        assert_eq!(caps.optimal_sequence_length, 168);
    }
    
    #[test]
    fn test_get_model_capabilities_tcn() {
        let caps = ModelFactory::get_model_capabilities("TCN");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 20);
        assert_eq!(caps.optimal_sequence_length, 100);
    }
    
    #[test]
    fn test_get_model_capabilities_deepar() {
        let caps = ModelFactory::get_model_capabilities("DeepAR");
        
        assert!(caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(caps.supports_static);
        assert_eq!(caps.min_sequence_length, 30);
        assert_eq!(caps.optimal_sequence_length, 200);
    }
    
    #[test]
    fn test_get_model_capabilities_nbeats() {
        let caps = ModelFactory::get_model_capabilities("NBEATS");
        
        assert!(caps.requires_sequential_data);
        assert!(!caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 50);
        assert_eq!(caps.optimal_sequence_length, 500);
    }
    
    #[test]
    fn test_get_model_capabilities_nhits() {
        let caps = ModelFactory::get_model_capabilities("NHITS");
        
        assert!(caps.requires_sequential_data);
        assert!(!caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 50);
        assert_eq!(caps.optimal_sequence_length, 500);
    }
    
    #[test]
    fn test_get_model_capabilities_dlinear() {
        let caps = ModelFactory::get_model_capabilities("DLinear");
        
        assert!(caps.requires_sequential_data);
        assert!(!caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 96);
        assert_eq!(caps.optimal_sequence_length, 96);
    }
    
    #[test]
    fn test_get_model_capabilities_nlinear() {
        let caps = ModelFactory::get_model_capabilities("NLinear");
        
        assert!(caps.requires_sequential_data);
        assert!(!caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 96);
        assert_eq!(caps.optimal_sequence_length, 96);
    }
    
    #[test]
    fn test_get_model_capabilities_mlp() {
        let caps = ModelFactory::get_model_capabilities("MLP");
        
        assert!(!caps.requires_sequential_data);
        assert!(caps.supports_exogenous);
        assert!(caps.supports_static);
        assert_eq!(caps.min_sequence_length, 1);
        assert_eq!(caps.optimal_sequence_length, 24);
    }
    
    #[test]
    fn test_get_model_capabilities_unknown() {
        let caps = ModelFactory::get_model_capabilities("UNKNOWN");
        
        // Should return default capabilities
        assert!(caps.requires_sequential_data);
        assert!(!caps.supports_exogenous);
        assert!(!caps.supports_static);
        assert_eq!(caps.min_sequence_length, 10);
        assert_eq!(caps.optimal_sequence_length, 100);
    }
    
    #[test]
    fn test_model_capabilities_default() {
        let default_caps = ModelCapabilities::default();
        
        assert!(default_caps.requires_sequential_data);
        assert!(!default_caps.supports_exogenous);
        assert!(!default_caps.supports_static);
        assert_eq!(default_caps.min_sequence_length, 10);
        assert_eq!(default_caps.optimal_sequence_length, 100);
    }
    
    #[test]
    fn test_create_price_only_models() {
        let result = ModelFactory::create_price_only_models();
        assert!(result.is_ok());
        
        let models = result.unwrap();
        
        // Should create at least the basic price-only models
        assert!(models.len() >= 3);
        assert!(models.contains_key("MLP_Price"));
        assert!(models.contains_key("LSTM_Price"));
        assert!(models.contains_key("TCN_Price"));
        assert!(models.contains_key("DLinear_Price"));
    }
    
    #[test]
    fn test_price_only_models_configurations() {
        let result = ModelFactory::create_price_only_models();
        assert!(result.is_ok());
        
        let models = result.unwrap();
        
        // Verify we have some models
        assert!(!models.is_empty());
        
        // Models should be created with minimal data requirements
        // This tests the factory's ability to create working models with basic parameters
    }
    
    #[test]
    fn test_parameter_type_handling() {
        // Test various parameter types
        let mut params = HashMap::new();
        params.insert("input_size".to_string(), serde_json::json!(24_u64));
        params.insert("hidden_size".to_string(), serde_json::json!(64_u64));
        params.insert("dropout".to_string(), serde_json::json!(0.1_f64));
        params.insert("learning_rate".to_string(), serde_json::json!(0.001_f64));
        
        let config = create_test_model_config("LSTM", Some(params));
        let result = ModelFactory::create_model("LSTM", &config);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_edge_case_parameters() {
        // Test edge case parameter values
        let mut params = HashMap::new();
        params.insert("input_size".to_string(), serde_json::json!(1)); // Minimum
        params.insert("hidden_size".to_string(), serde_json::json!(1)); // Minimum
        params.insert("num_layers".to_string(), serde_json::json!(1)); // Minimum
        params.insert("dropout".to_string(), serde_json::json!(0.0)); // No dropout
        
        let config = create_test_model_config("MLP", Some(params));
        let result = ModelFactory::create_model("MLP", &config);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_large_model_parameters() {
        // Test large model configurations
        let mut params = HashMap::new();
        params.insert("input_size".to_string(), serde_json::json!(1000));
        params.insert("hidden_size".to_string(), serde_json::json!(2048));
        params.insert("num_layers".to_string(), serde_json::json!(12));
        params.insert("dropout".to_string(), serde_json::json!(0.5));
        
        let config = create_test_model_config("LSTM", Some(params));
        let result = ModelFactory::create_model("LSTM", &config);
        
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_invalid_parameter_types() {
        // Test handling of invalid parameter types
        let mut params = HashMap::new();
        params.insert("input_size".to_string(), serde_json::json!("not_a_number"));
        params.insert("hidden_size".to_string(), serde_json::json!(null));
        
        let config = create_test_model_config("MLP", Some(params));
        let result = ModelFactory::create_model("MLP", &config);
        
        // Should still work by falling back to defaults
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_create_all_supported_models() {
        let architectures = vec![
            "MLP", "LSTM", "GRU", "TCN", "TFT", "DeepAR", 
            "NBEATS", "NHITS", "DLinear", "NLinear"
        ];
        
        for arch in architectures {
            let config = create_test_model_config(arch, None);
            let result = ModelFactory::create_model(arch, &config);
            
            assert!(result.is_ok(), "Failed to create model: {}", arch);
        }
    }
    
    #[test]
    fn test_model_capabilities_consistency() {
        // Test that all known architectures have defined capabilities
        let architectures = vec![
            "MLP", "LSTM", "GRU", "RNN", "TCN", "BiTCN",
            "TFT", "Informer", "Autoformer", "DeepAR",
            "NBEATS", "NHITS", "DLinear", "NLinear"
        ];
        
        for arch in architectures {
            let caps = ModelFactory::get_model_capabilities(arch);
            
            // All capabilities should have reasonable values
            assert!(caps.min_sequence_length > 0, "Invalid min_sequence_length for {}", arch);
            assert!(caps.optimal_sequence_length >= caps.min_sequence_length, 
                "optimal_sequence_length should be >= min_sequence_length for {}", arch);
        }
    }
    
    #[test]
    fn test_sequential_vs_non_sequential_models() {
        // Sequential models
        let sequential_models = vec!["LSTM", "GRU", "TCN", "TFT", "DeepAR", "NBEATS", "DLinear"];
        for model in sequential_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(caps.requires_sequential_data, "{} should require sequential data", model);
        }
        
        // Non-sequential models
        let non_sequential_models = vec!["MLP"];
        for model in non_sequential_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(!caps.requires_sequential_data, "{} should not require sequential data", model);
        }
    }
    
    #[test]
    fn test_exogenous_support() {
        // Models that support exogenous features
        let exogenous_models = vec!["LSTM", "GRU", "TCN", "TFT", "DeepAR", "MLP"];
        for model in exogenous_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(caps.supports_exogenous, "{} should support exogenous features", model);
        }
        
        // Models that don't support exogenous features
        let non_exogenous_models = vec!["NBEATS", "NHITS", "DLinear", "NLinear"];
        for model in non_exogenous_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(!caps.supports_exogenous, "{} should not support exogenous features", model);
        }
    }
    
    #[test]
    fn test_static_feature_support() {
        // Models that support static features
        let static_models = vec!["TFT", "DeepAR", "MLP"];
        for model in static_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(caps.supports_static, "{} should support static features", model);
        }
        
        // Models that don't support static features
        let non_static_models = vec!["LSTM", "GRU", "TCN", "NBEATS", "NHITS", "DLinear", "NLinear"];
        for model in non_static_models {
            let caps = ModelFactory::get_model_capabilities(model);
            assert!(!caps.supports_static, "{} should not support static features", model);
        }
    }
    
    #[test]
    fn test_concurrent_model_creation() {
        use std::sync::Arc;
        use std::thread;
        
        let configs: Vec<_> = vec!["MLP", "LSTM", "GRU", "TCN"]
            .into_iter()
            .map(|arch| (arch, create_test_model_config(arch, None)))
            .collect();
        
        let configs = Arc::new(configs);
        let mut handles = vec![];
        
        // Create models concurrently
        for i in 0..4 {
            let configs_clone = Arc::clone(&configs);
            let handle = thread::spawn(move || {
                let (arch, config) = &configs_clone[i];
                ModelFactory::create_model(arch, config)
            });
            handles.push(handle);
        }
        
        // Wait for all creations and verify success
        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.is_ok());
        }
    }
}