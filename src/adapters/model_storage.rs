//! Model Storage Module - Persistence for ruv-fann neural models
//!
//! This module provides versioned storage for ruv-fann neural models with metadata,
//! automatic versioning, atomic saves, and rollback capabilities.
//! 
//! This implementation works directly with ruv-fann's Network<f32> type and provides
//! Docker-compatible persistence with versioning, rollback, and checkpointing.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ruv_fann::Network;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use super::errors::{AdapterError, ErrorSeverity};
use super::vendor_bridge::{AsyncModelWrapper, SyncVendorModel, TrainingConfig};

/// Model metadata for tracking versions and performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub model_type: String,
    pub version: SemanticVersion,
    pub timestamp: DateTime<Utc>,
    pub accuracy: f64,
    pub loss: f64,
    pub training_params: TrainingParams,
    pub performance_metrics: PerformanceMetrics,
    pub checksum: String,
    pub training_duration_secs: u64,
    pub data_info: DataInfo,
}

/// Semantic versioning for models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemanticVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
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

    pub fn to_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::fmt::Display for SemanticVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Training parameters stored with model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingParams {
    pub learning_rate: f32,
    pub batch_size: usize,
    pub epochs: usize,
    pub optimizer: String,
    pub loss_function: String,
    pub early_stopping_patience: Option<usize>,
    pub validation_split: f32,
}

impl From<&TrainingConfig> for TrainingParams {
    fn from(config: &TrainingConfig) -> Self {
        Self {
            learning_rate: config.learning_rate,
            batch_size: config.batch_size,
            epochs: config.max_epochs,
            optimizer: "backprop".to_string(), // Default for ruv-fann
            loss_function: "mse".to_string(),
            early_stopping_patience: Some(config.early_stopping_patience),
            validation_split: config.validation_size,
        }
    }
}

/// Performance metrics for model evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub mae: f64,
    pub mse: f64,
    pub rmse: f64,
    pub mape: f64,
    pub r_squared: f64,
    pub validation_loss: f64,
    pub training_loss: f64,
}

/// Information about training data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataInfo {
    pub num_samples: usize,
    pub num_features: usize,
    pub symbol: String,
    pub time_range: (DateTime<Utc>, DateTime<Utc>),
}

/// Storage configuration
#[derive(Debug, Clone)]
pub struct ModelStorageConfig {
    pub base_path: PathBuf,
    pub max_versions_per_model: usize,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub checkpoint_frequency: usize, // Save checkpoint every N epochs
}

impl Default for ModelStorageConfig {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from("models"),
            max_versions_per_model: 10,
            enable_compression: true,
            enable_encryption: false,
            checkpoint_frequency: 100,
        }
    }
}

/// Model storage manager for ruv-fann models
#[derive(Debug)]
pub struct ModelStorage {
    config: ModelStorageConfig,
    version_history: Arc<RwLock<VecDeque<ModelVersion>>>,
}

/// Version entry for tracking model history
#[derive(Debug, Clone)]
pub struct ModelVersion {
    pub model_type: String,
    pub version: SemanticVersion,
    pub path: PathBuf,
    pub metadata_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub size_bytes: u64,
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

