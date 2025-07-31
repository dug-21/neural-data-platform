//! Unit tests for neural adapter initialization and core functionality
//! 
//! This module tests the neural adapter components including:
//! - Adapter initialization with various configurations
//! - Error handling for invalid configurations
//! - Connection lifecycle management
//! - State transitions and validation

use super::super::fann::{FannPredictor, FannModelConfig};
use crate::config::NeuralConfig;
use crate::adapters::enhanced_neural_adapter::{
    EnhancedNeuralAdapter,
    EnhancedNeuralConfig,
};
use crate::adapters::{DataAdapter, AdapterError};
use std::sync::Arc;
use anyhow::Result;
use tokio;

/// Helper function to create test neural config
fn create_test_neural_config(use_real_models: bool) -> NeuralConfig {
    NeuralConfig {
        memory_gb: 1.0,
        models: vec!["TimeMixer".to_string(), "NeuralForecast".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: true,
        model_timeout_seconds: 30,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    }
}

/// Helper function to create enhanced neural config
fn create_model_config(model_type: &str) -> EnhancedNeuralConfig {
    let mut neural_config = create_test_neural_config(false);
    neural_config.models = vec![model_type.to_string()];
    
    EnhancedNeuralConfig {
        neural: neural_config,
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    }
}

mod adapter_initialization_tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_adapter_creation() -> Result<()> {
        let config = create_model_config("TimeMixer");
        let adapter = EnhancedNeuralAdapter::new(config.clone()).await?;
        
        // Verify adapter is created with correct configuration
        assert_eq!(adapter.name(), "EnhancedNeuralAdapter");
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_adapter_connection_lifecycle() -> Result<()> {
        let config = create_model_config("NeuralForecast");
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        
        // Test initial state (EnhancedNeuralAdapter starts connected)
        assert!(adapter.is_connected());
        
        // Test disconnection  
        adapter.disconnect().await?;
        assert!(!adapter.is_connected());
        
        // Test reconnection
        adapter.connect().await?;
        assert!(adapter.is_connected());
        
        // Test repeated connection (should be idempotent)
        adapter.connect().await?;
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_configuration_handling() -> Result<()> {
        // Test with zero lookback window
        let mut config = create_model_config("TimesFM");
        config.lookback_window = 0;
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        let result = adapter.connect().await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Lookback window"));
        
        // Test with zero forecast horizon
        let mut config = create_model_config("TimesFM");
        config.forecast_horizon = 0;
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        let result = adapter.connect().await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Forecast horizon"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_adapter_reconnection_after_failure() -> Result<()> {
        // Create adapter with invalid config
        let mut config = create_model_config("DeepAR");
        config.lookback_window = 0;
        
        let mut adapter = EnhancedNeuralAdapter::new(config.clone()).await?;
        
        // First connection should fail
        let result = adapter.connect().await;
        assert!(result.is_err());
        assert!(!adapter.is_connected());
        
        // Fix configuration
        config.lookback_window = 24;
        adapter.update_config(config).await?;
        
        // Reconnection should succeed
        adapter.connect().await?;
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_fann_predictor_with_adapters() -> Result<()> {
        // Test with real models enabled
        let config = create_test_neural_config(true);
        let predictor = FannPredictor::new(config)?;
        
        assert!(predictor.has_neuro_divergent_adapter());
        
        // Test adapter status
        let status = predictor.get_enhanced_adapter_status().await;
        assert!(status.is_some());
        assert!(status.unwrap().contains("Connected: false")); // Not connected initially
        
        Ok(())
    }

    #[tokio::test]
    async fn test_fann_predictor_without_adapters() -> Result<()> {
        // Test with real models disabled
        let config = create_test_neural_config(false);
        let predictor = FannPredictor::new(config)?;
        
        assert!(!predictor.has_neuro_divergent_adapter());
        
        // Test adapter status
        let status = predictor.get_enhanced_adapter_status().await;
        assert!(status.is_none());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_model_specific_configurations() -> Result<()> {
        // Test TimeMixer configuration
        let timemixer_config = create_model_config("TimeMixer");
        let adapter = EnhancedNeuralAdapter::new(timemixer_config).await?;
        assert_eq!(adapter.name(), "NeuroDivergentAdapter");
        
        // Test NeuralForecast configuration
        let neural_forecast_config = create_model_config("NeuralForecast");
        let adapter = EnhancedNeuralAdapter::new(neural_forecast_config).await?;
        assert_eq!(adapter.name(), "NeuroDivergentAdapter");
        
        // Test TimesFM configuration
        let timesfm_config = create_model_config("TimesFM");
        let adapter = EnhancedNeuralAdapter::new(timesfm_config).await?;
        assert_eq!(adapter.name(), "NeuroDivergentAdapter");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_adapter_operations() -> Result<()> {
        let config = create_model_config("TimeMixer");
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        
        // Connect adapter
        adapter.connect().await?;
        
        // Spawn multiple tasks to check connection status
        let adapter_ref = &adapter;
        let tasks: Vec<_> = (0..10)
            .map(|_| async move {
                adapter_ref.is_connected()
            })
            .collect();
        
        // All tasks should report connected
        for task in tasks {
            assert!(task.await);
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_adapter_state_transitions() -> Result<()> {
        let config = create_model_config("DeepAR");
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        
        // Track state transitions
        assert!(!adapter.is_connected()); // Uninitialized
        
        adapter.connect().await?;
        assert!(adapter.is_connected()); // Ready
        
        adapter.disconnect().await?;
        assert!(!adapter.is_connected()); // Uninitialized
        
        // Multiple disconnects should be safe
        adapter.disconnect().await?;
        assert!(!adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_enhanced_adapter_initialization() -> Result<()> {
        let config = create_test_neural_config(true);
        let predictor = FannPredictor::new(config)?;
        
        // Initialize enhanced adapter
        predictor.init_enhanced_adapter().await?;
        
        // Check status after initialization
        let status = predictor.get_enhanced_adapter_status().await;
        assert!(status.is_some());
        assert!(status.unwrap().contains("Connected: true"));
        
        Ok(())
    }
}

mod configuration_update_tests {
    use super::*;

    #[tokio::test]
    async fn test_config_update_while_disconnected() -> Result<()> {
        let initial_config = create_model_config("TimeMixer");
        let mut adapter = EnhancedNeuralAdapter::new(initial_config);
        
        // Update config while disconnected (should succeed)
        let new_config = create_model_config("NeuralForecast");
        adapter.update_config(new_config).await?;
        
        // Verify update succeeded by connecting
        adapter.connect().await?;
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_config_update_while_connected() -> Result<()> {
        let initial_config = create_model_config("TimeMixer");
        let mut adapter = EnhancedNeuralAdapter::new(initial_config);
        
        // Connect adapter
        adapter.connect().await?;
        assert!(adapter.is_connected());
        
        // Try to update config while connected (should fail)
        let new_config = create_model_config("NeuralForecast");
        let result = adapter.update_config(new_config).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot update config while model is ready"));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_gpu_configuration() -> Result<()> {
        let mut config = create_model_config("TimeMixer");
        config.use_gpu = true;
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        // GPU configuration should be accepted (actual GPU usage depends on hardware)
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_size_configuration() -> Result<()> {
        let batch_sizes = vec![1, 16, 32, 64, 128];
        
        for batch_size in batch_sizes {
            let mut config = create_model_config("NeuralForecast");
            config.batch_size = batch_size;
            
            let mut adapter = EnhancedNeuralAdapter::new(config).await?;
            adapter.connect().await?;
            
            assert!(adapter.is_connected());
            adapter.disconnect().await?;
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_model_params_configuration() -> Result<()> {
        let mut config = create_model_config("TimesFM");
        config.model_params = serde_json::json!({
            "num_layers": 6,
            "hidden_size": 768,
            "num_attention_heads": 12,
            "dropout_rate": 0.1,
            "use_layer_norm": true,
        });
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }
}

mod edge_case_tests {
    use super::*;

    #[tokio::test]
    async fn test_extreme_lookback_window() -> Result<()> {
        let mut config = create_model_config("TimeMixer");
        config.lookback_window = 10_000; // Very large lookback
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_extreme_forecast_horizon() -> Result<()> {
        let mut config = create_model_config("NeuralForecast");
        config.forecast_horizon = 1_000; // Very large horizon
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_minimum_valid_configuration() -> Result<()> {
        let mut config = create_model_config("DeepAR");
        config.lookback_window = 1;
        config.forecast_horizon = 1;
        config.batch_size = 1;
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_unicode_model_params() -> Result<()> {
        let mut config = create_model_config("TimeMixer");
        config.model_params = serde_json::json!({
            "description": "测试配置 🚀",
            "name": "модель",
            "tags": ["تجربة", "テスト"],
        });
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_empty_model_params() -> Result<()> {
        let mut config = create_model_config("NeuralForecast");
        config.model_params = serde_json::json!({});
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }

    #[tokio::test]
    async fn test_null_model_params() -> Result<()> {
        let mut config = create_model_config("TimesFM");
        config.model_params = serde_json::json!(null);
        
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        adapter.connect().await?;
        
        assert!(adapter.is_connected());
        
        Ok(())
    }
}

/// Test to verify comprehensive coverage of neural adapter initialization
#[tokio::test]
async fn test_neural_adapter_initialization_coverage() -> Result<()> {
    println!("🧪 Testing Neural Adapter Initialization Coverage");
    
    // Test all supported model types
    let model_types = vec![
        "TimeMixer",
        "NeuralForecast", 
        "TimesFM",
        "DeepAR",
        "NHITS",
        "TCN",
    ];
    
    for model_type in model_types {
        let config = create_model_config(model_type);
        let mut adapter = EnhancedNeuralAdapter::new(config).await?;
        
        // Test connection
        adapter.connect().await?;
        assert!(adapter.is_connected(), "Model {} failed to connect", model_type);
        
        // Test disconnection
        adapter.disconnect().await?;
        assert!(!adapter.is_connected(), "Model {} failed to disconnect", model_type);
        
        println!("✅ {} adapter lifecycle test passed", model_type);
    }
    
    println!("✅ All neural adapter initialization tests passed - comprehensive coverage achieved");
    
    Ok(())
}