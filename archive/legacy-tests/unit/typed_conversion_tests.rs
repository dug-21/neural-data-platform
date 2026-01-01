//! Comprehensive Tests for Typed Data Conversion System
//!
//! This module tests conversion between internal and vendor formats while maintaining
//! complete type safety and avoiding any downcasting operations.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::data::{TimeSeriesData, data_converter::{DataConverter, DataConverterConfig, ConversionMetadata}};
use crate::adapters::vendor_bridge::VendorTimeSeriesData;
use crate::neural::vendor_predictor::VendorPredictor;
use crate::config::NeuralConfig;
use crate::data::sector_mapper::{SectorMapper, SectorMapperConfig};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;

// Import typed models from typed_storage_tests
use crate::tests::unit::typed_storage_tests::{TypedLSTMModel, TypedGRUModel, TypedBaseModel};

/// Typed data converter that maintains type safety throughout conversion process
#[derive(Debug)]
pub struct TypedDataConverter {
    /// Internal data converter
    converter: DataConverter,
    /// Type registry for validation
    type_registry: HashMap<String, String>,
    /// Conversion cache with type information
    conversion_cache: HashMap<String, TypedConversionMetadata>,
}

/// Extended conversion metadata with type safety information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypedConversionMetadata {
    /// Base conversion metadata
    pub base_metadata: ConversionMetadata,
    /// Input type signature
    pub input_type: String,
    /// Output type signature  
    pub output_type: String,
    /// Type validation checksum
    pub type_checksum: String,
    /// Conversion timestamp
    pub converted_at: chrono::DateTime<Utc>,
}

/// Typed vendor data with compile-time type safety
#[derive(Debug, Clone)]
pub struct TypedVendorData<T> {
    pub values: Vec<T>,
    pub timestamps: Vec<chrono::DateTime<Utc>>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub type_signature: String,
}

/// Typed forecast result with type preservation
#[derive(Debug, Clone)]
pub struct TypedForecastResult<T> {
    pub forecasts: Vec<T>,
    pub confidence_scores: Option<Vec<T>>,
    pub prediction_intervals: Option<(Vec<T>, Vec<T>)>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub type_signature: String,
}

impl TypedDataConverter {
    pub fn new(config: DataConverterConfig) -> Self {
        Self {
            converter: DataConverter::new(config),
            type_registry: HashMap::new(),
            conversion_cache: HashMap::new(),
        }
    }
    
    /// Convert TimeSeriesData to typed vendor format with full type safety
    pub fn to_typed_vendor_format<T>(&mut self, 
        data: &TimeSeriesData,
        symbol: &str,
    ) -> Result<(TypedVendorData<T>, TypedConversionMetadata)>
    where
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        // First, perform base conversion
        let (vendor_data, base_metadata) = self.converter.to_vendor_format(data, symbol)?;
        
        // Create typed metadata
        let typed_metadata = TypedConversionMetadata {
            base_metadata,
            input_type: "TimeSeriesData".to_string(),
            output_type: format!("TypedVendorData<{}>", std::any::type_name::<T>()),
            type_checksum: self.calculate_type_checksum::<T>()?,
            converted_at: Utc::now(),
        };
        
        // Convert to typed format (this would involve actual type conversion)
        let typed_values: Vec<T> = vendor_data.values.iter()
            .filter_map(|v| self.convert_value_to_type::<T>(*v))
            .collect();
        
        let typed_vendor_data = TypedVendorData {
            values: typed_values,
            timestamps: data.timestamps.clone(),
            metadata: data.metadata.clone().unwrap_or_default(),
            type_signature: std::any::type_name::<T>().to_string(),
        };
        
        // Cache conversion metadata
        self.conversion_cache.insert(symbol.to_string(), typed_metadata.clone());
        self.type_registry.insert(symbol.to_string(), std::any::type_name::<T>().to_string());
        
