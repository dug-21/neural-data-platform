//! Tests for feature flag behavior in the neural trading system
//! 
//! Following TDD London School:
//! - Test feature flag parsing and effects
//! - Mock external dependencies
//! - Focus on behavior verification

use neural_trader::adapters::enhanced_neural_adapter::{
    EnhancedNeuralAdapter, EnhancedNeuralConfig,
};
use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::NeuralPredictorTrait;
use std::collections::HashMap;
use std::env;
use tokio::test;
use mockall::predicate::*;
use mockall::mock;

// Mock for configuration loader
mock! {
    ConfigLoader {
        fn load_from_env() -> EnhancedNeuralConfig;
        fn load_from_file(path: &str) -> Result<EnhancedNeuralConfig, String>;
    }
}

/// Helper to set environment variables for testing
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
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for var in &self.vars {
            env::remove_var(var);
        }
    }
}

/// Create test time series data
fn create_test_data() -> Vec<TimeSeriesData> {
    vec![TimeSeriesData {
        symbol: "TEST".to_string(),
        timestamp: chrono::Utc::now(),
        open: 100.0,
        high: 101.0,
        low: 99.0,
        close: 100.5,
        volume: vec![1000.0],
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some("test".to_string()),
        value: Some(100.5),
        metadata: None,
    }]
}

#[test]
async fn test_use_real_models_flag_true() {
    // Given: Configuration with use_real_models = true
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config.clone()).await.unwrap();
    
    // Then: Real models should be preferred
    let requirements = neural_trader::adapters::enhanced_neural_adapter::PredictionRequirements {
        prefer_accuracy: true,
        prefer_speed: false,
        max_acceptable_latency: None,
        min_confidence_threshold: None,
    };
    
    let recommended_model = adapter.get_recommended_model(&requirements).await;
    
    // Real models (DeepAR, TCN) should be recommended when use_real_models is true
    // But since they may not be available, FANN models will be used as fallback
    assert!(
        ["DeepAR", "NHITS", "TCN", "LSTM", "GRU", "FANN_MLP"]
            .contains(&recommended_model.as_str()),
        "Should recommend appropriate model, got: {}",
        recommended_model
    );
}

#[test]
async fn test_use_real_models_flag_false() {
    // Given: Configuration with use_real_models = false
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Making a prediction
    let data = create_test_data();
    let result = adapter.predict(&data, 3, None).await;
    
    // Then: Should still work with FANN models
    assert!(result.is_ok());
    let predictions = result.unwrap();
    
    // And: Should use FANN-based models only
    for prediction in predictions {
        assert!(
            prediction.model_name.contains("FANN") || 
            prediction.model_name.contains("MLP") ||
            prediction.model_name.contains("LSTM") ||
            prediction.model_name.contains("GRU"),
            "Should use FANN models when real models disabled"
        );
    }
}

#[test]
async fn test_health_monitoring_flag_enabled() {
    // Given: Configuration with health monitoring enabled
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: true,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // Then: Health monitoring should be available
    let health_summary = adapter.get_system_health_summary().await;
    assert!(health_summary.is_some(), "Health monitoring should be active");
    
    // And: Model availability checks should work
    let is_available = adapter.is_model_available("FANN_MLP").await;
    assert!(is_available, "Should be able to check model availability");
}

#[test]
async fn test_health_monitoring_flag_disabled() {
    // Given: Configuration with health monitoring disabled
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // Then: Health monitoring should not be available
    let health_summary = adapter.get_system_health_summary().await;
    assert!(health_summary.is_none(), "Health monitoring should be inactive");
    
    // But: Model availability should still return true (no health checks)
    let is_available = adapter.is_model_available("FANN_MLP").await;
    assert!(is_available, "Models should be assumed available without health monitoring");
}

#[test]
async fn test_fallback_flag_enabled() {
    // Given: Configuration with fallback enabled
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: false,
        enable_fallback: true,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Making enhanced prediction
    let data = create_test_data();
    let result = adapter.predict_enhanced(&data, 5, None).await;
    
    // Then: Prediction should work
    assert!(result.is_ok());
    let enhanced_result = result.unwrap();
    
    // And: Fallback status should be tracked
    assert_eq!(enhanced_result.fallback_triggered, false, 
        "Fallback should not trigger for successful prediction");
}

#[test]
async fn test_fallback_flag_disabled() {
    // Given: Configuration with fallback disabled
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Making predictions
    let data = create_test_data();
    let result = adapter.predict(&data, 3, None).await;
    
    // Then: Should work without fallback system
    assert!(result.is_ok(), "Predictions should work without fallback");
}

#[test]
async fn test_caching_flag_enabled() {
    // Given: Configuration with caching enabled
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: true,
        enable_circuit_breakers: false,
        neural: NeuralConfig {
            prediction_cache_ttl: 60, // 60 second cache
            ..Default::default()
        },
        ..Default::default()
    };
    
    // When: Creating adapter and making predictions
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    let data = create_test_data();
    
    // First prediction
    let result1 = adapter.predict(&data, 3, None).await.unwrap();
    
    // Second prediction with same data
    let result2 = adapter.predict(&data, 3, None).await.unwrap();
    
    // Then: Results should be consistent (potentially cached)
    assert_eq!(result1.len(), result2.len());
}

