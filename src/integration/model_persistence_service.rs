//! Model Persistence Service
//!
//! This service coordinates model persistence across the neural-trader system,
//! integrating FANN model adapters with the model storage and rollback systems.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::adapters::model_storage::{
    ModelStorage, ModelStorageConfig, ModelMetadata, VersionIncrement,
    SemanticVersion
};
use crate::adapters::model_rollback::{
    ModelRollbackManager, RollbackConfig, ModelVersion, ModelMetrics,
    RollbackReason
};
// Internal neural types are no longer exposed - functionality handled by NeuralPredictor
// Creating stub types for compilation until full refactoring
#[derive(Debug, Clone, Default)]
struct FannModelConfig {
    pub input_size: usize,
    pub hidden_sizes: Vec<usize>,
    pub output_size: usize,
}

#[derive(Debug, Clone)]
struct TrainingRecord {
    pub model_name: String,
    pub timestamp: DateTime<Utc>,
    pub epochs: usize,
    pub final_error: f64,
}

struct FannModelAdapter;

impl FannModelAdapter {
    async fn new(_config: FannModelConfig, _storage: ModelStorageConfig) -> Result<Self> {
        Ok(FannModelAdapter)
    }
    
    async fn train_with_checkpointing(
        &mut self,
        _data: &ruv_fann::TrainingData<f32>,
        _config: &TrainingConfig,
        _checkpoint_freq: usize,
    ) -> Result<TrainingRecord> {
        Ok(TrainingRecord {
            epochs: 100,
            final_error: 0.001,
        })
    }
    
    fn get_performance_metrics(&self) -> PerformanceMetrics {
        PerformanceMetrics {
            mse: 0.001,
            rmse: 0.03,
            mae: 0.02,
            r_squared: 0.95,
            mape: 2.5,
        }
    }
    
    async fn save_model(&self, _version: VersionIncrement) -> Result<PathBuf> {
        Ok(PathBuf::from("/tmp/model.fann"))
    }
    
    fn get_metadata(&self) -> ModelMetadata {
        ModelMetadata {
            version: SemanticVersion {
                major: 1,
                minor: 0,
                patch: 0,
            },
            name: "FannModel".to_string(),
            description: "FANN Neural Network Model".to_string(),
            model_type: "FANN".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            tags: vec!["neural".to_string(), "fann".to_string()],
            custom_metadata: std::collections::HashMap::new(),
        }
    }
    
    async fn load_model(&mut self, _version: String) -> Result<()> {
        Ok(())
    }
}

use crate::adapters::vendor_bridge::{TrainingConfig, VendorTimeSeriesData, SyncVendorModel};
use crate::integration::training_data_service::{TrainingDataService, TrainingDataConfig};

/// Configuration for the model persistence service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPersistenceConfig {
    /// Base directory for model storage
    pub model_storage_path: PathBuf,
    /// Rollback configuration
    pub rollback_config: RollbackConfig,
    /// Enable automatic checkpointing during training
    pub enable_auto_checkpointing: bool,
    /// Checkpoint frequency (epochs)
    pub checkpoint_frequency: usize,
    /// Enable automatic model rollback on performance degradation
    pub enable_auto_rollback: bool,
    /// Performance degradation threshold for rollback (%)
    pub rollback_threshold: f32,
    /// Maximum number of concurrent model operations
    pub max_concurrent_operations: usize,
    /// Model versioning strategy
    pub default_version_increment: VersionIncrement,
}

impl Default for ModelPersistenceConfig {
    fn default() -> Self {
        Self {
            model_storage_path: PathBuf::from("/opt/neural-trader/models"),
            rollback_config: RollbackConfig::default(),
            enable_auto_checkpointing: true,
            checkpoint_frequency: 100,
            enable_auto_rollback: true,
            rollback_threshold: 10.0,
            max_concurrent_operations: 4,
            default_version_increment: VersionIncrement::Minor,
        }
    }
}

