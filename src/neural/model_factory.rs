//! Model Factory System for Typed Storage
//!
//! This module provides factory patterns for creating typed models that implement
//! the BaseModel trait, enabling type-safe instantiation and registration.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::neural::emergency_model::{BaseModel, EmergencyModel, ModelConfig as EmergencyModelConfig};
use crate::neural::typed_storage::ModelArchitectureInfo;

/// Factory trait for creating typed models
pub trait ModelFactory<T>: Send + Sync {
    /// Create a new model instance
    fn create(&self, config: ModelConfig) -> Result<Box<dyn BaseModel<T, State = (), Config = ()> + Send + Sync>>;
    
    /// Get the model type this factory produces
    fn model_type(&self) -> &str;
    
    /// Get supported architectures for this model type
    fn supported_architectures(&self) -> Vec<String>;
    
    /// Get default configuration for this model type
    fn default_config(&self) -> ModelConfig;
    
    /// Validate configuration before model creation
    fn validate_config(&self, config: &ModelConfig) -> Result<()>;
}

/// Universal model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model architecture name
    pub architecture: String,
    
    /// Input features size
    pub input_size: usize,
    
    /// Output size
    pub output_size: usize,
    
    /// Hidden layer sizes
    pub hidden_layers: Vec<usize>,
    
    /// Learning rate
    pub learning_rate: f64,
    
    /// Dropout rate
    pub dropout_rate: f32,
    
    /// Activation function
    pub activation: String,
    
    /// Model-specific parameters
    pub parameters: HashMap<String, serde_json::Value>,
    
    /// Training configuration
    pub training_config: Option<TrainingConfig>,
}

/// Training configuration for models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub batch_size: usize,
    pub epochs: usize,
    pub validation_split: f32,
    pub early_stopping_patience: Option<usize>,
    pub learning_rate_schedule: Option<String>,
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            architecture: "MLP".to_string(),
            input_size: 60,
            output_size: 1,
            hidden_layers: vec![128, 64, 32],
            learning_rate: 0.001,
            dropout_rate: 0.1,
            activation: "ReLU".to_string(),
            parameters: HashMap::new(),
            training_config: None,
        }
    }
}

impl ModelConfig {
    /// Create config from sector model definition
    pub fn from_sector_definition(
        model_def: &crate::config::SectorModelDefinition,
    ) -> Self {
        let mut config = Self::default();
        config.architecture = model_def.model_type.clone();
        
        // Set model-specific parameters from definition
        if let Some(ref params) = model_def.parameters {
            for (key, value) in params {
                config.parameters.insert(key.clone(), value.clone());
            }
        }
        
        // Extract specific parameters if available
        if let Some(input_size) = config.parameters.get("input_size")
            .and_then(|v| v.as_u64()) {
            config.input_size = input_size as usize;
        }
        
        if let Some(output_size) = config.parameters.get("output_size")
            .and_then(|v| v.as_u64()) {
            config.output_size = output_size as usize;
        }
        
        if let Some(layers) = config.parameters.get("hidden_layers")
            .and_then(|v| v.as_array()) {
            config.hidden_layers = layers.iter()
                .filter_map(|v| v.as_u64().map(|n| n as usize))
                .collect();
        }
        
        config
    }
    
    /// Convert to architecture info
    pub fn to_architecture_info(&self) -> ModelArchitectureInfo {
        ModelArchitectureInfo {
            input_size: self.input_size,
            output_size: self.output_size,
            hidden_layers: self.hidden_layers.clone(),
            activation_function: self.activation.clone(),
            parameter_count: self.estimate_parameter_count(),
        }
    }
    
    /// Estimate parameter count based on architecture
    fn estimate_parameter_count(&self) -> Option<usize> {
        if self.hidden_layers.is_empty() {
            return Some(self.input_size * self.output_size);
        }
        
        let mut params = 0;
        let mut prev_size = self.input_size;
        
        // Hidden layers
        for &layer_size in &self.hidden_layers {
            params += prev_size * layer_size + layer_size; // weights + biases
            prev_size = layer_size;
        }
        
        // Output layer
        params += prev_size * self.output_size + self.output_size;
        
        Some(params)
    }
}

