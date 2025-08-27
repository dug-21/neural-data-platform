//! Model Storage Implementation  
//!
//! Extracted and refactored from the trading-specific model storage to be domain agnostic.
//! Provides versioned model persistence with metadata, checksums, and rollback capabilities.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Model storage configuration
#[derive(Debug, Clone)]
pub struct ModelStorageConfig {
    pub base_path: PathBuf,
    pub max_versions_per_model: usize,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub checkpoint_frequency: usize,
}

impl Default for ModelStorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("./models"),
            max_versions_per_model: 10,
            enable_compression: true,
            enable_encryption: false,
            checkpoint_frequency: 100,
        }
    }
}

/// Semantic version for models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    
    pub fn increment_patch(&mut self) {
        self.patch += 1;
    }
    
    pub fn increment_minor(&mut self) {
        self.minor += 1;
        self.patch = 0;
    }
    
    pub fn increment_major(&mut self) {
        self.major += 1;
        self.minor = 0;
        self.patch = 0;
    }
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Model version information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVersion {
    pub model_type: String,
    pub version: SemanticVersion,
    pub path: PathBuf,
    pub metadata_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub size_bytes: u64,
    pub checksum: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
}

/// Version increment strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VersionIncrement {
    Patch,  // Bug fixes, minor improvements
    Minor,  // New features, backward compatible
    Major,  // Breaking changes
    Auto,   // Automatically determine based on metrics
}

/// Model metadata for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredModelMetadata {
    pub model_type: String,
    pub version: SemanticVersion,
    pub timestamp: DateTime<Utc>,
    pub checksum: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub metrics: std::collections::HashMap<String, f64>,
    pub artifacts: std::collections::HashMap<String, ArtifactMetadata>,
    pub training_info: Option<TrainingInfo>,
}

/// Artifact metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub artifact_type: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub checksum: String,
    pub created_at: DateTime<Utc>,
}

/// Training information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingInfo {
    pub training_duration_secs: u64,
    pub training_samples: u64,
    pub validation_samples: u64,
    pub training_config: std::collections::HashMap<String, serde_json::Value>,
}

/// Checkpoint metadata for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub epoch: usize,
    pub step: usize,
    pub training_loss: f64,
    pub validation_loss: Option<f64>,
    pub learning_rate: f64,
    pub timestamp: DateTime<Utc>,
    pub model_state_size: u64,
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub total_models: usize,
    pub total_versions: usize,
    pub total_size_bytes: u64,
    pub models_by_type: std::collections::HashMap<String, usize>,
    pub storage_path: PathBuf,
    pub last_cleanup: Option<DateTime<Utc>>,
}

/// Main model storage implementation
#[derive(Debug)]
pub struct ModelStorage {
    config: ModelStorageConfig,
    version_history: Arc<RwLock<VecDeque<ModelVersion>>>,
}

impl ModelStorage {
    /// Create new model storage instance
    pub async fn new(config: ModelStorageConfig) -> Result<Self> {
        // Ensure base directory exists
        fs::create_dir_all(&config.base_path).await?;
        
        let storage = Self {
            config,
            version_history: Arc::new(RwLock::new(VecDeque::new())),
        };
        
        // Load existing version history
        storage.load_version_history().await?;
        
        info!("Model storage initialized at: {:?}", storage.config.base_path);
        Ok(storage)
    }
    
    /// Save a model with metadata and versioning
    pub async fn save_model(
        &self,
        model_type: &str,
        model_data: &[u8],
        metadata: StoredModelMetadata,
        increment_type: VersionIncrement,
    ) -> Result<ModelVersion> {
        info!("Saving {} model with version increment {:?}", model_type, increment_type);
        
        // Get next version
        let version = self.get_next_version(model_type, increment_type).await?;
        
        // Create model directory structure
        let model_dir = self.config.base_path
            .join(model_type)
            .join(version.to_string());
        fs::create_dir_all(&model_dir).await?;
        
        // Generate file paths
        let model_path = model_dir.join("model.bin");
        let metadata_path = model_dir.join("metadata.json");
        let temp_model_path = model_dir.join("model.bin.tmp");
        
        // Save model data atomically (write to temp, then rename)
        fs::write(&temp_model_path, model_data).await?;
        fs::rename(&temp_model_path, &model_path).await?;
        
        // Calculate checksum
        let checksum = self.calculate_checksum(model_data);
        
        // Update metadata with final version and checksum
        let mut final_metadata = metadata;
        final_metadata.version = version.clone();
        final_metadata.checksum = checksum.clone();
        
        // Save metadata
        let metadata_json = serde_json::to_string_pretty(&final_metadata)?;
        fs::write(&metadata_path, metadata_json).await?;
        
        // Get file size
        let file_meta = fs::metadata(&model_path).await?;
        let size_bytes = file_meta.len();
        
        // Create version info
        let model_version = ModelVersion {
            model_type: model_type.to_string(),
            version: version.clone(),
            path: model_path,
            metadata_path,
            timestamp: Utc::now(),
            size_bytes,
            checksum,
            description: final_metadata.description,
            tags: final_metadata.tags,
        };
        
        // Update version history
        self.add_to_version_history(model_version.clone()).await?;
        
        info!("Model saved successfully: {} v{}", model_type, version);
        Ok(model_version)
    }
    
