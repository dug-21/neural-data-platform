//! Vendor Bridge Module - Optimized Async/Sync Bridge for Vendor Models
//! 
//! This module provides an efficient bridge between our async system and synchronous
//! vendor neural network models, with optimizations for performance and throughput.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Semaphore};
use tokio::task::JoinHandle;
use once_cell::sync::Lazy;
use thiserror::Error;

/// Global thread pool for CPU-intensive synchronous operations
static SYNC_THREAD_POOL: Lazy<Arc<rayon::ThreadPool>> = Lazy::new(|| {
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(2); // At least 2 threads
    
    Arc::new(rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .thread_name(|idx| format!("neural-sync-{}", idx))
        .build()
        .expect("Failed to create thread pool"))
});

/// Semaphore to limit concurrent sync operations
static SYNC_OPERATION_LIMITER: Lazy<Arc<Semaphore>> = Lazy::new(|| {
    // Limit to number of CPU cores to prevent oversubscription
    let permits = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    Arc::new(Semaphore::new(permits))
});

/// Vendor-specific time series data format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorTimeSeriesData {
    pub symbol: String,
    pub timestamps: Vec<DateTime<Utc>>,
    pub values: Vec<f32>,
    pub exogenous_historical: Option<Vec<Vec<f32>>>,
    pub exogenous_future: Option<Vec<Vec<f32>>>,
    pub static_features: Option<Vec<f32>>,
    pub time_features: Option<Vec<Vec<f32>>>,
}

impl VendorTimeSeriesData {
    pub fn new(symbol: String, timestamps: Vec<DateTime<Utc>>, values: Vec<f32>) -> Self {
        Self {
            symbol,
            timestamps,
            values,
            exogenous_historical: None,
            exogenous_future: None,
            static_features: None,
            time_features: None,
        }
    }
    
    pub fn with_exogenous_historical(mut self, exog: Vec<Vec<f32>>) -> Self {
        self.exogenous_historical = Some(exog);
        self
    }
    
    pub fn len(&self) -> usize {
        self.values.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Vendor model prediction result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub forecasts: Vec<f32>,
    pub timestamps: Vec<DateTime<Utc>>,
    pub series_id: String,
    pub metadata: HashMap<String, String>,
    pub confidence_intervals: Option<Vec<(f32, f32)>>,
    pub quantiles: Option<HashMap<String, Vec<f32>>>,
}

/// Training configuration for vendor models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    pub max_epochs: usize,
    pub learning_rate: f32,
    pub batch_size: usize,
    pub validation_size: f32,
    pub early_stopping_patience: usize,
    pub save_best_model: bool,
    pub verbose: bool,
    pub use_gpu: bool,
    pub gradient_clipping: Option<f32>,
    pub weight_decay: Option<f32>,
    pub scheduler_config: Option<SchedulerConfig>,
}

/// Learning rate scheduler configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub scheduler_type: String, // "cosine", "exponential", "step"
    pub step_size: Option<usize>,
    pub gamma: Option<f32>,
    pub min_lr: Option<f32>,
}

/// Vendor model errors
#[derive(Error, Debug, Clone)]
pub enum ModelError {
    #[error("Model not trained")]
    NotTrainedError,
    
    #[error("Invalid input data: {0}")]
    InvalidInputError(String),
    
    #[error("Training failed: {0}")]
    TrainingError(String),
    
    #[error("Prediction failed: {0}")]
    PredictionError(String),
    
    #[error("Model initialization failed: {0}")]
    InitializationError(String),
    
    #[error("Network creation failed")]
    NetworkCreationError,
    
    #[error("Network not initialized")]
    NetworkNotInitialized,
    
    #[error("GPU not available")]
    GpuNotAvailableError,
    
    #[error("Out of memory: {0}")]
    OutOfMemoryError(String),
    
    #[error("Timeout: operation took longer than {0:?}")]
    TimeoutError(Duration),
}

/// Optimized async/sync bridge for vendor models
pub struct AsyncSyncBridge {
    /// Maximum batch size for batch predictions
    max_batch_size: usize,
    /// Timeout for sync operations
    operation_timeout: Duration,
    /// Performance metrics
    metrics: Arc<RwLock<BridgeMetrics>>,
}

#[derive(Debug, Default, Clone)]
struct BridgeMetrics {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    total_duration: Duration,
    batch_operations: u64,
    average_batch_size: f32,
}

impl AsyncSyncBridge {
    pub fn new(max_batch_size: usize, operation_timeout: Duration) -> Self {
        Self {
            max_batch_size,
            operation_timeout,
            metrics: Arc::new(RwLock::new(BridgeMetrics::default())),
        }
    }
    
