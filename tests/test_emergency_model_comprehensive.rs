use anyhow::Result;
use std::collections::HashMap;

// Import the modules under test
use neural_trader::neural::emergency_model::{
    EmergencyModel, BaseModel, EmergencyModelFactory, ModelConfig
};

#[cfg(test)]
mod emergency_model_tests {
    use super::*;

    #[test]
    fn test_emergency_model_creation() {
        let model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            5
        );
        
        assert_eq!(model.get_model_type(), "LSTM");
        
        // Test Debug implementation
        let debug_output = format!("{:?}", model);
        assert!(debug_output.contains("EmergencyModel"));
        assert!(debug_output.contains("LSTM"));
        assert!(debug_output.contains("technology"));
    }

    #[test]
    fn test_emergency_model_basic_prediction() {
        let model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            5
        );
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let result = model.predict(&data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 3.0); // Average of [1,2,3,4,5]
    }

    #[test]
    fn test_emergency_model_edge_cases() {
        let model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            5
        );
        
        // Empty data
        let result = model.predict(&[]).unwrap();
        assert_eq!(result, vec![0.0]);
        
        // Single value
        let result = model.predict(&[42.0]).unwrap();
        assert_eq!(result, vec![42.0]);
        
        // Less than window size
        let result = model.predict(&[1.0, 2.0]).unwrap();
        assert_eq!(result, vec![1.5]);
        
        // Window larger than data
        let model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            10
        );
        let result = model.predict(&[1.0, 2.0, 3.0]).unwrap();
        assert_eq!(result, vec![2.0]); // Average of all 3 values
    }

    #[test]
    fn test_emergency_model_window_sizes() {
        // Test different window sizes
        let test_data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        
        // Window size 3
        let model = EmergencyModel::new("LSTM".to_string(), "tech".to_string(), 3);
        let result = model.predict(&test_data).unwrap();
        assert_eq!(result[0], 9.0); // Average of last 3: [8,9,10]
        
        // Window size 1
        let model = EmergencyModel::new("LSTM".to_string(), "tech".to_string(), 1);
        let result = model.predict(&test_data).unwrap();
        assert_eq!(result[0], 10.0); // Just the last value
        
        // Window size larger than data
        let model = EmergencyModel::new("LSTM".to_string(), "tech".to_string(), 20);
        let result = model.predict(&test_data).unwrap();
        assert_eq!(result[0], 5.5); // Average of all 10 values
    }

    #[test]
    fn test_emergency_model_negative_values() {
        let model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            3
        );
        
        // Negative values
        let data = vec![-1.0, -2.0, -3.0];
        let result = model.predict(&data).unwrap();
        assert_eq!(result[0], -2.0);
        
        // Mixed positive and negative
        let data = vec![-5.0, 0.0, 5.0];
        let result = model.predict(&data).unwrap();
        assert_eq!(result[0], 0.0);
    }

    #[test]
    fn test_emergency_model_state_methods() {
        let mut model = EmergencyModel::new(
            "LSTM".to_string(), 
            "technology".to_string(), 
            5
        );
        
        // Test state getter
        let state = model.get_state();
        assert_eq!(*state, ());
        
        // Test state setter (should not panic)
        model.set_state(());
        
        // State should remain unchanged
        let state = model.get_state();
        assert_eq!(*state, ());
    }

    #[test]
    fn test_emergency_model_factory() {
        // Test factory creation with default config
        let result = EmergencyModelFactory::create_emergency_model(
            "LSTM",
            "technology",
            None,
        );
        
        assert!(result.is_ok());
        let model = result.unwrap();
        
        // Verify it implements BaseModel trait
        let test_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let prediction = model.predict(&test_data).unwrap();
        assert_eq!(prediction.len(), 1);
        assert_eq!(prediction[0], 3.0);
    }

    #[test]
    fn test_emergency_model_factory_with_config() {
        let config = ModelConfig::default();
        
        let result = EmergencyModelFactory::create_emergency_model(
            "Transformer",
            "healthcare",
            Some(config),
        );
        
        assert!(result.is_ok());
        let model = result.unwrap();
        
        // Test prediction works
        let test_data = vec![10.0, 20.0, 30.0];
        let prediction = model.predict(&test_data).unwrap();
        assert_eq!(prediction.len(), 1);
        assert_eq!(prediction[0], 20.0);
    }

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        
        assert_eq!(config.input_features, 10);
        assert_eq!(config.hidden_units, 64);
        assert_eq!(config.layers, 2);
        assert_eq!(config.dropout_rate, 0.1);
        assert_eq!(config.bidirectional, false);
    }

    #[test]
    fn test_emergency_model_multiple_sectors() {
        // Test different sectors
        let sectors = ["technology", "healthcare", "finance", "energy"];
        
        for sector in sectors {
            let model = EmergencyModel::new(
                "LSTM".to_string(),
                sector.to_string(),
                5,
            );
            
            let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
            let result = model.predict(&data).unwrap();
            
            // All should work the same way regardless of sector
            assert_eq!(result[0], 3.0);
            
            let debug_str = format!("{:?}", model);
            assert!(debug_str.contains(sector));
        }
    }

    #[test]
    fn test_emergency_model_thread_safety() {
        // Test that the model can be created and used across threads
        let model = std::sync::Arc::new(EmergencyModel::new(
            "LSTM".to_string(),
            "technology".to_string(),
            5,
        ));
        
        let mut handles = vec![];
        
        for i in 0..10 {
            let model_clone = model.clone();
            let handle = std::thread::spawn(move || {
                let data = vec![i as f32; 5];
                let result = model_clone.predict(&data).unwrap();
                result[0]
            });
            handles.push(handle);
        }
        
        for handle in handles {
            let result = handle.join().unwrap();
            // Each thread should get its input value as the average
            assert!(result >= 0.0 && result <= 9.0);
        }
    }
}