    /// Load a model by type and version
    pub async fn load_model(
        &self,
        model_type: &str,
        version: Option<SemanticVersion>,
    ) -> Result<(Vec<u8>, StoredModelMetadata)> {
        let version = version.unwrap_or_else(|| {
            self.get_latest_version(model_type).unwrap_or(SemanticVersion::new(1, 0, 0))
        });
        
        info!("Loading {} model version {}", model_type, version);
        
        let model_dir = self.config.base_path
            .join(model_type)
            .join(version.to_string());
        let model_path = model_dir.join("model.bin");
        let metadata_path = model_dir.join("metadata.json");
        
        // Check if files exist
        if !model_path.exists() {
            return Err(anyhow!("Model file not found: {:?}", model_path));
        }
        
        // Load model data
        let model_data = fs::read(&model_path).await?;
        
        // Load metadata
        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: StoredModelMetadata = serde_json::from_str(&metadata_json)?;
        
        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&model_data);
        if calculated_checksum != metadata.checksum {
            warn!("Checksum mismatch for model {}: expected {}, got {}", 
                  model_type, metadata.checksum, calculated_checksum);
        }
        
        info!("Model loaded successfully: {} v{}", model_type, version);
        Ok((model_data, metadata))
    }
    
    /// Save model checkpoint during training
    pub async fn save_checkpoint(
        &self,
        model_type: &str,
        model_data: &[u8],
        checkpoint_metadata: CheckpointMetadata,
    ) -> Result<()> {
        debug!("Saving checkpoint for {} at epoch {}", model_type, checkpoint_metadata.epoch);
        
        let checkpoint_dir = self.config.base_path
            .join(model_type)
            .join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).await?;
        
        let checkpoint_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.bin", checkpoint_metadata.epoch));
        let metadata_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.json", checkpoint_metadata.epoch));
        
        // Save checkpoint data
        fs::write(&checkpoint_path, model_data).await?;
        
        // Save metadata
        let metadata_json = serde_json::to_string_pretty(&checkpoint_metadata)?;
        fs::write(&metadata_path, metadata_json).await?;
        
        // Clean up old checkpoints
        self.cleanup_old_checkpoints(&checkpoint_dir, 5).await?;
        
        Ok(())
    }
    
    /// Load checkpoint from specific epoch
    pub async fn load_checkpoint(
        &self,
        model_type: &str,
        epoch: usize,
    ) -> Result<(Vec<u8>, CheckpointMetadata)> {
        let checkpoint_dir = self.config.base_path
            .join(model_type)
            .join("checkpoints");
            
        let checkpoint_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.bin", epoch));
        let metadata_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.json", epoch));
        
        if !checkpoint_path.exists() {
            return Err(anyhow!("Checkpoint not found for epoch {}", epoch));
        }
        
        // Load checkpoint data
        let checkpoint_data = fs::read(&checkpoint_path).await?;
        
        // Load metadata
        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: CheckpointMetadata = serde_json::from_str(&metadata_json)?;
        
        Ok((checkpoint_data, metadata))
    }
    
    /// Rollback to a previous version
    pub async fn rollback(
        &self,
        model_type: &str,
        versions_back: usize,
    ) -> Result<(Vec<u8>, StoredModelMetadata)> {
        info!("Rolling back {} model {} versions", model_type, versions_back);
        
        let target_version = {
            let history = self.version_history.read().await;
            let model_versions: Vec<&ModelVersion> = history
                .iter()
                .filter(|v| v.model_type == model_type)
                .collect();
            
            if versions_back >= model_versions.len() {
                return Err(anyhow!(
                    "Cannot rollback {} versions, only {} versions available",
                    versions_back,
                    model_versions.len()
                ));
            }
            
            model_versions[model_versions.len() - versions_back - 1].version.clone()
        };
        
        self.load_model(model_type, Some(target_version)).await
    }
    
    /// List all available versions for a model
    pub async fn list_versions(&self, model_type: &str) -> Vec<(SemanticVersion, DateTime<Utc>)> {
        let history = self.version_history.read().await;
        history
            .iter()
            .filter(|v| v.model_type == model_type)
            .map(|v| (v.version.clone(), v.timestamp))
            .collect()
    }
    
    /// Get storage metrics
    pub async fn get_storage_metrics(&self) -> StorageMetrics {
        let history = self.version_history.read().await;
        let total_size: u64 = history.iter().map(|v| v.size_bytes).sum();
        let total_versions = history.len();
        
        let mut models_by_type = std::collections::HashMap::new();
        for version in history.iter() {
            *models_by_type.entry(version.model_type.clone()).or_insert(0) += 1;
        }
        
        // Count unique models
        let unique_models: std::collections::HashSet<_> = history
            .iter()
            .map(|v| &v.model_type)
            .collect();
        
        StorageMetrics {
            total_models: unique_models.len(),
            total_versions,
            total_size_bytes: total_size,
            models_by_type,
            storage_path: self.config.base_path.clone(),
            last_cleanup: None, // Would track actual cleanup times
        }
    }
    
    /// Delete a model and all its versions
    pub async fn delete_model(&self, model_type: &str) -> Result<usize> {
        info!("Deleting all versions of model: {}", model_type);
        
        // Remove from version history
        let mut removed_count = 0;
        {
            let mut history = self.version_history.write().await;
            let original_len = history.len();
            history.retain(|v| v.model_type != model_type);
            removed_count = original_len - history.len();
        }
        
        // Delete directory
        let model_dir = self.config.base_path.join(model_type);
        if model_dir.exists() {
            fs::remove_dir_all(&model_dir).await?;
        }
        
        info!("Deleted {} versions of model: {}", removed_count, model_type);
        Ok(removed_count)
    }
    
    /// Clean up old model versions beyond the retention limit
    pub async fn cleanup_old_versions(&self) -> Result<usize> {
        info!("Cleaning up old model versions");
        
        let mut cleanup_count = 0;
        let mut models_to_clean: std::collections::HashMap<String, Vec<ModelVersion>> = 
            std::collections::HashMap::new();
        
        // Group versions by model type
        {
            let history = self.version_history.read().await;
            for version in history.iter() {
                models_to_clean
                    .entry(version.model_type.clone())
                    .or_default()
                    .push(version.clone());
            }
        }
        
        // Clean up each model type
        for (model_type, mut versions) in models_to_clean {
            if versions.len() > self.config.max_versions_per_model {
                // Sort by timestamp (oldest first)
                versions.sort_by_key(|v| v.timestamp);
                
                // Remove excess versions
                let excess_count = versions.len() - self.config.max_versions_per_model;
                let versions_to_remove = &versions[..excess_count];
                
                for version_to_remove in versions_to_remove {
                    // Delete version files
                    if let Some(parent) = version_to_remove.path.parent() {
                        if parent.exists() {
                            fs::remove_dir_all(parent).await?;
                            cleanup_count += 1;
                        }
                    }
                }
                
                // Update version history
                {
                    let mut history = self.version_history.write().await;
                    history.retain(|v| {
                        !(v.model_type == model_type && 
                          versions_to_remove.iter().any(|vtr| vtr.version == v.version))
                    });
                }
            }
        }
        
        info!("Cleanup completed: removed {} old versions", cleanup_count);
        Ok(cleanup_count)
    }
    
    // Private helper methods
    
    async fn get_next_version(
        &self,
        model_type: &str,
        increment_type: VersionIncrement,
    ) -> Result<SemanticVersion> {
        let current = self.get_latest_version(model_type);
        
        let mut next_version = current.unwrap_or_else(|| SemanticVersion::new(1, 0, 0));
        
        match increment_type {
            VersionIncrement::Patch => next_version.increment_patch(),
            VersionIncrement::Minor => next_version.increment_minor(),
            VersionIncrement::Major => next_version.increment_major(),
            VersionIncrement::Auto => {
                // Auto-increment based on some criteria
                next_version.increment_patch();
            }
        }
        
        Ok(next_version)
    }
    
    fn get_latest_version(&self, model_type: &str) -> Option<SemanticVersion> {
        // Use futures::executor::block_on to handle async in sync context
        let history = futures::executor::block_on(self.version_history.read());
        history
            .iter()
            .filter(|v| v.model_type == model_type)
            .map(|v| &v.version)
            .max()
            .cloned()
    }
    
    async fn load_version_history(&self) -> Result<()> {
        let models_dir = &self.config.base_path;
        if !models_dir.exists() {
            return Ok(());
        }
        
        let mut history = self.version_history.write().await;
        
        // Scan each model type directory
        let mut entries = fs::read_dir(models_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }
            
            let model_type = entry.file_name().to_string_lossy().to_string();
            if model_type == "checkpoints" {
                continue;
            }
            
            // Scan version directories
            let mut version_entries = fs::read_dir(entry.path()).await?;
            while let Some(version_entry) = version_entries.next_entry().await? {
                if !version_entry.file_type().await?.is_dir() {
                    continue;
                }
                
                let version_str = version_entry.file_name().to_string_lossy().to_string();
                if let Ok(version) = self.parse_version(&version_str) {
                    let model_path = version_entry.path().join("model.bin");
                    let metadata_path = version_entry.path().join("metadata.json");
                    
                    if model_path.exists() && metadata_path.exists() {
                        // Load metadata to get additional info
                        let metadata_json = fs::read_to_string(&metadata_path).await?;
                        let metadata: StoredModelMetadata = serde_json::from_str(&metadata_json)?;
                        
                        let file_meta = fs::metadata(&model_path).await?;
                        let timestamp = file_meta
                            .modified()?
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs();
                        
                        history.push_back(ModelVersion {
                            model_type: model_type.clone(),
                            version,
                            path: model_path,
                            metadata_path,
                            timestamp: DateTime::from_timestamp(timestamp as i64, 0)
                                .unwrap_or_else(Utc::now),
                            size_bytes: file_meta.len(),
                            checksum: metadata.checksum,
                            description: metadata.description,
                            tags: metadata.tags,
                        });
                    }
                }
            }
        }
        
        // Sort by timestamp
        let mut history_vec: Vec<_> = history.drain(..).collect();
        history_vec.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        history.extend(history_vec);
        
        info!("Loaded {} model versions from storage", history.len());
        Ok(())
    }
    
    async fn add_to_version_history(&self, version: ModelVersion) -> Result<()> {
        let mut history = self.version_history.write().await;
        history.push_back(version);
        
        // Enforce global version limit (across all models)
        let max_global_versions = self.config.max_versions_per_model * 50; // Reasonable global limit
        while history.len() > max_global_versions {
            if let Some(old_version) = history.pop_front() {
                // Clean up old version files
                if let Some(parent) = old_version.path.parent() {
                    if parent.exists() {
                        let _ = fs::remove_dir_all(parent).await; // Ignore errors for cleanup
                    }
                }
            }
        }
        
        Ok(())
    }
    
    fn parse_version(&self, version_str: &str) -> Result<SemanticVersion> {
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("Invalid version format: {}", version_str));
        }
        
        Ok(SemanticVersion {
            major: parts[0].parse()?,
            minor: parts[1].parse()?,
            patch: parts[2].parse()?,
        })
    }
    
    async fn cleanup_old_checkpoints(&self, checkpoint_dir: &Path, keep_count: usize) -> Result<()> {
        let mut entries = fs::read_dir(checkpoint_dir).await?;
        let mut checkpoints = Vec::new();
        
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_name().to_string_lossy().starts_with("checkpoint_epoch_") {
                let metadata = entry.metadata().await?;
                checkpoints.push((entry.path(), metadata.modified()?));
            }
        }
        
        // Sort by modification time (newest first)
        checkpoints.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Delete old checkpoints
        for (path, _) in checkpoints.iter().skip(keep_count) {
            debug!("Removing old checkpoint: {:?}", path);
            let _ = fs::remove_file(path).await; // Ignore errors
            
            // Also remove corresponding metadata file
            if let Some(stem) = path.file_stem() {
                let metadata_path = path.with_file_name(format!("{}.json", stem.to_string_lossy()));
                let _ = fs::remove_file(metadata_path).await;
            }
        }
        
        Ok(())
    }
    
    fn calculate_checksum(&self, data: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn create_test_storage() -> (ModelStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };
        
        let storage = ModelStorage::new(config).await.unwrap();
        (storage, temp_dir)
    }
    
    #[tokio::test]
    async fn test_model_save_and_load() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let model_data = b"test model data";
        let metadata = StoredModelMetadata {
            model_type: "test".to_string(),
            version: SemanticVersion::new(1, 0, 0),
            timestamp: Utc::now(),
            checksum: String::new(), // Will be calculated
            description: Some("Test model".to_string()),
            tags: vec!["test".to_string()],
            metrics: std::collections::HashMap::new(),
            artifacts: std::collections::HashMap::new(),
            training_info: None,
        };
        
        // Save model
        let saved_version = storage
            .save_model("test", model_data, metadata, VersionIncrement::Patch)
            .await
            .unwrap();
        
        assert!(saved_version.path.exists());
        
        // Load model
        let (loaded_data, loaded_metadata) = storage
            .load_model("test", None)
            .await
            .unwrap();
        
        assert_eq!(loaded_data, model_data);
        assert_eq!(loaded_metadata.model_type, "test");
        assert_eq!(loaded_metadata.description, Some("Test model".to_string()));
    }
    
    #[tokio::test]
    async fn test_version_management() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let model_data = b"test model data";
        
        // Save multiple versions
        for i in 0..3 {
            let metadata = StoredModelMetadata {
                model_type: "test".to_string(),
                version: SemanticVersion::new(1, 0, 0), // Will be incremented
                timestamp: Utc::now(),
                checksum: String::new(),
                description: Some(format!("Test model v{}", i)),
                tags: vec!["test".to_string()],
                metrics: std::collections::HashMap::new(),
                artifacts: std::collections::HashMap::new(),
                training_info: None,
            };
            
            storage
                .save_model("test", model_data, metadata, VersionIncrement::Patch)
                .await
                .unwrap();
        }
        
        // List versions
        let versions = storage.list_versions("test").await;
        assert_eq!(versions.len(), 3);
        
        // Check version numbers
        assert_eq!(versions[0].0, SemanticVersion::new(1, 0, 0));
        assert_eq!(versions[1].0, SemanticVersion::new(1, 0, 1));
        assert_eq!(versions[2].0, SemanticVersion::new(1, 0, 2));
    }
    
    #[tokio::test]
    async fn test_checkpoint_save_and_load() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        let checkpoint_data = b"checkpoint data";
        let checkpoint_metadata = CheckpointMetadata {
            epoch: 10,
            step: 1000,
            training_loss: 0.5,
            validation_loss: Some(0.6),
            learning_rate: 0.001,
            timestamp: Utc::now(),
            model_state_size: checkpoint_data.len() as u64,
        };
        
        // Save checkpoint
        storage
            .save_checkpoint("test", checkpoint_data, checkpoint_metadata.clone())
            .await
            .unwrap();
        
        // Load checkpoint
        let (loaded_data, loaded_metadata) = storage
            .load_checkpoint("test", 10)
            .await
            .unwrap();
        
        assert_eq!(loaded_data, checkpoint_data);
        assert_eq!(loaded_metadata.epoch, 10);
        assert_eq!(loaded_metadata.training_loss, 0.5);
    }
    
    #[tokio::test]
    async fn test_rollback() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        // Save multiple versions with different data
        for i in 0..3 {
            let model_data = format!("test model data v{}", i);
            let metadata = StoredModelMetadata {
                model_type: "test".to_string(),
                version: SemanticVersion::new(1, 0, 0),
                timestamp: Utc::now(),
                checksum: String::new(),
                description: Some(format!("Version {}", i)),
                tags: vec!["test".to_string()],
                metrics: std::collections::HashMap::new(),
                artifacts: std::collections::HashMap::new(),
                training_info: None,
            };
            
            storage
                .save_model("test", model_data.as_bytes(), metadata, VersionIncrement::Patch)
                .await
                .unwrap();
            
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await; // Ensure different timestamps
        }
        
        // Rollback to previous version
        let (rollback_data, rollback_metadata) = storage
            .rollback("test", 1)
            .await
            .unwrap();
        
        assert_eq!(rollback_data, b"test model data v1");
        assert_eq!(rollback_metadata.description, Some("Version 1".to_string()));
    }
}