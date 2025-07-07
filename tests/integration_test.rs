//! Integration test for Neural Trader application
//! 
//! Tests the full compilation and basic functionality

use autonomous_platform::config::{Config, DatabaseConfig, RedisConfig, NeuralConfig};
use autonomous_platform::neural::NeuralPredictor;
use autonomous_platform::agents::AutonomousAgent;
use autonomous_platform::data::TimeSeriesData;
use chrono::Utc;
use std::collections::HashMap;

#[tokio::test]
async fn test_neural_trader_components() {
    // Test neural predictor initialization
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
    };
    
    let predictor = NeuralPredictor::new(neural_config).expect("Failed to create neural predictor");
    
    // Test autonomous agent initialization
    let agent = AutonomousAgent::default();
    
    // Test with sample data
    let sample_data = vec![
        TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now(),
            open: 50000.0,
            high: 50100.0,
            low: 49900.0,
            close: 50050.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(50050.0),
            metadata: None,
        }
    ];
    
    // Test neural prediction
    let predictions = predictor.predict(&sample_data, 5, None).await;
    assert!(predictions.is_ok(), "Neural prediction should work");
    
    let predictions = predictions.unwrap();
    assert_eq!(predictions.len(), 5, "Should return 5 predictions");
    
    println!("✅ Neural Trader components test passed");
    println!("✅ Neural predictor: OK");
    println!("✅ Autonomous agent: OK");
    println!("✅ Predictions generated: {} steps", predictions.len());
}

#[test]
fn test_config_creation() {
    let config = Config {
        database: DatabaseConfig {
            host: "localhost".to_string(),
            port: 5432,
            username: "test".to_string(),
            password: "test".to_string(),
            database: "neural_trader".to_string(),
            max_connections: 10,
            connection_timeout: 30,
            query_timeout: 30,
            enable_migrations: true,
            pool_idle_timeout: 600,
        },
        redis: RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            database: 0,
            max_connections: 10,
            connection_timeout: 5,
            command_timeout: 5,
            enable_cluster: false,
            key_prefix: "neural_trader".to_string(),
        },
        neural: NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        },
    };
    
    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.redis.port, 6379);
    assert_eq!(config.neural.models[0], "MLP");
    
    println!("✅ Configuration creation test passed");
}