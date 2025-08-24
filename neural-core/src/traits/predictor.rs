//! Neural prediction model traits
//! Module size: <150 lines as per requirements

use crate::errors::{CoreError, Result};
use crate::types::{MarketData, PredictionResult};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Training configuration for models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub learning_rate: f64,
    pub batch_size: usize,
    pub epochs: u32,
    pub validation_split: f64,
    pub early_stopping_patience: Option<u32>,
    pub regularization_l1: Option<f64>,
    pub regularization_l2: Option<f64>,
    pub dropout_rate: Option<f64>,
    pub optimizer: OptimizerType,
    pub loss_function: LossFunction,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.001,
            batch_size: 32,
            epochs: 100,
            validation_split: 0.2,
            early_stopping_patience: Some(10),
            regularization_l1: None,
            regularization_l2: Some(0.001),
            dropout_rate: Some(0.2),
            optimizer: OptimizerType::Adam,
            loss_function: LossFunction::MeanSquaredError,
        }
    }
}

/// Optimizer types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizerType {
    SGD,
    Adam,
    RMSprop,
    AdaGrad,
}

/// Loss function types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LossFunction {
    MeanSquaredError,
    MeanAbsoluteError,
    Huber,
    BinaryCrossentropy,
    CategoricalCrossentropy,
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub accuracy: Option<f64>,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub f1_score: Option<f64>,
    pub mse: Option<f64>,
    pub mae: Option<f64>,
    pub rmse: Option<f64>,
    pub r_squared: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub total_return: Option<f64>,
    pub validation_loss: Option<f64>,
    pub training_loss: Option<f64>,
}

impl Default for ModelMetrics {
    fn default() -> Self {
        Self {
            accuracy: None,
            precision: None,
            recall: None,
            f1_score: None,
            mse: None,
            mae: None,
            rmse: None,
            r_squared: None,
            sharpe_ratio: None,
            max_drawdown: None,
            total_return: None,
            validation_loss: None,
            training_loss: None,
        }
    }
}

/// Neural predictor trait for price/trend prediction
#[async_trait]
pub trait Predictor: Send + Sync {
    /// Make prediction from market data
    async fn predict(&self, market_data: &[MarketData]) -> Result<PredictionResult>;
    
    /// Train model with historical data
    async fn train(&mut self, training_data: &[MarketData], config: &TrainingConfig) -> Result<ModelMetrics>;
    
    /// Evaluate model performance
    async fn evaluate(&self, test_data: &[MarketData]) -> Result<ModelMetrics>;
    
    /// Update model with new data (online learning)
    async fn update(&mut self, new_data: &[MarketData]) -> Result<()>;
    
    /// Save model to storage
    async fn save_model(&self, path: &str) -> Result<()>;
    
    /// Load model from storage
    async fn load_model(&mut self, path: &str) -> Result<()>;
    
    /// Get model metadata
    fn get_model_info(&self) -> ModelInfo;
    
    /// Set model parameters
    fn set_parameters(&mut self, params: HashMap<String, f64>) -> Result<()>;
    
    /// Get current model parameters
    fn get_parameters(&self) -> HashMap<String, f64>;
    
    /// Check if model is trained and ready
    fn is_ready(&self) -> bool;
    
    /// Get required input features
    fn required_features(&self) -> Vec<String>;
    
    /// Validate input data
    fn validate_input(&self, data: &[MarketData]) -> Result<()> {
        if data.is_empty() {
            return Err(CoreError::Validation("Input data cannot be empty".to_string()));
        }
        
        // Basic validation - can be overridden by implementations
        for market_data in data {
            market_data.validate()?;
        }
        
        Ok(())
    }
}

/// Model information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub version: String,
    pub model_type: String,
    pub created_at: DateTime<Utc>,
    pub last_trained: Option<DateTime<Utc>>,
    pub training_samples: Option<u64>,
    pub input_features: Vec<String>,
    pub output_dimension: usize,
    pub architecture_summary: String,
}

impl ModelInfo {
    pub fn new(name: String, model_type: String) -> Self {
        Self {
            name,
            version: "1.0.0".to_string(),
            model_type,
            created_at: Utc::now(),
            last_trained: None,
            training_samples: None,
            input_features: Vec::new(),
            output_dimension: 1,
            architecture_summary: "Default architecture".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert_eq!(config.learning_rate, 0.001);
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.epochs, 100);
        assert_eq!(config.validation_split, 0.2);
    }
    
    #[test]
    fn test_model_info_creation() {
        let info = ModelInfo::new("LSTM".to_string(), "RNN".to_string());
        assert_eq!(info.name, "LSTM");
        assert_eq!(info.model_type, "RNN");
        assert_eq!(info.version, "1.0.0");
        assert_eq!(info.output_dimension, 1);
    }
    
    #[test]
    fn test_model_metrics_default() {
        let metrics = ModelMetrics::default();
        assert!(metrics.accuracy.is_none());
        assert!(metrics.mse.is_none());
        assert!(metrics.sharpe_ratio.is_none());
    }
}