        Ok(storage)
    }

    /// Save a ruv-fann Network<f32> model with metadata
    pub async fn save_model(
        &self,
        network: &Network<f32>,
        model_type: &str,
        metadata: ModelMetadata,
        increment_type: VersionIncrement,
    ) -> Result<ModelVersion> {
        info!(
            "Saving {} model with version increment {:?}",
            model_type, increment_type
        );

        // Get next version
        let version = self.get_next_version(model_type, increment_type).await?;

        // Create model directory structure
        let model_dir = self
            .config
            .base_path
            .join(model_type)
            .join(version.to_string());
        fs::create_dir_all(&model_dir).await?;

        // Generate file paths
        let model_path = model_dir.join("model.ruv");
        let metadata_path = model_dir.join("metadata.json");
        let temp_model_path = model_dir.join("model.ruv.tmp");

        // Save network atomically (write to temp, then rename)
        {
            // Serialize network to bytes
            let network_bytes = network.to_bytes();

            // Write to temporary file
            fs::write(&temp_model_path, network_bytes).await
                .with_context(|| format!("Failed to write network to temp file: {:?}", temp_model_path))?;

            // Rename temp file to final location (atomic operation)
            fs::rename(&temp_model_path, &model_path).await
                .with_context(|| format!("Failed to rename temp file to final location"))?;
        }

        // Update metadata with final version
        let mut final_metadata = metadata;
        final_metadata.version = version.clone();

        // Calculate checksum
        let model_data = fs::read(&model_path).await?;
        final_metadata.checksum = self.calculate_checksum(&model_data);

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
            path: model_path.clone(),
            metadata_path,
            timestamp: Utc::now(),
            size_bytes,
        };

        // Update version history
        {
            let mut history = self.version_history.write().await;
            history.push_back(model_version.clone());

            // Enforce max versions limit
            while history.len() > self.config.max_versions_per_model {
                if let Some(old_version) = history.pop_front() {
                    self.delete_version(&old_version).await?;
                }
            }
        }

        info!("Network saved successfully to {:?}", model_path);
        Ok(model_version)
    }

    /// Load a ruv-fann Network<f32> with specific version
    pub async fn load_model(
        &self,
        model_type: &str,
        version: Option<SemanticVersion>,
    ) -> Result<(Network<f32>, ModelMetadata)> {
        let version = version.unwrap_or_else(|| {
            debug!("No version specified, loading latest");
            self.get_latest_version(model_type).unwrap_or(SemanticVersion::new(1, 0, 0))
        });

        info!("Loading {} model version {}", model_type, version);

        let model_dir = self
            .config
            .base_path
            .join(model_type)
            .join(version.to_string());
        let model_path = model_dir.join("model.ruv");
        let metadata_path = model_dir.join("metadata.json");

        // Check if files exist
        if !model_path.exists() {
            return Err(anyhow!(
                "Model file not found: {:?}",
                model_path
            ));
        }

        // Load network from bytes
        let network_bytes = fs::read(&model_path).await
            .with_context(|| format!("Failed to read network file: {:?}", model_path))?;
            
        let network = Network::<f32>::from_bytes(&network_bytes)
            .map_err(|e| anyhow!("Failed to deserialize network: {}", e))?;

        // Load metadata
        let metadata_json = fs::read_to_string(&metadata_path).await?;
        let metadata: ModelMetadata = serde_json::from_str(&metadata_json)?;

        // Verify checksum
        let calculated_checksum = self.calculate_checksum(&network_bytes);
        if calculated_checksum != metadata.checksum {
            warn!(
                "Checksum mismatch for model {}: expected {}, got {}",
                model_type, metadata.checksum, calculated_checksum
            );
        }

        info!("Network loaded successfully from {:?}", model_path);
        Ok((network, metadata))
    }

    /// Save checkpoint during training
    pub async fn save_checkpoint(
        &self,
        network: &Network<f32>,
        model_type: &str,
        epoch: usize,
        metrics: CheckpointMetrics,
    ) -> Result<()> {
        debug!("Saving checkpoint for {} at epoch {}", model_type, epoch);

        let checkpoint_dir = self
            .config
            .base_path
            .join(model_type)
            .join("checkpoints");
        fs::create_dir_all(&checkpoint_dir).await?;

        let checkpoint_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.ruv", epoch));
        let metrics_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.json", epoch));

        // Save network
        let network_bytes = network.to_bytes();
        fs::write(&checkpoint_path, network_bytes).await
            .with_context(|| format!("Failed to save checkpoint: {:?}", checkpoint_path))?;

        // Save metrics
        let metrics_json = serde_json::to_string_pretty(&metrics)?;
        fs::write(&metrics_path, metrics_json).await?;

        // Clean up old checkpoints (keep last 5)
        self.cleanup_old_checkpoints(&checkpoint_dir, 5).await?;

        Ok(())
    }

    /// Rollback to a previous version
    pub async fn rollback(
        &self,
        model_type: &str,
        versions_back: usize,
    ) -> Result<(Network<f32>, ModelMetadata)> {
        info!(
            "Rolling back {} model {} versions",
            model_type, versions_back
        );

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
        }; // Lock is released here

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
        let model_count = history.len();

        let mut models_by_type = std::collections::HashMap::new();
        for version in history.iter() {
            *models_by_type.entry(version.model_type.clone()).or_insert(0) += 1;
        }

        StorageMetrics {
            total_size_bytes: total_size,
            total_models: model_count,
            models_by_type,
            storage_path: self.config.base_path.clone(),
        }
    }

    /// Load checkpoint from specific epoch
    pub async fn load_checkpoint(
        &self,
        model_type: &str,
        epoch: usize,
    ) -> Result<(Network<f32>, CheckpointMetrics)> {
        let checkpoint_dir = self
            .config
            .base_path
            .join(model_type)
            .join("checkpoints");
            
        let checkpoint_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.ruv", epoch));
        let metrics_path = checkpoint_dir.join(format!("checkpoint_epoch_{}.json", epoch));
        
        if !checkpoint_path.exists() {
            return Err(anyhow!("Checkpoint not found for epoch {}", epoch));
        }
        
        // Load network
        let network_bytes = fs::read(&checkpoint_path).await?;
        let network = Network::<f32>::from_bytes(&network_bytes)
            .map_err(|e| anyhow!("Failed to deserialize checkpoint network: {}", e))?;
            
        // Load metrics
        let metrics_json = fs::read_to_string(&metrics_path).await?;
        let metrics: CheckpointMetrics = serde_json::from_str(&metrics_json)?;
        
        Ok((network, metrics))
    }

    /// Private helper methods

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
                // Auto-increment based on performance improvement
                next_version.increment_patch();
            }
        }

        Ok(next_version)
    }

    fn get_latest_version(&self, model_type: &str) -> Option<SemanticVersion> {
        // This is synchronous but should be called within an async context
        let history = futures::executor::block_on(self.version_history.read());
        history
            .iter()
            .filter(|v| v.model_type == model_type)
            .map(|v| &v.version)
            .max()
            .cloned()
    }

    async fn load_version_history(&self) -> Result<()> {
        // Scan models directory for existing versions
        let models_dir = &self.config.base_path;
        if !models_dir.exists() {
            return Ok(());
        }

        let mut history = self.version_history.write().await;

        // Read each model type directory
        let mut entries = fs::read_dir(models_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let model_type = entry.file_name().to_string_lossy().to_string();
            if model_type == "checkpoints" {
                continue;
            }

            // Read version directories
            let mut version_entries = fs::read_dir(entry.path()).await?;
            while let Some(version_entry) = version_entries.next_entry().await? {
                if !version_entry.file_type().await?.is_dir() {
                    continue;
                }

                let version_str = version_entry.file_name().to_string_lossy().to_string();
                if let Ok(version) = self.parse_version(&version_str) {
                    let model_path = version_entry.path().join("model.ruv");
                    let metadata_path = version_entry.path().join("metadata.json");

                    if model_path.exists() {
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
                        });
                    }
                }
            }
        }

        // Sort by version
        history.make_contiguous().sort_by(|a, b| {
            a.model_type
                .cmp(&b.model_type)
                .then(a.version.cmp(&b.version))
        });

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

    async fn delete_version(&self, version: &ModelVersion) -> Result<()> {
        warn!("Deleting old version: {} v{}", version.model_type, version.version);
        
        let version_dir = version.path.parent().ok_or_else(|| {
            anyhow!("Failed to get parent directory for {:?}", version.path)
        })?;

        fs::remove_dir_all(version_dir).await?;
        Ok(())
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
            fs::remove_file(path).await?;
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

/// Version increment strategy
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum VersionIncrement {
    Patch,  // Bug fixes, minor improvements
    Minor,  // New features, backward compatible
    Major,  // Breaking changes
    Auto,   // Automatically determine based on metrics
}

/// Checkpoint metrics saved during training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetrics {
    pub epoch: usize,
    pub training_loss: f64,
    pub validation_loss: f64,
    pub learning_rate: f32,
    pub timestamp: DateTime<Utc>,
}

