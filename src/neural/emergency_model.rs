use anyhow::Result;
use std::fmt::Debug;

/// Architecture information for model introspection
#[derive(Debug, Clone)]
pub struct ModelArchitectureInfo {
    pub input_size: usize,
    pub output_size: usize,
    pub hidden_layers: Vec<usize>,
    pub activation_function: String,
    pub parameter_count: Option<usize>,
}

impl Default for ModelArchitectureInfo {
    fn default() -> Self {
        Self {
            input_size: 60,
            output_size: 1,
            hidden_layers: vec![128, 64, 32],
            activation_function: "ReLU".to_string(),
            parameter_count: None,
        }
    }
}

/// Emergency model implementation for Phase 1 stabilization
/// Uses Simple Moving Average (SMA) for basic predictions
pub struct EmergencyModel {
    model_type: String,
    sector: String,
    window_size: usize,
}

impl EmergencyModel {
    pub fn new(model_type: String, sector: String, window_size: usize) -> Self {
        Self {
            model_type,
            sector,
            window_size,
        }
    }
}

/// Minimal BaseModel trait implementation for emergency stabilization
pub trait BaseModel<T>: Send + Sync + std::fmt::Debug {
    type State;
    type Config;
    
    fn predict(&self, data: &[T]) -> Result<Vec<T>>;
    fn get_state(&self) -> &Self::State;
    fn set_state(&mut self, state: Self::State);
    fn get_model_type(&self) -> &str;
    fn get_architecture_info(&self) -> ModelArchitectureInfo;
}

impl BaseModel<f32> for EmergencyModel {
    type State = ();
    type Config = ();
    
    fn predict(&self, data: &[f32]) -> Result<Vec<f32>> {
        // Handle empty data case
        if data.is_empty() {
            return Ok(vec![0.0]);
        }
        
        // Calculate SMA with configured window size
        let window = self.window_size.min(data.len());
        let sum: f32 = data.iter()
            .rev()
            .take(window)
            .sum();
        let avg = sum / window as f32;
        
        // Return single prediction value
        Ok(vec![avg])
    }
    
    fn get_state(&self) -> &Self::State {
        &()
    }
    
    fn set_state(&mut self, _state: Self::State) {
        // No state for emergency model
    }
    
    fn get_model_type(&self) -> &str {
        &self.model_type
    }
    
    fn get_architecture_info(&self) -> ModelArchitectureInfo {
        ModelArchitectureInfo {
            input_size: self.window_size,
            output_size: 1,
            hidden_layers: vec![], // Emergency model has no hidden layers
            activation_function: "Linear".to_string(),
            parameter_count: Some(self.window_size), // Simple moving average weights
        }
    }
}

impl Debug for EmergencyModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EmergencyModel({}, {})", self.model_type, self.sector)
    }
}

/// Factory for creating emergency models
pub struct EmergencyModelFactory;

impl EmergencyModelFactory {
    pub fn create_emergency_model(
        model_type: &str,
        sector: &str,
        _config: Option<ModelConfig>,
    ) -> Result<Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>> {
        // Phase 1: All models use EmergencyModel implementation
        let model = EmergencyModel::new(
            model_type.to_string(),
            sector.to_string(),
            5, // Default SMA window size
        );
        
        Ok(Box::new(model))
    }
}

/// Placeholder for model configuration
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub input_features: usize,
    pub hidden_units: usize,
    pub layers: usize,
    pub dropout_rate: f32,
    pub bidirectional: bool,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            input_features: 10,
            hidden_units: 64,
            layers: 2,
            dropout_rate: 0.1,
            bidirectional: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_emergency_model_basic_prediction() {
        let model = EmergencyModel::new("LSTM".to_string(), "technology".to_string(), 5);
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        let result = model.predict(&data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 3.0); // Average of [1,2,3,4,5]
    }
    
    #[test]
    fn test_emergency_model_edge_cases() {
        let model = EmergencyModel::new("LSTM".to_string(), "technology".to_string(), 5);
        
        // Empty data
        let result = model.predict(&[]).unwrap();
        assert_eq!(result, vec![0.0]);
        
        // Single value
        let result = model.predict(&[42.0]).unwrap();
        assert_eq!(result, vec![42.0]);
        
        // Less than window size
        let result = model.predict(&[1.0, 2.0]).unwrap();
        assert_eq!(result, vec![1.5]);
    }
}