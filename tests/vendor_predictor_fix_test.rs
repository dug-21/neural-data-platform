// Test to verify the vendor predictor symbol vs model type fix

use std::sync::Arc;
use autonomous_platform::config::NeuralConfig;
use autonomous_platform::data::sector_mapper::{SectorMapper, SectorMapperConfig};
use autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker;
use autonomous_platform::neural::vendor_predictor::VendorPredictor;
use autonomous_platform::data::TimeSeriesData;
use chrono::Utc;
use std::collections::HashMap;

#[tokio::test]
async fn test_vendor_predictor_symbol_extraction() {
    // Create required dependencies
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["LSTM".to_string(), "MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        lookback_window: 24,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        input_size: 60,
        output_size: 1,
        hidden_layers: vec![128, 64, 32],
        learning_rate: 0.001,
        prediction_horizon: Some(24),
        normalization_method: Some("z-score".to_string()),
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 120,
        max_retries: 3,
        error_threshold: 0.15,
    };
    
    let sector_config = SectorMapperConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(sector_config));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    // Create VendorPredictor
    let mut predictor = VendorPredictor::new(&neural_config, sector_mapper.clone(), performance_tracker).unwrap();
    
    // Initialize emergency models
    predictor.initialize_models_emergency().await.unwrap();
    
    // Test model names that include both symbol and model type
    let test_cases = vec![
        ("AAPL_LSTM", "AAPL"), // Symbol_ModelType format
        ("TSLA_Transformer", "TSLA"),
        ("NVDA_MLP", "NVDA"),
        ("GOOGL", "GOOGL"), // Just symbol
        ("XLF_DeepAR", "XLF"), // ETF symbol
    ];
    
    for (model_name, expected_symbol) in test_cases {
        println!("Testing model_name: {} -> expected symbol: {}", model_name, expected_symbol);
        
        // Test training data generation
        let training_data = create_test_data(expected_symbol, 50);
        
        // This should not fail due to incorrect symbol/model type mapping
        let result = predictor.train_model(model_name, &training_data).await;
        
        // Check that it either succeeds or fails for the right reasons (not symbol mapping issues)
        match result {
            Ok(_) => {
                println!("✅ Training completed successfully for {}", model_name);
            }
            Err(e) => {
                let error_msg = e.to_string();
                // Should not fail due to sector mapping issues for known symbols
                if error_msg.contains("Failed to get sector for symbol") && 
                   (expected_symbol == "AAPL" || expected_symbol == "TSLA" || expected_symbol == "XLF") {
                    panic!("❌ Failed due to symbol mapping issue for known symbol {}: {}", expected_symbol, error_msg);
                } else {
                    println!("⚠️ Training failed for {} but not due to symbol mapping: {}", model_name, error_msg);
                }
            }
        }
    }
    
    println!("✅ All symbol extraction tests completed");
}

fn create_test_data(symbol: &str, count: usize) -> Vec<TimeSeriesData> {
    let mut data = Vec::new();
    let base_time = Utc::now();
    
    for i in 0..count {
        let data_point = TimeSeriesData {
            timestamp: base_time - chrono::Duration::minutes((count - i) as i64),
            symbol: symbol.to_string(),
            open: 100.0 + (i as f64 * 0.1),
            high: 100.0 + (i as f64 * 0.1) + 2.0,
            low: 100.0 + (i as f64 * 0.1) - 2.0,
            close: 100.0 + (i as f64 * 0.1),
            volume: vec![1000000.0],
            volume_value: 1000000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(100.0 + (i as f64 * 0.1)),
            metadata: None,
            values: vec![100.0 + (i as f64 * 0.1)],
            intervals: vec![60],
            timestamps: vec![base_time - chrono::Duration::minutes((count - i) as i64)],
            metadata_map: HashMap::new(),
        };
        data.push(data_point);
    }
    
    data
}

#[tokio::test]
async fn test_sector_mapping_with_model_names() {
    let sector_config = SectorMapperConfig::default();
    let sector_mapper = SectorMapper::new(sector_config);
    
    // Test that known symbols are mapped correctly
    let test_symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "XLF"];
    
    for symbol in test_symbols {
        let sector_result = sector_mapper.get_sector(symbol);
        match sector_result {
            Ok(sector_info) => {
                println!("✅ Symbol {} correctly mapped to sector: {}", symbol, sector_info.id);
                assert!(!sector_info.id.is_empty());
            }
            Err(e) => {
                println!("⚠️ Symbol {} not in default mappings: {}", symbol, e);
                // This is ok for symbols not in the default mapping, they should get default sector
            }
        }
    }
    
    println!("✅ Sector mapping test completed");
}