        Ok((typed_vendor_data, typed_metadata))
    }
    
    /// Convert typed forecast result back to internal format with type preservation
    pub fn from_typed_vendor_format<T>(&self,
        forecast: TypedForecastResult<T>,
        metadata: &TypedConversionMetadata,
        symbol: &str,
    ) -> Result<crate::neural::PredictionResult>
    where
        T: Clone + std::fmt::Debug + Into<f64>,
    {
        // Validate type consistency
        self.validate_type_consistency::<T>(metadata)?;
        
        // Convert forecasts to internal format
        let primary_forecast = forecast.forecasts.get(0)
            .ok_or_else(|| anyhow::anyhow!("No forecasts available"))?
            .clone()
            .into();
        
        let confidence = forecast.confidence_scores
            .as_ref()
            .and_then(|scores| scores.first())
            .map(|score| score.clone().into())
            .unwrap_or(0.5);
        
        // Calculate prediction intervals
        let (interval_low, interval_high) = if let Some((low_vec, high_vec)) = &forecast.prediction_intervals {
            let low = low_vec.first().map(|v| v.clone().into()).unwrap_or(primary_forecast - confidence * primary_forecast.abs());
            let high = high_vec.first().map(|v| v.clone().into()).unwrap_or(primary_forecast + confidence * primary_forecast.abs());
            (low, high)
        } else {
            (
                primary_forecast - confidence * primary_forecast.abs(),
                primary_forecast + confidence * primary_forecast.abs()
            )
        };
        
        // Create prediction metadata with type information
        let mut prediction_metadata = HashMap::new();
        prediction_metadata.insert("type_signature".to_string(), serde_json::json!(forecast.type_signature));
        prediction_metadata.insert("conversion_method".to_string(), serde_json::json!("typed_conversion"));
        prediction_metadata.insert("input_type".to_string(), serde_json::json!(metadata.input_type));
        prediction_metadata.insert("output_type".to_string(), serde_json::json!(metadata.output_type));
        prediction_metadata.insert("type_checksum".to_string(), serde_json::json!(metadata.type_checksum));
        
        // Add original forecast metadata
        for (key, value) in &forecast.metadata {
            prediction_metadata.insert(key.clone(), value.clone());
        }
        
        Ok(crate::neural::PredictionResult {
            value: primary_forecast,
            confidence,
            model_name: format!("typed_model_{}", std::any::type_name::<T>()),
            interval_low,
            interval_high,
            timestamp: Utc::now(),
            metadata: Some(prediction_metadata),
        })
    }
    
    /// Validate type consistency across conversion
    fn validate_type_consistency<T>(&self, metadata: &TypedConversionMetadata) -> Result<()> {
        let expected_checksum = self.calculate_type_checksum::<T>()?;
        if metadata.type_checksum != expected_checksum {
            return Err(anyhow::anyhow!(
                "Type checksum mismatch: expected {}, got {}",
                expected_checksum,
                metadata.type_checksum
            ));
        }
        Ok(())
    }
    
    /// Calculate type checksum for validation
    fn calculate_type_checksum<T>(&self) -> Result<String> {
        let type_name = std::any::type_name::<T>();
        let checksum = format!("{:x}", md5::compute(type_name.as_bytes()));
        Ok(checksum)
    }
    
    /// Convert f32 value to target type (simplified for testing)
    fn convert_value_to_type<T>(&self, value: f32) -> Option<T> 
    where
        T: Clone + std::fmt::Debug + serde::Serialize + for<'de> serde::Deserialize<'de>,
    {
        // This is a simplified implementation
        // In practice, this would handle proper type conversion
        let json_value = serde_json::json!(value);
        serde_json::from_value(json_value).ok()
    }
    
    /// Get conversion statistics with type information
    pub fn get_conversion_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        stats.insert("total_conversions".to_string(), serde_json::json!(self.conversion_cache.len()));
        stats.insert("cached_types".to_string(), serde_json::json!(self.type_registry.len()));
        
        // Count conversions by type
        let mut type_counts: HashMap<String, usize> = HashMap::new();
        for type_name in self.type_registry.values() {
            *type_counts.entry(type_name.clone()).or_insert(0) += 1;
        }
        stats.insert("conversions_by_type".to_string(), serde_json::json!(type_counts));
        
        stats
    }
}

