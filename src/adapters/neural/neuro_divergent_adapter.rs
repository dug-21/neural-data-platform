//! Neuro-divergent adapter implementation
//! 
//! This adapter provides the interface for interacting with neuro-divergent models,
//! handling model initialization, data conversion, and prediction workflows.

use async_trait::async_trait;
use thiserror::Error;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use crate::adapters::{AdapterError, DataAdapter};
use crate::data::TimeSeriesData;

/// Errors specific to neural adapter operations
#[derive(Error, Debug)]
pub enum NeuralAdapterError {
    #[error("Model initialization failed: {0}")]
    ModelInit(String),
    
    #[error("Prediction failed: {0}")]
    Prediction(String),
    
    #[error("Data conversion failed: {0}")]
    Conversion(String),
    
    #[error("Model not initialized")]
    NotInitialized,
    
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<NeuralAdapterError> for AdapterError {
    fn from(err: NeuralAdapterError) -> Self {
        match err {
            NeuralAdapterError::ModelInit(msg) => AdapterError::Connection(msg),
            NeuralAdapterError::Prediction(msg) => AdapterError::Query(msg),
            NeuralAdapterError::Conversion(msg) => AdapterError::Serialization(msg),
            NeuralAdapterError::NotInitialized => AdapterError::Connection("Model not initialized".to_string()),
            NeuralAdapterError::InvalidConfig(msg) => AdapterError::Configuration(msg),
        }
    }
}

impl From<anyhow::Error> for NeuralAdapterError {
    fn from(err: anyhow::Error) -> Self {
        NeuralAdapterError::Conversion(err.to_string())
    }
}

/// Configuration for neuro-divergent models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralModelConfig {
    /// Model type (e.g., "TimeMixer", "NeuralForecast", "TimesFM")
    pub model_type: String,
    
    /// Lookback window size
    pub lookback_window: usize,
    
    /// Forecast horizon
    pub forecast_horizon: usize,
    
    /// Batch size for predictions
    pub batch_size: usize,
    
    /// Whether to use GPU acceleration
    pub use_gpu: bool,
    
    /// Model-specific parameters
    pub model_params: serde_json::Value,
}

impl Default for NeuralModelConfig {
    fn default() -> Self {
        Self {
            model_type: "TimeMixer".to_string(),
            lookback_window: 24,
            forecast_horizon: 6,
            batch_size: 32,
            use_gpu: false,
            model_params: serde_json::json!({}),
        }
    }
}

/// State of the neural model
#[derive(Debug, Clone)]
enum ModelState {
    Uninitialized,
    Initialized,
    Ready,
    Failed(String),
}

/// Adapter for neuro-divergent neural models
pub struct NeuroDivergentAdapter {
    /// Model configuration
    config: NeuralModelConfig,
    
    /// Current model state
    state: Arc<Mutex<ModelState>>,
    
    /// Model handle (placeholder for actual model integration)
    model_handle: Arc<Mutex<Option<String>>>,
}

