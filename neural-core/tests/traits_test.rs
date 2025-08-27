//! Integration tests for storage and predictor traits
//! Test implementations and trait behavior

use neural_core::traits::{Storage, StorageBackend, Predictor, TrainingConfig, ModelMetrics};
use neural_core::traits::storage::{StorageHealth, StorageStats};
use neural_core::traits::predictor::ModelInfo;
use neural_core::types::MarketData;
use neural_core::errors::{CoreError, Result};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Mock storage implementation for testing
struct MockStorage {
    data: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Storage for MockStorage {
    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }
    
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self.data.lock().unwrap();
        Ok(data.get(key).cloned())
    }
    
    async fn delete(&self, key: &str) -> Result<bool> {
        let mut data = self.data.lock().unwrap();
        Ok(data.remove(key).is_some())
    }
    
    async fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.lock().unwrap();
        Ok(data.contains_key(key))
    }
    
    async fn set_with_ttl(&self, key: &str, value: &[u8], _ttl_seconds: u64) -> Result<()> {
        // Simple implementation - just set without TTL for testing
        self.set(key, value).await
    }
    
    async fn get_many(&self, keys: &[&str]) -> Result<HashMap<String, Vec<u8>>> {
        let data = self.data.lock().unwrap();
        let mut result = HashMap::new();
        
        for key in keys {
            if let Some(value) = data.get(*key) {
                result.insert(key.to_string(), value.clone());
            }
        }
        
        Ok(result)
    }
    
    async fn set_many(&self, items: &HashMap<String, Vec<u8>>) -> Result<()> {
        let mut data = self.data.lock().unwrap();
        for (key, value) in items {
            data.insert(key.clone(), value.clone());
        }
        Ok(())
    }
    
    async fn list_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let data = self.data.lock().unwrap();
        let keys: Vec<String> = data.keys()
            .filter(|k| k.contains(pattern))
            .cloned()
            .collect();
        Ok(keys)
    }
    
    fn backend_type(&self) -> StorageBackend {
        StorageBackend::Memory
    }
    
    async fn health_check(&self) -> Result<StorageHealth> {
        Ok(StorageHealth {
            is_healthy: true,
            response_time_ms: 1,
            error_message: None,
            last_check: Utc::now(),
        })
    }
    
    async fn stats(&self) -> Result<StorageStats> {
        let data = self.data.lock().unwrap();
        Ok(StorageStats {
            total_keys: data.len() as u64,
            memory_usage_bytes: data.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>() as u64,
            operations_per_second: 1000.0,
            hit_ratio: 0.95,
            error_rate: 0.01,
            uptime_seconds: 3600,
        })
    }
}

// Mock predictor implementation for testing
struct MockPredictor {
    is_trained: bool,
    parameters: HashMap<String, f64>,
}

impl MockPredictor {
    fn new() -> Self {
        let mut params = HashMap::new();
        params.insert("learning_rate".to_string(), 0.001);
        params.insert("epochs".to_string(), 100.0);
        
        Self {
            is_trained: false,
            parameters: params,
        }
    }
}

#[async_trait]
impl Predictor for MockPredictor {
    async fn predict(&self, market_data: &[MarketData]) -> Result<neural_core::types::PredictionResult> {
        if !self.is_trained {
            return Err(CoreError::ModelError("Model not trained".to_string()));
        }
        
        self.validate_input(market_data)?;
        
        let predictions = market_data.iter().map(|data| {
            neural_core::types::Prediction::new(data.price() * 1.01, 0.7) // Simple prediction
        }).collect();
        
        Ok(neural_core::types::PredictionResult::new("mock_model".to_string(), predictions))
    }
    
    async fn train(&mut self, training_data: &[MarketData], _config: &TrainingConfig) -> Result<ModelMetrics> {
        self.validate_input(training_data)?;
        
        self.is_trained = true;
        
        Ok(ModelMetrics {
            accuracy: Some(0.85),
            mse: Some(0.05),
            mae: Some(0.03),
            r_squared: Some(0.75),
            validation_loss: Some(0.04),
            training_loss: Some(0.035),
            ..Default::default()
        })
    }
    
    async fn evaluate(&self, test_data: &[MarketData]) -> Result<ModelMetrics> {
        if !self.is_trained {
            return Err(CoreError::ModelError("Model not trained".to_string()));
        }
        
        self.validate_input(test_data)?;
        
        Ok(ModelMetrics {
            accuracy: Some(0.82),
            precision: Some(0.78),
            recall: Some(0.85),
            f1_score: Some(0.81),
            ..Default::default()
        })
    }
    
    async fn update(&mut self, new_data: &[MarketData]) -> Result<()> {
        self.validate_input(new_data)?;
        // Mock update - in real implementation would retrain incrementally
        Ok(())
    }
    
    async fn save_model(&self, _path: &str) -> Result<()> {
        if !self.is_trained {
            return Err(CoreError::ModelError("Cannot save untrained model".to_string()));
        }
        Ok(())
    }
    
