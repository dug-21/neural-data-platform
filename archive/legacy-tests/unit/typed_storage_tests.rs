//! Comprehensive Tests for Typed Storage System
//!
//! This module provides extensive testing for the refactored system with typed BaseModel<f32>
//! integration, ensuring 100% type safety validation and no downcasting operations.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use std::any::Any;

use crate::config::NeuralConfig;
use crate::data::{
    TimeSeriesData, 
    sector_mapper::{SectorMapper, SectorMapperConfig},
};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::{
    vendor_predictor::{
        VendorPredictor, ModelKey, ClusterModelPool, ClusterPoolConfig,
        VendorPredictorConfig,
    },
    NeuralPredictorTrait, PredictionResult,
};
use crate::adapters::vendor_bridge::VendorTimeSeriesData;
use crate::data::data_converter::{DataConverter, DataConverterConfig, ConversionMetadata};

// ===== TYPED MODEL DEFINITIONS =====

/// Typed BaseModel<f32> trait - ensures type safety at compile time
pub trait TypedBaseModel: Send + Sync {
    type Input;
    type Output;
    type Config;
    type State;
    
    /// Predict with typed inputs and outputs
    fn predict_typed(&self, input: &Self::Input) -> Result<Self::Output>;
    
    /// Get model configuration 
    fn get_config(&self) -> &Self::Config;
    
    /// Get model state
    fn get_state(&self) -> &Self::State;
    
    /// Get model type information
    fn model_type(&self) -> &str;
    
    /// Validate input types at runtime
    fn validate_input(&self, input: &Self::Input) -> Result<()>;
}

/// Typed LSTM model implementing BaseModel<f32>
#[derive(Debug)]
pub struct TypedLSTMModel {
    pub weights: Vec<f32>,
    pub biases: Vec<f32>,
    pub hidden_size: usize,
    pub input_size: usize,
    pub output_size: usize,
    pub config: LSTMConfig,
    pub state: LSTMState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LSTMConfig {
    pub learning_rate: f32,
    pub dropout_rate: f32,
    pub layers: usize,
    pub activation: String,
}

#[derive(Debug, Clone)]
pub struct LSTMState {
    pub hidden: Vec<f32>,
    pub cell: Vec<f32>,
    pub training: bool,
}

impl TypedLSTMModel {
    pub fn new(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        Self {
            weights: vec![0.5; input_size * hidden_size + hidden_size * output_size],
            biases: vec![0.1; hidden_size + output_size],
            hidden_size,
            input_size, 
            output_size,
            config: LSTMConfig {
                learning_rate: 0.01,
                dropout_rate: 0.1,
                layers: 2,
                activation: "tanh".to_string(),
            },
            state: LSTMState {
                hidden: vec![0.0; hidden_size],
                cell: vec![0.0; hidden_size], 
                training: false,
            },
        }
    }
    
    /// Create with specific prediction value for testing
    pub fn new_with_prediction(
        input_size: usize,
        hidden_size: usize, 
        output_size: usize,
        prediction_value: f32,
    ) -> Self {
        let mut model = Self::new(input_size, hidden_size, output_size);
        // Adjust weights to produce specific prediction
        model.weights[0] = prediction_value / 100.0;
        model
    }
}

impl TypedBaseModel for TypedLSTMModel {
    type Input = Vec<f32>;
    type Output = Vec<f32>;
    type Config = LSTMConfig;
    type State = LSTMState;
    
    fn predict_typed(&self, input: &Self::Input) -> Result<Self::Output> {
        if input.is_empty() {
            return Err(anyhow::anyhow!("Input cannot be empty"));
        }
        
        // Simplified LSTM forward pass for testing
        let mut output = vec![0.0; self.output_size];
        
        // Basic matrix multiplication simulation
        for i in 0..self.output_size {
            let mut sum = 0.0;
            for j in 0..input.len().min(self.input_size) {
                let weight_idx = i * self.input_size + j;
                if weight_idx < self.weights.len() {
                    sum += input[j] * self.weights[weight_idx];
                }
            }
            sum += self.biases.get(i).copied().unwrap_or(0.0);
            output[i] = sum.tanh(); // tanh activation
        }
        
        Ok(output)
    }
    
