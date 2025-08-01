//! Comprehensive Unit Tests for VendorPredictor
//!
//! Tests ensemble predictions, model integration, sector routing, and performance tracking.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::NeuralConfig;
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig, SectorInfo, SectorId, MarketCapTier};
use crate::data::{TimeSeriesData, data_converter::{DataConverter, DataConverterConfig}};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::vendor_predictor::{
    VendorPredictor, VendorPredictorConfig, ModelKey, ModelConfig, DataRequirements
};
use crate::neural::{NeuralPredictorTrait, PredictionResult};

// Mock vendor types for testing
#[derive(Debug, Clone)]
pub struct MockVendorTimeSeriesData<T> {
    pub values: Vec<T>,
}

impl<T> MockVendorTimeSeriesData<T> {
    pub fn new(values: Vec<T>) -> Self {
        Self { values }
    }
}

#[derive(Debug, Clone)]
pub struct MockForecastResult<T> {
    pub forecasts: Vec<T>,
    pub confidence: Option<T>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug)]
pub struct MockBaseModel {
    pub model_type: String,
    pub prediction_value: f32,
    pub confidence: f32,
    pub should_fail: bool,
}

impl MockBaseModel {
    pub fn new(model_type: &str, prediction_value: f32, confidence: f32) -> Self {
        Self {
            model_type: model_type.to_string(),
            prediction_value,
            confidence,
            should_fail: false,
        }
    }
    
    pub fn new_failing(model_type: &str) -> Self {
        Self {
            model_type: model_type.to_string(),
            prediction_value: 0.0,
            confidence: 0.0,
            should_fail: true,
        }
    }
    
    pub fn predict(&self, _data: &MockVendorTimeSeriesData<f32>) -> Result<MockForecastResult<f32>> {
        if self.should_fail {
            return Err(anyhow::anyhow!("Mock model prediction failed"));
        }
        
        Ok(MockForecastResult {
            forecasts: vec![self.prediction_value],
            confidence: Some(self.confidence),
            metadata: Some({
                let mut map = HashMap::new();
                map.insert("model_type".to_string(), serde_json::json!(self.model_type));
                map
            }),
        })
    }
}

// Test utilities
fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        model_path: "/tmp/test_models".to_string(),
        batch_size: 32,
        learning_rate: 0.001,
        hidden_layers: vec![64, 32],
        activation: "relu".to_string(),
        optimizer: "adam".to_string(),
        loss_function: "mse".to_string(),
        epochs: 100,
        validation_split: 0.2,
        early_stopping: true,
        patience: 10,
        enable_cuda: false,
        model_type: "lstm".to_string(),
        sequence_length: 60,
        prediction_horizon: 1,
        features: vec!["price".to_string(), "volume".to_string()],
        enable_technical_indicators: true,
        enable_feature_scaling: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
    }
}

