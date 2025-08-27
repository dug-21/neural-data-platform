//! Model Registry Implementation
//!
//! Extracted and refactored from trading-specific model storage to be domain agnostic.
//! Provides model registration, versioning, search, and lifecycle management.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use super::{
    ArtifactInfo, ArtifactType, ModelAccessControl, ModelComparison, ModelInfo, ModelLineage,
    ModelRegistryConfig, ModelRegistryTrait, ModelSearchCriteria, ModelStatus, Permission,
    RegistryStats, ModelType, ModelMetrics,
};
use super::storage::{ModelStorage, ModelVersion, VersionIncrement};

/// Main model registry implementation
pub struct ModelRegistry {
    config: ModelRegistryConfig,
    storage: Arc<ModelStorage>,
    models: Arc<RwLock<HashMap<String, ModelInfo>>>,
    lineage: Arc<RwLock<HashMap<String, ModelLineage>>>,
    access_control: Arc<RwLock<HashMap<String, ModelAccessControl>>>,
    stats: Arc<RwLock<RegistryStatistics>>,
}

/// Internal registry statistics
#[derive(Debug, Default)]
struct RegistryStatistics {
    total_models: usize,
    total_versions: usize,
    total_size_bytes: u64,
    models_by_type: HashMap<String, usize>,
    models_by_status: HashMap<String, usize>,
    last_cleanup: Option<DateTime<Utc>>,
    operations_count: u64,
}

/// Model metadata for storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub model_type: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub status: String,
    pub metrics: serde_json::Value,
    pub artifacts: HashMap<String, ArtifactInfo>,
    pub lineage: Option<ModelLineage>,
}

impl ModelRegistry {
    /// Create a new model registry
    pub async fn new(config: ModelRegistryConfig) -> Result<Self> {
        info!("Initializing Model Registry at: {:?}", config.storage_path);
        
        // Create storage directory
        tokio::fs::create_dir_all(&config.storage_path).await?;
        
        // Initialize model storage
        let storage_config = super::storage::ModelStorageConfig {
            base_path: config.storage_path.clone(),
            max_versions_per_model: config.max_versions_per_model,
            enable_compression: config.enable_compression,
            enable_encryption: config.enable_encryption,
            checkpoint_frequency: 100,
        };
        
        let storage = Arc::new(ModelStorage::new(storage_config).await?);
        
        let registry = Self {
            config,
            storage,
            models: Arc::new(RwLock::new(HashMap::new())),
            lineage: Arc::new(RwLock::new(HashMap::new())),
            access_control: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(RegistryStatistics::default())),
        };
        
        // Load existing models
        registry.load_existing_models().await?;
        
        // Start background tasks
        registry.start_cleanup_task().await;
        if registry.config.backup_enabled {
            registry.start_backup_task().await;
        }
        