/// Registry for model factories
pub struct ModelFactoryRegistry<T> {
    factories: HashMap<String, Arc<dyn ModelFactory<T> + Send + Sync>>,
    default_configs: HashMap<String, ModelConfig>,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> ModelFactoryRegistry<T> {
    /// Create new registry
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            default_configs: HashMap::new(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Register a model factory
    pub fn register<F: ModelFactory<T> + Send + Sync + 'static>(&mut self, factory: F) {
        let model_type = factory.model_type().to_string();
        let default_config = factory.default_config();
        
        self.default_configs.insert(model_type.clone(), default_config);
        self.factories.insert(model_type.clone(), Arc::new(factory));
        
        info!("✅ Registered model factory for type: {}", model_type);
    }
    
    /// Create model using registered factory
    pub fn create_model(
        &self,
        model_type: &str,
        config: ModelConfig,
    ) -> Result<Box<dyn BaseModel<T, State = (), Config = ()> + Send + Sync>> {
        let factory = self.factories.get(model_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown model type: {}", model_type))?;
        
        factory.validate_config(&config)?;
        let model = factory.create(config)?;
        debug!("✅ Created {} model", model_type);
        Ok(model)
    }
    
    /// Create model with default configuration
    pub fn create_model_default(&self, model_type: &str) -> Result<Box<dyn BaseModel<T, State = (), Config = ()> + Send + Sync>> {
        let config = self.get_default_config(model_type)?;
        self.create_model(model_type, config)
    }
    
    /// Get default configuration for model type
    pub fn get_default_config(&self, model_type: &str) -> Result<ModelConfig> {
        self.default_configs.get(model_type)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No default config for model type: {}", model_type))
    }
    
    /// List registered model types
    pub fn list_model_types(&self) -> Vec<String> {
        self.factories.keys().cloned().collect()
    }
    
    /// Get supported architectures for model type
    pub fn get_supported_architectures(&self, model_type: &str) -> Result<Vec<String>> {
        let factory = self.factories.get(model_type)
            .ok_or_else(|| anyhow::anyhow!("Unknown model type: {}", model_type))?;
        Ok(factory.supported_architectures())
    }
}

impl<T> Default for ModelFactoryRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Emergency model factory implementation
pub struct EmergencyModelFactory;

impl ModelFactory<f32> for EmergencyModelFactory {
    fn create(&self, config: ModelConfig) -> Result<Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>> {
        // Extract window size from parameters or use default
        let window_size = config.parameters
            .get("window_size")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(5);
        
        let model = EmergencyModel::new(
            config.architecture.clone(),
            "universal".to_string(), // Emergency models are universal
            window_size,
        );
        
        debug!("Created emergency model: {} with window size {}", config.architecture, window_size);
        Ok(Box::new(model))
    }
    
    fn model_type(&self) -> &str {
        "EmergencyModel"
    }
    
    fn supported_architectures(&self) -> Vec<String> {
        vec![
            "SMA".to_string(),
            "EMA".to_string(),
            "Linear".to_string(),
            "Constant".to_string(),
        ]
    }
    
    fn default_config(&self) -> ModelConfig {
        let mut config = ModelConfig::default();
        config.architecture = "SMA".to_string();
        config.parameters.insert("window_size".to_string(), serde_json::json!(5));
        config
    }
    
    fn validate_config(&self, config: &ModelConfig) -> Result<()> {
        if !self.supported_architectures().contains(&config.architecture) {
            return Err(anyhow::anyhow!(
                "Unsupported architecture for EmergencyModel: {}. Supported: {:?}",
                config.architecture,
                self.supported_architectures()
            ));
        }
        
        // Validate window size
        if let Some(window_size) = config.parameters.get("window_size") {
            if let Some(size) = window_size.as_u64() {
                if size == 0 || size > 100 {
                    return Err(anyhow::anyhow!("Window size must be between 1 and 100, got: {}", size));
                }
            }
        }
        
        Ok(())
    }
}

/// LSTM model factory (placeholder for future implementation)
pub struct LSTMModelFactory;

impl ModelFactory<f32> for LSTMModelFactory {
    fn create(&self, config: ModelConfig) -> Result<Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>> {
        // For now, create an emergency model that acts as LSTM placeholder
        let window_size = config.input_size.min(20).max(5); // Reasonable window for LSTM-like behavior
        
        let model = EmergencyModel::new(
            "LSTM".to_string(),
            config.parameters.get("sector")
                .and_then(|v| v.as_str())
                .unwrap_or("universal")
                .to_string(),
            window_size,
        );
        
        debug!("Created LSTM placeholder model with window size {}", window_size);
        Ok(Box::new(model))
    }
    
    fn model_type(&self) -> &str {
        "LSTM"
    }
    
    fn supported_architectures(&self) -> Vec<String> {
        vec![
            "LSTM".to_string(),
            "BiLSTM".to_string(),
            "LSTM_Attention".to_string(),
        ]
    }
    
    fn default_config(&self) -> ModelConfig {
        let mut config = ModelConfig::default();
        config.architecture = "LSTM".to_string();
        config.hidden_layers = vec![64, 32];
        config.parameters.insert("sequence_length".to_string(), serde_json::json!(20));
        config.parameters.insert("num_layers".to_string(), serde_json::json!(2));
        config
    }
    
    fn validate_config(&self, config: &ModelConfig) -> Result<()> {
        if !self.supported_architectures().contains(&config.architecture) {
            return Err(anyhow::anyhow!(
                "Unsupported architecture for LSTM: {}. Supported: {:?}",
                config.architecture,
                self.supported_architectures()
            ));
        }
        
        // Validate sequence length
        if let Some(seq_len) = config.parameters.get("sequence_length") {
            if let Some(len) = seq_len.as_u64() {
                if len < 5 || len > 100 {
                    return Err(anyhow::anyhow!("Sequence length must be between 5 and 100, got: {}", len));
                }
            }
        }
        
        Ok(())
    }
}

/// Transformer model factory (placeholder for future implementation)
pub struct TransformerModelFactory;

impl ModelFactory<f32> for TransformerModelFactory {
    fn create(&self, config: ModelConfig) -> Result<Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>> {
        // For now, create an emergency model that acts as Transformer placeholder
        let window_size = config.input_size.min(30).max(10); // Larger window for Transformer-like behavior
        
        let model = EmergencyModel::new(
            "Transformer".to_string(),
            config.parameters.get("sector")
                .and_then(|v| v.as_str())
                .unwrap_or("universal")
                .to_string(),
            window_size,
        );
        
        debug!("Created Transformer placeholder model with window size {}", window_size);
        Ok(Box::new(model))
    }
    
    fn model_type(&self) -> &str {
        "Transformer"
    }
    
    fn supported_architectures(&self) -> Vec<String> {
        vec![
            "Transformer".to_string(),
            "GPT".to_string(),
            "BERT".to_string(),
            "T5".to_string(),
        ]
    }
    
    fn default_config(&self) -> ModelConfig {
        let mut config = ModelConfig::default();
        config.architecture = "Transformer".to_string();
        config.hidden_layers = vec![256, 128, 64];
        config.parameters.insert("num_heads".to_string(), serde_json::json!(8));
        config.parameters.insert("num_layers".to_string(), serde_json::json!(6));
        config.parameters.insert("d_model".to_string(), serde_json::json!(256));
        config
    }
    
    fn validate_config(&self, config: &ModelConfig) -> Result<()> {
        if !self.supported_architectures().contains(&config.architecture) {
            return Err(anyhow::anyhow!(
                "Unsupported architecture for Transformer: {}. Supported: {:?}",
                config.architecture,
                self.supported_architectures()
            ));
        }
        
        // Validate attention heads
        if let Some(num_heads) = config.parameters.get("num_heads") {
            if let Some(heads) = num_heads.as_u64() {
                if heads < 1 || heads > 16 || (heads & (heads - 1)) != 0 {
                    return Err(anyhow::anyhow!("Number of heads must be a power of 2 between 1 and 16, got: {}", heads));
                }
            }
        }
        
        Ok(())
    }
}

/// Create a fully configured model factory registry
pub fn create_default_registry() -> ModelFactoryRegistry<f32> {
    let registry = ModelFactoryRegistry::new();
    
    info!("✅ Created default model factory registry with {} factories", 
          registry.list_model_types().len());
    
    registry
}

/// Model creation utilities
pub struct ModelCreationUtils;

impl ModelCreationUtils {
    /// Create model from sector configuration (Phase 1: disabled pending config integration)
    pub fn create_from_sector_config(
        registry: &ModelFactoryRegistry<f32>,
        _model_name: &str,
        _model_def: &crate::config::SectorModelDefinition,
    ) -> Result<Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>> {
        // Phase 1: Use default emergency model
        // Phase 1: Create emergency model in Box format
        use crate::neural::emergency_model::EmergencyModel;
        Ok(Box::new(EmergencyModel::new(
            "Emergency".to_string(),
            "universal".to_string(),
            5
        )) as Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>)
    }
    
    /// Batch create models from sector configuration (Phase 1: simplified default creation)
    pub fn batch_create_default_models(
        registry: &ModelFactoryRegistry<f32>,
    ) -> Result<Vec<(String, String, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>)>> {
        let mut models = Vec::new();
        
        // Phase 1: Create default models for main sectors
        let model_types = ["EmergencyModel", "LSTM", "Transformer"];
        let sectors = ["technology", "healthcare", "financial"];
        
        for model_type in &model_types {
            for sector in &sectors {
                // Phase 1: Create emergency model directly
                use crate::neural::emergency_model::EmergencyModel;
                let model_name = format!("{}-{}", model_type, sector);
                let model = Box::new(EmergencyModel::new(
                    model_type.to_string(),
                    sector.to_string(),
                    5
                )) as Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>;
                models.push((
                    model_name,
                    sector.to_string(),
                    model,
                ));
            }
        }
        
        info!("✅ Created {} default models", models.len());
        Ok(models)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_emergency_model_factory() {
        let factory = EmergencyModelFactory;
        let config = factory.default_config();
        
        // Test factory properties
        assert_eq!(factory.model_type(), "EmergencyModel");
        assert!(!factory.supported_architectures().is_empty());
        
        // Test model creation
        let model = factory.create(config).unwrap();
        assert_eq!(model.get_model_type(), "SMA");
        
        // Test prediction
        let test_data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let prediction = model.predict(&test_data).unwrap();
        assert!(!prediction.is_empty());
    }
    
    #[test]
    fn test_model_factory_registry() {
        let mut registry = ModelFactoryRegistry::new();
        registry.register(EmergencyModelFactory);
        registry.register(LSTMModelFactory);
        
        // Test model type listing
        let types = registry.list_model_types();
        assert!(types.contains(&"EmergencyModel".to_string()));
        assert!(types.contains(&"LSTM".to_string()));
        
        // Test model creation
        let model = registry.create_model_default("EmergencyModel").unwrap();
        assert_eq!(model.get_model_type(), "SMA");
        
        // Test configuration
        let config = registry.get_default_config("LSTM").unwrap();
        assert_eq!(config.architecture, "LSTM");
        assert!(config.hidden_layers.len() >= 2);
    }
    
    #[test]
    fn test_model_config_parameter_estimation() {
        let config = ModelConfig {
            input_size: 10,
            output_size: 1,
            hidden_layers: vec![20, 10],
            ..Default::default()
        };
        
        let param_count = config.estimate_parameter_count().unwrap();
        // Expected: (10*20 + 20) + (20*10 + 10) + (10*1 + 1) = 220 + 210 + 11 = 441
        assert_eq!(param_count, 441);
    }
    
    #[test]
    fn test_config_validation() {
        let factory = EmergencyModelFactory;
        
        // Valid config
        let valid_config = factory.default_config();
        assert!(factory.validate_config(&valid_config).is_ok());
        
        // Invalid architecture
        let mut invalid_config = valid_config.clone();
        invalid_config.architecture = "INVALID".to_string();
        assert!(factory.validate_config(&invalid_config).is_err());
        
        // Invalid window size
        let mut invalid_window = valid_config.clone();
        invalid_window.parameters.insert("window_size".to_string(), serde_json::json!(0));
        assert!(factory.validate_config(&invalid_window).is_err());
    }
}