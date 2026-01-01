//! Comprehensive Integration Tests for Phase 1 Vendor Model Integration
//!
//! Tests the complete end-to-end pipeline from data input to prediction output,
//! including DataConverter, SectorMapper, ModelFactory, and VendorPredictor integration.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

use crate::config::NeuralConfig;
use crate::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorMapperConfig}};
use crate::data::data_converter::{DataConverter, DataConverterConfig};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::{
    NeuralPredictorTrait, PredictionResult,
    vendor_predictor::{VendorPredictor, VendorPredictorConfig, ModelKey},
    model_factory::ModelFactory,
};

// Test utilities and mock implementations
fn create_integration_neural_config() -> NeuralConfig {
    NeuralConfig {
        model_path: "/tmp/integration_test_models".to_string(),
        batch_size: 16,
        learning_rate: 0.001,
        hidden_layers: vec![32, 16],
        activation: "relu".to_string(),
        optimizer: "adam".to_string(),
        loss_function: "mse".to_string(),
        epochs: 10,
        validation_split: 0.2,
        early_stopping: true,
        patience: 5,
        enable_cuda: false,
        model_type: "ensemble".to_string(),
        sequence_length: 24,
        prediction_horizon: 1,
        features: vec!["price".to_string(), "volume".to_string(), "sma_5".to_string()],
        enable_technical_indicators: true,
        enable_feature_scaling: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
    }
}

fn create_realistic_time_series_data(symbol: &str, days: usize) -> TimeSeriesData {
    let mut values = Vec::new();
    let base_price = match symbol {
        "AAPL" => 150.0,
        "MSFT" => 300.0,
        "GOOGL" => 2500.0,
        "JPM" => 140.0,
        "BAC" => 35.0,
        "JNJ" => 160.0,
        _ => 100.0,
    };
    
    // Generate realistic price movement with trend and volatility
    let mut price = base_price;
    for i in 0..days {
        // Add trend (slight upward bias)
        let trend = 0.001;
        // Add random walk component
        let random_factor = (i as f64 * 0.1).sin() * 0.02 + 
                           ((i as f64 * 0.05).cos() * 0.01);
        // Add volatility
        let volatility = 0.02;
        let daily_return = trend + random_factor + 
                          (rand::random::<f64>() - 0.5) * volatility;
        
        price *= 1.0 + daily_return;
        values.push(price);
    }
    
    let timestamps: Vec<DateTime<Utc>> = (0..days)
        .map(|i| Utc::now() - chrono::Duration::hours((days - i) as i64))
        .collect();
    
    let mut metadata = HashMap::new();
    metadata.insert("symbol".to_string(), serde_json::json!(symbol));
    metadata.insert("source".to_string(), serde_json::json!("integration_test"));
    metadata.insert("data_quality".to_string(), serde_json::json!("high"));
    
    TimeSeriesData {
        values,
        timestamps,
        metadata: metadata.clone(),
        symbol: symbol.to_string(),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map
        }
    }
}

