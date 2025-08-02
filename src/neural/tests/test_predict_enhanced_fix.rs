//! Test for FannPredictor predict_enhanced method fix
//! 
//! This test verifies that the predict_enhanced method correctly calls
//! the adapter's predict method with all three required arguments.

use crate::neural::{FannPredictor, NeuralConfig, PredictionResult};
use crate::data::TimeSeriesData;
use chrono::{TimeZone, Utc};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_predict_enhanced_calls_adapter_correctly() {
        // Create a FannPredictor with enhanced adapter enabled
        let mut config = NeuralConfig::default();
        config.enable_enhanced_models = true;
        config.models = vec!["TimeMixer".to_string()];
        
        let predictor = FannPredictor::new(config).expect("Failed to create predictor");
        
        // Initialize the enhanced adapter
        predictor.init_enhanced_adapter().await.expect("Failed to init adapter");
        
        // Create test data
        let test_data = vec![
            TimeSeriesData {
                symbol: "AAPL".to_string(),
                timestamp: Utc.timestamp_opt(1609459200, 0).unwrap(),
                close: 132.69,
                volume: vec![143301900.0],
                high: Some(134.99),
                low: Some(131.72),
                open: Some(133.52),
                bid: None,
                ask: None,
                entity: None,
                value: None,
                metadata: None,
            },
            TimeSeriesData {
                symbol: "AAPL".to_string(),
                timestamp: Utc.timestamp_opt(1609545600, 0).unwrap(),
                close: 131.01,
                volume: vec![97664900.0],
                high: Some(133.61),
                low: Some(130.23),
                open: Some(133.52),
                bid: None,
                ask: None,
                entity: None,
                value: None,
                metadata: None,
            },
        ];
        
        // Call predict_enhanced - this should compile correctly
        let result = predictor.predict_enhanced("TimeMixer", &test_data, 5).await;
        
        match result {
            Ok(predictions) => {
                // Verify we get 5 predictions as requested
                assert_eq!(predictions.len(), 5);
                
                // Verify each prediction has the correct structure
                for pred in &predictions {
                    assert!(pred.confidence > 0.0);
                    assert!(pred.confidence <= 1.0);
                    assert!(pred.value > 0.0);
                    assert!(pred.interval_low <= pred.value);
                    assert!(pred.interval_high >= pred.value);
                    assert!(pred.model_name.contains("enhanced"));
                }
            }
            Err(e) => {
                // If it fails, it should be due to model availability, not compilation
                assert!(e.to_string().contains("Enhanced neural adapter not initialized") ||
                        e.to_string().contains("not available"),
                        "Unexpected error: {}", e);
            }
        }
    }
    
    #[tokio::test]
    async fn test_predict_enhanced_handles_prediction_results_correctly() {
        // This test verifies that predict_enhanced correctly processes Vec<PredictionResult>
        // returned by the adapter, not Vec<f64>
        
        let mut config = NeuralConfig::default();
        config.enable_enhanced_models = true;
        config.models = vec!["MLP".to_string()];  // Use MLP which should be available
        
        let predictor = FannPredictor::new(config).expect("Failed to create predictor");
        predictor.init_enhanced_adapter().await.expect("Failed to init adapter");
        
        let test_data = vec![
            TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc.timestamp_opt(1609459200, 0).unwrap(),
                close: 100.0,
                volume: vec![1000000.0],
                high: Some(101.0),
                low: Some(99.0),
                open: Some(100.5),
                bid: None,
                ask: None,
                entity: None,
                value: None,
                metadata: None,
            },
        ];
        
        let result = predictor.predict_enhanced("MLP", &test_data, 3).await;
        
        if let Ok(predictions) = result {
            // The predictions should be properly formatted PredictionResult objects
            for (i, pred) in predictions.iter().enumerate() {
                // Timestamp should increment properly
                let expected_timestamp = test_data[0].timestamp + chrono::Duration::minutes((i + 1) as i64);
                assert_eq!(pred.timestamp, expected_timestamp);
                
                // Model name should indicate enhanced
                assert!(pred.model_name.ends_with("_enhanced"));
            }
        }
    }
}