/// Storage metrics for monitoring
#[derive(Debug, Clone)]
pub struct StorageMetrics {
    pub total_size_bytes: u64,
    pub total_models: usize,
    pub models_by_type: std::collections::HashMap<String, usize>,
    pub storage_path: PathBuf,
}

/// Trait for models that support persistence
#[async_trait]
pub trait PersistableModel: SyncVendorModel {
    /// Get current model metadata
    async fn get_metadata(&self) -> Result<ModelMetadata>;

    /// Update model from checkpoint
    async fn load_checkpoint(&mut self, checkpoint_path: &Path) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    // Create a mock network for testing
    fn create_mock_network() -> Network<f32> {
        // Create a simple 2-2-1 network for testing
        Network::new(&[2, 2, 1])
    }

    #[tokio::test]
    async fn test_model_storage_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let storage = ModelStorage::new(config).await.unwrap();
        let network = create_mock_network();

        let metadata = ModelMetadata {
            model_type: "test".to_string(),
            version: SemanticVersion::new(1, 0, 0),
            timestamp: Utc::now(),
            accuracy: 0.95,
            loss: 0.05,
            training_params: TrainingParams {
                learning_rate: 0.001,
                batch_size: 32,
                epochs: 100,
                optimizer: "backprop".to_string(),
                loss_function: "mse".to_string(),
                early_stopping_patience: Some(10),
                validation_split: 0.2,
            },
            performance_metrics: PerformanceMetrics {
                mae: 0.02,
                mse: 0.001,
                rmse: 0.03,
                mape: 2.5,
                r_squared: 0.98,
                validation_loss: 0.05,
                training_loss: 0.04,
            },
            checksum: String::new(),
            training_duration_secs: 3600,
            data_info: DataInfo {
                num_samples: 10000,
                num_features: 2,
                symbol: "BTC/USD".to_string(),
                time_range: (
                    Utc::now() - chrono::Duration::days(30),
                    Utc::now(),
                ),
            },
        };

