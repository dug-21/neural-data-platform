//! Model Factory for Vendor Model Creation
//!
//! This module provides factory methods for creating vendor models
//! from the neuro-divergent library with proper configuration.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

// Use the actual vendor library types
use neuro_divergent_core::traits::BaseModel;
use neuro_divergent_core::data::TimeSeriesDataset;
use neuro_divergent_models::foundation::ForecastOutput as ForecastResult;

// Type alias for convenience 
type VendorDataset = TimeSeriesDataset<f32>;

// Use actual vendor models and their configs
use neuro_divergent_models::recurrent::LSTM;
use neuro_divergent::builders::{LSTMBuilder, ModelBuilder};

// Import vendor predictor types
use crate::neural::vendor_predictor::{ModelConfig, DataRequirements};

/// Model capabilities for different architectures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub requires_sequential_data: bool,
    pub supports_exogenous: bool,
    pub supports_static: bool,
    pub min_sequence_length: usize,
    pub optimal_sequence_length: usize,
}

impl Default for ModelCapabilities {
    fn default() -> Self {
        Self {
            requires_sequential_data: true,
            supports_exogenous: false,
            supports_static: false,
            min_sequence_length: 10,
            optimal_sequence_length: 100,
        }
    }
}

// Factory for vendor models - Phase 2 model creation
pub struct ModelFactory;

impl ModelFactory {
    pub fn new() -> Self {
        Self
    }
    
    /// Create a model based on architecture and configuration
    pub fn create_model(architecture: &str, config: &ModelConfig) -> Result<Box<dyn std::any::Any + Send + Sync>> {
        debug!("Creating model: {} with config: {:?}", architecture, config);
        
        match architecture {
            "MLP" | "LSTM" | "GRU" | "RNN" | "TCN" | "BiTCN" | "TFT" | "Informer" | 
            "Autoformer" | "DeepAR" | "NBEATS" | "NHITS" | "DLinear" | "NLinear" => {
                // Create mock model for Phase 2 (real implementation will use vendor models)
                let mock_model = format!("MockModel_{}_{}", architecture, 
                    config.parameters.get("input_size").unwrap_or(&serde_json::json!(24)));
                info!("Created mock {} model successfully", architecture);
                Ok(Box::new(mock_model))
            }
            _ => {
                error!("Unsupported model architecture: {}", architecture);
                Err(anyhow!("Unsupported model architecture: {}", architecture))
            }
        }
    }
    
    /// Get model capabilities for an architecture
    pub fn get_model_capabilities(architecture: &str) -> ModelCapabilities {
        match architecture {
            "MLP" => ModelCapabilities {
                requires_sequential_data: false,
                supports_exogenous: true,
                supports_static: true,
                min_sequence_length: 1,
                optimal_sequence_length: 24,
            },
            "LSTM" | "GRU" | "RNN" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: false,
                min_sequence_length: 10,
                optimal_sequence_length: 100,
            },
            "TCN" | "BiTCN" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: false,
                min_sequence_length: 20,
                optimal_sequence_length: 100,
            },
            "TFT" | "Informer" | "Autoformer" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: true,
                min_sequence_length: 24,
                optimal_sequence_length: 168,
            },
            "DeepAR" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: true,
                min_sequence_length: 30,
                optimal_sequence_length: 200,
            },
            "NBEATS" | "NHITS" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: false,
                supports_static: false,
                min_sequence_length: 50,
                optimal_sequence_length: 500,
            },
            "DLinear" | "NLinear" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: false,
                supports_static: false,
                min_sequence_length: 96,
                optimal_sequence_length: 96,
            },
            _ => ModelCapabilities::default(),
        }
    }
    
    /// Create price-only models for quick testing
    pub fn create_price_only_models() -> Result<HashMap<String, Box<dyn std::any::Any + Send + Sync>>> {
        let mut models = HashMap::new();
        
        let price_config = ModelConfig {
            architecture: "price_only".to_string(),
            parameters: {
                let mut params = HashMap::new();
                params.insert("input_size".to_string(), serde_json::json!(1));
                params.insert("hidden_size".to_string(), serde_json::json!(32));
                params
            },
            data_requirements: DataRequirements {
                required: vec!["price".to_string()],
                optional: vec![],
                min_history: 10,
            },
        };
        
        // Create basic price-only models
        for arch in &["MLP", "LSTM", "TCN", "DLinear"] {
            let model = Self::create_model(arch, &price_config)?;
            models.insert(format!("{}_Price", arch), model);
        }
        
        Ok(models)
    }
    
    pub fn create_lstm(&self) -> Result<Box<dyn std::any::Any + Send + Sync>> {
        // Phase 2 implementation - ready for vendor model integration
        info!("Creating LSTM model using Phase 2 factory");
        let mock_model = "MockLSTM_Phase2".to_string();
        Ok(Box::new(mock_model))
    }
}

// Add needed imports
use crate::adapters::vendor_bridge::VendorTimeSeriesData;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

/// Enhanced model factory for neural predictor integration
pub struct EnhancedModelFactory {
    config: NeuralConfig,
    model_cache: HashMap<String, Box<dyn std::any::Any + Send + Sync>>,
}

impl EnhancedModelFactory {
    /// Create a new enhanced model factory
    pub fn new(config: NeuralConfig) -> Self {
        Self {
            config,
            model_cache: HashMap::new(),
        }
    }

    /// Create and configure an LSTM model
    pub async fn create_lstm_model(
        &mut self,
        model_type: &str,
        config: &NeuralConfig,
    ) -> Result<Box<dyn std::any::Any + Send + Sync>> {
        debug!("Creating LSTM model of type: {}", model_type);
        
        // Mock LSTM configuration and creation
        // This is a compilation stub for Phase 1
        
        // In real implementation, this would:
        // 1. Create LSTMConfig with proper parameters
        // 2. Use LSTMBuilder to construct the model
        // 3. Return properly typed BaseModel<f32>
        
        info!("LSTM model created successfully (mock implementation)");
        Ok(Box::new(format!("MockLSTM_{}", model_type)))
    }

    /// Build model configuration
    fn build_model_config(&self, model_type: &str) -> Result<HashMap<String, String>> {
        let mut config = HashMap::new();
        
        match model_type {
            "lstm" => {
                config.insert("type".to_string(), "LSTM".to_string());
                config.insert("hidden_size".to_string(), "128".to_string());
                config.insert("num_layers".to_string(), "2".to_string());
                config.insert("dropout".to_string(), "0.1".to_string());
            }
            _ => {
                return Err(anyhow!("Unsupported model type: {}", model_type));
            }
        }
        
        Ok(config)
    }

    /// Cache model for reuse
    pub fn cache_model(&mut self, key: String, model: Box<dyn std::any::Any + Send + Sync>) {
        self.model_cache.insert(key, model);
    }

    /// Get cached model
    pub fn get_cached_model(&self, key: &str) -> Option<&Box<dyn std::any::Any + Send + Sync>> {
        self.model_cache.get(key)
    }

    /// Clear model cache
    pub fn clear_cache(&mut self) {
        self.model_cache.clear();
    }
}

impl Default for EnhancedModelFactory {
    fn default() -> Self {
        Self::new(NeuralConfig::default())
    }
}