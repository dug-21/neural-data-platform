//! Edge Cases and Error Handling Tests for Phase 1 Vendor Integration
//!
//! Tests boundary conditions, error scenarios, and resilience of the vendor model integration.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;

use crate::config::NeuralConfig;
use crate::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorMapperConfig}};
use crate::data::data_converter::{DataConverter, DataConverterConfig};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::vendor_predictor::{VendorPredictor, ModelKey};
use crate::neural::NeuralPredictorTrait;

// Edge case test utilities
fn create_edge_case_config() -> NeuralConfig {
    NeuralConfig {
        model_path: "/tmp/edge_case_models".to_string(),
        batch_size: 1, // Minimum batch size
        learning_rate: 0.0001, // Very small learning rate
        hidden_layers: vec![1], // Minimal network
        activation: "relu".to_string(),
        optimizer: "sgd".to_string(),
        loss_function: "mse".to_string(),
        epochs: 1, // Minimal training
        validation_split: 0.1, // Minimal validation
        early_stopping: false,
        patience: 1,
        enable_cuda: false,
        model_type: "minimal".to_string(),
        sequence_length: 1, // Minimum sequence
        prediction_horizon: 1,
        features: vec!["price".to_string()], // Single feature
        enable_technical_indicators: false,
        enable_feature_scaling: true,
        dropout_rate: 0.0, // No dropout
        l2_regularization: 0.0, // No regularization
    }
}