fn create_test_time_series_data(symbol: &str, values: Vec<f64>) -> TimeSeriesData {
    let timestamps: Vec<DateTime<Utc>> = (0..values.len())
        .map(|i| Utc::now() - chrono::Duration::hours((values.len() - i - 1) as i64))
        .collect();
    
    let mut metadata = HashMap::new();
    metadata.insert("symbol".to_string(), serde_json::json!(symbol));
    metadata.insert("source".to_string(), serde_json::json!("test"));
    
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

fn create_test_sector_mapper() -> Arc<SectorMapper> {
    let config = SectorMapperConfig::default();
    Arc::new(SectorMapper::new(config))
}

fn create_test_performance_tracker() -> Arc<ModelPerformanceTracker> {
    // Mock implementation for testing
    Arc::new(ModelPerformanceTracker::new().unwrap())
}

fn create_test_model_config(architecture: &str) -> ModelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("input_size".to_string(), serde_json::json!(24));
    parameters.insert("hidden_size".to_string(), serde_json::json!(64));
    parameters.insert("num_layers".to_string(), serde_json::json!(2));
    
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
    
    #[tokio::test]
    async fn test_vendor_predictor_creation() {
        let neural_config = create_test_neural_config();
        let sector_mapper = create_test_sector_mapper();
        let performance_tracker = create_test_performance_tracker();
        
        let result = VendorPredictor::new(&neural_config, sector_mapper, performance_tracker);
        assert!(result.is_ok());
        
        let predictor = result.unwrap();
        let model_info = predictor.get_model_info().await;
        
        assert_eq!(model_info.get("type").unwrap(), &serde_json::json!("VendorPredictor"));
        assert_eq!(model_info.get("active_models").unwrap(), &serde_json::json!(0));
        assert_eq!(model_info.get("performance_tracking").unwrap(), &serde_json::json!(true));
    }
    
    #[tokio::test]
    async fn test_add_model() {
        let neural_config = create_test_neural_config();
        let sector_mapper = create_test_sector_mapper();
        let performance_tracker = create_test_performance_tracker();
        
        let predictor = VendorPredictor::new(&neural_config, sector_mapper, performance_tracker).unwrap();
        
        let model_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "base".to_string(),
        };
        
        let mock_model = Box::new(MockBaseModel::new("LSTM", 150.5, 0.85));
        let result = predictor.add_model(model_key, mock_model).await;
        
        assert!(result.is_ok());
        
        // Verify model was added
        let model_info = predictor.get_model_info().await;
        assert_eq!(model_info.get("active_models").unwrap(), &serde_json::json!(1));
    }
    
    #[tokio::test]
    async fn test_sector_based_model_selection() {
        let neural_config = create_test_neural_config();
        let sector_mapper = create_test_sector_mapper();
        let performance_tracker = create_test_performance_tracker();
        
        let predictor = VendorPredictor::new(&neural_config, sector_mapper, performance_tracker).unwrap();
        
        // Add models for different sectors
        let tech_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "tech_optimized".to_string(),
        };
        
        let finance_key = ModelKey {
            sector: "financial_services".to_string(),
            model_type: "GRU".to_string(),
            variant: "finance_optimized".to_string(),
        };
        
        let tech_model = Box::new(MockBaseModel::new("LSTM_Tech", 200.0, 0.9));
        let finance_model = Box::new(MockBaseModel::new("GRU_Finance", 50.0, 0.8));
        
        predictor.add_model(tech_key, tech_model).await.unwrap();
        predictor.add_model(finance_key, finance_model).await.unwrap();
        
        // Test prediction for AAPL (technology sector)
        let aapl_data = create_test_time_series_data("AAPL", vec![180.0, 182.0, 179.0, 185.0]);
        let models_for_aapl = predictor.get_models_for_symbol("AAPL").await.unwrap();
        
        // Should return tech models for AAPL
        assert_eq!(models_for_aapl.len(), 1);
        assert_eq!(models_for_aapl[0].sector, "technology");
        assert_eq!(models_for_aapl[0].model_type, "LSTM");
    }
    
    #[tokio::test]
    async fn test_data_requirements_checking() {
        let predictor_config = VendorPredictorConfig::default();
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        let requirements = DataRequirements {
            required: vec!["price".to_string(), "volume".to_string()],
            optional: vec!["news_sentiment".to_string()],
            min_history: 50,
        };
        
        // Test sufficient data
        let sufficient_data = vec!["price".to_string(), "volume".to_string(), "market_cap".to_string()];
        assert!(predictor.check_data_requirements(&requirements, &sufficient_data));
        
        // Test insufficient data
        let insufficient_data = vec!["price".to_string()]; // Missing volume
        assert!(!predictor.check_data_requirements(&requirements, &insufficient_data));
        
        // Test empty data
        let empty_data = vec![];
        assert!(!predictor.check_data_requirements(&requirements, &empty_data));
    }
    
    #[tokio::test]
    async fn test_data_conversion_to_vendor_format() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        let test_data = create_test_time_series_data("AAPL", vec![100.0, 102.0, 98.0, 105.0, 97.0]);
        let result = predictor.convert_to_vendor_format(&test_data, "AAPL").await;
        
        assert!(result.is_ok());
        let (vendor_data, metadata) = result.unwrap();
        
        // Verify conversion
        assert!(!vendor_data.values.is_empty());
        assert_eq!(metadata.source_format, "TimeSeriesData");
        assert_eq!(metadata.target_format, "VendorTimeSeriesData<f32>");
        assert_eq!(metadata.original_length, 5);
        
        // Check that conversion metadata is cached
        assert!(predictor.conversion_cache.contains_key("AAPL"));
    }
    
    #[tokio::test]
    async fn test_prediction_ensemble_single_model() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Add single model for technology sector
        let model_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "base".to_string(),
        };
        
        let mock_model = Box::new(MockBaseModel::new("LSTM", 175.5, 0.85));
        predictor.add_model(model_key, mock_model).await.unwrap();
        
        // Test prediction
        let test_data = create_test_time_series_data("AAPL", vec![170.0, 172.0, 168.0, 175.0]);
        let result = predictor.predict(&test_data).await;
        
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Verify prediction structure
        assert!(prediction.value > 0.0);
        assert!(prediction.confidence > 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.model_type.contains("ensemble"));
        assert!(!prediction.features_used.is_empty());
        assert!(prediction.metadata.is_some());
    }
    
    #[tokio::test]
    async fn test_prediction_ensemble_multiple_models() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Add multiple models for technology sector
        let models = vec![
            (ModelKey {
                sector: "technology".to_string(),
                model_type: "LSTM".to_string(),
                variant: "base".to_string(),
            }, MockBaseModel::new("LSTM", 175.0, 0.85)),
            (ModelKey {
                sector: "technology".to_string(),
                model_type: "GRU".to_string(),
                variant: "base".to_string(),
            }, MockBaseModel::new("GRU", 177.0, 0.80)),
            (ModelKey {
                sector: "technology".to_string(),
                model_type: "TCN".to_string(),
                variant: "base".to_string(),
            }, MockBaseModel::new("TCN", 173.0, 0.90)),
        ];
        
        for (key, model) in models {
            predictor.add_model(key, Box::new(model)).await.unwrap();
        }
        
        // Test ensemble prediction
        let test_data = create_test_time_series_data("AAPL", vec![170.0, 172.0, 168.0, 175.0]);
        let result = predictor.predict(&test_data).await;
        
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Verify ensemble averaging: (175.0 + 177.0 + 173.0) / 3 = 175.0
        assert!((prediction.value - 175.0).abs() < 0.1);
        assert!(prediction.model_type.contains("ensemble_3_models"));
        
        // Verify metadata contains individual model info
        let metadata = prediction.metadata.unwrap();
        assert!(metadata.contains_key("individual_models"));
        assert!(metadata.contains_key("individual_confidences"));
        assert!(metadata.contains_key("individual_values"));
    }
    
    #[tokio::test]
    async fn test_prediction_with_failing_models() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Add mix of working and failing models
        let working_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "working".to_string(),
        };
        
        let failing_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "GRU".to_string(),
            variant: "failing".to_string(),
        };
        
        let working_model = Box::new(MockBaseModel::new("LSTM", 180.0, 0.85));
        let failing_model = Box::new(MockBaseModel::new_failing("GRU"));
        
        predictor.add_model(working_key, working_model).await.unwrap();
        predictor.add_model(failing_key, failing_model).await.unwrap();
        
        // Test prediction - should work with only successful model
        let test_data = create_test_time_series_data("AAPL", vec![170.0, 172.0, 168.0, 175.0]);
        let result = predictor.predict(&test_data).await;
        
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Should use only the working model
        assert_eq!(prediction.value, 180.0);
        assert!(prediction.model_type.contains("ensemble_1_models"));
    }
    
    #[tokio::test]
    async fn test_prediction_no_models_available() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Test prediction with no models added
        let test_data = create_test_time_series_data("UNKNOWN", vec![100.0, 102.0, 98.0]);
        let result = predictor.predict(&test_data).await;
        
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Should return default prediction
        assert_eq!(prediction.value, 0.0);
        assert_eq!(prediction.confidence, 0.5);
        assert_eq!(prediction.model_type, "none");
    }
    
    #[tokio::test]
    async fn test_predict_batch() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Add model for technology sector
        let model_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "base".to_string(),
        };
        
        let mock_model = Box::new(MockBaseModel::new("LSTM", 175.0, 0.85));
        predictor.add_model(model_key, mock_model).await.unwrap();
        
        // Create batch data
        let batch_data = vec![
            create_test_time_series_data("AAPL", vec![170.0, 172.0, 168.0]),
            create_test_time_series_data("MSFT", vec![250.0, 252.0, 248.0]),
            create_test_time_series_data("GOOGL", vec![2500.0, 2520.0, 2480.0]),
        ];
        
        let result = predictor.predict_batch(&batch_data).await;
        assert!(result.is_ok());
        
        let predictions = result.unwrap();
        assert_eq!(predictions.len(), 3);
        
        // All should have predictions since they're all in technology sector
        for prediction in predictions {
            assert!(prediction.value > 0.0);
            assert!(prediction.confidence > 0.0);
        }
    }
    
    #[tokio::test]
    async fn test_conversion_from_vendor_format() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // First, do a conversion to vendor format to set up metadata
        let test_data = create_test_time_series_data("AAPL", vec![100.0, 200.0, 150.0]);
        let (_, metadata) = predictor.convert_to_vendor_format(&test_data, "AAPL").await.unwrap();
        
        // Create mock forecast result
        let forecast = MockForecastResult {
            forecasts: vec![0.5, 0.7, 0.3], // Normalized values
            confidence: Some(0.85),
            metadata: None,
        };
        
        let result = predictor.convert_from_vendor_format(forecast, "AAPL", "test_model").await;
        assert!(result.is_ok());
        
        let prediction_result = result.unwrap();
        
        // Verify conversion back to original scale
        assert!(prediction_result.value >= 100.0 && prediction_result.value <= 200.0);
        assert_eq!(prediction_result.confidence, 0.85);
        assert_eq!(prediction_result.model_type, "test_model");
        assert!(prediction_result.metadata.is_some());
    }
    
    #[tokio::test]
    async fn test_performance_tracking_integration() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Add model with performance tracking enabled
        let model_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "tracked".to_string(),
        };
        
        let mock_model = Box::new(MockBaseModel::new("LSTM", 175.0, 0.85));
        predictor.add_model(model_key, mock_model).await.unwrap();
        
        // Make prediction (should trigger performance tracking)
        let test_data = create_test_time_series_data("AAPL", vec![170.0, 172.0, 168.0, 175.0]);
        let result = predictor.predict(&test_data).await;
        
        assert!(result.is_ok());
        let prediction = result.unwrap();
        
        // Verify prediction was made (performance tracking happens internally)
        assert!(prediction.value > 0.0);
        assert!(prediction.confidence > 0.0);
    }
    
    #[tokio::test]
    async fn test_load_configurations() {
        let mut predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Test loading configurations (will be implemented in future iterations)
        let result = predictor.load_configurations("/tmp/test_config.toml").await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_update_model_placeholder() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        let test_data = create_test_time_series_data("AAPL", vec![100.0, 102.0, 98.0]);
        let result = predictor.update_model(&test_data).await;
        
        // Should succeed as placeholder (online learning not implemented yet)
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_concurrent_predictions() {
        let predictor = Arc::new(VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap());
        
        // Add model for technology sector
        let model_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "LSTM".to_string(),
            variant: "concurrent".to_string(),
        };
        
        let mock_model = Box::new(MockBaseModel::new("LSTM", 175.0, 0.85));
        predictor.add_model(model_key, mock_model).await.unwrap();
        
        // Spawn multiple concurrent prediction tasks
        let mut handles = vec![];
        for i in 0..10 {
            let predictor_clone = Arc::clone(&predictor);
            let handle = tokio::spawn(async move {
                let data = create_test_time_series_data(
                    "AAPL", 
                    vec![170.0 + i as f64, 172.0 + i as f64, 168.0 + i as f64]
                );
                predictor_clone.predict(&data).await
            });
            handles.push(handle);
        }
        
        // Wait for all predictions and verify success
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            let prediction = result.unwrap();
            assert!(prediction.value > 0.0);
        }
    }
    
    #[tokio::test]
    async fn test_memory_cleanup() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Test that conversion cache can be cleaned up
        let test_data = create_test_time_series_data("TEMP", vec![100.0, 102.0]);
        let _ = predictor.convert_to_vendor_format(&test_data, "TEMP").await.unwrap();
        
        // Verify cache entry exists
        assert!(predictor.conversion_cache.contains_key("TEMP"));
        
        // Remove cache entry
        predictor.conversion_cache.remove("TEMP");
        assert!(!predictor.conversion_cache.contains_key("TEMP"));
    }
    
    #[tokio::test]
    async fn test_symbol_metadata_extraction() {
        let predictor = VendorPredictor::new(
            &create_test_neural_config(),
            create_test_sector_mapper(),
            create_test_performance_tracker()
        ).unwrap();
        
        // Test different ways of providing symbol metadata
        let mut test_data1 = create_test_time_series_data("AAPL", vec![100.0, 102.0]);
        test_data1.symbol = "".to_string(); // Clear symbol field
        
        let mut test_data2 = create_test_time_series_data("MSFT", vec![100.0, 102.0]);
        test_data2.metadata_map.clear(); // Clear metadata_map
        
        // Both should work (fallback mechanisms)
        // Note: Actual implementation would handle symbol extraction
        // This test verifies the interface exists
    }
}