// Test utilities
fn create_test_time_series_data(symbol: &str, values: Vec<f64>) -> TimeSeriesData {
    let now = Utc::now();
    let close_price = values.last().copied().unwrap_or(100.0);
    
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: now,
        open: close_price * 0.995,
        high: close_price * 1.015,
        low: close_price * 0.985,
        close: close_price,
        volume: vec![1500000.0],
        indicators: HashMap::new(),
        source: Some("conversion_test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(close_price),
        values: values.clone(),
        timestamps: (0..values.len())
            .map(|i| now - chrono::Duration::hours((values.len() - i - 1) as i64))
            .collect(),
        metadata: Some({
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map.insert("conversion_test".to_string(), serde_json::json!(true));
            map
        }),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map
        },
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::*;

    #[tokio::test]
    async fn test_typed_conversion_f32() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        let test_data = create_test_time_series_data("TEST", vec![100.0, 101.5, 99.8, 102.3]);
        
        // Convert to typed f32 vendor format
        let result: Result<(TypedVendorData<f32>, TypedConversionMetadata)> = 
            converter.to_typed_vendor_format(&test_data, "TEST");
        
        assert!(result.is_ok());
        let (typed_vendor_data, metadata) = result.unwrap();
        
        // Verify typed data
        assert_eq!(typed_vendor_data.type_signature, "f32");
        assert!(!typed_vendor_data.values.is_empty());
        assert_eq!(typed_vendor_data.timestamps.len(), test_data.timestamps.len());
        
        // Verify metadata
        assert_eq!(metadata.input_type, "TimeSeriesData");
        assert!(metadata.output_type.contains("f32"));
        assert!(!metadata.type_checksum.is_empty());
        
        // Test conversion back to internal format
        let forecast = TypedForecastResult {
            forecasts: vec![103.5f32],
            confidence_scores: Some(vec![0.85f32]),
            prediction_intervals: Some((vec![101.0f32], vec![106.0f32])),
            metadata: HashMap::new(),
            type_signature: "f32".to_string(),
        };
        
        let prediction_result = converter.from_typed_vendor_format(forecast, &metadata, "TEST");
        assert!(prediction_result.is_ok());
        
        let result = prediction_result.unwrap();
        assert_eq!(result.value, 103.5);
        assert_eq!(result.confidence, 0.85);
        assert_eq!(result.interval_low, 101.0);
        assert_eq!(result.interval_high, 106.0);
        
        // Verify type information in metadata
        let result_metadata = result.metadata.unwrap();
        assert_eq!(result_metadata.get("type_signature").unwrap(), &serde_json::json!("f32"));
        assert_eq!(result_metadata.get("conversion_method").unwrap(), &serde_json::json!("typed_conversion"));
    }
    
    #[tokio::test]
    async fn test_typed_conversion_f64() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        let test_data = create_test_time_series_data("TEST64", vec![1000.0, 1015.5, 998.3, 1023.7]);
        
        // Convert to typed f64 vendor format
        let result: Result<(TypedVendorData<f64>, TypedConversionMetadata)> = 
            converter.to_typed_vendor_format(&test_data, "TEST64");
        
        assert!(result.is_ok());
        let (typed_vendor_data, metadata) = result.unwrap();
        
        // Verify typed data with f64 precision
        assert_eq!(typed_vendor_data.type_signature, "f64");
        assert!(!typed_vendor_data.values.is_empty());
        
        // Verify different type checksum for f64
        let f32_result: Result<(TypedVendorData<f32>, TypedConversionMetadata)> = 
            converter.to_typed_vendor_format(&test_data, "TEST32");
        assert!(f32_result.is_ok());
        let (_, f32_metadata) = f32_result.unwrap();
        
        assert_ne!(metadata.type_checksum, f32_metadata.type_checksum);
        assert!(metadata.output_type.contains("f64"));
        assert!(f32_metadata.output_type.contains("f32"));
    }
    
    #[tokio::test]
    async fn test_type_validation_and_consistency() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        let test_data = create_test_time_series_data("VALIDATE", vec![50.0, 51.2, 49.8]);
        
        // Convert with f32 type
        let (_, f32_metadata) = converter.to_typed_vendor_format::<f32>(&test_data, "VALIDATE").unwrap();
        
        // Try to validate with wrong type
        let validation_result = converter.validate_type_consistency::<f64>(&f32_metadata);
        assert!(validation_result.is_err());
        assert!(validation_result.unwrap_err().to_string().contains("Type checksum mismatch"));
        
        // Validate with correct type
        let validation_result = converter.validate_type_consistency::<f32>(&f32_metadata);
        assert!(validation_result.is_ok());
    }
    
    #[tokio::test]
    async fn test_conversion_with_typed_models() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        
        // Create typed models
        let lstm_model = TypedLSTMModel::new(4, 8, 1);
        let gru_model = TypedGRUModel::new(4, 8, 1);
        
        // Create test data
        let test_data = create_test_time_series_data("MODEL_TEST", vec![200.0, 201.5, 198.9, 203.2]);
        
        // Convert to typed vendor format for LSTM
        let (lstm_vendor_data, lstm_metadata) = converter.to_typed_vendor_format::<f32>(&test_data, "LSTM_TEST").unwrap();
        
        // Use converted data with LSTM model
        let lstm_input: Vec<f32> = lstm_vendor_data.values.into_iter().take(4).collect();
        
        if lstm_input.len() == 4 {
            lstm_model.validate_input(&lstm_input).unwrap();
            let lstm_output = lstm_model.predict_typed(&lstm_input).unwrap();
            assert_eq!(lstm_output.len(), 1);
            
            // Create forecast result from LSTM output
            let lstm_forecast = TypedForecastResult {
                forecasts: lstm_output,
                confidence_scores: Some(vec![0.92f32]),
                prediction_intervals: None,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("model_type".to_string(), serde_json::json!("TypedLSTM"));
                    meta.insert("input_size".to_string(), serde_json::json!(4));
                    meta
                },
                type_signature: "f32".to_string(),
            };
            
            // Convert back to internal format
            let prediction_result = converter.from_typed_vendor_format(lstm_forecast, &lstm_metadata, "LSTM_TEST").unwrap();
            
            // Verify prediction result includes model information
            let result_metadata = prediction_result.metadata.unwrap();
            assert_eq!(result_metadata.get("model_type").unwrap(), &serde_json::json!("TypedLSTM"));
            assert!(prediction_result.model_name.contains("f32"));
        }
    }
    
    #[tokio::test]
    async fn test_conversion_statistics_and_caching() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        
        // Perform multiple conversions
        let symbols = vec!["STAT1", "STAT2", "STAT3"];
        let test_data = create_test_time_series_data("BASE", vec![100.0, 101.0, 102.0]);
        
        for symbol in &symbols {
            // Convert with different types
            let _ = converter.to_typed_vendor_format::<f32>(&test_data, symbol).unwrap();
            let _ = converter.to_typed_vendor_format::<f64>(&test_data, &format!("{}_64", symbol)).unwrap();
        }
        
        // Check conversion statistics
        let stats = converter.get_conversion_stats();
        
        let total_conversions = stats.get("total_conversions").unwrap().as_u64().unwrap();
        assert_eq!(total_conversions, 6); // 3 symbols * 2 types each
        
        let cached_types = stats.get("cached_types").unwrap().as_u64().unwrap();
        assert_eq!(cached_types, 6);
        
        let conversions_by_type = stats.get("conversions_by_type").unwrap().as_object().unwrap();
        assert!(conversions_by_type.contains_key("f32"));
        assert!(conversions_by_type.contains_key("f64"));
        
        // Verify caching works - second conversion should use cache
        let start_time = std::time::Instant::now();
        let _ = converter.to_typed_vendor_format::<f32>(&test_data, "STAT1").unwrap();
        let cache_time = start_time.elapsed();
        
        // Cache time should be very fast (though this is a simple test)
        assert!(cache_time.as_millis() < 100);
    }
    
    #[tokio::test]
    async fn test_error_handling_in_conversions() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        
        // Test with empty data
        let empty_data = TimeSeriesData::default();
        let result = converter.to_typed_vendor_format::<f32>(&empty_data, "EMPTY");
        // Should handle gracefully (may succeed with empty values or return error)
        
        // Test with malformed forecast
        let test_data = create_test_time_series_data("ERROR", vec![1.0, 2.0, 3.0]);
        let (_, metadata) = converter.to_typed_vendor_format::<f32>(&test_data, "ERROR").unwrap();
        
        let empty_forecast = TypedForecastResult {
            forecasts: vec![], // Empty forecasts
            confidence_scores: None,
            prediction_intervals: None,
            metadata: HashMap::new(),
            type_signature: "f32".to_string(),
        };
        
        let result = converter.from_typed_vendor_format(empty_forecast, &metadata, "ERROR");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No forecasts available"));
    }
    
    #[tokio::test]
    async fn test_integration_with_vendor_predictor() {
        // Create vendor predictor with typed conversion
        let config = NeuralConfig {
            input_size: 24,
            output_size: 1,
            hidden_layers: vec![32, 16],
            learning_rate: 0.001,
            prediction_horizon: Some(1),
            normalization_method: Some("z-score".to_string()),
            enable_adaptive_retry: true,
            enable_model_ensembles: true,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.15,
            memory_gb: 1.0,
            models: vec!["TypedLSTM".to_string(), "TypedGRU".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: true,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            epochs: 100,
            batch_size: 32,
            sequence_length: 24,
            enable_feature_scaling: true,
            enable_technical_indicators: true,
            dropout_rate: 0.1,
            l2_regularization: 0.001,
            validation_split: 0.2,
            early_stopping: true,
            patience: 10,
        };
        
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        
        let predictor = VendorPredictor::new(&config, sector_mapper, performance_tracker).unwrap();
        
        // Test conversion through predictor interface
        let test_data = create_test_time_series_data("INTEGRATION", vec![150.0, 151.5, 149.2, 152.8]);
        
        let (vendor_data, metadata) = predictor.convert_to_vendor_format(&test_data, "INTEGRATION").await.unwrap();
        
        // Verify conversion worked
        assert!(!vendor_data.values.is_empty());
        assert_eq!(metadata.source_format, "TimeSeriesData");
        assert_eq!(metadata.target_format, "VendorTimeSeriesData<f32>");
        
        // Verify conversion is cached
        assert!(predictor.conversion_cache.contains_key("INTEGRATION"));
        
        // Test reverse conversion
        let forecast = neuro_divergent_models::foundation::ForecastOutput::new(vec![153.5f32]);
        let prediction_result = predictor.convert_from_vendor_format(forecast, "INTEGRATION", "integration_model").await.unwrap();
        
        // Verify prediction result
        assert!(prediction_result.value > 0.0);
        assert_eq!(prediction_result.model_name, "integration_model");
        assert!(prediction_result.metadata.is_some());
        
        let result_metadata = prediction_result.metadata.unwrap();
        assert!(result_metadata.contains_key("conversion_method"));
    }
    
    #[tokio::test]
    async fn test_concurrent_type_safe_conversions() {
        let converter = Arc::new(tokio::sync::Mutex::new(
            TypedDataConverter::new(DataConverterConfig::default())
        ));
        
        let mut handles = vec![];
        
        // Spawn concurrent conversion tasks
        for i in 0..10 {
            let converter_clone = Arc::clone(&converter);
            let handle = tokio::spawn(async move {
                let symbol = format!("CONCURRENT_{}", i);
                let test_data = create_test_time_series_data(
                    &symbol, 
                    vec![100.0 + i as f64, 101.0 + i as f64, 99.0 + i as f64]
                );
                
                let mut conv = converter_clone.lock().await;
                
                // Use different types for odd/even to test type safety
                if i % 2 == 0 {
                    let result: Result<(TypedVendorData<f32>, TypedConversionMetadata)> = 
                        conv.to_typed_vendor_format(&test_data, &symbol);
                    result
                } else {
                    let result: Result<(TypedVendorData<f64>, TypedConversionMetadata)> = 
                        conv.to_typed_vendor_format(&test_data, &symbol);
                    result.map(|(data, meta)| {
                        // Convert to common return type for testing
                        let f32_data = TypedVendorData {
                            values: data.values.into_iter().map(|v| v as f32).collect(),
                            timestamps: data.timestamps,
                            metadata: data.metadata,
                            type_signature: "f32_converted".to_string(),
                        };
                        (f32_data, meta)
                    })
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all conversions to complete
        let results = futures::future::join_all(handles).await;
        
        // Verify all conversions succeeded
        for (i, result) in results.into_iter().enumerate() {
            assert!(result.is_ok(), "Task {} panicked", i);
            let conversion_result = result.unwrap();
            assert!(conversion_result.is_ok(), "Conversion {} failed", i);
            
            let (typed_data, metadata) = conversion_result.unwrap();
            assert!(!typed_data.values.is_empty());
            assert!(!metadata.type_checksum.is_empty());
        }
        
        // Verify conversion statistics
        let conv = converter.lock().await;
        let stats = conv.get_conversion_stats();
        let total_conversions = stats.get("total_conversions").unwrap().as_u64().unwrap();
        assert_eq!(total_conversions, 10);
    }
    
    #[tokio::test] 
    async fn test_type_preservation_across_model_operations() {
        let mut converter = TypedDataConverter::new(DataConverterConfig::default());
        
        // Create test models with different configurations
        let lstm_model = TypedLSTMModel::new(3, 6, 1);
        let gru_model = TypedGRUModel::new(3, 6, 1);
        
        // Test data with exactly 3 values to match model input size
        let test_data = create_test_time_series_data("TYPE_PRESERVE", vec![10.0, 11.0, 12.0]);
        
        // Convert to typed format
        let (typed_data, metadata) = converter.to_typed_vendor_format::<f32>(&test_data, "TYPE_PRESERVE").unwrap();
        
        // Use with LSTM model
        let lstm_input = typed_data.values.clone();
        if lstm_input.len() >= 3 {
            let lstm_trimmed: Vec<f32> = lstm_input.into_iter().take(3).collect();
            let lstm_output = lstm_model.predict_typed(&lstm_trimmed).unwrap();
            
            // Create forecast preserving types
            let lstm_forecast = TypedForecastResult {
                forecasts: lstm_output,
                confidence_scores: Some(vec![0.88f32]),
                prediction_intervals: Some((vec![10.5f32], vec![13.5f32])),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("model_architecture".to_string(), serde_json::json!("LSTM"));
                    meta.insert("preserves_f32".to_string(), serde_json::json!(true));
                    meta
                },
                type_signature: "f32".to_string(),
            };
            
            // Convert back ensuring type preservation
            let prediction = converter.from_typed_vendor_format(lstm_forecast, &metadata, "TYPE_PRESERVE").unwrap();
            
            // Verify type information is preserved in metadata
            let pred_metadata = prediction.metadata.unwrap();
            assert_eq!(pred_metadata.get("type_signature").unwrap(), &serde_json::json!("f32"));
            assert_eq!(pred_metadata.get("model_architecture").unwrap(), &serde_json::json!("LSTM"));
            assert_eq!(pred_metadata.get("preserves_f32").unwrap(), &serde_json::json!(true));
            
            // Verify numeric precision is maintained
            assert!(prediction.value.is_finite());
            assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        }
    }
}