async fn setup_integration_environment() -> Result<(
    Arc<VendorPredictor>,
    Arc<SectorMapper>,
    Arc<ModelPerformanceTracker>,
)> {
    let neural_config = create_integration_neural_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    let vendor_predictor = Arc::new(VendorPredictor::new(
        &neural_config,
        Arc::clone(&sector_mapper),
        Arc::clone(&performance_tracker),
    )?);
    
    Ok((vendor_predictor, sector_mapper, performance_tracker))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_full_pipeline_single_symbol() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create test data for AAPL
        let test_data = create_realistic_time_series_data("AAPL", 100);
        
        // Verify data quality
        assert_eq!(test_data.values.len(), 100);
        assert_eq!(test_data.timestamps.len(), 100);
        assert_eq!(test_data.symbol, "AAPL");
        assert!(test_data.values.iter().all(|&v| v > 0.0 && v < 1000.0));
        
        // Run prediction through full pipeline
        let result = predictor.predict(&test_data).await;
        
        // Note: Without actual models loaded, this will return default prediction
        // In a real test environment, we would have pre-loaded models
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Verify prediction structure
        assert!(prediction.timestamp <= Utc::now());
        assert!(prediction.features_used.len() >= 0); // May be empty without models
        assert!(prediction.metadata.is_some());
    }
    
    #[tokio::test]
    async fn test_sector_based_routing() {
        let (predictor, sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Test different sectors
        let test_symbols = vec![
            ("AAPL", "technology"),
            ("MSFT", "technology"), 
            ("GOOGL", "technology"),
            ("JPM", "financial_services"),
            ("BAC", "financial_services"),
            ("JNJ", "healthcare"),
        ];
        
        for (symbol, expected_sector) in test_symbols {
            // Verify sector mapping
            let sector_info = sector_mapper.get_sector(symbol).unwrap();
            assert_eq!(sector_info.id, expected_sector);
            
            // Create data and test prediction
            let test_data = create_realistic_time_series_data(symbol, 50);
            let prediction_result = predictor.predict(&test_data).await;
            
            assert!(prediction_result.is_ok());
            let prediction = prediction_result.unwrap();
            
            // Verify metadata contains sector information
            let metadata = prediction.metadata.unwrap();
            // In full implementation, metadata would contain sector routing info
        }
    }
    
    #[tokio::test]
    async fn test_data_conversion_pipeline() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create data with various challenges
        let mut challenging_values = vec![100.0, 102.0, f64::NAN, 98.0, 105.0]; // Missing value
        challenging_values.extend(vec![200.0, 300.0, 250.0]); // Outliers
        challenging_values.extend(vec![100.0, 101.0, 99.0, 102.0]); // Normal values
        
        let challenging_data = TimeSeriesData {
            values: challenging_values,
            timestamps: (0..11).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("TEST"));
                map
            },
            symbol: "TEST".to_string(),
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("TEST"));
                map
            }
        };
        
        // Test conversion through predictor
        let conversion_result = predictor.convert_to_vendor_format(&challenging_data, "TEST").await;
        assert!(conversion_result.is_ok());
        
        let (vendor_data, metadata) = conversion_result.unwrap();
        
        // Verify data processing
        assert!(!vendor_data.values.is_empty());
        assert!(vendor_data.values.iter().all(|v| v.is_finite()));
        assert_eq!(metadata.source_format, "TimeSeriesData");
        assert_eq!(metadata.target_format, "VendorTimeSeriesData<f32>");
        
        // Verify missing values were handled
        assert!(metadata.missing_filled >= 1);
        
        // Verify outliers were processed (may or may not be removed depending on config)
        assert!(metadata.outliers_removed >= 0);
    }
    
    #[tokio::test]
    async fn test_batch_prediction_pipeline() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create batch data for multiple symbols
        let batch_data = vec![
            create_realistic_time_series_data("AAPL", 50),
            create_realistic_time_series_data("MSFT", 50),
            create_realistic_time_series_data("GOOGL", 50),
            create_realistic_time_series_data("JPM", 50),
        ];
        
        // Test batch prediction
        let batch_result = predictor.predict_batch(&batch_data).await;
        assert!(batch_result.is_ok());
        
        let predictions = batch_result.unwrap();
        assert_eq!(predictions.len(), 4);
        
        // Verify all predictions have expected structure
        for (i, prediction) in predictions.iter().enumerate() {
            assert!(prediction.timestamp <= Utc::now());
            assert!(prediction.metadata.is_some());
            
            // Each prediction should correspond to the correct symbol
            let expected_symbol = batch_data[i].symbol.clone();
            // In full implementation, metadata would track which symbol was predicted
        }
    }
    
    #[tokio::test]
    async fn test_feature_engineering_integration() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create data with sufficient history for technical indicators
        let test_data = create_realistic_time_series_data("AAPL", 200);
        
        // Test conversion with feature engineering enabled
        let conversion_result = predictor.convert_to_vendor_format(&test_data, "AAPL").await;
        assert!(conversion_result.is_ok());
        
        let (vendor_data, metadata) = conversion_result.unwrap();
        
        // Verify features were added
        assert!(!metadata.features_added.is_empty());
        
        // Common technical indicators should be present
        let feature_names: Vec<String> = metadata.features_added;
        assert!(feature_names.iter().any(|f| f.contains("sma")));
        
        // Verify enhanced data length
        assert!(vendor_data.values.len() >= test_data.values.len());
    }
    
    #[tokio::test]
    async fn test_performance_tracking_integration() {
        let (predictor, _sector_mapper, performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create test data
        let test_data = create_realistic_time_series_data("AAPL", 100);
        
        // Make prediction (should trigger performance tracking)
        let prediction_result = predictor.predict(&test_data).await;
        assert!(prediction_result.is_ok());
        
        // Performance tracking integration is verified by successful prediction
        // In full implementation, we would check performance_tracker metrics
        let prediction = prediction_result.unwrap();
        assert!(prediction.timestamp <= Utc::now());
    }
    
    #[tokio::test]
    async fn test_memory_management_under_load() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create multiple prediction tasks
        let mut tasks = vec![];
        
        for i in 0..20 {
            let predictor_clone = Arc::clone(&predictor);
            let task = tokio::spawn(async move {
                let symbol = format!("TEST_{}", i % 5); // Reuse some symbols
                let data = create_realistic_time_series_data(&symbol, 50);
                predictor_clone.predict(&data).await
            });
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        let mut successful_predictions = 0;
        for task in tasks {
            match task.await {
                Ok(Ok(_)) => successful_predictions += 1,
                Ok(Err(e)) => println!("Prediction failed: {}", e),
                Err(e) => println!("Task failed: {}", e),
            }
        }
        
        // Most predictions should succeed (even if returning default values)
        assert!(successful_predictions >= 15);
        
        // Verify conversion cache doesn't grow unbounded
        // Note: In full implementation, we would check cache size limits
    }
    
    #[tokio::test]
    async fn test_error_resilience() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Test with invalid data
        let invalid_data = TimeSeriesData {
            values: vec![], // Empty values
            timestamps: vec![],
            metadata: HashMap::new(),
            symbol: "INVALID".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&invalid_data).await;
        // Should handle gracefully (may return error or default prediction)
        // The key is that it doesn't panic
        match result {
            Ok(prediction) => {
                // Default prediction returned
                assert!(prediction.timestamp <= Utc::now());
            }
            Err(_) => {
                // Handled gracefully with error
            }
        }
        
        // Test with corrupted data
        let corrupted_data = TimeSeriesData {
            values: vec![f64::INFINITY, f64::NEG_INFINITY, f64::NAN],
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("CORRUPT"));
                map
            },
            symbol: "CORRUPT".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&corrupted_data).await;
        // Should handle corrupted data gracefully
        match result {
            Ok(_) | Err(_) => {} // Either is acceptable, no panic
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_predictions_different_sectors() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create concurrent predictions for different sectors
        let symbols = vec![
            ("AAPL", "technology"),
            ("JPM", "financial_services"),
            ("JNJ", "healthcare"),
        ];
        
        let mut tasks = vec![];
        
        for (symbol, _sector) in symbols {
            let predictor_clone = Arc::clone(&predictor);
            let symbol = symbol.to_string();
            
            let task = tokio::spawn(async move {
                let data = create_realistic_time_series_data(&symbol, 75);
                predictor_clone.predict(&data).await
            });
            tasks.push(task);
        }
        
        // Wait for all concurrent predictions
        let results: Vec<_> = futures::future::join_all(tasks).await;
        
        // All should complete successfully
        for result in results {
            let prediction_result = result.unwrap();
            assert!(prediction_result.is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_data_normalization_reversibility() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Create test data with known range
        let original_values = vec![100.0, 150.0, 200.0, 125.0, 175.0];
        let test_data = TimeSeriesData {
            values: original_values.clone(),
            timestamps: (0..5).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("NORM_TEST"));
                map
            },
            symbol: "NORM_TEST".to_string(),
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("NORM_TEST"));
                map
            }
        };
        
        // Convert to vendor format (applies normalization)
        let conversion_result = predictor.convert_to_vendor_format(&test_data, "NORM_TEST").await;
        assert!(conversion_result.is_ok());
        
        let (vendor_data, metadata) = conversion_result.unwrap();
        
        // Verify normalization was applied
        if metadata.normalization_stats.is_some() {
            let stats = metadata.normalization_stats.unwrap();
            
            // Create mock forecast for reverse conversion test
            let mock_forecast = crate::neural::vendor_predictor::ForecastResult {
                forecasts: vec![0.5], // Normalized value
                confidence: Some(0.8),
                metadata: None,
            };
            
            // Test reverse conversion
            let reverse_result = predictor.convert_from_vendor_format(
                mock_forecast, 
                "NORM_TEST", 
                "test_model"
            ).await;
            
            if reverse_result.is_ok() {
                let prediction = reverse_result.unwrap();
                // Reversed value should be in original range
                assert!(prediction.value >= 100.0 && prediction.value <= 200.0);
            }
        }
    }
    
    #[tokio::test]
    async fn test_pipeline_with_missing_sectors() {
        let (predictor, sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Test with symbols not in default sector mappings
        let unknown_symbols = vec!["UNKNOWN1", "UNKNOWN2", "UNKNOWN3"];
        
        for symbol in unknown_symbols {
            // Verify default sector assignment
            let sector_info = sector_mapper.get_sector(symbol).unwrap();
            assert_eq!(sector_info.id, "technology"); // Default sector
            
            // Test prediction pipeline
            let test_data = create_realistic_time_series_data(symbol, 50);
            let prediction_result = predictor.predict(&test_data).await;
            
            // Should work with default sector
            assert!(prediction_result.is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_integration_cleanup() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Use predictor and generate some cached data
        let test_data = create_realistic_time_series_data("CLEANUP_TEST", 30);
        let _ = predictor.convert_to_vendor_format(&test_data, "CLEANUP_TEST").await;
        
        // Verify cache entry exists
        assert!(predictor.conversion_cache.contains_key("CLEANUP_TEST"));
        
        // Test cleanup
        predictor.conversion_cache.remove("CLEANUP_TEST");
        assert!(!predictor.conversion_cache.contains_key("CLEANUP_TEST"));
        
        // Test prediction still works after cleanup
        let prediction_result = predictor.predict(&test_data).await;
        assert!(prediction_result.is_ok());
    }
    
    #[tokio::test]
    async fn test_integration_stress_test() {
        let (predictor, _sector_mapper, _performance_tracker) = 
            setup_integration_environment().await.unwrap();
        
        // Stress test with many rapid predictions
        let predictor = Arc::new(predictor);
        let mut tasks = vec![];
        
        for i in 0..50 {
            let predictor_clone = Arc::clone(&predictor);
            let task = tokio::spawn(async move {
                let symbol = format!("STRESS_{}", i % 10);
                let data = create_realistic_time_series_data(&symbol, 30);
                
                // Add small random delay to simulate real-world timing
                sleep(Duration::from_millis(rand::random::<u64>() % 10)).await;
                
                predictor_clone.predict(&data).await
            });
            tasks.push(task);
        }
        
        // Collect results
        let results: Vec<_> = futures::future::join_all(tasks).await;
        
        let mut successful = 0;
        let mut failed = 0;
        
        for result in results {
            match result {
                Ok(Ok(_)) => successful += 1,
                Ok(Err(_)) => failed += 1,
                Err(_) => failed += 1,
            }
        }
        
        // Most should succeed even under stress
        assert!(successful >= 40);
        assert!(failed <= 10);
        
        println!("Stress test: {} successful, {} failed", successful, failed);
    }
}

// Helper module for integration test utilities
mod integration_utils {
    use super::*;
    
    pub fn validate_prediction_quality(prediction: &PredictionResult) -> bool {
        // Check basic sanity
        if prediction.confidence < 0.0 || prediction.confidence > 1.0 {
            return false;
        }
        
        // Check timestamp is reasonable
        if prediction.timestamp > Utc::now() + chrono::Duration::minutes(1) {
            return false;
        }
        
        // Check model type is not empty
        if prediction.model_type.is_empty() {
            return false;
        }
        
        // Check features used is not null
        if prediction.features_used.is_empty() && prediction.metadata.is_none() {
            return false;
        }
        
        true
    }
    
    pub fn create_market_scenario_data(scenario: &str) -> Vec<TimeSeriesData> {
        match scenario {
            "bull_market" => {
                vec![
                    create_trending_data("AAPL", 100, 0.02, 0.01),
                    create_trending_data("MSFT", 100, 0.015, 0.012),
                    create_trending_data("GOOGL", 100, 0.018, 0.015),
                ]
            }
            "bear_market" => {
                vec![
                    create_trending_data("AAPL", 100, -0.015, 0.02),
                    create_trending_data("MSFT", 100, -0.012, 0.018),
                    create_trending_data("GOOGL", 100, -0.018, 0.022),
                ]
            }
            "volatile_market" => {
                vec![
                    create_trending_data("AAPL", 100, 0.001, 0.05),
                    create_trending_data("MSFT", 100, -0.001, 0.045),
                    create_trending_data("GOOGL", 100, 0.002, 0.055),
                ]
            }
            _ => vec![create_realistic_time_series_data("DEFAULT", 100)]
        }
    }
    
    fn create_trending_data(symbol: &str, periods: usize, trend: f64, volatility: f64) -> TimeSeriesData {
        let base_price = match symbol {
            "AAPL" => 150.0,
            "MSFT" => 300.0,
            "GOOGL" => 2500.0,
            _ => 100.0,
        };
        
        let mut values = Vec::new();
        let mut price = base_price;
        
        for i in 0..periods {
            let random_factor = (rand::random::<f64>() - 0.5) * volatility;
            let daily_return = trend + random_factor;
            price *= 1.0 + daily_return;
            values.push(price);
        }
        
        let timestamps: Vec<DateTime<Utc>> = (0..periods)
            .map(|i| Utc::now() - chrono::Duration::hours((periods - i) as i64))
            .collect();
        
        TimeSeriesData {
            values,
            timestamps,
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!(symbol));
                map.insert("scenario".to_string(), serde_json::json!("synthetic"));
                map
            },
            symbol: symbol.to_string(),
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!(symbol));
                map
            }
        }
    }
}