        // Save model
        let saved_version = storage
            .save_model(&network, "test", metadata.clone(), VersionIncrement::Patch)
            .await
            .unwrap();

        assert!(saved_version.path.exists());

        // Load model
        let (loaded_network, loaded_metadata) = storage
            .load_model("test", None)
            .await
            .unwrap();

        assert_eq!(loaded_metadata.model_type, "test");
        assert_eq!(loaded_metadata.accuracy, 0.95);
        assert_eq!(loaded_network.num_layers(), network.num_layers());
    }

    #[tokio::test]
    async fn test_version_management() {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            max_versions_per_model: 3,
            ..Default::default()
        };

        let storage = ModelStorage::new(config).await.unwrap();
        let network = create_mock_network();

        // Save multiple versions
        for i in 0..5 {
            let metadata = ModelMetadata {
                model_type: "test".to_string(),
                version: SemanticVersion::new(1, 0, 0),
                timestamp: Utc::now(),
                accuracy: 0.90 + (i as f64 * 0.01),
                loss: 0.10 - (i as f64 * 0.01),
                training_params: TrainingParams {
                    learning_rate: 0.001,
                    batch_size: 32,
                    epochs: 100,
                    optimizer: "backprop".to_string(),
                    loss_function: "mse".to_string(),
                    early_stopping_patience: Some(10),
                    validation_split: 0.2,
                },
                performance_metrics: PerformanceMetrics {
                    mae: 0.02,
                    mse: 0.001,
                    rmse: 0.03,
                    mape: 2.5,
                    r_squared: 0.98,
                    validation_loss: 0.05,
                    training_loss: 0.04,
                },
                checksum: String::new(),
                training_duration_secs: 3600,
                data_info: DataInfo {
                    num_samples: 10000,
                    num_features: 2,
                    symbol: "BTC/USD".to_string(),
                    time_range: (
                        Utc::now() - chrono::Duration::days(30),
                        Utc::now(),
                    ),
                },
            };

            storage
                .save_model(&network, "test", metadata, VersionIncrement::Patch)
                .await
                .unwrap();

            // Small delay to ensure different timestamps
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Check that only 3 versions are kept
        let versions = storage.list_versions("test").await;
        assert_eq!(versions.len(), 3);

        // Verify versions are the latest ones
        assert_eq!(versions[0].0, SemanticVersion::new(1, 0, 2));
        assert_eq!(versions[1].0, SemanticVersion::new(1, 0, 3));
        assert_eq!(versions[2].0, SemanticVersion::new(1, 0, 4));
    }

    #[tokio::test]
    async fn test_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let storage = ModelStorage::new(config).await.unwrap();
        let network = create_mock_network();

        // Save multiple versions with different accuracies
        for i in 0..3 {
            let metadata = ModelMetadata {
                model_type: "test".to_string(),
                version: SemanticVersion::new(1, 0, 0),
                timestamp: Utc::now(),
                accuracy: 0.90 + (i as f64 * 0.02),
                loss: 0.10 - (i as f64 * 0.02),
                training_params: TrainingParams {
                    learning_rate: 0.001,
                    batch_size: 32,
                    epochs: 100,
                    optimizer: "backprop".to_string(),
                    loss_function: "mse".to_string(),
                    early_stopping_patience: Some(10),
                    validation_split: 0.2,
                },
                performance_metrics: PerformanceMetrics {
                    mae: 0.02,
                    mse: 0.001,
                    rmse: 0.03,
                    mape: 2.5,
                    r_squared: 0.98,
                    validation_loss: 0.05,
                    training_loss: 0.04,
                },
                checksum: String::new(),
                training_duration_secs: 3600,
                data_info: DataInfo {
                    num_samples: 10000,
                    num_features: 2,
                    symbol: "BTC/USD".to_string(),
                    time_range: (
                        Utc::now() - chrono::Duration::days(30),
                        Utc::now(),
                    ),
                },
            };

            storage
                .save_model(&network, "test", metadata, VersionIncrement::Patch)
                .await
                .unwrap();

            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Rollback to previous version
        let (rollback_network, rollback_metadata) = storage
            .rollback("test", 1)
            .await
            .unwrap();

        assert_eq!(rollback_metadata.accuracy, 0.92);
        assert_eq!(rollback_metadata.version, SemanticVersion::new(1, 0, 1));
        assert_eq!(rollback_network.num_layers(), network.num_layers());
    }

    #[tokio::test]
    async fn test_checkpoint_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelStorageConfig {
            base_path: temp_dir.path().to_path_buf(),
            ..Default::default()
        };

        let storage = ModelStorage::new(config).await.unwrap();
        let network = create_mock_network();

        let checkpoint_metrics = CheckpointMetrics {
            epoch: 50,
            training_loss: 0.05,
            validation_loss: 0.06,
            learning_rate: 0.001,
            timestamp: Utc::now(),
        };

        // Save checkpoint
        storage
            .save_checkpoint(&network, "test", 50, checkpoint_metrics.clone())
            .await
            .unwrap();

        // Load checkpoint
        let (loaded_network, loaded_metrics) = storage
            .load_checkpoint("test", 50)
            .await
            .unwrap();

        assert_eq!(loaded_metrics.epoch, 50);
        assert_eq!(loaded_metrics.training_loss, 0.05);
        assert_eq!(loaded_network.num_layers(), network.num_layers());
    }
}