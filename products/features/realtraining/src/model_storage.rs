//! Model Storage System - TimescaleDB-inspired filesystem persistence
//! 
//! This module provides persistent storage for neural network models using a
//! filesystem-based approach similar to TimescaleDB's data volume management.
//! Models are stored with metadata, versioning, and efficient cleanup policies.

use anyhow::{Result, Context, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Model storage configuration
#[derive(Debug, Clone)]
pub struct ModelStorageConfig {
    /// Base path for model storage (like TimescaleDB data directory)
    pub base_path: PathBuf,
    
    /// Maximum number of checkpoints to retain per model
    pub max_checkpoints_per_model: usize,
    
    /// Maximum age for archived models (days)
    pub archive_retention_days: u32,
    
    /// Enable compression for archived models
    pub enable_compression: bool,
    
    /// Storage quota per model type (MB)
    pub storage_quota_mb: u64,
}

impl Default for ModelStorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("/workspaces/neural-trader/models"),
            max_checkpoints_per_model: 10,
            archive_retention_days: 90,
            enable_compression: true,
            storage_quota_mb: 5000, // 5GB per model type
        }
    }
}

/// Model metadata stored alongside model files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    /// Unique model ID
    pub model_id: String,
    
    /// Model type (NHITS, TCN, DeepAR, etc.)
    pub model_type: String,
    
    /// Model version
    pub version: String,
    
    /// Training metadata
    pub training_info: TrainingInfo,
    
    /// Performance metrics at save time
    pub performance_metrics: PerformanceMetrics,
    
    /// Timestamp when model was saved
    pub saved_at: DateTime<Utc>,
    
    /// File paths for model components
    pub file_paths: ModelFilePaths,
    
    /// Storage size in bytes
    pub storage_size_bytes: u64,
    
    /// Compression ratio if compressed
    pub compression_ratio: Option<f32>,
    
    /// Model status
    pub status: ModelStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingInfo {
    /// Number of training epochs
    pub epochs: u32,
    
    /// Training duration in seconds
    pub duration_secs: u64,
    
    /// Number of training samples
    pub num_samples: usize,
    
    /// Training loss
    pub final_loss: f64,
    
    /// Validation loss
    pub validation_loss: Option<f64>,
    
    /// Training configuration
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Model accuracy
    pub accuracy: f64,
    
    /// Sharpe ratio
    pub sharpe_ratio: f64,
    
    /// Win rate
    pub win_rate: f64,
    
    /// Average prediction time (ms)
    pub avg_prediction_time_ms: f64,
    
    /// Memory usage (MB)
    pub memory_usage_mb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFilePaths {
    /// Main model file
    pub model_file: PathBuf,
    
    /// Configuration file
    pub config_file: PathBuf,
    
    /// Weights file (if separate)
    pub weights_file: Option<PathBuf>,
    
    /// Metadata file
    pub metadata_file: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ModelStatus {
    /// Model is in training
    Training,
    
    /// Model is available for use
    Active,
    
    /// Model is archived
    Archived,
    
    /// Model failed validation
    Failed,
    
    /// Model is being compressed
    Compressing,
}

/// Main model storage system
pub struct ModelStorage {
    config: ModelStorageConfig,
    metadata_cache: Arc<RwLock<std::collections::HashMap<String, ModelMetadata>>>,
}

impl ModelStorage {
    /// Create new model storage instance
    pub fn new(config: ModelStorageConfig) -> Result<Self> {
        // Ensure directories exist
        Self::ensure_directories(&config.base_path)?;
        
        Ok(Self {
            config,
            metadata_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }
    
    /// Ensure storage directories exist
    fn ensure_directories(base_path: &Path) -> Result<()> {
        let dirs = ["checkpoints", "production", "archive"];
        
        for dir in &dirs {
            let path = base_path.join(dir);
            fs::create_dir_all(&path)
                .with_context(|| format!("Failed to create directory: {:?}", path))?;
        }
        
        Ok(())
    }
    
    /// Save a model to storage
    pub async fn save_model(
        &self,
        model_type: &str,
        model_data: Vec<u8>,
        config_data: Vec<u8>,
        training_info: TrainingInfo,
        performance_metrics: PerformanceMetrics,
    ) -> Result<String> {
        let model_id = Uuid::new_v4().to_string();
        let version = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        
        // Create model directory
        let model_dir = self.config.base_path
            .join("checkpoints")
            .join(model_type)
            .join(&version);
        
        fs::create_dir_all(&model_dir)
            .with_context(|| format!("Failed to create model directory: {:?}", model_dir))?;
        
        // Save model files
        let model_file = model_dir.join("model.bin");
        let config_file = model_dir.join("config.json");
        let metadata_file = model_dir.join("metadata.json");
        
        // Write model data
        fs::write(&model_file, &model_data)
            .with_context(|| "Failed to write model file")?;
        
        // Write config
        fs::write(&config_file, &config_data)
            .with_context(|| "Failed to write config file")?;
        
        // Calculate storage size
        let storage_size_bytes = model_data.len() as u64 + config_data.len() as u64;
        
        // Create metadata
        let metadata = ModelMetadata {
            model_id: model_id.clone(),
            model_type: model_type.to_string(),
            version: version.clone(),
            training_info,
            performance_metrics,
            saved_at: Utc::now(),
            file_paths: ModelFilePaths {
                model_file: model_file.clone(),
                config_file: config_file.clone(),
                weights_file: None,
                metadata_file: metadata_file.clone(),
            },
            storage_size_bytes,
            compression_ratio: None,
            status: ModelStatus::Active,
        };
        
        // Save metadata
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_file, metadata_json)
            .with_context(|| "Failed to write metadata file")?;
        
        // Update cache
        self.metadata_cache.write().await.insert(model_id.clone(), metadata);
        
        // Apply retention policies
        self.apply_retention_policies(model_type).await?;
        
        info!(
            "Saved model {} of type {} with version {}", 
            model_id, model_type, version
        );
        
        Ok(model_id)
    }
    
    /// Load a model from storage
    pub async fn load_model(&self, model_id: &str) -> Result<(Vec<u8>, ModelMetadata)> {
        // Check cache first
        if let Some(metadata) = self.metadata_cache.read().await.get(model_id) {
            if metadata.status == ModelStatus::Active {
                let model_data = fs::read(&metadata.file_paths.model_file)
                    .with_context(|| format!("Failed to read model file for {}", model_id))?;
                
                return Ok((model_data, metadata.clone()));
            }
        }
        
        // Search for model in filesystem
        let metadata = self.find_model_metadata(model_id).await?;
        
        if metadata.status != ModelStatus::Active {
            bail!("Model {} is not active (status: {:?})", model_id, metadata.status);
        }
        
        let model_data = fs::read(&metadata.file_paths.model_file)
            .with_context(|| format!("Failed to read model file for {}", model_id))?;
        
        // Update cache
        self.metadata_cache.write().await.insert(model_id.to_string(), metadata.clone());
        
        Ok((model_data, metadata))
    }
    
    /// Find model metadata by ID
    async fn find_model_metadata(&self, model_id: &str) -> Result<ModelMetadata> {
        let search_dirs = ["checkpoints", "production", "archive"];
        
        for dir in &search_dirs {
            let base_dir = self.config.base_path.join(dir);
            if let Ok(metadata) = self.search_directory_for_model(&base_dir, model_id).await {
                return Ok(metadata);
            }
        }
        
        bail!("Model {} not found", model_id)
    }
    
    /// Search a directory for a model
    async fn search_directory_for_model(&self, dir: &Path, model_id: &str) -> Result<ModelMetadata> {
        if !dir.exists() {
            bail!("Directory does not exist: {:?}", dir);
        }
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Recursively search subdirectories
                if let Ok(metadata) = self.search_directory_for_model(&path, model_id).await {
                    return Ok(metadata);
                }
            } else if path.file_name() == Some(std::ffi::OsStr::new("metadata.json")) {
                // Check if this metadata matches our model ID
                let metadata_content = fs::read_to_string(&path)?;
                if let Ok(metadata) = serde_json::from_str::<ModelMetadata>(&metadata_content) {
                    if metadata.model_id == model_id {
                        return Ok(metadata);
                    }
                }
            }
        }
        
        bail!("Model {} not found in directory {:?}", model_id, dir)
    }
    
    /// Apply retention policies to maintain storage limits
    async fn apply_retention_policies(&self, model_type: &str) -> Result<()> {
        let checkpoints_dir = self.config.base_path.join("checkpoints").join(model_type);
        
        if !checkpoints_dir.exists() {
            return Ok(());
        }
        
        // Get all versions sorted by timestamp
        let mut versions: Vec<(PathBuf, DateTime<Utc>)> = Vec::new();
        
        for entry in fs::read_dir(&checkpoints_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    let metadata_content = fs::read_to_string(&metadata_path)?;
                    if let Ok(metadata) = serde_json::from_str::<ModelMetadata>(&metadata_content) {
                        versions.push((path, metadata.saved_at));
                    }
                }
            }
        }
        
        // Sort by timestamp (newest first)
        versions.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Keep only the configured number of checkpoints
        if versions.len() > self.config.max_checkpoints_per_model {
            for (path, _) in versions.iter().skip(self.config.max_checkpoints_per_model) {
                // Archive or delete old checkpoints
                self.archive_checkpoint(path).await?;
            }
        }
        
        // Check storage quota
        self.enforce_storage_quota(model_type).await?;
        
        Ok(())
    }
    
    /// Archive a checkpoint
    async fn archive_checkpoint(&self, checkpoint_path: &Path) -> Result<()> {
        let archive_dir = self.config.base_path.join("archive");
        let checkpoint_name = checkpoint_path.file_name()
            .ok_or_else(|| anyhow::anyhow!("Invalid checkpoint path"))?;
        
        let archive_path = archive_dir.join(checkpoint_name);
        
        // Move to archive
        fs::rename(checkpoint_path, &archive_path)
            .with_context(|| format!("Failed to archive checkpoint: {:?}", checkpoint_path))?;
        
        // Update metadata status
        let metadata_path = archive_path.join("metadata.json");
        if metadata_path.exists() {
            let mut metadata: ModelMetadata = serde_json::from_str(
                &fs::read_to_string(&metadata_path)?
            )?;
            metadata.status = ModelStatus::Archived;
            
            let updated_metadata = serde_json::to_string_pretty(&metadata)?;
            fs::write(&metadata_path, updated_metadata)?;
        }
        
        info!("Archived checkpoint: {:?}", checkpoint_name);
        
        Ok(())
    }
    
    /// Enforce storage quota for a model type
    async fn enforce_storage_quota(&self, model_type: &str) -> Result<()> {
        let model_dir = self.config.base_path.join("checkpoints").join(model_type);
        
        if !model_dir.exists() {
            return Ok(());
        }
        
        let total_size = self.calculate_directory_size(&model_dir)?;
        let quota_bytes = self.config.storage_quota_mb * 1024 * 1024;
        
        if total_size > quota_bytes {
            warn!(
                "Model type {} exceeds storage quota: {} MB > {} MB",
                model_type,
                total_size / (1024 * 1024),
                self.config.storage_quota_mb
            );
            
            // TODO: Implement quota enforcement (delete oldest, compress, etc.)
        }
        
        Ok(())
    }
    
    /// Calculate total size of a directory
    fn calculate_directory_size(&self, dir: &Path) -> Result<u64> {
        let mut total_size = 0u64;
        
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_file() {
                total_size += entry.metadata()?.len();
            } else if path.is_dir() {
                total_size += self.calculate_directory_size(&path)?;
            }
        }
        
        Ok(total_size)
    }
    
    /// List all available models
    pub async fn list_models(&self, model_type: Option<&str>) -> Result<Vec<ModelMetadata>> {
        let mut models = Vec::new();
        let search_dirs = ["checkpoints", "production"];
        
        for dir in &search_dirs {
            let base_dir = self.config.base_path.join(dir);
            if base_dir.exists() {
                self.collect_models_from_directory(&base_dir, model_type, &mut models).await?;
            }
        }
        
        // Sort by timestamp (newest first)
        models.sort_by(|a, b| b.saved_at.cmp(&a.saved_at));
        
        Ok(models)
    }
    
    /// Collect models from a directory
    async fn collect_models_from_directory(
        &self,
        dir: &Path,
        model_type: Option<&str>,
        models: &mut Vec<ModelMetadata>,
    ) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                // Check for metadata file
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    let metadata_content = fs::read_to_string(&metadata_path)?;
                    if let Ok(metadata) = serde_json::from_str::<ModelMetadata>(&metadata_content) {
                        if let Some(filter_type) = model_type {
                            if metadata.model_type == filter_type {
                                models.push(metadata);
                            }
                        } else {
                            models.push(metadata);
                        }
                    }
                } else {
                    // Recursively search subdirectories
                    self.collect_models_from_directory(&path, model_type, models).await?;
                }
            }
        }
        
        Ok(())
    }
    
    /// Clean up old archived models
    pub async fn cleanup_archives(&self) -> Result<()> {
        let archive_dir = self.config.base_path.join("archive");
        if !archive_dir.exists() {
            return Ok(());
        }
        
        let cutoff_date = Utc::now() - chrono::Duration::days(self.config.archive_retention_days as i64);
        let mut deleted_count = 0;
        
        for entry in fs::read_dir(&archive_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.is_dir() {
                let metadata_path = path.join("metadata.json");
                if metadata_path.exists() {
                    let metadata_content = fs::read_to_string(&metadata_path)?;
                    if let Ok(metadata) = serde_json::from_str::<ModelMetadata>(&metadata_content) {
                        if metadata.saved_at < cutoff_date {
                            fs::remove_dir_all(&path)?;
                            deleted_count += 1;
                        }
                    }
                }
            }
        }
        
        if deleted_count > 0 {
            info!("Cleaned up {} archived models older than {} days", 
                deleted_count, self.config.archive_retention_days);
        }
        
        Ok(())
    }
    
    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        let mut stats = StorageStats::default();
        
        // Calculate sizes for each directory
        let dirs = [
            ("checkpoints", &mut stats.checkpoints_size_mb),
            ("production", &mut stats.production_size_mb),
            ("archive", &mut stats.archive_size_mb),
        ];
        
        for (dir_name, size_field) in &dirs {
            let dir_path = self.config.base_path.join(dir_name);
            if dir_path.exists() {
                let size_bytes = self.calculate_directory_size(&dir_path)?;
                **size_field = (size_bytes as f64) / (1024.0 * 1024.0);
                stats.total_size_mb += **size_field;
            }
        }
        
        // Count models
        let models = self.list_models(None).await?;
        stats.total_models = models.len();
        
        for model in &models {
            *stats.models_by_type.entry(model.model_type.clone()).or_insert(0) += 1;
            
            match model.status {
                ModelStatus::Active => stats.active_models += 1,
                ModelStatus::Archived => stats.archived_models += 1,
                _ => {}
            }
        }
        
        Ok(stats)
    }
}