    /// Execute a synchronous operation in the thread pool without nested runtime
    pub async fn execute_sync<F, T>(&self, operation: F) -> Result<T, ModelError>
    where
        F: FnOnce() -> Result<T, ModelError> + Send + 'static,
        T: Send + 'static,
    {
        // Acquire permit to limit concurrent operations
        let _permit = SYNC_OPERATION_LIMITER.acquire().await
            .map_err(|_| ModelError::InitializationError("Failed to acquire sync permit".to_string()))?;
        
        // Use a oneshot channel for communication
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        // Spawn the operation in the thread pool
        SYNC_THREAD_POOL.spawn(move || {
            let result = operation();
            // Ignore send errors if receiver is dropped
            let _ = tx.send(result);
        });
        
        // Wait for result with timeout
        match tokio::time::timeout(self.operation_timeout, rx).await {
            Ok(Ok(result)) => {
                self.update_metrics(true).await;
                result
            }
            Ok(Err(_)) => {
                self.update_metrics(false).await;
                Err(ModelError::InitializationError("Operation cancelled".to_string()))
            }
            Err(_) => {
                self.update_metrics(false).await;
                Err(ModelError::TimeoutError(self.operation_timeout))
            }
        }
    }
    
    /// Execute a batch of synchronous operations efficiently
    pub async fn execute_batch<F, T, I>(&self, items: I, operation: F) -> Vec<Result<T, ModelError>>
    where
        F: Fn(&I::Item) -> Result<T, ModelError> + Send + Sync + 'static,
        T: Send + 'static,
        I: IntoIterator,
        I::Item: Send + Sync + 'static,
        I::IntoIter: Send,
        <I as IntoIterator>::Item: Clone,
    {
        let items: Vec<_> = items.into_iter().collect();
        let total_items = items.len();
        
        if total_items == 0 {
            return vec![];
        }
        
        // Update batch metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.batch_operations += 1;
            let current_avg = metrics.average_batch_size;
            let batch_count = metrics.batch_operations as f32;
            metrics.average_batch_size = 
                (current_avg * (batch_count - 1.0) + total_items as f32) / batch_count;
        }
        
        // Process in chunks to respect max_batch_size
        let chunks: Vec<_> = items
            .chunks(self.max_batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();
        
        let operation = Arc::new(operation);
        let mut all_results = Vec::with_capacity(total_items);
        
        for chunk in chunks {
            let chunk_size = chunk.len();
            let operation = Arc::clone(&operation);
            
            // Process chunk in parallel within thread pool
            let chunk_results = self.execute_sync(move || {
                let results: Vec<_> = chunk
                    .into_iter()
                    .map(|item| operation(&item))
                    .collect();
                Ok::<Vec<_>, ModelError>(results)
            }).await;
            
            match chunk_results {
                Ok(results) => all_results.extend(results),
                Err(e) => {
                    // If batch processing fails, return errors for remaining items
                    for _ in 0..chunk_size {
                        all_results.push(Err(e.clone()));
                    }
                }
            }
        }
        
        all_results
    }
    
    /// Update performance metrics
    async fn update_metrics(&self, success: bool) {
        let mut metrics = self.metrics.write().await;
        metrics.total_operations += 1;
        if success {
            metrics.successful_operations += 1;
        } else {
            metrics.failed_operations += 1;
        }
    }
    
    /// Get current performance metrics
    pub async fn get_metrics(&self) -> BridgeMetrics {
        let guard = self.metrics.read().await;
        guard.clone()
    }
}

/// Trait for synchronous vendor models
pub trait SyncVendorModel: Send + Sync {
    /// Train the model (synchronous)
    fn train(&mut self, data: &VendorTimeSeriesData, config: &TrainingConfig) -> Result<(), ModelError>;
    
    /// Make predictions (synchronous)
    fn predict(&self, data: &VendorTimeSeriesData) -> Result<PredictionResult, ModelError>;
    
    /// Get model name
    fn name(&self) -> &str;
    
    /// Check if model is trained
    fn is_trained(&self) -> bool;
    
    /// Save model to path
    fn save(&self, path: &str) -> Result<(), ModelError>;
    
    /// Load model from path
    fn load(&mut self, path: &str) -> Result<(), ModelError>;
}

/// Async wrapper for synchronous vendor models
pub struct AsyncModelWrapper<M: SyncVendorModel> {
    model: Arc<RwLock<M>>,
    bridge: AsyncSyncBridge,
}