impl NeuroDivergentAdapter {
    /// Create a new neural adapter with the given configuration
    pub fn new(config: NeuralModelConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(ModelState::Uninitialized)),
            model_handle: Arc::new(Mutex::new(None)),
        }
    }
    
    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(NeuralModelConfig::default())
    }
    
    /// Update model configuration
    pub async fn update_config(&mut self, config: NeuralModelConfig) -> Result<(), NeuralAdapterError> {
        // Check if model is currently running
        let state = self.state.lock().await;
        match &*state {
            ModelState::Ready => {
                return Err(NeuralAdapterError::InvalidConfig(
                    "Cannot update config while model is ready. Disconnect first.".to_string()
                ));
            }
            _ => {}
        }
        drop(state);
        
        self.config = config;
        Ok(())
    }
    
    /// Initialize the model with current configuration
    async fn initialize_model(&self) -> Result<(), NeuralAdapterError> {
        // Validate configuration
        if self.config.lookback_window == 0 {
            return Err(NeuralAdapterError::InvalidConfig(
                "Lookback window must be greater than 0".to_string()
            ));
        }
        
        if self.config.forecast_horizon == 0 {
            return Err(NeuralAdapterError::InvalidConfig(
                "Forecast horizon must be greater than 0".to_string()
            ));
        }
        
        // Simulate model initialization
        // In actual implementation, this would initialize the neuro-divergent model
        let model_id = format!("{}-{}", self.config.model_type, uuid::Uuid::new_v4());
        
        let mut handle = self.model_handle.lock().await;
        *handle = Some(model_id);
        
        Ok(())
    }
    
    /// Make predictions using the model
    pub async fn predict(&self, data: &[TimeSeriesData]) -> Result<Vec<f64>, NeuralAdapterError> {
        // Check if model is ready
        let state = self.state.lock().await;
        match &*state {
            ModelState::Ready => {},
            ModelState::Failed(msg) => return Err(NeuralAdapterError::Prediction(msg.clone())),
            _ => return Err(NeuralAdapterError::NotInitialized),
        }
        drop(state);
        
        // Validate input data
        if data.len() < self.config.lookback_window {
            return Err(NeuralAdapterError::Prediction(format!(
                "Insufficient data: need at least {} points, got {}",
                self.config.lookback_window,
                data.len()
            )));
        }
        
        // Convert data to model format using the data converter
        // Note: Normalization is handled upstream, we only do format conversion
        let converter = super::data_converter::DataConverter::new();
        let model_input = converter.to_model_format(data, &self.config)?;
        
        // Simulate prediction
        // In actual implementation, this would call the neuro-divergent model
        let predictions = vec![0.0; self.config.forecast_horizon];
        
        Ok(predictions)
    }
    
    /// Get current model state
    pub async fn get_state(&self) -> ModelState {
        self.state.lock().await.clone()
    }
}

#[async_trait]
impl DataAdapter for NeuroDivergentAdapter {
    async fn connect(&mut self) -> Result<(), AdapterError> {
        let mut state = self.state.lock().await;
        
        match &*state {
            ModelState::Ready => {
                return Ok(()); // Already connected
            }
            ModelState::Failed(_) => {
                *state = ModelState::Uninitialized;
            }
            _ => {}
        }
        
        *state = ModelState::Initialized;
        drop(state);
        
        // Initialize the model
        match self.initialize_model().await {
            Ok(()) => {
                let mut state = self.state.lock().await;
                *state = ModelState::Ready;
                Ok(())
            }
            Err(e) => {
                let mut state = self.state.lock().await;
                *state = ModelState::Failed(e.to_string());
                Err(e.into())
            }
        }
    }
    
    async fn disconnect(&mut self) -> Result<(), AdapterError> {
        let mut state = self.state.lock().await;
        *state = ModelState::Uninitialized;
        
        let mut handle = self.model_handle.lock().await;
        *handle = None;
        
        Ok(())
    }
    
    fn is_connected(&self) -> bool {
        // Use try_lock to avoid blocking
        if let Ok(state) = self.state.try_lock() {
            matches!(&*state, ModelState::Ready)
        } else {
            false
        }
    }
    
    fn name(&self) -> &str {
        "NeuroDivergentAdapter"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    
    fn create_test_data(points: usize) -> Vec<TimeSeriesData> {
        (0..points).map(|i| {
            TimeSeriesData {
                symbol: "TEST".to_string(),
                timestamp: Utc::now(),
                open: 100.0 + i as f64,
                high: 101.0 + i as f64,
                low: 99.0 + i as f64,
                close: 100.5 + i as f64,
                volume: 1000.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: None,
                value: None,
                metadata: None,
            }
        }).collect()
    }
    
    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let mut adapter = NeuroDivergentAdapter::default();
        
        // Initially disconnected
        assert!(!adapter.is_connected());
        
        // Connect
        adapter.connect().await.unwrap();
        assert!(adapter.is_connected());
        
        // Disconnect
        adapter.disconnect().await.unwrap();
        assert!(!adapter.is_connected());
    }
    
    #[tokio::test]
    async fn test_config_validation() {
        let mut config = NeuralModelConfig::default();
        config.lookback_window = 0;
        
        let mut adapter = NeuroDivergentAdapter::new(config);
        let result = adapter.connect().await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Lookback window"));
    }
    
    #[tokio::test]
    async fn test_prediction_validation() {
        let mut adapter = NeuroDivergentAdapter::default();
        adapter.connect().await.unwrap();
        
        // Test with insufficient data
        let data = create_test_data(10); // Less than default lookback window (24)
        let result = adapter.predict(&data).await;
        
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Insufficient data"));
    }
}

// Add uuid dependency for model ID generation
use uuid;