        info!("Model Registry initialized successfully");
        Ok(registry)
    }
    
    /// Register a new model
    pub async fn register_model(&self, mut model_info: ModelInfo) -> Result<String> {
        info!("Registering model: {} ({})", model_info.name, model_info.id);
        
        // Validate model info
        self.validate_model_info(&model_info)?;
        
        // Check if model already exists
        {
            let models = self.models.read().await;
            if models.contains_key(&model_info.id) {
                return Err(anyhow!("Model with ID '{}' already exists", model_info.id));
            }
        }
        
        // Set timestamps
        let now = Utc::now();
        model_info.created_at = now;
        model_info.updated_at = now;
        
        // Set initial status if not specified
        if matches!(model_info.status, ModelStatus::Draft) {
            model_info.status = ModelStatus::Draft;
        }
        
        // Create model lineage
        let lineage = ModelLineage {
            model_id: model_info.id.clone(),
            parent_models: Vec::new(),
            child_models: Vec::new(),
            training_job_id: None,
            dataset_versions: Vec::new(),
            feature_versions: Vec::new(),
            created_at: now,
        };
        
        // Store model
        {
            let mut models = self.models.write().await;
            models.insert(model_info.id.clone(), model_info.clone());
        }
        
        // Store lineage
        {
            let mut lineage_map = self.lineage.write().await;
            lineage_map.insert(model_info.id.clone(), lineage);
        }
        
        // Create default access control
        let access_control = ModelAccessControl {
            model_id: model_info.id.clone(),
            owner: model_info.created_by.clone().unwrap_or_else(|| "system".to_string()),
            permissions: HashMap::new(),
            public_access: false,
            access_log: Vec::new(),
        };
        
        {
            let mut ac = self.access_control.write().await;
            ac.insert(model_info.id.clone(), access_control);
        }
        
        // Update statistics
        self.update_stats_on_register(&model_info).await;
        
        // Persist to storage
        self.persist_model(&model_info).await?;
        
        info!("Model registered successfully: {}", model_info.id);
        Ok(model_info.id)
    }
    
    /// Update existing model
    pub async fn update_model(&self, mut model_info: ModelInfo) -> Result<()> {
        info!("Updating model: {}", model_info.id);
        
        // Check if model exists
        let existing_model = {
            let models = self.models.read().await;
            models.get(&model_info.id).cloned()
        };
        
        let mut existing_model = existing_model
            .ok_or_else(|| anyhow!("Model not found: {}", model_info.id))?;
        
        // Preserve creation info
        model_info.created_at = existing_model.created_at;
        model_info.created_by = existing_model.created_by.clone();
        model_info.updated_at = Utc::now();
        
        // Update model
        {
            let mut models = self.models.write().await;
            models.insert(model_info.id.clone(), model_info.clone());
        }
        
        // Update statistics
        self.update_stats_on_update(&existing_model, &model_info).await;
        
        // Persist changes
        self.persist_model(&model_info).await?;
        
        info!("Model updated successfully: {}", model_info.id);
        Ok(())
    }
    
    /// Get model by ID
    pub async fn get_model_info(&self, model_id: &str) -> Result<ModelInfo> {
        debug!("Retrieving model: {}", model_id);
        
        let models = self.models.read().await;
        models
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("Model not found: {}", model_id))
    }
    
    /// List models matching search criteria
    pub async fn list_models(&self, criteria: Option<ModelSearchCriteria>) -> Result<Vec<ModelInfo>> {
        let criteria = criteria.unwrap_or_default();
        debug!("Listing models with criteria: {:?}", criteria);
        
        let models = self.models.read().await;
        let mut filtered_models: Vec<ModelInfo> = models
            .values()
            .filter(|model| self.matches_criteria(model, &criteria))
            .cloned()
            .collect();
        
        // Sort by created_at descending (most recent first)
        filtered_models.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        // Apply pagination
        if let Some(offset) = criteria.offset {
            if offset < filtered_models.len() {
                filtered_models.drain(0..offset);
            } else {
                filtered_models.clear();
            }
        }
        
        if let Some(limit) = criteria.limit {
            filtered_models.truncate(limit);
        }
        
        info!("Found {} models matching criteria", filtered_models.len());
        Ok(filtered_models)
    }
    
    /// Delete model
    pub async fn delete_model(&self, model_id: &str) -> Result<()> {
        info!("Deleting model: {}", model_id);
        
        // Check if model exists
        let model = {
            let models = self.models.read().await;
            models.get(model_id).cloned()
        };
        
        let model = model.ok_or_else(|| anyhow!("Model not found: {}", model_id))?;
        
        // Check for child models in lineage
        {
            let lineage_map = self.lineage.read().await;
            if let Some(lineage) = lineage_map.get(model_id) {
                if !lineage.child_models.is_empty() {
                    warn!("Model {} has {} child models", model_id, lineage.child_models.len());
                }
            }
        }
        
        // Remove from memory
        {
            let mut models = self.models.write().await;
            models.remove(model_id);
        }
        
        {
            let mut lineage_map = self.lineage.write().await;
            lineage_map.remove(model_id);
        }
        
        {
            let mut ac = self.access_control.write().await;
            ac.remove(model_id);
        }
        
        // Update statistics
        self.update_stats_on_delete(&model).await;
        
        // Delete from storage (this would delete all model files)
        self.delete_model_files(model_id).await?;
        
        info!("Model deleted successfully: {}", model_id);
        Ok(())
    }
    
    /// Import model from file
    pub async fn import_model(&self, path: &Path, model_id: &str) -> Result<()> {
        info!("Importing model from: {:?}", path);
        
        if !path.exists() {
            return Err(anyhow!("Model file not found: {:?}", path));
        }
        
        // Read model metadata (this would depend on the file format)
        let model_info = self.read_model_metadata(path).await?;
        
        // Register the imported model
        self.register_model(model_info).await?;
        
        info!("Model imported successfully: {}", model_id);
        Ok(())
    }
    
    /// Export model to file
    pub async fn export_model(&self, model_id: &str, output_path: &Path) -> Result<()> {
        info!("Exporting model {} to: {:?}", model_id, output_path);
        
        let model = self.get_model_info(model_id).await?;
        
        // Create output directory
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Export model (simplified - would export actual model files)
        let export_data = serde_json::to_string_pretty(&model)?;
        tokio::fs::write(output_path, export_data).await?;
        
        info!("Model exported successfully: {}", model_id);
        Ok(())
    }
    
    /// Compare two models
    pub async fn compare_models(
        &self,
        baseline_id: &str,
        candidate_id: &str,
    ) -> Result<ModelComparison> {
        info!("Comparing models: {} vs {}", baseline_id, candidate_id);
        
        let baseline_model = self.get_model_info(baseline_id).await?;
        let candidate_model = self.get_model_info(candidate_id).await?;
        
        // Calculate metric differences
        let mut metric_differences = HashMap::new();
        
        // Compare accuracy
        if let (Some(baseline_acc), Some(candidate_acc)) = (
            baseline_model.metrics.accuracy,
            candidate_model.metrics.accuracy,
        ) {
            metric_differences.insert("accuracy".to_string(), candidate_acc - baseline_acc);
        }
        
        // Compare loss
        if let (Some(baseline_loss), Some(candidate_loss)) = (
            baseline_model.metrics.loss,
            candidate_model.metrics.loss,
        ) {
            metric_differences.insert("loss".to_string(), baseline_loss - candidate_loss); // Lower is better
        }
        
        // Calculate overall improvement percentage
        let improvement_percentage = if let Some(acc_diff) = metric_differences.get("accuracy") {
            if baseline_model.metrics.accuracy.unwrap_or(0.0) > 0.0 {
                (acc_diff / baseline_model.metrics.accuracy.unwrap()) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        // Simple recommendation logic
        let recommendation = if improvement_percentage > 5.0 {
            super::ComparisonRecommendation::PromoteCandidate
        } else if improvement_percentage < -2.0 {
            super::ComparisonRecommendation::KeepBaseline
        } else {
            super::ComparisonRecommendation::InconclusivedResults
        };
        
        Ok(ModelComparison {
            baseline_model,
            candidate_model,
            metric_differences,
            improvement_percentage,
            statistical_significance: 0.85, // Would calculate actual significance
            recommendation,
            comparison_timestamp: Utc::now(),
        })
    }
    
    /// Get model lineage
    pub async fn get_model_lineage(&self, model_id: &str) -> Result<ModelLineage> {
        let lineage_map = self.lineage.read().await;
        lineage_map
            .get(model_id)
            .cloned()
            .ok_or_else(|| anyhow!("Lineage not found for model: {}", model_id))
    }
    
    /// Set model parent relationships
    pub async fn set_model_parents(&self, model_id: &str, parent_ids: Vec<String>) -> Result<()> {
        info!("Setting parents for model {}: {:?}", model_id, parent_ids);
        
        // Validate parent models exist
        {
            let models = self.models.read().await;
            for parent_id in &parent_ids {
                if !models.contains_key(parent_id) {
                    return Err(anyhow!("Parent model not found: {}", parent_id));
                }
            }
        }
        
        // Update lineage
        {
            let mut lineage_map = self.lineage.write().await;
            if let Some(lineage) = lineage_map.get_mut(model_id) {
                lineage.parent_models = parent_ids.clone();
            }
            
            // Update parent models to include this as a child
            for parent_id in &parent_ids {
                if let Some(parent_lineage) = lineage_map.get_mut(parent_id) {
                    if !parent_lineage.child_models.contains(&model_id.to_string()) {
                        parent_lineage.child_models.push(model_id.to_string());
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get registry statistics
    pub async fn get_registry_stats(&self) -> Result<RegistryStats> {
        let stats = self.stats.read().await;
        let models = self.models.read().await;
        
        let mut newest_timestamp = None;
        let mut oldest_timestamp = None;
        let mut total_size = 0u64;
        
        for model in models.values() {
            // Calculate size from artifacts
            for artifact in model.artifacts.values() {
                total_size += artifact.size_bytes;
            }
            
            if newest_timestamp.is_none() || Some(model.created_at) > newest_timestamp {
                newest_timestamp = Some(model.created_at);
            }
            
            if oldest_timestamp.is_none() || Some(model.created_at) < oldest_timestamp {
                oldest_timestamp = Some(model.created_at);
            }
        }
        
        let average_size = if models.len() > 0 {
            total_size as f64 / models.len() as f64
        } else {
            0.0
        };
        
        Ok(RegistryStats {
            total_models: stats.total_models,
            models_by_type: stats.models_by_type.clone(),
            models_by_status: stats.models_by_status.clone(),
            total_size_bytes: total_size,
            average_model_size_bytes: average_size,
            most_recent_model: newest_timestamp,
            oldest_model: oldest_timestamp,
        })
    }
    
    /// Clean up old model versions
    pub async fn cleanup_old_versions(&self) -> Result<usize> {
        info!("Starting cleanup of old model versions");
        
        let cutoff_date = Utc::now() - Duration::days(30); // Keep last 30 days
        let mut cleaned_count = 0;
        
        // This would integrate with the storage layer to clean up old versions
        // For now, we'll update the stats
        {
            let mut stats = self.stats.write().await;
            stats.last_cleanup = Some(Utc::now());
        }
        
        info!("Cleanup completed, removed {} old versions", cleaned_count);
        Ok(cleaned_count)
    }
    
    // Private methods
    
    async fn load_existing_models(&self) -> Result<()> {
        info!("Loading existing models from storage");
        
        // This would scan the storage directory and load model metadata
        // For now, we'll just initialize empty collections
        
        info!("Loaded existing models");
        Ok(())
    }
    
    fn validate_model_info(&self, model_info: &ModelInfo) -> Result<()> {
        if model_info.id.is_empty() {
            return Err(anyhow!("Model ID cannot be empty"));
        }
        
        if model_info.name.is_empty() {
            return Err(anyhow!("Model name cannot be empty"));
        }
        
        if model_info.version.is_empty() {
            return Err(anyhow!("Model version cannot be empty"));
        }
        
        Ok(())
    }
    
    fn matches_criteria(&self, model: &ModelInfo, criteria: &ModelSearchCriteria) -> bool {
        // Name pattern matching
        if let Some(pattern) = &criteria.name_pattern {
            if !model.name.contains(pattern) && !model.id.contains(pattern) {
                return false;
            }
        }
        
        // Model type matching
        if let Some(model_type) = &criteria.model_type {
            if std::mem::discriminant(&model.model_type) != std::mem::discriminant(model_type) {
                return false;
            }
        }
        
        // Status matching
        if let Some(status) = &criteria.status {
            if std::mem::discriminant(&model.status) != std::mem::discriminant(status) {
                return false;
            }
        }
        
        // Tag matching
        if !criteria.tags.is_empty() {
            let has_any_tag = criteria.tags.iter().any(|tag| model.tags.contains(tag));
            if !has_any_tag {
                return false;
            }
        }
        
        // Date range filtering
        if let Some(after) = criteria.created_after {
            if model.created_at < after {
                return false;
            }
        }
        
        if let Some(before) = criteria.created_before {
            if model.created_at > before {
                return false;
            }
        }
        
        // Metric filtering
        if let Some(min_acc) = criteria.min_accuracy {
            if model.metrics.accuracy.unwrap_or(0.0) < min_acc {
                return false;
            }
        }
        
        if let Some(max_loss) = criteria.max_loss {
            if model.metrics.loss.unwrap_or(f64::INFINITY) > max_loss {
                return false;
            }
        }
        
        true
    }
    
    async fn update_stats_on_register(&self, model: &ModelInfo) {
        let mut stats = self.stats.write().await;
        stats.total_models += 1;
        stats.operations_count += 1;
        
        // Update type statistics
        let type_key = format!("{:?}", model.model_type);
        *stats.models_by_type.entry(type_key).or_insert(0) += 1;
        
        // Update status statistics
        let status_key = format!("{:?}", model.status);
        *stats.models_by_status.entry(status_key).or_insert(0) += 1;
    }
    
    async fn update_stats_on_update(&self, _old: &ModelInfo, new: &ModelInfo) {
        let mut stats = self.stats.write().await;
        stats.operations_count += 1;
        
        // Update status statistics if changed
        let status_key = format!("{:?}", new.status);
        *stats.models_by_status.entry(status_key).or_insert(0) += 1;
    }
    
    async fn update_stats_on_delete(&self, model: &ModelInfo) {
        let mut stats = self.stats.write().await;
        stats.total_models = stats.total_models.saturating_sub(1);
        stats.operations_count += 1;
        
        // Update type statistics
        let type_key = format!("{:?}", model.model_type);
        if let Some(count) = stats.models_by_type.get_mut(&type_key) {
            *count = count.saturating_sub(1);
        }
        
        // Update status statistics
        let status_key = format!("{:?}", model.status);
        if let Some(count) = stats.models_by_status.get_mut(&status_key) {
            *count = count.saturating_sub(1);
        }
    }
    
    async fn persist_model(&self, model: &ModelInfo) -> Result<()> {
        // Create model metadata for storage
        let metadata = ModelMetadata {
            id: model.id.clone(),
            name: model.name.clone(),
            version: model.version.clone(),
            model_type: format!("{:?}", model.model_type),
            created_at: model.created_at,
            created_by: model.created_by.clone(),
            description: model.description.clone(),
            tags: model.tags.clone(),
            status: format!("{:?}", model.status),
            metrics: serde_json::to_value(&model.metrics)?,
            artifacts: model.artifacts.clone(),
            lineage: {
                let lineage_map = self.lineage.read().await;
                lineage_map.get(&model.id).cloned()
            },
        };
        
        // Save metadata to storage (simplified)
        let metadata_path = self.config.storage_path.join(&model.id).join("metadata.json");
        tokio::fs::create_dir_all(metadata_path.parent().unwrap()).await?;
        
        let metadata_json = serde_json::to_string_pretty(&metadata)?;
        tokio::fs::write(&metadata_path, metadata_json).await?;
        
        Ok(())
    }
    
    async fn delete_model_files(&self, model_id: &str) -> Result<()> {
        let model_dir = self.config.storage_path.join(model_id);
        if model_dir.exists() {
            tokio::fs::remove_dir_all(&model_dir).await?;
        }
        Ok(())
    }
    
    async fn read_model_metadata(&self, path: &Path) -> Result<ModelInfo> {
        // Read and parse model metadata from file
        let content = tokio::fs::read_to_string(path).await?;
        let metadata: ModelMetadata = serde_json::from_str(&content)?;
        
        // Convert to ModelInfo (simplified)
        Ok(ModelInfo {
            id: metadata.id,
            name: metadata.name,
            version: metadata.version,
            model_type: ModelType::Custom(metadata.model_type),
            status: ModelStatus::Draft, // Default for imported models
            created_at: metadata.created_at,
            updated_at: Utc::now(),
            created_by: metadata.created_by,
            description: metadata.description,
            tags: metadata.tags,
            metrics: ModelMetrics::default(), // Would deserialize from JSON
            artifacts: metadata.artifacts,
        })
    }
    
    async fn start_cleanup_task(&self) {
        let config = self.config.clone();
        let registry_weak = Arc::downgrade(&self.stats);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(config.cleanup_interval_hours as u64 * 3600)
            );
            
            loop {
                interval.tick().await;
                
                if let Some(_stats) = registry_weak.upgrade() {
                    debug!("Running scheduled cleanup task");
                    // Cleanup would be performed here
                } else {
                    break; // Registry has been dropped
                }
            }
        });
    }
    
    async fn start_backup_task(&self) {
        let config = self.config.clone();
        let registry_weak = Arc::downgrade(&self.stats);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(config.backup_interval_hours as u64 * 3600)
            );
            
            loop {
                interval.tick().await;
                
                if let Some(_stats) = registry_weak.upgrade() {
                    debug!("Running scheduled backup task");
                    // Backup would be performed here
                } else {
                    break; // Registry has been dropped
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl ModelRegistryTrait for ModelRegistry {
    async fn register_model(&self, model_info: ModelInfo) -> Result<String> {
        self.register_model(model_info).await
    }
    
    async fn update_model(&self, model_info: ModelInfo) -> Result<()> {
        self.update_model(model_info).await
    }
    
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>> {
        match self.get_model_info(model_id).await {
            Ok(model) => Ok(Some(model)),
            Err(_) => Ok(None),
        }
    }
    
    async fn list_models(&self, criteria: Option<ModelSearchCriteria>) -> Result<Vec<ModelInfo>> {
        self.list_models(criteria).await
    }
    
    async fn delete_model(&self, model_id: &str) -> Result<()> {
        self.delete_model(model_id).await
    }
    
    async fn add_version(&self, model_id: &str, version_info: ModelVersion) -> Result<()> {
        // This would integrate with the storage layer
        info!("Adding version {} to model {}", version_info.version, model_id);
        Ok(())
    }
    
    async fn get_versions(&self, _model_id: &str) -> Result<Vec<ModelVersion>> {
        // This would retrieve versions from storage
        Ok(vec![])
    }
    
    async fn store_artifact(
        &self,
        model_id: &str,
        _artifact_type: ArtifactType,
        data: &[u8],
    ) -> Result<String> {
        let artifact_id = Uuid::new_v4().to_string();
        let artifact_path = self.config.storage_path
            .join(model_id)
            .join("artifacts")
            .join(&artifact_id);
        
        tokio::fs::create_dir_all(artifact_path.parent().unwrap()).await?;
        tokio::fs::write(&artifact_path, data).await?;
        
        info!("Stored artifact {} for model {}", artifact_id, model_id);
        Ok(artifact_id)
    }
    
    async fn retrieve_artifact(&self, model_id: &str, artifact_id: &str) -> Result<Vec<u8>> {
        let artifact_path = self.config.storage_path
            .join(model_id)
            .join("artifacts")
            .join(artifact_id);
        
        tokio::fs::read(&artifact_path).await.map_err(|e| anyhow!("Failed to read artifact: {}", e))
    }
    
    async fn get_stats(&self) -> Result<RegistryStats> {
        self.get_registry_stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn create_test_registry() -> (ModelRegistry, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = ModelRegistryConfig {
            storage_path: temp_dir.path().to_path_buf(),
            ..ModelRegistryConfig::default()
        };
        
        let registry = ModelRegistry::new(config).await.unwrap();
        (registry, temp_dir)
    }
    
    #[tokio::test]
    async fn test_model_registration() {
        let (registry, _temp_dir) = create_test_registry().await;
        
        let model_info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            version: "1.0.0".to_string(),
            model_type: ModelType::NeuralNetwork,
            status: ModelStatus::Draft,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: Some("test-user".to_string()),
            description: Some("Test model".to_string()),
            tags: vec!["test".to_string()],
            metrics: ModelMetrics::default(),
            artifacts: HashMap::new(),
        };
        
        let model_id = registry.register_model(model_info.clone()).await.unwrap();
        assert_eq!(model_id, "test-model");
        
        let retrieved = registry.get_model_info(&model_id).await.unwrap();
        assert_eq!(retrieved.name, "Test Model");
        assert_eq!(retrieved.version, "1.0.0");
    }
    
    #[tokio::test]
    async fn test_model_search() {
        let (registry, _temp_dir) = create_test_registry().await;
        
        // Register multiple models
        for i in 1..=3 {
            let model_info = ModelInfo {
                id: format!("test-model-{}", i),
                name: format!("Test Model {}", i),
                version: "1.0.0".to_string(),
                model_type: ModelType::NeuralNetwork,
                status: ModelStatus::Draft,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                created_by: None,
                description: None,
                tags: vec!["test".to_string()],
                metrics: ModelMetrics::default(),
                artifacts: HashMap::new(),
            };
            
            registry.register_model(model_info).await.unwrap();
        }
        
        // Search all models
        let all_models = registry.list_models(None).await.unwrap();
        assert_eq!(all_models.len(), 3);
        
        // Search with criteria
        let criteria = ModelSearchCriteria {
            name_pattern: Some("Model 2".to_string()),
            ..ModelSearchCriteria::default()
        };
        
        let filtered_models = registry.list_models(Some(criteria)).await.unwrap();
        assert_eq!(filtered_models.len(), 1);
        assert_eq!(filtered_models[0].name, "Test Model 2");
    }
}