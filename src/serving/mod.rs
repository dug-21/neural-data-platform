// MCP Model Serving Infrastructure
// Expose ruv-FANN models and training capabilities via MCP protocol

pub mod resources;
pub mod tools;
pub mod server;
pub mod versioning;
pub mod ab_testing;

use crate::models::{FannModel, ModelRegistry, TimeSeriesModel, ensemble::EnsembleModel};
use crate::features::{FeaturePipeline, FeatureConfig, StreamingFeaturePipeline};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResource {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub model_type: String,
    pub description: String,
    pub uri: String,
    pub mime_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub metrics: ModelMetrics,
    pub feature_config: FeatureConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub mse: f32,
    pub mae: f32,
    pub directional_accuracy: f32,
    pub sharpe_ratio: f32,
    pub max_drawdown: f32,
    pub training_epochs: u32,
    pub inference_time_ms: f64,
}

impl From<crate::models::ModelMetrics> for ModelMetrics {
    fn from(metrics: crate::models::ModelMetrics) -> Self {
        ModelMetrics {
            mse: metrics.mse,
            mae: metrics.mae,
            directional_accuracy: metrics.directional_accuracy,
            sharpe_ratio: metrics.sharpe_ratio,
            max_drawdown: metrics.max_drawdown,
            training_epochs: metrics.training_epochs,
            inference_time_ms: metrics.inference_time_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionRequest {
    pub model_id: String,
    pub input_data: Vec<f32>,
    pub include_confidence: bool,
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResponse {
    pub model_id: String,
    pub predictions: Vec<f32>,
    pub confidence: Option<Vec<f32>>,
    pub inference_time_ms: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRequest {
    pub model_id: String,
    pub training_data: Vec<Vec<f32>>,
    pub target_data: Vec<Vec<f32>>,
    pub validation_split: f32,
    pub max_epochs: Option<u32>,
    pub early_stopping: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResponse {
    pub model_id: String,
    pub training_metrics: ModelMetrics,
    pub validation_metrics: ModelMetrics,
    pub training_time_ms: f64,
    pub converged: bool,
}

// Main serving infrastructure
pub struct ModelServingServer {
    pub models: Arc<RwLock<ModelRegistry>>,
    pub feature_pipelines: Arc<RwLock<HashMap<String, FeaturePipeline>>>,
    pub streaming_pipelines: Arc<RwLock<HashMap<String, StreamingFeaturePipeline>>>,
    pub model_resources: Arc<RwLock<HashMap<String, ModelResource>>>,
    pub ab_tests: Arc<RwLock<HashMap<String, ab_testing::ABTest>>>,
    pub version_manager: Arc<versioning::ModelVersionManager>,
}

impl ModelServingServer {
    pub fn new() -> Self {
        ModelServingServer {
            models: Arc::new(RwLock::new(ModelRegistry::new())),
            feature_pipelines: Arc::new(RwLock::new(HashMap::new())),
            streaming_pipelines: Arc::new(RwLock::new(HashMap::new())),
            model_resources: Arc::new(RwLock::new(HashMap::new())),
            ab_tests: Arc::new(RwLock::new(HashMap::new())),
            version_manager: Arc::new(versioning::ModelVersionManager::new()),
        }
    }
    
    // Model registration and management
    pub async fn register_model(
        &self,
        id: String,
        model: Box<dyn FannModel + Send>,
        feature_config: FeatureConfig,
        description: String,
    ) -> Result<ModelResource, Box<dyn std::error::Error + Send + Sync>> {
        let mut models = self.models.write().await;
        let mut resources = self.model_resources.write().await;
        let mut pipelines = self.feature_pipelines.write().await;
        
        // Register model
        models.register_model(id.clone(), model);
        
        // Create feature pipeline
        let pipeline = FeaturePipeline::new(feature_config.clone());
        pipelines.insert(id.clone(), pipeline);
        
        // Create resource metadata
        let resource = ModelResource {
            id: id.clone(),
            name: id.clone(),
            version: 1,
            model_type: "TimeSeriesModel".to_string(),
            description,
            uri: format!("fann://models/{}", id),
            mime_type: "application/fann".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp(),
            metrics: ModelMetrics {
                mse: f32::MAX,
                mae: f32::MAX,
                directional_accuracy: 0.0,
                sharpe_ratio: 0.0,
                max_drawdown: 0.0,
                training_epochs: 0,
                inference_time_ms: 0.0,
            },
            feature_config,
        };
        
        resources.insert(id.clone(), resource.clone());
        
        // Register with version manager
        self.version_manager.register_model(&id, 1).await?;
        
        Ok(resource)
    }
    
    pub async fn predict(
        &self,
        request: PredictionRequest,
    ) -> Result<PredictionResponse, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();
        
        let models = self.models.read().await;
        let pipelines = self.feature_pipelines.read().await;
        
        // Get model and pipeline
        let mut model_registry = models;
        if let Err(_) = model_registry.set_active_model(&request.model_id) {
            return Err(format!("Model {} not found", request.model_id).into());
        }
        
        let model = model_registry.get_active_model()
            .ok_or("No active model")?;
        
        let pipeline = pipelines.get(&request.model_id)
            .ok_or("Feature pipeline not found")?;
        
        // Extract features
        let features = pipeline.extract_features(&request.input_data);
        
        // Make prediction
        let (predictions, confidence) = if request.include_confidence {
            model.predict_with_confidence(&features.features)?
        } else {
            let pred = model.predict(&features.features)?;
            (pred, None)
        };
        
        let inference_time = start_time.elapsed().as_millis() as f64;
        
        Ok(PredictionResponse {
            model_id: request.model_id,
            predictions,
            confidence,
            inference_time_ms: inference_time,
            timestamp: chrono::Utc::now().timestamp(),
        })
    }
    
    pub async fn train_model(
        &self,
        request: TrainingRequest,
    ) -> Result<TrainingResponse, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();
        
        let models = self.models.read().await;
        let pipelines = self.feature_pipelines.read().await;
        
        // Get model and pipeline
        let mut model_registry = models;
        model_registry.set_active_model(&request.model_id)?;
        
        let model = model_registry.get_active_model()
            .ok_or("No active model")?;
        
        let pipeline = pipelines.get(&request.model_id)
            .ok_or("Feature pipeline not found")?;
        
        // Prepare training data
        let mut training_inputs = Vec::new();
        let mut training_outputs = Vec::new();
        
        for (input, target) in request.training_data.iter().zip(&request.target_data) {
            let features = pipeline.extract_features(input);
            training_inputs.push(features.features);
            training_outputs.push(target.clone());
        }
        
        // Split data for validation
        let split_idx = (training_inputs.len() as f32 * (1.0 - request.validation_split)) as usize;
        let train_inputs = &training_inputs[..split_idx];
        let train_outputs = &training_outputs[..split_idx];
        let val_inputs = &training_inputs[split_idx..];
        let val_outputs = &training_outputs[split_idx..];
        
        // Create training data
        let train_data = fann::TrainData::new(train_inputs, train_outputs)?;
        
        // Train model
        let training_metrics = model.train(&train_data)?;
        
        // Validate
        let mut validation_errors = Vec::new();
        for (input, expected) in val_inputs.iter().zip(val_outputs) {
            let prediction = model.predict(input)?;
            let error = prediction.iter().zip(expected)
                .map(|(p, e)| (p - e).powi(2))
                .sum::<f32>() / prediction.len() as f32;
            validation_errors.push(error);
        }
        
        let val_mse = validation_errors.iter().sum::<f32>() / validation_errors.len() as f32;
        let validation_metrics = ModelMetrics {
            mse: val_mse,
            mae: val_mse.sqrt(),
            directional_accuracy: self.calculate_directional_accuracy(&predictions, &validation_errors),
            sharpe_ratio: 0.0,
            max_drawdown: 0.0,
            training_epochs: training_metrics.training_epochs,
            inference_time_ms: 0.0,
        };
        
        let training_time = start_time.elapsed().as_millis() as f64;
        
        Ok(TrainingResponse {
            model_id: request.model_id,
            training_metrics: training_metrics.into(),
            validation_metrics,
            training_time_ms: training_time,
            converged: training_metrics.mse < 0.01,
        })
    }
    
    pub async fn list_models(&self) -> Vec<ModelResource> {
        let resources = self.model_resources.read().await;
        resources.values().cloned().collect()
    }
    
    pub async fn get_model(&self, id: &str) -> Option<ModelResource> {
        let resources = self.model_resources.read().await;
        resources.get(id).cloned()
    }
    
    pub async fn create_streaming_pipeline(
        &self,
        model_id: String,
        feature_config: FeatureConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut streaming_pipelines = self.streaming_pipelines.write().await;
        
        let pipeline = StreamingFeaturePipeline::new(feature_config);
        streaming_pipelines.insert(model_id, pipeline);
        
        Ok(())
    }
    
    pub async fn stream_predict(
        &self,
        model_id: &str,
        value: f32,
    ) -> Result<Option<PredictionResponse>, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();
        
        let mut streaming_pipelines = self.streaming_pipelines.write().await;
        let models = self.models.read().await;
        
        // Get streaming pipeline
        let pipeline = streaming_pipelines.get_mut(model_id)
            .ok_or("Streaming pipeline not found")?;
        
        // Update with new value
        if let Some(features) = pipeline.update(value) {
            // Get model
            let mut model_registry = models;
            model_registry.set_active_model(model_id)?;
            
            let model = model_registry.get_active_model()
                .ok_or("No active model")?;
            
            // Make prediction
            let predictions = model.predict(&features.features)?;
            let inference_time = start_time.elapsed().as_millis() as f64;
            
            Ok(Some(PredictionResponse {
                model_id: model_id.to_string(),
                predictions,
                confidence: None,
                inference_time_ms: inference_time,
                timestamp: chrono::Utc::now().timestamp(),
            }))
        } else {
            Ok(None)
        }
    }

    /// Calculate directional accuracy - percentage of correct directional predictions
    fn calculate_directional_accuracy(&self, predictions: &[f32], actual_deltas: &[f32]) -> f32 {
        if predictions.len() != actual_deltas.len() || predictions.is_empty() {
            return 0.0;
        }

        let correct_directions = predictions.iter()
            .zip(actual_deltas.iter())
            .filter(|(pred, actual)| {
                // Both positive or both negative (same direction)
                (**pred > 0.0 && **actual > 0.0) || (**pred < 0.0 && **actual < 0.0)
            })
            .count();

        correct_directions as f32 / predictions.len() as f32 * 100.0
    }
}

// MCP resource implementations
#[derive(Debug, Clone, Serialize)]
pub struct McpModelResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

impl From<ModelResource> for McpModelResource {
    fn from(resource: ModelResource) -> Self {
        McpModelResource {
            uri: resource.uri,
            name: resource.name,
            description: resource.description,
            mime_type: resource.mime_type,
        }
    }
}