async fn setup_edge_case_environment() -> Result<Arc<VendorPredictor>> {
    let neural_config = create_edge_case_config();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    let performance_tracker = Arc::new(ModelPerformanceTracker::new()?);
    
    let vendor_predictor = Arc::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?);
    
    Ok(vendor_predictor)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_empty_data() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let empty_data = TimeSeriesData {
            values: vec![],
            timestamps: vec![],
            metadata: HashMap::new(),
            symbol: "EMPTY".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&empty_data).await;
        
        // Should handle empty data gracefully
        match result {
            Ok(prediction) => {
                // Returns default prediction
                assert_eq!(prediction.value, 0.0);
                assert_eq!(prediction.confidence, 0.5);
                assert_eq!(prediction.model_type, "none");
            }
            Err(e) => {
                // Or returns appropriate error
                assert!(e.to_string().contains("empty") || e.to_string().contains("insufficient"));
            }
        }
    }
    
    #[tokio::test]
    async fn test_single_data_point() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let single_point_data = TimeSeriesData {
            values: vec![42.0],
            timestamps: vec![Utc::now()],
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("SINGLE"));
                map
            },
            symbol: "SINGLE".to_string(),
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("SINGLE"));
                map
            }
        };
        
        let result = predictor.predict(&single_point_data).await;
        
        // Should handle single point gracefully
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.timestamp <= Utc::now());
    }
    
    #[tokio::test]
    async fn test_all_nan_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let nan_data = TimeSeriesData {
            values: vec![f64::NAN, f64::NAN, f64::NAN],
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("NAN_TEST"));
                map
            },
            symbol: "NAN_TEST".to_string(),
            metadata_map: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("NAN_TEST"));
                map
            }
        };
        
        let result = predictor.predict(&nan_data).await;
        
        // Should either handle NaN gracefully or return appropriate error
        match result {
            Ok(prediction) => {
                // If handled, prediction should be valid
                assert!(prediction.value.is_finite() || prediction.value == 0.0);
            }
            Err(e) => {
                // Or return appropriate error
                assert!(e.to_string().contains("missing") || e.to_string().contains("invalid"));
            }
        }
    }
    
    #[tokio::test]
    async fn test_infinite_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let infinite_data = TimeSeriesData {
            values: vec![f64::INFINITY, f64::NEG_INFINITY, 100.0],
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("INF_TEST"));
                map
            },
            symbol: "INF_TEST".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&infinite_data).await;
        
        // Should handle infinite values gracefully
        match result {
            Ok(prediction) => {
                assert!(prediction.value.is_finite() || prediction.value == 0.0);
            }
            Err(_) => {
                // Error is acceptable for infinite values
            }
        }
    }
    
    #[tokio::test]
    async fn test_extremely_large_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let large_values_data = TimeSeriesData {
            values: vec![1e50, 1e60, 1e70], // Extremely large numbers
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("LARGE"));
                map
            },
            symbol: "LARGE".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&large_values_data).await;
        
        // Should handle large values through normalization
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value.is_finite());
    }
    
    #[tokio::test]
    async fn test_extremely_small_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let small_values_data = TimeSeriesData {
            values: vec![1e-50, 1e-60, 1e-70], // Extremely small numbers
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("SMALL"));
                map
            },
            symbol: "SMALL".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&small_values_data).await;
        
        // Should handle small values gracefully
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value.is_finite());
    }
    
    #[tokio::test]
    async fn test_all_identical_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let identical_data = TimeSeriesData {
            values: vec![42.0; 100], // All same value
            timestamps: (0..100).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("IDENTICAL"));
                map
            },
            symbol: "IDENTICAL".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&identical_data).await;
        
        // Should handle constant values (zero variance)
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value.is_finite());
    }
    
    #[tokio::test]
    async fn test_alternating_extreme_values() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let alternating_data = TimeSeriesData {
            values: (0..50).map(|i| if i % 2 == 0 { 1000000.0 } else { 0.001 }).collect(),
            timestamps: (0..50).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("ALTERNATING"));
                map
            },
            symbol: "ALTERNATING".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&alternating_data).await;
        
        // Should handle extreme volatility through normalization
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value.is_finite());
    }
    
    #[tokio::test]
    async fn test_invalid_timestamps() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        // Create timestamps with invalid ordering
        let mut invalid_timestamps = vec![
            Utc::now(),
            Utc::now() + chrono::Duration::hours(1), // Future timestamp
            Utc::now() - chrono::Duration::hours(1),
        ];
        
        let invalid_time_data = TimeSeriesData {
            values: vec![100.0, 101.0, 99.0],
            timestamps: invalid_timestamps,
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("INVALID_TIME"));
                map
            },
            symbol: "INVALID_TIME".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&invalid_time_data).await;
        
        // Should handle invalid timestamps gracefully
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.timestamp <= Utc::now() + chrono::Duration::seconds(1));
    }
    
    #[tokio::test]
    async fn test_missing_metadata() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let no_metadata_data = TimeSeriesData {
            values: vec![100.0, 101.0, 99.0],
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: HashMap::new(), // Empty metadata
            symbol: "".to_string(), // Empty symbol
            metadata_map: HashMap::new(), // Empty metadata_map
        };
        
        let result = predictor.predict(&no_metadata_data).await;
        
        // Should handle missing metadata by using defaults
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.timestamp <= Utc::now());
    }
    
    #[tokio::test]
    async fn test_malformed_symbol_names() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        let malformed_symbols = vec![
            "", // Empty
            " ", // Whitespace only
            "VERY_LONG_SYMBOL_NAME_THAT_EXCEEDS_NORMAL_LENGTH", // Very long
            "123", // Numeric only
            "symbol with spaces", // Spaces
            "SYMBOL@#$%", // Special characters
            "中文符号", // Unicode characters
        ];
        
        for symbol in malformed_symbols {
            let test_data = TimeSeriesData {
                values: vec![100.0, 101.0, 99.0],
                timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
                metadata: {
                    let mut map = HashMap::new();
                    map.insert("symbol".to_string(), serde_json::json!(symbol));
                    map
                },
                symbol: symbol.to_string(),
                metadata_map: {
                    let mut map = HashMap::new();
                    map.insert("symbol".to_string(), serde_json::json!(symbol));
                    map
                }
            };
            
            let result = predictor.predict(&test_data).await;
            
            // Should handle malformed symbols gracefully
            assert!(result.is_ok(), "Failed to handle symbol: '{}'", symbol);
            let prediction = result.unwrap();
            assert!(prediction.timestamp <= Utc::now());
        }
    }
    
    #[tokio::test]
    async fn test_memory_pressure() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        // Create very large dataset to test memory handling
        let large_values: Vec<f64> = (0..100000).map(|i| i as f64 * 0.01).collect();
        let large_timestamps: Vec<DateTime<Utc>> = (0..100000)
            .map(|i| Utc::now() - chrono::Duration::seconds(i))
            .collect();
        
        let large_data = TimeSeriesData {
            values: large_values,
            timestamps: large_timestamps,
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("LARGE_DATASET"));
                map
            },
            symbol: "LARGE_DATASET".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&large_data).await;
        
        // Should handle large datasets efficiently
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value.is_finite());
    }
    
    #[tokio::test]
    async fn test_data_converter_edge_cases() {
        let mut converter = DataConverter::new(DataConverterConfig {
            max_missing_percent: 0.0, // Don't allow any missing values
            normalize_data: true,
            normalization_method: "minmax".to_string(),
            remove_outliers: true,
            outlier_method: "iqr".to_string(),
            ..Default::default()
        });
        
        // Test data that exceeds missing value threshold
        let high_missing_data = TimeSeriesData {
            values: vec![100.0, f64::NAN, f64::NAN, f64::NAN, 105.0], // 60% missing
            timestamps: (0..5).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("HIGH_MISSING"));
                map
            },
            symbol: "HIGH_MISSING".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = converter.to_vendor_format(&high_missing_data, "HIGH_MISSING");
        
        // Should reject data with too many missing values
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Too many missing values"));
    }
    
    #[tokio::test]
    async fn test_sector_mapper_edge_cases() {
        let sector_mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Test sector mapping with edge case symbols
        let edge_case_symbols = vec![
            "", // Empty string
            "NULL", // Reserved word
            "UNDEFINED", // Reserved word
            "NaN", // Confusing with number
            "Infinity", // Mathematical constant
        ];
        
        for symbol in edge_case_symbols {
            let sector_result = sector_mapper.get_sector(symbol);
            
            // Should handle edge cases by returning default sector
            assert!(sector_result.is_ok(), "Failed to get sector for: '{}'", symbol);
            let sector_info = sector_result.unwrap();
            assert_eq!(sector_info.id, "technology"); // Default sector
        }
    }
    
    #[tokio::test]
    async fn test_concurrent_edge_cases() {
        let predictor = Arc::new(setup_edge_case_environment().await.unwrap());
        
        // Create multiple concurrent edge case scenarios
        let edge_cases = vec![
            ("EMPTY", vec![]),
            ("SINGLE", vec![42.0]),
            ("NAN", vec![f64::NAN, f64::NAN]),
            ("INF", vec![f64::INFINITY, f64::NEG_INFINITY]),
            ("ZERO", vec![0.0, 0.0, 0.0]),
        ];
        
        let mut tasks = vec![];
        
        for (symbol, values) in edge_cases {
            let predictor_clone = Arc::clone(&predictor);
            let symbol = symbol.to_string();
            
            let task = tokio::spawn(async move {
                let timestamps: Vec<DateTime<Utc>> = (0..values.len())
                    .map(|i| Utc::now() - chrono::Duration::hours(i as i64))
                    .collect();
                
                let test_data = TimeSeriesData {
                    values,
                    timestamps,
                    metadata: {
                        let mut map = HashMap::new();
                        map.insert("symbol".to_string(), serde_json::json!(symbol));
                        map
                    },
                    symbol: symbol.clone(),
                    metadata_map: HashMap::new(),
                };
                
                predictor_clone.predict(&test_data).await
            });
            tasks.push(task);
        }
        
        // Wait for all concurrent edge case tests
        let results: Vec<_> = futures::future::join_all(tasks).await;
        
        // All should complete without panicking
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(prediction_result) => {
                    // Either success or handled error
                    match prediction_result {
                        Ok(prediction) => {
                            assert!(prediction.timestamp <= Utc::now() + chrono::Duration::seconds(1));
                        }
                        Err(_) => {
                            // Handled error is acceptable
                        }
                    }
                }
                Err(e) => {
                    panic!("Task {} panicked: {}", i, e);
                }
            }
        }
    }
    
    #[tokio::test]
    async fn test_normalization_edge_cases() {
        let mut converter = DataConverter::new(DataConverterConfig {
            normalize_data: true,
            normalization_method: "zscore".to_string(),
            ..Default::default()
        });
        
        // Test z-score normalization with zero variance
        let zero_variance_data = TimeSeriesData {
            values: vec![100.0; 50], // All identical values
            timestamps: (0..50).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("ZERO_VAR"));
                map
            },
            symbol: "ZERO_VAR".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = converter.to_vendor_format(&zero_variance_data, "ZERO_VAR");
        
        // Should handle zero variance gracefully
        assert!(result.is_ok());
        let (vendor_data, _metadata) = result.unwrap();
        assert!(vendor_data.values.iter().all(|v| v.is_finite()));
    }
    
    #[tokio::test]
    async fn test_resource_cleanup_on_error() {
        let predictor = setup_edge_case_environment().await.unwrap();
        
        // Generate multiple failing predictions to test cleanup
        for i in 0..10 {
            let invalid_data = TimeSeriesData {
                values: vec![], // Empty - should fail or return default
                timestamps: vec![],
                metadata: HashMap::new(),
                symbol: format!("FAIL_{}", i),
                metadata_map: HashMap::new(),
            };
            
            let _ = predictor.predict(&invalid_data).await;
            // Don't care about result, just that it doesn't leak resources
        }
        
        // Verify system is still responsive after multiple failures
        let valid_data = TimeSeriesData {
            values: vec![100.0, 101.0, 99.0],
            timestamps: (0..3).map(|i| Utc::now() - chrono::Duration::hours(i)).collect(),
            metadata: {
                let mut map = HashMap::new();
                map.insert("symbol".to_string(), serde_json::json!("RECOVERY"));
                map
            },
            symbol: "RECOVERY".to_string(),
            metadata_map: HashMap::new(),
        };
        
        let result = predictor.predict(&valid_data).await;
        assert!(result.is_ok());
    }
}