impl<M: SyncVendorModel + 'static> AsyncModelWrapper<M> {
    pub fn new(model: M, max_batch_size: usize, timeout: Duration) -> Self {
        Self {
            model: Arc::new(RwLock::new(model)),
            bridge: AsyncSyncBridge::new(max_batch_size, timeout),
        }
    }
    
    /// Train the model asynchronously
    pub async fn train(&self, data: VendorTimeSeriesData, config: TrainingConfig) -> Result<(), ModelError> {
        let model = Arc::clone(&self.model);
        
        self.bridge.execute_sync(move || {
            let mut model = model.blocking_write();
            model.train(&data, &config)
        }).await
    }
    
    /// Make predictions asynchronously
    pub async fn predict(&self, data: VendorTimeSeriesData) -> Result<PredictionResult, ModelError> {
        let model = Arc::clone(&self.model);
        
        self.bridge.execute_sync(move || {
            let model = model.blocking_read();
            model.predict(&data)
        }).await
    }
    
    /// Batch predictions for efficiency
    pub async fn predict_batch(&self, data_batch: Vec<VendorTimeSeriesData>) -> Vec<Result<PredictionResult, ModelError>> {
        let model = Arc::clone(&self.model);
        
        self.bridge.execute_batch(data_batch, move |data| {
            let model = model.blocking_read();
            model.predict(data)
        }).await
    }
    
    /// Get model name
    pub async fn name(&self) -> String {
        let model = self.model.read().await;
        model.name().to_string()
    }
    
    /// Check if model is trained
    pub async fn is_trained(&self) -> bool {
        let model = self.model.read().await;
        model.is_trained()
    }
    
    /// Get bridge performance metrics
    pub async fn get_performance_metrics(&self) -> BridgeMetrics {
        self.bridge.get_metrics().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock synchronous vendor model for testing
    struct MockVendorModel {
        trained: bool,
        name: String,
    }
    
    impl SyncVendorModel for MockVendorModel {
        fn train(&mut self, _data: &VendorTimeSeriesData, _config: &TrainingConfig) -> Result<(), ModelError> {
            std::thread::sleep(Duration::from_millis(100)); // Simulate work
            self.trained = true;
            Ok(())
        }
        
        fn predict(&self, data: &VendorTimeSeriesData) -> Result<PredictionResult, ModelError> {
            if !self.trained {
                return Err(ModelError::NotTrainedError);
            }
            
            std::thread::sleep(Duration::from_millis(50)); // Simulate work
            
            Ok(PredictionResult {
                forecasts: vec![0.0; 10],
                timestamps: vec![Utc::now(); 10],
                series_id: data.symbol.clone(),
                metadata: HashMap::new(),
                confidence_intervals: None,
                quantiles: None,
            })
        }
        
        fn name(&self) -> &str {
            &self.name
        }
        
        fn is_trained(&self) -> bool {
            self.trained
        }
        
        fn save(&self, _path: &str) -> Result<(), ModelError> {
            Ok(())
        }
        
        fn load(&mut self, _path: &str) -> Result<(), ModelError> {
            self.trained = true;
            Ok(())
        }
    }
    
    #[tokio::test]
    async fn test_async_sync_bridge() {
        let model = MockVendorModel {
            trained: false,
            name: "test_model".to_string(),
        };
        
        let wrapper = AsyncModelWrapper::new(model, 10, Duration::from_secs(5));
        
        // Test training
        let data = VendorTimeSeriesData::new(
            "TEST".to_string(),
            vec![Utc::now(); 100],
            vec![1.0; 100],
        );
        
        let config = TrainingConfig {
            max_epochs: 10,
            learning_rate: 0.001,
            batch_size: 32,
            validation_size: 0.2,
            early_stopping_patience: 5,
            save_best_model: false,
            verbose: false,
            use_gpu: false,
            gradient_clipping: None,
            weight_decay: None,
            scheduler_config: None,
        };
        
        assert!(wrapper.train(data.clone(), config).await.is_ok());
        assert!(wrapper.is_trained().await);
        
        // Test prediction
        let result = wrapper.predict(data).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_batch_predictions() {
        let model = MockVendorModel {
            trained: true,
            name: "test_model".to_string(),
        };
        
        let wrapper = AsyncModelWrapper::new(model, 5, Duration::from_secs(5));
        
        // Create batch of data
        let batch: Vec<_> = (0..10)
            .map(|i| VendorTimeSeriesData::new(
                format!("TEST_{}", i),
                vec![Utc::now(); 50],
                vec![1.0; 50],
            ))
            .collect();
        
        let results = wrapper.predict_batch(batch).await;
        assert_eq!(results.len(), 10);
        assert!(results.iter().all(|r| r.is_ok()));
        
        // Check metrics
        let metrics = wrapper.get_performance_metrics().await;
        assert_eq!(metrics.batch_operations, 1);
        assert_eq!(metrics.average_batch_size, 10.0);
    }
}