    async fn load_model(&mut self, _path: &str) -> Result<()> {
        self.is_trained = true;
        Ok(())
    }
    
    fn get_model_info(&self) -> ModelInfo {
        ModelInfo::new("MockPredictor".to_string(), "Mock".to_string())
    }
    
    fn set_parameters(&mut self, params: HashMap<String, f64>) -> Result<()> {
        self.parameters.extend(params);
        Ok(())
    }
    
    fn get_parameters(&self) -> HashMap<String, f64> {
        self.parameters.clone()
    }
    
    fn is_ready(&self) -> bool {
        self.is_trained
    }
    
    fn required_features(&self) -> Vec<String> {
        vec!["price".to_string(), "volume".to_string(), "timestamp".to_string()]
    }
}

#[tokio::test]
async fn test_storage_basic_operations() {
    let storage = MockStorage::new();
    
    // Test set and get
    let key = "test_key";
    let value = b"test_value";
    
    storage.set(key, value).await.unwrap();
    let retrieved = storage.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.to_vec()));
    
    // Test exists
    assert!(storage.exists(key).await.unwrap());
    assert!(!storage.exists("non_existent").await.unwrap());
    
    // Test delete
    assert!(storage.delete(key).await.unwrap());
    assert!(!storage.exists(key).await.unwrap());
}

#[tokio::test]
async fn test_storage_batch_operations() {
    let storage = MockStorage::new();
    
    // Test set_many
    let mut items = HashMap::new();
    items.insert("key1".to_string(), b"value1".to_vec());
    items.insert("key2".to_string(), b"value2".to_vec());
    items.insert("key3".to_string(), b"value3".to_vec());
    
    storage.set_many(&items).await.unwrap();
    
    // Test get_many
    let keys = ["key1", "key2", "key3"];
    let result = storage.get_many(&keys).await.unwrap();
    
    assert_eq!(result.len(), 3);
    assert_eq!(result.get("key1"), Some(&b"value1".to_vec()));
    
    // Test list_keys
    let keys = storage.list_keys("key").await.unwrap();
    assert_eq!(keys.len(), 3);
}

#[tokio::test]
async fn test_storage_health_and_stats() {
    let storage = MockStorage::new();
    
    let health = storage.health_check().await.unwrap();
    assert!(health.is_healthy);
    assert_eq!(health.response_time_ms, 1);
    
    let stats = storage.stats().await.unwrap();
    assert_eq!(stats.hit_ratio, 0.95);
    assert_eq!(stats.error_rate, 0.01);
}

#[tokio::test]
async fn test_predictor_training_workflow() {
    let mut predictor = MockPredictor::new();
    
    // Initially not ready
    assert!(!predictor.is_ready());
    
    // Create training data
    let training_data = vec![
        MarketData::new("AAPL".to_string(), 150.0, 1000000, Utc::now()),
        MarketData::new("AAPL".to_string(), 151.0, 1100000, Utc::now()),
        MarketData::new("AAPL".to_string(), 149.0, 900000, Utc::now()),
    ];
    
    let config = TrainingConfig::default();
    let metrics = predictor.train(&training_data, &config).await.unwrap();
    
    // Should be trained now
    assert!(predictor.is_ready());
    assert!(metrics.accuracy.unwrap() > 0.8);
    assert!(metrics.mse.is_some());
}

#[tokio::test]
async fn test_predictor_prediction_workflow() {
    let mut predictor = MockPredictor::new();
    
    let market_data = vec![
        MarketData::new("AAPL".to_string(), 150.0, 1000000, Utc::now()),
    ];
    
    // Should fail when not trained
    let result = predictor.predict(&market_data).await;
    assert!(result.is_err());
    
    // Train first
    let config = TrainingConfig::default();
    predictor.train(&market_data, &config).await.unwrap();
    
    // Now prediction should work
    let prediction_result = predictor.predict(&market_data).await.unwrap();
    assert_eq!(prediction_result.model_name, "mock_model");
    assert!(!prediction_result.predictions.is_empty());
}

#[tokio::test]
async fn test_predictor_parameters() {
    let mut predictor = MockPredictor::new();
    
    let initial_params = predictor.get_parameters();
    assert!(initial_params.contains_key("learning_rate"));
    
    let mut new_params = HashMap::new();
    new_params.insert("new_param".to_string(), 42.0);
    
    predictor.set_parameters(new_params).unwrap();
    
    let updated_params = predictor.get_parameters();
    assert_eq!(updated_params.get("new_param"), Some(&42.0));
}

#[tokio::test]
async fn test_predictor_validation() {
    let predictor = MockPredictor::new();
    
    // Test empty data validation
    let empty_data = vec![];
    let result = predictor.validate_input(&empty_data);
    assert!(result.is_err());
    
    // Test valid data
    let valid_data = vec![
        MarketData::new("AAPL".to_string(), 150.0, 1000000, Utc::now()),
    ];
    let result = predictor.validate_input(&valid_data);
    assert!(result.is_ok());
}