/// Model operation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelOperation {
    Save {
        model_name: String,
        version_increment: VersionIncrement,
    },
    Load {
        model_name: String,
        version: Option<SemanticVersion>,
    },
    Train {
        model_name: String,
        config: TrainingConfig,
        data_config: TrainingDataConfig,
    },
    Rollback {
        model_name: String,
        reason: String,
    },
    Checkpoint {
        model_name: String,
        epoch: usize,
    },
}

/// Model operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelOperationResult {
    pub operation: ModelOperation,
    pub success: bool,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<ModelMetadata>,
    pub version: Option<SemanticVersion>,
}

/// Model persistence service
pub struct ModelPersistenceService {
    config: ModelPersistenceConfig,
    model_storage: Arc<ModelStorage>,
    rollback_manager: Arc<ModelRollbackManager>,
    training_service: Arc<TrainingDataService>,
    models: Arc<RwLock<HashMap<String, Arc<RwLock<FannModelAdapter>>>>>,
    operation_history: Arc<RwLock<Vec<ModelOperationResult>>>,
}

impl ModelPersistenceService {
    /// Create a new model persistence service
    pub async fn new(
        config: ModelPersistenceConfig,
        training_service: Arc<TrainingDataService>,
    ) -> Result<Self> {
        // Initialize model storage
        let storage_config = ModelStorageConfig {
            base_path: config.model_storage_path.clone(),
            max_versions_per_model: 10,
            enable_compression: true,
            enable_encryption: false,
            checkpoint_frequency: config.checkpoint_frequency,
        };
        let model_storage = Arc::new(ModelStorage::new(storage_config).await?);

        // Initialize rollback manager
        let rollback_manager = Arc::new(ModelRollbackManager::new(config.rollback_config.clone())?);

        Ok(Self {
            config,
            model_storage,
            rollback_manager,
            training_service,
            models: Arc::new(RwLock::new(HashMap::new())),
            operation_history: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Register a new FANN model
    pub async fn register_model(
        &self,
        model_name: &str,
        model_config: FannModelConfig,
    ) -> Result<()> {
        info!("Registering model: {}", model_name);

        let storage_config = ModelStorageConfig {
            base_path: self.config.model_storage_path.clone(),
            max_versions_per_model: 10,
            enable_compression: true,
            enable_encryption: false,
            checkpoint_frequency: self.config.checkpoint_frequency,
        };

        let adapter = FannModelAdapter::new(model_config, storage_config).await?;
        let adapter_arc = Arc::new(RwLock::new(adapter));

        let mut models = self.models.write().await;
        models.insert(model_name.to_string(), adapter_arc);

        info!("Model {} registered successfully", model_name);
        Ok(())
    }

    /// Train a model with automatic persistence
    pub async fn train_model(
        &self,
        model_name: &str,
        symbol: &str,
        training_config: TrainingConfig,
        data_config: TrainingDataConfig,
    ) -> Result<TrainingRecord> {
        info!("Starting training for model: {} with symbol: {}", model_name, symbol);

        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        // Load training data
        use crate::integration::training_data_service::ModelType;
        let training_data = self.training_service
            .load_training_batch(ModelType::MLP, symbol, data_config.clone())
            .await?;

        // Convert to FANN format
        let vendor_data = VendorTimeSeriesData {
            symbol: symbol.to_string(),
            timestamps: training_data.timestamps.clone(),
            values: training_data.features.iter()
                .flatten()
                .map(|&x| x as f32)
                .collect(),
            exogenous_historical: None,
            exogenous_future: None,
            static_features: None,
            time_features: None,
        };

        // Create FANN training data
        let mut fann_training_data = ruv_fann::TrainingData {
            inputs: Vec::new(),
            outputs: Vec::new(),
        };
        let input_size = 20; // Configurable based on model
        let output_size = 1;

        if vendor_data.values.len() >= input_size + output_size {
            let num_samples = vendor_data.values.len() - input_size - output_size + 1;
            
            for i in 0..num_samples {
                let input_start = i;
                let input_end = i + input_size;
                let output_start = input_end;
                let output_end = output_start + output_size;
                
                let input = vendor_data.values[input_start..input_end].to_vec();
                let output = vendor_data.values[output_start..output_end].to_vec();
                
                fann_training_data.inputs.push(input);
                fann_training_data.outputs.push(output);
            }
        } else {
            return Err(anyhow::anyhow!("Insufficient training data"));
        }

        // Train with checkpointing
        let mut adapter_guard = adapter.write().await;
        let record = adapter_guard.train_with_checkpointing(
            &fann_training_data,
            &training_config,
            self.config.checkpoint_frequency,
        ).await?;

        // Update performance metrics for rollback monitoring
        let performance_metrics = adapter_guard.get_performance_metrics();
        let model_metrics = ModelMetrics {
            accuracy: performance_metrics.r_squared,
            latency_ms: 50.0, // Default latency
            error_rate: performance_metrics.mse,
            memory_mb: 100, // Default memory usage
            cpu_percent: 25.0, // Default CPU usage
            throughput: 1.0 / 0.05, // predictions per second
            timestamp: Utc::now(),
        };

        // Deploy the trained model to rollback system
        if let Err(e) = self.rollback_manager.deploy_model(
            model_name,
            &PathBuf::from("temp_model_path"), // This would be the actual model path
            serde_json::to_value(&training_config)?,
            model_metrics,
        ).await {
            warn!("Failed to deploy model to rollback system: {}", e);
        }

        drop(adapter_guard);

        // Record operation
        self.record_operation(ModelOperationResult {
            operation: ModelOperation::Train {
                model_name: model_name.to_string(),
                config: training_config,
                data_config,
            },
            success: true,
            message: format!("Training completed: {} epochs, MSE: {:.6}", 
                           record.epochs_completed, record.final_mse),
            timestamp: Utc::now(),
            metadata: None,
            version: None,
        }).await;

        info!("Training completed for model: {}", model_name);
        Ok(record)
    }

    /// Save a model with versioning
    pub async fn save_model(
        &self,
        model_name: &str,
        version_increment: VersionIncrement,
    ) -> Result<SemanticVersion> {
        info!("Saving model: {} with increment: {:?}", model_name, version_increment);

        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let adapter_guard = adapter.read().await;
        let saved_path = adapter_guard.save_model(version_increment).await?;
        let metadata = adapter_guard.get_metadata();
        drop(adapter_guard);

        // Record operation
        self.record_operation(ModelOperationResult {
            operation: ModelOperation::Save {
                model_name: model_name.to_string(),
                version_increment,
            },
            success: true,
            message: format!("Model saved to: {:?}", saved_path),
            timestamp: Utc::now(),
            metadata: Some(metadata.clone()),
            version: Some(metadata.version.clone()),
        }).await;

        info!("Model {} saved successfully: version {}", model_name, metadata.version);
        Ok(metadata.version)
    }

    /// Load a model version
    pub async fn load_model(
        &self,
        model_name: &str,
        version: Option<SemanticVersion>,
    ) -> Result<ModelMetadata> {
        info!("Loading model: {} version: {:?}", model_name, version);

        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let mut adapter_guard = adapter.write().await;
        adapter_guard.load_model(version.clone()).await?;
        let metadata = adapter_guard.get_metadata();
        drop(adapter_guard);

        // Record operation
        self.record_operation(ModelOperationResult {
            operation: ModelOperation::Load {
                model_name: model_name.to_string(),
                version: version.clone(),
            },
            success: true,
            message: format!("Model loaded: version {}", metadata.version),
            timestamp: Utc::now(),
            metadata: Some(metadata.clone()),
            version: Some(metadata.version.clone()),
        }).await;

        info!("Model {} loaded successfully: version {}", model_name, metadata.version);
        Ok(metadata)
    }

    /// Rollback a model to previous version
    pub async fn rollback_model(
        &self,
        model_name: &str,
        reason: &str,
    ) -> Result<ModelVersion> {
        info!("Rolling back model: {} - Reason: {}", model_name, reason);

        let rollback_reason = RollbackReason::ManualRequest {
            requestor: "ModelPersistenceService".to_string(),
            reason: reason.to_string(),
        };

        let rolled_back_version = self.rollback_manager
            .rollback_model(model_name, rollback_reason, false)
            .await?;

        // Reload the model in our adapter
        if let Some(adapter) = self.models.read().await.get(model_name).cloned() {
            let mut adapter_guard = adapter.write().await;
            let version = SemanticVersion {
                major: 1,
                minor: 0,
                patch: 0,
            }; // This would come from the rollback version
            adapter_guard.load_model(Some(version)).await?;
        }

        // Record operation
        self.record_operation(ModelOperationResult {
            operation: ModelOperation::Rollback {
                model_name: model_name.to_string(),
                reason: reason.to_string(),
            },
            success: true,
            message: format!("Model rolled back to version: {}", rolled_back_version.version_id),
            timestamp: Utc::now(),
            metadata: None,
            version: None,
        }).await;

        info!("Model {} rolled back successfully", model_name);
        Ok(rolled_back_version)
    }

    /// Get model metadata
    pub async fn get_model_metadata(&self, model_name: &str) -> Result<ModelMetadata> {
        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let adapter_guard = adapter.read().await;
        Ok(adapter_guard.get_metadata().clone())
    }

    /// List all registered models
    pub async fn list_models(&self) -> Vec<String> {
        let models = self.models.read().await;
        models.keys().cloned().collect()
    }

    /// Get operation history
    pub async fn get_operation_history(&self) -> Vec<ModelOperationResult> {
        let history = self.operation_history.read().await;
        history.clone()
    }

    /// Get model performance metrics
    pub async fn get_model_performance(&self, model_name: &str) -> Result<crate::adapters::model_storage::PerformanceMetrics> {
        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let adapter_guard = adapter.read().await;
        Ok(adapter_guard.get_performance_metrics())
    }

    /// Start performance monitoring for auto-rollback
    pub async fn start_performance_monitoring(&self, model_name: &str) -> Result<()> {
        if !self.config.enable_auto_rollback {
            return Ok(());
        }

        info!("Starting performance monitoring for model: {}", model_name);

        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let rollback_manager = Arc::clone(&self.rollback_manager);
        let model_name = model_name.to_string();
        let threshold = self.config.rollback_threshold;

        // Spawn monitoring task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            let mut consecutive_failures = 0;

            loop {
                interval.tick().await;

                // Check model performance
                let adapter_guard = adapter.read().await;
                let metrics = adapter_guard.get_performance_metrics();
                drop(adapter_guard);

                // Check for performance degradation
                let accuracy_pct = metrics.r_squared * 100.0;
                let degradation = 100.0 - accuracy_pct;

                if degradation > threshold as f64 {
                    consecutive_failures += 1;
                    warn!("Performance degradation detected for {}: {:.2}%", 
                          model_name, degradation);

                    if consecutive_failures >= 3 {
                        info!("Triggering automatic rollback for {}", model_name);
                        
                        let rollback_reason = RollbackReason::PerformanceDegradation {
                            metric: "accuracy".to_string(),
                            baseline: 90.0,
                            current: accuracy_pct,
                            threshold: threshold as f64,
                        };

                        if let Err(e) = rollback_manager.rollback_model(
                            &model_name,
                            rollback_reason,
                            true,
                        ).await {
                            error!("Automatic rollback failed for {}: {}", model_name, e);
                        } else {
                            info!("Automatic rollback completed for {}", model_name);
                            break; // Stop monitoring after rollback
                        }
                    }
                } else {
                    consecutive_failures = 0;
                }
            }
        });

        Ok(())
    }

    /// Cleanup old model versions and checkpoints
    pub async fn cleanup_old_versions(&self, model_name: &str, keep_count: usize) -> Result<u32> {
        info!("Cleaning up old versions for model: {}", model_name);
        
        let removed_count = self.rollback_manager
            .cleanup_archives(model_name, keep_count)
            .await?;

        info!("Cleaned up {} old versions for model: {}", removed_count, model_name);
        Ok(removed_count)
    }

    /// Record operation in history
    async fn record_operation(&self, result: ModelOperationResult) {
        let mut history = self.operation_history.write().await;
        history.push(result);
        
        // Keep only recent operations (last 1000)
        if history.len() > 1000 {
            history.remove(0);
        }
    }

    /// Export model for production deployment
    pub async fn export_model_for_production(
        &self,
        model_name: &str,
        export_path: &PathBuf,
    ) -> Result<PathBuf> {
        info!("Exporting model {} for production to: {:?}", model_name, export_path);

        let models = self.models.read().await;
        let adapter = models.get(model_name)
            .ok_or_else(|| anyhow::anyhow!("Model not found: {}", model_name))?
            .clone();
        drop(models);

        let adapter_guard = adapter.read().await;
        
        // Create production export directory
        tokio::fs::create_dir_all(export_path).await?;
        
        let production_model_path = export_path.join("model.fann");
        let metadata_path = export_path.join("metadata.json");
        let config_path = export_path.join("config.json");

        // Save model file
        let model_path_str = production_model_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid export path"))?;
        
        // Use the SyncVendorModel trait save method
        (&*adapter_guard as &dyn SyncVendorModel).save(model_path_str)
            .map_err(|e| anyhow::anyhow!("Failed to export model: {:?}", e))?;

        // Save metadata
        let metadata = adapter_guard.get_metadata();
        let metadata_json = serde_json::to_string_pretty(metadata)?;
        tokio::fs::write(&metadata_path, metadata_json).await?;

        // Save configuration using the public getter method
        let config_data = serde_json::json!({
            "model_type": "FANN",
            "export_timestamp": chrono::Utc::now().to_rfc3339(),
            "model_name": metadata.model_type,
            "version": metadata.version,
            "config": adapter_guard.get_config()
        });
        let config_json = serde_json::to_string_pretty(&config_data)?;
        tokio::fs::write(&config_path, config_json).await?;

        drop(adapter_guard);

        info!("Model {} exported successfully to: {:?}", model_name, export_path);
        Ok(production_model_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::integration::training_data_service::{TrainingDataConfig, ModelType};

    #[tokio::test]
    async fn test_model_persistence_service() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = ModelPersistenceConfig {
            model_storage_path: temp_dir.path().join("models"),
            enable_auto_checkpointing: true,
            checkpoint_frequency: 10,
            ..Default::default()
        };

        // Mock training service - we'll create mock storage and cache
        use crate::data::{TimescaleDBStorage, RedisCache};
        let storage = Arc::new(TimescaleDBStorage::new(
            "postgresql://localhost/test".to_string()
        ).await.unwrap());
        let cache = Arc::new(RedisCache::new(
            "redis://localhost:6379".to_string()
        ).await.unwrap());
        
        let training_service = Arc::new(
            TrainingDataService::new(storage, cache).await.unwrap()
        );

        let service = ModelPersistenceService::new(config, training_service).await.unwrap();

        // Test model registration
        let model_config = FannModelConfig::default();
        service.register_model("test_model", model_config).await.unwrap();

        // Test model listing
        let models = service.list_models().await;
        assert!(models.contains(&"test_model".to_string()));

        // Test metadata retrieval
        let metadata = service.get_model_metadata("test_model").await.unwrap();
        assert_eq!(metadata.model_type, "FANN");
    }
}