    fn get_config(&self) -> &Self::Config {
        &self.config
    }
    
    fn get_state(&self) -> &Self::State {
        &self.state
    }
    
    fn model_type(&self) -> &str {
        "TypedLSTM"
    }
    
    fn validate_input(&self, input: &Self::Input) -> Result<()> {
        if input.len() != self.input_size {
            return Err(anyhow::anyhow!(
                "Input size mismatch: expected {}, got {}",
                self.input_size,
                input.len()
            ));
        }
        
        // Check for NaN or infinite values
        for (i, &val) in input.iter().enumerate() {
            if !val.is_finite() {
                return Err(anyhow::anyhow!(
                    "Invalid input value at index {}: {}",
                    i, val
                ));
            }
        }
        
        Ok(())
    }
}

/// Typed GRU model implementing BaseModel<f32>
#[derive(Debug)]
pub struct TypedGRUModel {
    pub weights: Vec<f32>,
    pub biases: Vec<f32>,
    pub hidden_size: usize,
    pub input_size: usize,
    pub output_size: usize,
    pub config: GRUConfig,
    pub state: GRUState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GRUConfig {
    pub learning_rate: f32,
    pub reset_gate_bias: f32,
    pub update_gate_bias: f32,
}

#[derive(Debug, Clone)]
pub struct GRUState {
    pub hidden: Vec<f32>,
    pub training: bool,
}

impl TypedGRUModel {
    pub fn new(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        Self {
            weights: vec![0.3; input_size * hidden_size * 3 + hidden_size * output_size], // 3x for gates
            biases: vec![0.05; hidden_size * 3 + output_size],
            hidden_size,
            input_size,
            output_size,
            config: GRUConfig {
                learning_rate: 0.015,
                reset_gate_bias: 0.1,
                update_gate_bias: 0.1,
            },
            state: GRUState {
                hidden: vec![0.0; hidden_size],
                training: false,
            },
        }
    }
    
    pub fn new_with_prediction(
        input_size: usize,
        hidden_size: usize,
        output_size: usize,
        prediction_value: f32,
    ) -> Self {
        let mut model = Self::new(input_size, hidden_size, output_size);
        model.weights[0] = prediction_value / 50.0;
        model
    }
}

impl TypedBaseModel for TypedGRUModel {
    type Input = Vec<f32>;
    type Output = Vec<f32>;
    type Config = GRUConfig;
    type State = GRUState;
    
    fn predict_typed(&self, input: &Self::Input) -> Result<Self::Output> {
        if input.is_empty() {
            return Err(anyhow::anyhow!("Input cannot be empty"));
        }
        
        // Simplified GRU forward pass
        let mut output = vec![0.0; self.output_size];
        
        for i in 0..self.output_size {
            let mut sum = 0.0;
            for j in 0..input.len().min(self.input_size) {
                let weight_idx = i * self.input_size + j;
                if weight_idx < self.weights.len() {
                    sum += input[j] * self.weights[weight_idx];
                }
            }
            sum += self.biases.get(i).copied().unwrap_or(0.0);
            output[i] = 1.0 / (1.0 + (-sum).exp()); // sigmoid activation
        }
        
        Ok(output)
    }
    
    fn get_config(&self) -> &Self::Config {
        &self.config
    }
    
    fn get_state(&self) -> &Self::State {
        &self.state
    }
    
    fn model_type(&self) -> &str {
        "TypedGRU"
    }
    