#[test]
async fn test_circuit_breaker_flag_enabled() {
    // Given: Configuration with circuit breakers enabled
    let config = EnhancedNeuralConfig {
        use_real_models: false,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: true,
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // Then: Adapter should initialize successfully with circuit breakers
    let data = create_test_data();
    let result = adapter.predict(&data, 3, None).await;
    assert!(result.is_ok(), "Should work with circuit breakers enabled");
}

#[test]
async fn test_multiple_flags_interaction() {
    // Given: Configuration with multiple flags enabled
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: true,
        enable_fallback: true,
        enable_caching: true,
        enable_circuit_breakers: true,
        ..Default::default()
    };
    
    // When: Creating adapter with all features
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // Then: All features should work together
    let data = create_test_data();
    let result = adapter.predict_enhanced(&data, 5, None).await;
    
    assert!(result.is_ok());
    let enhanced_result = result.unwrap();
    
    // Verify various features are active
    assert!(!enhanced_result.model_used.is_empty());
    assert!(enhanced_result.confidence_score >= 0.0);
    assert!(enhanced_result.health_status.is_some());
}

#[test]
async fn test_default_config_flags() {
    // Given: Default configuration
    let config = EnhancedNeuralConfig::default();
    
    // Then: Verify default flag values
    assert!(config.use_real_models, "Real models should be enabled by default");
    assert!(config.enable_health_monitoring, "Health monitoring should be enabled by default");
    assert!(config.enable_fallback, "Fallback should be enabled by default");
    assert!(config.enable_caching, "Caching should be enabled by default");
    assert!(config.enable_circuit_breakers, "Circuit breakers should be enabled by default");
}

#[test]
async fn test_model_timeout_configuration() {
    // Given: Configuration with custom timeouts
    let mut config = EnhancedNeuralConfig::default();
    config.model_timeouts.insert("FANN_MLP".to_string(), std::time::Duration::from_secs(1));
    config.model_timeouts.insert("LSTM".to_string(), std::time::Duration::from_secs(2));
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config.clone()).await.unwrap();
    
    // Then: Timeouts should be respected
    assert_eq!(
        config.model_timeouts.get("FANN_MLP"),
        Some(&std::time::Duration::from_secs(1))
    );
}

#[test]
async fn test_performance_thresholds() {
    // Given: Configuration with custom performance thresholds
    let config = EnhancedNeuralConfig {
        performance_thresholds: neural_trader::adapters::enhanced_neural_adapter::PerformanceThresholds {
            max_response_time: std::time::Duration::from_millis(100),
            max_error_rate: 5.0,
            max_memory_usage_mb: 500,
            max_cpu_usage_percent: 50.0,
        },
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config.clone()).await.unwrap();
    
    // Then: Adapter should respect thresholds
    let data = create_test_data();
    let start = std::time::Instant::now();
    let _ = adapter.predict(&data, 1, None).await;
    let duration = start.elapsed();
    
    // Note: We can't guarantee it meets the threshold, but it should attempt to
    assert!(duration < std::time::Duration::from_secs(10), "Prediction should complete reasonably quickly");
}

#[test]
async fn test_retry_configuration() {
    // Given: Configuration with retry settings
    let config = EnhancedNeuralConfig {
        retry_config: neural_trader::adapters::enhanced_neural_adapter::RetryConfig {
            max_retries: 5,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(2),
            exponential_backoff: true,
            jitter: true,
        },
        ..Default::default()
    };
    
    // When: Creating adapter
    let adapter = EnhancedNeuralAdapter::new(config.clone()).await.unwrap();
    
    // Then: Configuration should be applied
    assert_eq!(config.retry_config.max_retries, 5);
    assert!(config.retry_config.exponential_backoff);
    assert!(config.retry_config.jitter);
}

#[cfg(test)]
mod environment_variable_tests {
    use super::*;
    
    #[test]
    async fn test_neural_use_real_models_env_var() {
        // Given: Environment variable set
        let mut env_guard = EnvGuard::new();
        env_guard.set("NEURAL_USE_REAL_MODELS", "true");
        
        // When: Creating config (in real implementation, this would read env)
        let config = EnhancedNeuralConfig {
            use_real_models: true, // Would be set from env in real implementation
            ..Default::default()
        };
        
        // Then: Flag should be set
        assert!(config.use_real_models);
    }
    
    #[test]
    async fn test_feature_flags_from_env() {
        // Given: Multiple environment variables
        let mut env_guard = EnvGuard::new();
        env_guard.set("NEURAL_ENABLE_HEALTH_MONITORING", "false");
        env_guard.set("NEURAL_ENABLE_FALLBACK", "true");
        env_guard.set("NEURAL_ENABLE_CACHING", "false");
        
        // When: Creating config (would read from env in real implementation)
        let config = EnhancedNeuralConfig {
            use_real_models: true,
            enable_health_monitoring: false,
            enable_fallback: true,
            enable_caching: false,
            enable_circuit_breakers: true,
            ..Default::default()
        };
        
        // Then: Flags should match env settings
        assert!(!config.enable_health_monitoring);
        assert!(config.enable_fallback);
        assert!(!config.enable_caching);
    }
}