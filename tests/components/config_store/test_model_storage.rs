//! Model Storage and Versioning Tests
//!
//! Comprehensive tests for Config Store model storage functionality,
//! covering versioning, binary storage, metadata management, and lifecycle.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Mock neural network model for testing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MockNeuralModel {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub version: String,
    pub architecture: ModelArchitecture,
    pub weights: Vec<f32>,
    pub training_metadata: TrainingMetadata,
    pub performance_metrics: PerformanceMetrics,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelArchitecture {
    pub input_size: usize,
    pub hidden_layers: Vec<usize>,
    pub output_size: usize,
    pub activation_function: String,
    pub learning_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrainingMetadata {
    pub dataset_version: String,
    pub training_epochs: u32,
    pub batch_size: u32,
    pub loss_function: String,
    pub optimizer: String,
    pub final_loss: f64,
    pub validation_accuracy: f64,
    pub training_duration_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PerformanceMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub inference_latency_ms: f64,
    pub throughput_ops_per_sec: f64,
    pub memory_usage_mb: f64,
}

#[derive(Debug, Clone)]
pub struct ModelVersion {
    pub version: String,
    pub model: MockNeuralModel,
    pub checksum: String,
    pub size_bytes: u64,
    pub storage_path: String,
    pub tags: Vec<String>,
    pub status: ModelStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ModelStatus {
    Training,
    Validating,
    Active,
    Deprecated,
    Archived,
}

/// Model storage implementation for Config Store
#[derive(Debug, Clone)]
pub struct ModelStorage {
    models: std::sync::Arc<Mutex<HashMap<String, Vec<ModelVersion>>>>,
    active_models: std::sync::Arc<Mutex<HashMap<String, String>>>, // model_id -> active_version
    storage_quotas: std::sync::Arc<Mutex<HashMap<String, StorageQuota>>>,
}

#[derive(Debug, Clone)]
struct StorageQuota {
    max_versions_per_model: usize,
    max_total_size_gb: f64,
    current_size_bytes: u64,
}

impl ModelStorage {
    pub fn new() -> Self {
        Self {
            models: std::sync::Arc::new(Mutex::new(HashMap::new())),
            active_models: std::sync::Arc::new(Mutex::new(HashMap::new())),
            storage_quotas: std::sync::Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Store a new model version
    pub async fn store_model(&self, model: MockNeuralModel, tags: Vec<String>) -> Result<ModelVersion> {
        let model_id = model.id.clone();
        let version = model.version.clone();
        
        // Serialize model to binary
        let serialized = bincode::serialize(&model)?;
        let checksum = format!("{:x}", md5::compute(&serialized));
        let size_bytes = serialized.len() as u64;
        
        // Check storage quota
        self.check_storage_quota(&model_id, size_bytes).await?;
        
        let model_version = ModelVersion {
            version: version.clone(),
            model,
            checksum,
            size_bytes,
            storage_path: format!("/models/{}/{}.bin", model_id, version),
            tags,
            status: ModelStatus::Training,
        };

        // Store the version
        let mut models = self.models.lock().await;
        let versions = models.entry(model_id.clone()).or_insert_with(Vec::new);
        
        // Check for duplicate versions
        if versions.iter().any(|v| v.version == version) {
            return Err(anyhow::anyhow!("Model version {} already exists for model {}", version, model_id));
        }
        
        versions.push(model_version.clone());
        
        // Sort versions by creation date (newest first)
        versions.sort_by(|a, b| b.model.created_at.cmp(&a.model.created_at));
        
        // Apply retention policy
        self.apply_retention_policy(&model_id, versions).await;
        
        // Update storage quota
        self.update_storage_quota(&model_id, size_bytes as i64).await;
        
        Ok(model_version)
    }

    /// Retrieve a specific model version
    pub async fn get_model(&self, model_id: &str, version: Option<&str>) -> Result<Option<MockNeuralModel>> {
        let models = self.models.lock().await;
        
        if let Some(versions) = models.get(model_id) {
            let target_version = if let Some(v) = version {
                v.to_string()
            } else {
                // Get latest version
                self.get_latest_version(model_id).await.unwrap_or_default()
            };
            
            if let Some(model_version) = versions.iter().find(|v| v.version == target_version) {
                Ok(Some(model_version.model.clone()))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// List all versions of a model
    pub async fn list_model_versions(&self, model_id: &str) -> Result<Vec<ModelVersion>> {
        let models = self.models.lock().await;
        Ok(models.get(model_id).cloned().unwrap_or_default())
    }

    /// Set active model version
    pub async fn set_active_version(&self, model_id: &str, version: &str) -> Result<()> {
        // Verify version exists
        let models = self.models.lock().await;
        if let Some(versions) = models.get(model_id) {
            if !versions.iter().any(|v| v.version == version) {
                return Err(anyhow::anyhow!("Version {} not found for model {}", version, model_id));
            }
        } else {
            return Err(anyhow::anyhow!("Model {} not found", model_id));
        }
        
        // Update status
        if let Some(versions) = models.get(model_id) {
            for version_data in versions {
                // This would need to be mutable in real implementation
                // For now, we'll just track active versions separately
            }
        }
        
        drop(models);
        
        // Set as active
        let mut active_models = self.active_models.lock().await;
        active_models.insert(model_id.to_string(), version.to_string());
        
        Ok(())
    }

    /// Get active model version
    pub async fn get_active_model(&self, model_id: &str) -> Result<Option<MockNeuralModel>> {
        let active_models = self.active_models.lock().await;
        if let Some(version) = active_models.get(model_id) {
            self.get_model(model_id, Some(version)).await
        } else {
            self.get_model(model_id, None).await // Get latest if no active version set
        }
    }

    /// Delete a specific model version
    pub async fn delete_model_version(&self, model_id: &str, version: &str) -> Result<bool> {
        let mut models = self.models.lock().await;
        
        if let Some(versions) = models.get_mut(model_id) {
            if let Some(pos) = versions.iter().position(|v| v.version == version) {
                let removed_version = versions.remove(pos);
                
                // Update storage quota
                self.update_storage_quota(model_id, -(removed_version.size_bytes as i64)).await;
                
                // If this was the active version, clear it
                let mut active_models = self.active_models.lock().await;
                if active_models.get(model_id) == Some(&version.to_string()) {
                    active_models.remove(model_id);
                }
                
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Compare two model versions
    pub async fn compare_models(
        &self,
        model_id: &str,
        version1: &str,
        version2: &str,
    ) -> Result<Option<ModelComparison>> {
        let model1 = self.get_model(model_id, Some(version1)).await?;
        let model2 = self.get_model(model_id, Some(version2)).await?;
        
        match (model1, model2) {
            (Some(m1), Some(m2)) => {
                let comparison = ModelComparison {
                    model_id: model_id.to_string(),
                    version1: version1.to_string(),
                    version2: version2.to_string(),
                    architecture_changed: m1.architecture != m2.architecture,
                    performance_delta: PerformanceDelta {
                        accuracy_delta: m2.performance_metrics.accuracy - m1.performance_metrics.accuracy,
                        latency_delta: m2.performance_metrics.inference_latency_ms - m1.performance_metrics.inference_latency_ms,
                        memory_delta: m2.performance_metrics.memory_usage_mb - m1.performance_metrics.memory_usage_mb,
                    },
                    training_differences: TrainingDifferences {
                        epochs_delta: m2.training_metadata.training_epochs as i32 - m1.training_metadata.training_epochs as i32,
                        loss_delta: m2.training_metadata.final_loss - m1.training_metadata.final_loss,
                        validation_accuracy_delta: m2.training_metadata.validation_accuracy - m1.training_metadata.validation_accuracy,
                    },
                    size_delta_bytes: (m2.architecture.hidden_layers.len() * 4) as i64 - (m1.architecture.hidden_layers.len() * 4) as i64,
                };
                Ok(Some(comparison))
            }
            _ => Ok(None),
        }
    }

    /// Get storage statistics
    pub async fn get_storage_stats(&self) -> Result<StorageStats> {
        let models = self.models.lock().await;
        let quotas = self.storage_quotas.lock().await;
        
        let mut total_models = 0;
        let mut total_versions = 0;
        let mut total_size_bytes = 0u64;
        let mut model_stats = HashMap::new();
        
        for (model_id, versions) in models.iter() {
            total_models += 1;
            total_versions += versions.len();
            let model_size: u64 = versions.iter().map(|v| v.size_bytes).sum();
            total_size_bytes += model_size;
            
            model_stats.insert(model_id.clone(), ModelStats {
                version_count: versions.len(),
                total_size_bytes: model_size,
                latest_version: versions.first().map(|v| v.version.clone()).unwrap_or_default(),
                oldest_version: versions.last().map(|v| v.version.clone()).unwrap_or_default(),
            });
        }
        
        Ok(StorageStats {
            total_models,
            total_versions,
            total_size_bytes,
            total_size_gb: total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
            model_stats,
            quota_usage: quotas.clone(),
        })
    }

    // Helper methods

    async fn check_storage_quota(&self, model_id: &str, new_size_bytes: u64) -> Result<()> {
        let quotas = self.storage_quotas.lock().await;
        if let Some(quota) = quotas.get(model_id) {
            if quota.current_size_bytes + new_size_bytes > (quota.max_total_size_gb * 1024.0 * 1024.0 * 1024.0) as u64 {
                return Err(anyhow::anyhow!("Storage quota exceeded for model {}", model_id));
            }
        }
        Ok(())
    }

    async fn update_storage_quota(&self, model_id: &str, size_delta_bytes: i64) {
        let mut quotas = self.storage_quotas.lock().await;
        let quota = quotas.entry(model_id.to_string()).or_insert_with(|| StorageQuota {
            max_versions_per_model: 10,
            max_total_size_gb: 10.0,
            current_size_bytes: 0,
        });
        
        if size_delta_bytes >= 0 {
            quota.current_size_bytes += size_delta_bytes as u64;
        } else {
            quota.current_size_bytes = quota.current_size_bytes.saturating_sub((-size_delta_bytes) as u64);
        }
    }

    async fn apply_retention_policy(&self, model_id: &str, versions: &mut Vec<ModelVersion>) {
        let quotas = self.storage_quotas.lock().await;
        if let Some(quota) = quotas.get(model_id) {
            if versions.len() > quota.max_versions_per_model {
                // Remove oldest versions beyond quota
                let excess_count = versions.len() - quota.max_versions_per_model;
                versions.truncate(versions.len() - excess_count);
            }
        }
    }

    async fn get_latest_version(&self, model_id: &str) -> Option<String> {
        let models = self.models.lock().await;
        models.get(model_id)?.first().map(|v| v.version.clone())
    }

    pub async fn set_storage_quota(&self, model_id: &str, max_versions: usize, max_size_gb: f64) {
        let mut quotas = self.storage_quotas.lock().await;
        quotas.insert(model_id.to_string(), StorageQuota {
            max_versions_per_model: max_versions,
            max_total_size_gb: max_size_gb,
            current_size_bytes: 0,
        });
    }
}

#[derive(Debug, Clone)]
pub struct ModelComparison {
    pub model_id: String,
    pub version1: String,
    pub version2: String,
    pub architecture_changed: bool,
    pub performance_delta: PerformanceDelta,
    pub training_differences: TrainingDifferences,
    pub size_delta_bytes: i64,
}

#[derive(Debug, Clone)]
pub struct PerformanceDelta {
    pub accuracy_delta: f64,
    pub latency_delta: f64,
    pub memory_delta: f64,
}

#[derive(Debug, Clone)]
pub struct TrainingDifferences {
    pub epochs_delta: i32,
    pub loss_delta: f64,
    pub validation_accuracy_delta: f64,
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_models: usize,
    pub total_versions: usize,
    pub total_size_bytes: u64,
    pub total_size_gb: f64,
    pub model_stats: HashMap<String, ModelStats>,
    pub quota_usage: HashMap<String, StorageQuota>,
}

#[derive(Debug, Clone)]
pub struct ModelStats {
    pub version_count: usize,
    pub total_size_bytes: u64,
    pub latest_version: String,
    pub oldest_version: String,
}

/// Test utilities for creating mock models
impl MockNeuralModel {
    pub fn new_test_model(id: &str, version: &str) -> Self {
        Self {
            id: id.to_string(),
            name: format!("Test Model {}", id),
            model_type: "MLP".to_string(),
            version: version.to_string(),
            architecture: ModelArchitecture {
                input_size: 10,
                hidden_layers: vec![20, 15, 10],
                output_size: 1,
                activation_function: "ReLU".to_string(),
                learning_rate: 0.001,
            },
            weights: vec![0.1; 100], // Mock weights
            training_metadata: TrainingMetadata {
                dataset_version: "v1.0".to_string(),
                training_epochs: 100,
                batch_size: 32,
                loss_function: "MSE".to_string(),
                optimizer: "Adam".to_string(),
                final_loss: 0.01,
                validation_accuracy: 0.95,
                training_duration_seconds: 3600,
            },
            performance_metrics: PerformanceMetrics {
                accuracy: 0.94,
                precision: 0.93,
                recall: 0.95,
                f1_score: 0.94,
                inference_latency_ms: 2.5,
                throughput_ops_per_sec: 400.0,
                memory_usage_mb: 256.0,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn with_performance(mut self, accuracy: f64, latency_ms: f64, memory_mb: f64) -> Self {
        self.performance_metrics.accuracy = accuracy;
        self.performance_metrics.inference_latency_ms = latency_ms;
        self.performance_metrics.memory_usage_mb = memory_mb;
        self
    }

    pub fn with_training_metadata(mut self, epochs: u32, final_loss: f64, val_accuracy: f64) -> Self {
        self.training_metadata.training_epochs = epochs;
        self.training_metadata.final_loss = final_loss;
        self.training_metadata.validation_accuracy = val_accuracy;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_store_and_retrieve_model() {
        let storage = ModelStorage::new();
        let model = MockNeuralModel::new_test_model("test_model_1", "v1.0");
        
        // Store model
        let stored_version = storage.store_model(model.clone(), vec!["test".to_string()]).await.unwrap();
        assert_eq!(stored_version.version, "v1.0");
        assert!(!stored_version.checksum.is_empty());
        assert!(stored_version.size_bytes > 0);

        // Retrieve model
        let retrieved = storage.get_model("test_model_1", Some("v1.0")).await.unwrap().unwrap();
        assert_eq!(retrieved.id, model.id);
        assert_eq!(retrieved.version, model.version);
        assert_eq!(retrieved.architecture, model.architecture);
    }

    #[tokio::test]
    async fn test_model_versioning() {
        let storage = ModelStorage::new();
        let model_id = "versioned_model";
        
        // Store multiple versions
        for i in 1..=5 {
            let version = format!("v1.{}", i);
            let model = MockNeuralModel::new_test_model(model_id, &version)
                .with_performance(0.9 + (i as f64 * 0.01), 2.0 + (i as f64), 256.0);
            
            storage.store_model(model, vec![format!("version_{}", i)]).await.unwrap();
        }

        // List all versions
        let versions = storage.list_model_versions(model_id).await.unwrap();
        assert_eq!(versions.len(), 5);

        // Versions should be sorted by creation date (newest first)
        assert_eq!(versions[0].version, "v1.5");
        assert_eq!(versions[4].version, "v1.1");

        // Get specific version
        let specific = storage.get_model(model_id, Some("v1.3")).await.unwrap().unwrap();
        assert_eq!(specific.version, "v1.3");
        assert_eq!(specific.performance_metrics.accuracy, 0.93);

        // Get latest version (should be v1.5)
        let latest = storage.get_model(model_id, None).await.unwrap().unwrap();
        assert_eq!(latest.version, "v1.5");
    }

    #[tokio::test]
    async fn test_active_model_management() {
        let storage = ModelStorage::new();
        let model_id = "active_model_test";

        // Store two versions
        let model_v1 = MockNeuralModel::new_test_model(model_id, "v1.0");
        let model_v2 = MockNeuralModel::new_test_model(model_id, "v2.0");

        storage.store_model(model_v1, vec!["stable".to_string()]).await.unwrap();
        storage.store_model(model_v2, vec!["latest".to_string()]).await.unwrap();

        // Set v1.0 as active
        storage.set_active_version(model_id, "v1.0").await.unwrap();
        let active = storage.get_active_model(model_id).await.unwrap().unwrap();
        assert_eq!(active.version, "v1.0");

        // Change active to v2.0
        storage.set_active_version(model_id, "v2.0").await.unwrap();
        let active = storage.get_active_model(model_id).await.unwrap().unwrap();
        assert_eq!(active.version, "v2.0");

        // Test setting non-existent version as active
        let result = storage.set_active_version(model_id, "v3.0").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_model_comparison() {
        let storage = ModelStorage::new();
        let model_id = "comparison_test";

        // Store two models with different performance
        let model_v1 = MockNeuralModel::new_test_model(model_id, "v1.0")
            .with_performance(0.90, 5.0, 256.0)
            .with_training_metadata(50, 0.05, 0.89);

        let model_v2 = MockNeuralModel::new_test_model(model_id, "v2.0")
            .with_performance(0.95, 3.0, 300.0)
            .with_training_metadata(100, 0.02, 0.94);

        storage.store_model(model_v1, vec![]).await.unwrap();
        storage.store_model(model_v2, vec![]).await.unwrap();

        // Compare models
        let comparison = storage.compare_models(model_id, "v1.0", "v2.0").await.unwrap().unwrap();
        
        assert_eq!(comparison.version1, "v1.0");
        assert_eq!(comparison.version2, "v2.0");
        assert_eq!(comparison.performance_delta.accuracy_delta, 0.05);
        assert_eq!(comparison.performance_delta.latency_delta, -2.0); // Improved (reduced)
        assert_eq!(comparison.performance_delta.memory_delta, 44.0); // Increased
        assert_eq!(comparison.training_differences.epochs_delta, 50);
        assert_eq!(comparison.training_differences.loss_delta, -0.03); // Improved (reduced)
    }

    #[tokio::test]
    async fn test_model_deletion() {
        let storage = ModelStorage::new();
        let model_id = "deletion_test";

        // Store model
        let model = MockNeuralModel::new_test_model(model_id, "v1.0");
        storage.store_model(model, vec![]).await.unwrap();

        // Verify it exists
        let retrieved = storage.get_model(model_id, Some("v1.0")).await.unwrap();
        assert!(retrieved.is_some());

        // Set as active
        storage.set_active_version(model_id, "v1.0").await.unwrap();
        let active = storage.get_active_model(model_id).await.unwrap();
        assert!(active.is_some());

        // Delete the version
        let deleted = storage.delete_model_version(model_id, "v1.0").await.unwrap();
        assert!(deleted);

        // Verify it's gone
        let retrieved = storage.get_model(model_id, Some("v1.0")).await.unwrap();
        assert!(retrieved.is_none());

        // Active model should also be cleared
        let active = storage.get_active_model(model_id).await.unwrap();
        assert!(active.is_none());

        // Deleting non-existent version should return false
        let deleted = storage.delete_model_version(model_id, "v2.0").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_storage_quotas() {
        let storage = ModelStorage::new();
        let model_id = "quota_test";

        // Set strict quota
        storage.set_storage_quota(model_id, 2, 0.001).await; // Max 2 versions, 1MB total

        // Store first model (should work)
        let model1 = MockNeuralModel::new_test_model(model_id, "v1.0");
        storage.store_model(model1, vec![]).await.unwrap();

        // Store second model (should work)
        let model2 = MockNeuralModel::new_test_model(model_id, "v2.0");
        storage.store_model(model2, vec![]).await.unwrap();

        // Verify quota enforcement by checking versions
        let versions = storage.list_model_versions(model_id).await.unwrap();
        assert!(versions.len() <= 2);

        // Check storage stats
        let stats = storage.get_storage_stats().await.unwrap();
        assert!(stats.model_stats.contains_key(model_id));
        assert!(stats.total_size_gb > 0.0);
    }

    #[tokio::test]
    async fn test_model_serialization_integrity() {
        let storage = ModelStorage::new();
        let model_id = "serialization_test";

        // Create model with complex data
        let mut model = MockNeuralModel::new_test_model(model_id, "v1.0");
        model.weights = vec![1.5, -2.3, 0.0, f32::MAX, f32::MIN]; // Test edge cases
        model.architecture.hidden_layers = vec![1000, 500, 250, 100];

        // Store and retrieve
        storage.store_model(model.clone(), vec!["integrity_test".to_string()]).await.unwrap();
        let retrieved = storage.get_model(model_id, Some("v1.0")).await.unwrap().unwrap();

        // Verify all data is identical
        assert_eq!(retrieved.weights, model.weights);
        assert_eq!(retrieved.architecture.hidden_layers, model.architecture.hidden_layers);
        assert_eq!(retrieved.training_metadata.final_loss, model.training_metadata.final_loss);
        assert_eq!(retrieved.performance_metrics.accuracy, model.performance_metrics.accuracy);
    }

    #[tokio::test]
    async fn test_concurrent_model_operations() {
        let storage = std::sync::Arc::new(ModelStorage::new());
        let model_id = "concurrent_test";

        // Spawn multiple tasks to store models concurrently
        let handles: Vec<_> = (1..=10).map(|i| {
            let storage_clone = storage.clone();
            let model_id = model_id.to_string();
            tokio::spawn(async move {
                let model = MockNeuralModel::new_test_model(&model_id, &format!("v1.{}", i))
                    .with_performance(0.8 + (i as f64 * 0.01), i as f64, 200.0 + i as f64);
                storage_clone.store_model(model, vec![format!("concurrent_{}", i)]).await
            })
        }).collect();

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }

        // Verify all versions were stored
        let versions = storage.list_model_versions(model_id).await.unwrap();
        assert_eq!(versions.len(), 10);

        // Test concurrent reads
        let read_handles: Vec<_> = (1..=10).map(|i| {
            let storage_clone = storage.clone();
            let model_id = model_id.to_string();
            tokio::spawn(async move {
                storage_clone.get_model(&model_id, Some(&format!("v1.{}", i))).await
            })
        }).collect();

        for handle in read_handles {
            let result = handle.await.unwrap().unwrap();
            assert!(result.is_some());
        }
    }

    #[tokio::test]
    async fn test_storage_performance() {
        let storage = ModelStorage::new();
        let model_id = "performance_test";

        // Test store performance
        let model = MockNeuralModel::new_test_model(model_id, "v1.0");
        
        let start = Instant::now();
        storage.store_model(model, vec![]).await.unwrap();
        let store_duration = start.elapsed();

        // Store operations should complete within reasonable time
        assert!(store_duration < Duration::from_millis(100), 
               "Store took {}ms, should be <100ms", store_duration.as_millis());

        // Test retrieve performance
        let start = Instant::now();
        let _retrieved = storage.get_model(model_id, Some("v1.0")).await.unwrap();
        let retrieve_duration = start.elapsed();

        // Retrieve operations should be very fast
        assert!(retrieve_duration < Duration::from_millis(10), 
               "Retrieve took {}ms, should be <10ms", retrieve_duration.as_millis());

        // Test list performance with multiple versions
        for i in 2..=50 {
            let model = MockNeuralModel::new_test_model(model_id, &format!("v1.{}", i));
            storage.store_model(model, vec![]).await.unwrap();
        }

        let start = Instant::now();
        let versions = storage.list_model_versions(model_id).await.unwrap();
        let list_duration = start.elapsed();

        assert_eq!(versions.len(), 50);
        assert!(list_duration < Duration::from_millis(50), 
               "List took {}ms for 50 versions, should be <50ms", list_duration.as_millis());
    }

    #[tokio::test]
    async fn test_model_metadata_integrity() {
        let storage = ModelStorage::new();
        let model_id = "metadata_test";

        let model = MockNeuralModel::new_test_model(model_id, "v1.0");
        let original_created_at = model.created_at;

        // Store model
        let version_info = storage.store_model(model.clone(), vec!["metadata".to_string()]).await.unwrap();
        
        // Verify checksum is generated
        assert!(!version_info.checksum.is_empty());
        assert!(version_info.size_bytes > 0);
        assert_eq!(version_info.status, ModelStatus::Training);

        // Set as active and verify status tracking works
        storage.set_active_version(model_id, "v1.0").await.unwrap();
        let active = storage.get_active_model(model_id).await.unwrap().unwrap();
        assert_eq!(active.created_at, original_created_at);

        // Verify storage stats include metadata
        let stats = storage.get_storage_stats().await.unwrap();
        let model_stats = stats.model_stats.get(model_id).unwrap();
        assert_eq!(model_stats.version_count, 1);
        assert_eq!(model_stats.latest_version, "v1.0");
        assert_eq!(model_stats.oldest_version, "v1.0");
        assert!(model_stats.total_size_bytes > 0);
    }

    #[tokio::test]
    async fn test_duplicate_version_prevention() {
        let storage = ModelStorage::new();
        let model_id = "duplicate_test";

        // Store original model
        let model = MockNeuralModel::new_test_model(model_id, "v1.0");
        storage.store_model(model.clone(), vec![]).await.unwrap();

        // Try to store same version again (should fail)
        let result = storage.store_model(model, vec![]).await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("already exists"));

        // Verify only one version exists
        let versions = storage.list_model_versions(model_id).await.unwrap();
        assert_eq!(versions.len(), 1);
    }
}