    fn validate_input(&self, input: &Self::Input) -> Result<()> {
        if input.len() != self.input_size {
            return Err(anyhow::anyhow!(
                "Input size mismatch: expected {}, got {}",
                self.input_size,
                input.len()
            ));
        }
        
        Ok(())
    }
}

/// Typed model storage that maintains type safety
#[derive(Debug)]
pub struct TypedModelStorage {
    /// Storage for typed LSTM models
    lstm_models: Arc<RwLock<HashMap<String, TypedLSTMModel>>>,
    /// Storage for typed GRU models  
    gru_models: Arc<RwLock<HashMap<String, TypedGRUModel>>>,
    /// Metadata storage
    model_metadata: Arc<RwLock<HashMap<String, ModelMetadata>>>,
    /// Type registry for validation
    type_registry: Arc<RwLock<HashMap<String, String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub model_type: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: String,
    pub size_bytes: usize,
    pub performance_metrics: PerformanceMetrics,
    pub type_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub accuracy: f64,
    pub latency_ms: f64,
    pub throughput: f64,
    pub memory_usage_mb: f64,
}

impl TypedModelStorage {
    pub fn new() -> Self {
        Self {
            lstm_models: Arc::new(RwLock::new(HashMap::new())),
            gru_models: Arc::new(RwLock::new(HashMap::new())),
            model_metadata: Arc::new(RwLock::new(HashMap::new())),
            type_registry: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Store typed LSTM model with compile-time type safety
    pub async fn store_lstm_model(
        &self,
        id: String,
        model: TypedLSTMModel,
    ) -> Result<()> {
        let metadata = ModelMetadata {
            id: id.clone(),
            model_type: "TypedLSTM".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(), 
            version: "1.0.0".to_string(),
            size_bytes: model.weights.len() * 4 + model.biases.len() * 4,
            performance_metrics: PerformanceMetrics {
                accuracy: 0.95,
                latency_ms: 10.0,
                throughput: 100.0,
                memory_usage_mb: 5.0,
            },
            type_signature: "BaseModel<f32>".to_string(),
        };
        
        self.lstm_models.write().await.insert(id.clone(), model);
        self.model_metadata.write().await.insert(id.clone(), metadata);
        self.type_registry.write().await.insert(id, "TypedLSTM".to_string());
        
        Ok(())
    }
    
    /// Store typed GRU model with compile-time type safety
    pub async fn store_gru_model(
        &self,
        id: String,
        model: TypedGRUModel,
    ) -> Result<()> {
        let metadata = ModelMetadata {
            id: id.clone(),
            model_type: "TypedGRU".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            size_bytes: model.weights.len() * 4 + model.biases.len() * 4,
            performance_metrics: PerformanceMetrics {
                accuracy: 0.93,
                latency_ms: 8.0,
                throughput: 120.0,
                memory_usage_mb: 4.0,
            },
            type_signature: "BaseModel<f32>".to_string(),
        };
        
        self.gru_models.write().await.insert(id.clone(), model);
        self.model_metadata.write().await.insert(id.clone(), metadata);
        self.type_registry.write().await.insert(id, "TypedGRU".to_string());
        
        Ok(())
    }
    
    /// Retrieve typed LSTM model without downcasting
    pub async fn get_lstm_model(&self, id: &str) -> Result<Option<TypedLSTMModel>> {
        let models = self.lstm_models.read().await;
        Ok(models.get(id).cloned())
    }
    
    /// Retrieve typed GRU model without downcasting
    pub async fn get_gru_model(&self, id: &str) -> Result<Option<TypedGRUModel>> {
        let models = self.gru_models.read().await;
        Ok(models.get(id).cloned())
    }
    
    /// Get model metadata
    pub async fn get_metadata(&self, id: &str) -> Result<Option<ModelMetadata>> {
        let metadata = self.model_metadata.read().await;
        Ok(metadata.get(id).cloned())
    }
    
    /// List all models with type information
    pub async fn list_models(&self) -> Result<Vec<(String, String)>> {
        let registry = self.type_registry.read().await;
        Ok(registry.iter().map(|(id, model_type)| (id.clone(), model_type.clone())).collect())
    }
    
    /// Validate type safety at runtime
    pub async fn validate_type_safety(&self, id: &str, expected_type: &str) -> Result<bool> {
        let registry = self.type_registry.read().await;
        Ok(registry.get(id).map(|t| t == expected_type).unwrap_or(false))
    }
    
    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<HashMap<String, usize>> {
        let lstm_count = self.lstm_models.read().await.len();
        let gru_count = self.gru_models.read().await.len();
        
        let mut stats = HashMap::new();
        stats.insert("lstm_models".to_string(), lstm_count);
        stats.insert("gru_models".to_string(), gru_count);
        stats.insert("total_models".to_string(), lstm_count + gru_count);
        
        Ok(stats)
    }
}

// ===== TEST UTILITIES =====

fn create_test_time_series_data(symbol: &str, values: Vec<f64>) -> TimeSeriesData {
    let now = Utc::now();
    let close_price = values.last().copied().unwrap_or(100.0);
    
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: now,
        open: close_price * 0.99,
        high: close_price * 1.01,
        low: close_price * 0.98,
        close: close_price,
        volume: vec![1000000.0],
        indicators: HashMap::new(),
        source: Some("test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(close_price),
        values: values.clone(),
        timestamps: (0..values.len())
            .map(|i| now - chrono::Duration::hours((values.len() - i - 1) as i64))
            .collect(),
        metadata: Some({
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map
        }),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map
        },
    }
}

fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        input_size: 60,
        output_size: 1,
        hidden_layers: vec![64, 32],
        learning_rate: 0.001,
        prediction_horizon: Some(1),
        normalization_method: Some("z-score".to_string()),
        enable_adaptive_retry: true,
        enable_model_ensembles: true,
        model_timeout_seconds: 120,
        max_retries: 3,
        error_threshold: 0.15,
        memory_gb: 1.0,
        models: vec!["LSTM".to_string(), "GRU".to_string()],
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
        sequence_length: 60,
        enable_feature_scaling: true,
        enable_technical_indicators: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
        validation_split: 0.2,
        early_stopping: true,
        patience: 10,
    }
}

// ===== COMPREHENSIVE TESTS =====

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_typed_lstm_model_creation_and_prediction() {
        let model = TypedLSTMModel::new(10, 20, 1);
        
        // Test type information
        assert_eq!(model.model_type(), "TypedLSTM");
        assert_eq!(model.input_size, 10);
        assert_eq!(model.hidden_size, 20);
        assert_eq!(model.output_size, 1);
        
        // Test configuration access
        let config = model.get_config();
        assert_eq!(config.learning_rate, 0.01);
        assert_eq!(config.activation, "tanh");
        
        // Test state access
        let state = model.get_state();
        assert_eq!(state.hidden.len(), 20);
        assert!(!state.training);
        
        // Test typed prediction
        let input = vec![1.0; 10];
        model.validate_input(&input).unwrap();
        
        let output = model.predict_typed(&input).unwrap();
        assert_eq!(output.len(), 1);
        assert!(output[0].is_finite());
    }
    