/// Storage statistics
#[derive(Debug, Default)]
pub struct StorageStats {
    pub total_models: usize,
    pub active_models: usize,
    pub archived_models: usize,
    pub total_size_mb: f64,
    pub checkpoints_size_mb: f64,
    pub production_size_mb: f64,
    pub archive_size_mb: f64,
    pub models_by_type: std::collections::HashMap<String, usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_model_storage_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let storage = ModelStorage::new(config).unwrap();
        
        // Test saving a model
        let training_info = TrainingInfo {
            epochs: 100,
            duration_secs: 3600,
            num_samples: 10000,
            final_loss: 0.05,
            validation_loss: Some(0.06),
            config: serde_json::json!({"learning_rate": 0.001}),
        };
        
        let performance_metrics = PerformanceMetrics {
            accuracy: 0.92,
            sharpe_ratio: 1.5,
            win_rate: 0.62,
            avg_prediction_time_ms: 10.5,
            memory_usage_mb: 512.0,
        };
        
        let model_data = vec![1, 2, 3, 4, 5]; // Dummy model data
        let config_data = b"{}".to_vec();
        
        let model_id = storage.save_model(
            "NHITS",
            model_data.clone(),
            config_data,
            training_info,
            performance_metrics,
        ).await.unwrap();
        
        // Test loading the model
        let (loaded_data, metadata) = storage.load_model(&model_id).await.unwrap();
        assert_eq!(loaded_data, model_data);
        assert_eq!(metadata.model_type, "NHITS");
        
        // Test listing models
        let models = storage.list_models(Some("NHITS")).await.unwrap();
        assert_eq!(models.len(), 1);
        
        // Test storage stats
        let stats = storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.total_models, 1);
        assert_eq!(stats.active_models, 1);
    }
}