    #[tokio::test]
    async fn test_typed_gru_model_creation_and_prediction() {
        let model = TypedGRUModel::new(15, 25, 2);
        
        // Test type information
        assert_eq!(model.model_type(), "TypedGRU");
        assert_eq!(model.input_size, 15);
        assert_eq!(model.hidden_size, 25);
        assert_eq!(model.output_size, 2);
        
        // Test configuration access
        let config = model.get_config();
        assert_eq!(config.learning_rate, 0.015);
        assert_eq!(config.reset_gate_bias, 0.1);
        
        // Test typed prediction
        let input = vec![0.5; 15];
        model.validate_input(&input).unwrap();
        
        let output = model.predict_typed(&input).unwrap();
        assert_eq!(output.len(), 2);
        assert!(output.iter().all(|&x| x.is_finite()));
    }
    
    #[tokio::test]
    async fn test_typed_model_storage_without_downcasting() {
        let storage = TypedModelStorage::new();
        
        // Store typed LSTM model
        let lstm_model = TypedLSTMModel::new_with_prediction(10, 20, 1, 150.0);
        storage.store_lstm_model("lstm_1".to_string(), lstm_model).await.unwrap();
        
        // Store typed GRU model
        let gru_model = TypedGRUModel::new_with_prediction(10, 20, 1, 175.0);
        storage.store_gru_model("gru_1".to_string(), gru_model).await.unwrap();
        
        // Retrieve models without downcasting
        let retrieved_lstm = storage.get_lstm_model("lstm_1").await.unwrap();
        assert!(retrieved_lstm.is_some());
        let lstm = retrieved_lstm.unwrap();
        assert_eq!(lstm.model_type(), "TypedLSTM");
        
        let retrieved_gru = storage.get_gru_model("gru_1").await.unwrap();
        assert!(retrieved_gru.is_some());
        let gru = retrieved_gru.unwrap();
        assert_eq!(gru.model_type(), "TypedGRU");
        
        // Test metadata retrieval
        let lstm_metadata = storage.get_metadata("lstm_1").await.unwrap();
        assert!(lstm_metadata.is_some());
        let metadata = lstm_metadata.unwrap();
        assert_eq!(metadata.model_type, "TypedLSTM");
        assert_eq!(metadata.type_signature, "BaseModel<f32>");
        
        // Test storage statistics
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.get("lstm_models").unwrap(), &1);
        assert_eq!(stats.get("gru_models").unwrap(), &1);
        assert_eq!(stats.get("total_models").unwrap(), &2);
    }
    
    #[tokio::test]
    async fn test_type_safety_validation() {
        let storage = TypedModelStorage::new();
        
        // Store models
        let lstm_model = TypedLSTMModel::new(10, 20, 1);
        storage.store_lstm_model("test_lstm".to_string(), lstm_model).await.unwrap();
        
        let gru_model = TypedGRUModel::new(10, 20, 1);
        storage.store_gru_model("test_gru".to_string(), gru_model).await.unwrap();
        
        // Test type validation
        assert!(storage.validate_type_safety("test_lstm", "TypedLSTM").await.unwrap());
        assert!(storage.validate_type_safety("test_gru", "TypedGRU").await.unwrap());
        
        // Test invalid type validation
        assert!(!storage.validate_type_safety("test_lstm", "TypedGRU").await.unwrap());
        assert!(!storage.validate_type_safety("test_gru", "TypedLSTM").await.unwrap());
        
        // Test non-existent model
        assert!(!storage.validate_type_safety("non_existent", "TypedLSTM").await.unwrap());
    }
    
    #[tokio::test] 
    async fn test_prediction_flow_end_to_end_with_type_safety() {
        let storage = TypedModelStorage::new();
        
        // Store models with specific predictions
        let lstm_model = TypedLSTMModel::new_with_prediction(5, 10, 1, 100.0);
        let gru_model = TypedGRUModel::new_with_prediction(5, 10, 1, 200.0);
        
        storage.store_lstm_model("predictor_lstm".to_string(), lstm_model).await.unwrap();
        storage.store_gru_model("predictor_gru".to_string(), gru_model).await.unwrap();
        
        // Retrieve models and make predictions
        let lstm = storage.get_lstm_model("predictor_lstm").await.unwrap().unwrap();
        let gru = storage.get_gru_model("predictor_gru").await.unwrap().unwrap();
        
        let input = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        
        // Test LSTM prediction
        lstm.validate_input(&input).unwrap();
        let lstm_output = lstm.predict_typed(&input).unwrap();
        assert_eq!(lstm_output.len(), 1);
        assert!(lstm_output[0].is_finite());
        
        // Test GRU prediction
        gru.validate_input(&input).unwrap();
        let gru_output = gru.predict_typed(&input).unwrap();
        assert_eq!(gru_output.len(), 1);
        assert!(gru_output[0].is_finite());
        
        // Verify different models produce different outputs (due to different weights)
        assert_ne!(lstm_output[0], gru_output[0]);
    }
    
    #[tokio::test]
    async fn test_cluster_model_pool_with_typed_models() {
        let config = ClusterPoolConfig::default();
        let pool = ClusterModelPool::new("technology".to_string(), config).await.unwrap();
        
        // Create typed models
        let lstm_model = TypedLSTMModel::new(10, 20, 1);
        let gru_model = TypedGRUModel::new(10, 20, 1);
        
        // Add typed models to pool (boxed as Any for compatibility)
        let lstm_boxed: Box<dyn Any + Send + Sync> = Box::new(lstm_model);
        let gru_boxed: Box<dyn Any + Send + Sync> = Box::new(gru_model);
        
        pool.add_shared_model("LSTM", lstm_boxed, 5.0).await.unwrap();
        pool.add_shared_model("GRU", gru_boxed, 4.0).await.unwrap();
        
        // Verify models are stored
        assert_eq!(pool.shared_models.len(), 2);
        assert!(pool.get_shared_model("LSTM").is_some());
        assert!(pool.get_shared_model("GRU").is_some());
        
        // Test memory tracking
        let (_, memory_mb) = pool.get_memory_usage().await;
        assert!(memory_mb > 0.0);
        
        // Test pool statistics
        let stats = pool.get_pool_stats().await;
        assert_eq!(stats.get("model_count").unwrap(), &serde_json::json!(2));
        assert_eq!(stats.get("sector_id").unwrap(), &serde_json::json!("technology"));
    }
    
    #[tokio::test]
    async fn test_typed_model_input_validation() {
        let lstm_model = TypedLSTMModel::new(5, 10, 1);
        let gru_model = TypedGRUModel::new(5, 10, 1);
        
        // Test valid input
        let valid_input = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!(lstm_model.validate_input(&valid_input).is_ok());
        assert!(gru_model.validate_input(&valid_input).is_ok());
        
        // Test invalid input size
        let invalid_input = vec![1.0, 2.0, 3.0]; // Too short
        assert!(lstm_model.validate_input(&invalid_input).is_err());
        assert!(gru_model.validate_input(&invalid_input).is_err());
        
        // Test invalid values (LSTM checks for finite values)
        let nan_input = vec![1.0, 2.0, f32::NAN, 4.0, 5.0];
        assert!(lstm_model.validate_input(&nan_input).is_err());
        
        let inf_input = vec![1.0, 2.0, f32::INFINITY, 4.0, 5.0];
        assert!(lstm_model.validate_input(&inf_input).is_err());
        
        // Empty input
        let empty_input = vec![];
        assert!(lstm_model.validate_input(&empty_input).is_err());
        assert!(gru_model.validate_input(&empty_input).is_err());
    }
    
    #[tokio::test]
    async fn test_concurrent_typed_model_operations() {
        let storage = Arc::new(TypedModelStorage::new());
        let mut handles = vec![];
        
        // Spawn concurrent storage operations
        for i in 0..10 {
            let storage_clone = Arc::clone(&storage);
            let handle = tokio::spawn(async move {
                let lstm_model = TypedLSTMModel::new_with_prediction(10, 20, 1, 100.0 + i as f32);
                let gru_model = TypedGRUModel::new_with_prediction(10, 20, 1, 200.0 + i as f32);
                
                let lstm_id = format!("concurrent_lstm_{}", i);
                let gru_id = format!("concurrent_gru_{}", i);
                
                // Store models
                storage_clone.store_lstm_model(lstm_id.clone(), lstm_model).await?;
                storage_clone.store_gru_model(gru_id.clone(), gru_model).await?;
                
                // Retrieve and validate
                let retrieved_lstm = storage_clone.get_lstm_model(&lstm_id).await?;
                let retrieved_gru = storage_clone.get_gru_model(&gru_id).await?;
                
                assert!(retrieved_lstm.is_some());
                assert!(retrieved_gru.is_some());
                
                // Make predictions
                let input = vec![1.0; 10];
                let lstm = retrieved_lstm.unwrap();
                let gru = retrieved_gru.unwrap();
                
                let lstm_output = lstm.predict_typed(&input)?;
                let gru_output = gru.predict_typed(&input)?;
                
                Ok::<_, anyhow::Error>((lstm_output, gru_output))
            });
            
            handles.push(handle);
        }
        
        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            
            let (lstm_output, gru_output) = result.unwrap();
            assert_eq!(lstm_output.len(), 1);
            assert_eq!(gru_output.len(), 1);
            assert!(lstm_output[0].is_finite());
            assert!(gru_output[0].is_finite());
        }
        
        // Verify all models were stored
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.get("lstm_models").unwrap(), &10);
        assert_eq!(stats.get("gru_models").unwrap(), &10);
        assert_eq!(stats.get("total_models").unwrap(), &20);
    }
    
    #[tokio::test]
    async fn test_memory_management_with_typed_pools() {
        let mut config = ClusterPoolConfig::default();
        config.max_memory_mb = 20.0; // Small limit for testing
        config.enable_lazy_loading = true;
        
        let pool = ClusterModelPool::new("test_sector".to_string(), config).await.unwrap();
        
        // Add models until memory limit approaches
        let model1 = TypedLSTMModel::new(50, 100, 10); // Larger model
        let model2 = TypedGRUModel::new(50, 100, 10);
        let model3 = TypedLSTMModel::new(30, 60, 5);
        
        let boxed1: Box<dyn Any + Send + Sync> = Box::new(model1);
        let boxed2: Box<dyn Any + Send + Sync> = Box::new(model2);
        let boxed3: Box<dyn Any + Send + Sync> = Box::new(model3);
        
        // Add models with estimated memory usage
        pool.add_shared_model("LSTM_Large", boxed1, 8.0).await.unwrap();
        pool.add_shared_model("GRU_Large", boxed2, 8.0).await.unwrap();
        
        // This should trigger eviction or lazy loading
        let result3 = pool.add_shared_model("LSTM_Small", boxed3, 6.0).await;
        
        // Either succeeds with eviction or fails with memory limit
        if result3.is_ok() {
            // Lazy loading worked - some model was evicted
            let (_, memory_mb) = pool.get_memory_usage().await;
            assert!(memory_mb <= 20.0);
        } else {
            // Memory limit enforced
            assert_eq!(pool.shared_models.len(), 2);
        }
        
        // Test memory tracking accuracy
        let (memory_bytes, memory_mb) = pool.get_memory_usage().await;
        assert!(memory_bytes > 0);
        assert!(memory_mb > 0.0);
        assert_eq!(memory_bytes as f64, memory_mb * 1024.0 * 1024.0);
    }
    
    #[tokio::test]
    async fn test_type_preservation_in_conversion() {
        let storage = TypedModelStorage::new();
        
        // Store model
        let original_model = TypedLSTMModel::new(10, 20, 1);
        let original_config = original_model.get_config().clone();
        let original_weights = original_model.weights.clone();
        
        storage.store_lstm_model("conversion_test".to_string(), original_model).await.unwrap();
        
        // Retrieve model
        let retrieved_model = storage.get_lstm_model("conversion_test").await.unwrap().unwrap();
        
        // Verify type preservation
        assert_eq!(retrieved_model.model_type(), "TypedLSTM");
        assert_eq!(retrieved_model.get_config().learning_rate, original_config.learning_rate);
        assert_eq!(retrieved_model.get_config().activation, original_config.activation);
        assert_eq!(retrieved_model.weights, original_weights);
        
        // Verify metadata type signature
        let metadata = storage.get_metadata("conversion_test").await.unwrap().unwrap();
        assert_eq!(metadata.type_signature, "BaseModel<f32>");
        assert_eq!(metadata.model_type, "TypedLSTM");
    }
    
    #[tokio::test]
    async fn test_error_handling_in_typed_operations() {
        let storage = TypedModelStorage::new();
        let lstm_model = TypedLSTMModel::new(10, 20, 1);
        
        // Test successful storage
        assert!(storage.store_lstm_model("error_test".to_string(), lstm_model).await.is_ok());
        
        // Test retrieval of non-existent model
        let non_existent = storage.get_lstm_model("non_existent").await.unwrap();
        assert!(non_existent.is_none());
        
        // Test metadata retrieval of non-existent model
        let metadata = storage.get_metadata("non_existent").await.unwrap();
        assert!(metadata.is_none());
        
        // Test prediction with invalid input
        let model = storage.get_lstm_model("error_test").await.unwrap().unwrap();
        let invalid_input = vec![1.0, 2.0]; // Wrong size
        assert!(model.validate_input(&invalid_input).is_err());
        assert!(model.predict_typed(&invalid_input).is_err());
        
        // Test with empty input
        let empty_input = vec![];
        assert!(model.validate_input(&empty_input).is_err());
        assert!(model.predict_typed(&empty_input).is_err());
    }
    
    #[tokio::test]
    async fn test_performance_metrics_with_typed_models() {
        let storage = TypedModelStorage::new();
        
        // Store models with different sizes
        let small_model = TypedLSTMModel::new(5, 10, 1);
        let large_model = TypedGRUModel::new(50, 100, 10);
        
        storage.store_lstm_model("small".to_string(), small_model).await.unwrap();
        storage.store_gru_model("large".to_string(), large_model).await.unwrap();
        
        // Verify metadata includes performance metrics
        let small_metadata = storage.get_metadata("small").await.unwrap().unwrap();
        let large_metadata = storage.get_metadata("large").await.unwrap().unwrap();
        
        // Small model should have smaller size_bytes
        assert!(small_metadata.size_bytes < large_metadata.size_bytes);
        
        // Verify performance metrics structure
        assert!(small_metadata.performance_metrics.accuracy > 0.0);
        assert!(small_metadata.performance_metrics.latency_ms > 0.0);
        assert!(small_metadata.performance_metrics.throughput > 0.0);
        assert!(small_metadata.performance_metrics.memory_usage_mb > 0.0);
        
        // Verify model type consistency
        assert_eq!(small_metadata.type_signature, "BaseModel<f32>");
        assert_eq!(large_metadata.type_signature, "BaseModel<f32>");
    }
    
    #[tokio::test]
    async fn test_model_versioning_with_type_safety() {
        let storage = TypedModelStorage::new();
        
        // Store different versions of the same model type
        let v1_model = TypedLSTMModel::new(10, 20, 1);
        let v2_model = TypedLSTMModel::new_with_prediction(10, 25, 1, 150.0); // Different hidden size
        
        storage.store_lstm_model("model_v1".to_string(), v1_model).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await; // Ensure different timestamps
        storage.store_lstm_model("model_v2".to_string(), v2_model).await.unwrap();
        
        // Retrieve and compare
        let v1 = storage.get_lstm_model("model_v1").await.unwrap().unwrap();
        let v2 = storage.get_lstm_model("model_v2").await.unwrap().unwrap();
        
        // Both should be same type but different configurations
        assert_eq!(v1.model_type(), v2.model_type());
        assert_eq!(v1.hidden_size, 20);
        assert_eq!(v2.hidden_size, 25);
        
        // Verify metadata versioning
        let v1_meta = storage.get_metadata("model_v1").await.unwrap().unwrap();
        let v2_meta = storage.get_metadata("model_v2").await.unwrap().unwrap();
        
        assert!(v1_meta.created_at < v2_meta.created_at);
        assert_eq!(v1_meta.version, v2_meta.version); // Same version string
        assert_ne!(v1_meta.size_bytes, v2_meta.size_bytes); // Different sizes
    }
}