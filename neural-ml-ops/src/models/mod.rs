//! Model Registry Module
//!
//! Domain-agnostic model storage, versioning, and management system extracted
//! from trading-specific model storage code.

pub mod registry;
pub mod storage;

pub use registry::{ModelRegistry, ModelMetadata};
pub use storage::{ModelStorage, ModelVersion, VersionIncrement};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Model information structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub model_type: ModelType,
    pub status: ModelStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: Option<String>,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub metrics: ModelMetrics,
    pub artifacts: HashMap<String, ArtifactInfo>,
}

/// Types of ML models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelType {
    NeuralNetwork,
    RandomForest,
    SVM,
    LinearRegression,
    LogisticRegression,
    DecisionTree,
    XGBoost,
    LightGBM,
    Transformer,
    LSTM,
    CNN,
    Custom(String),
}

/// Model deployment status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelStatus {
    Draft,
    Training,
    Trained,
    Validated,
    Deployed,
    Deprecated,
    Archived,
    Failed,
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub accuracy: Option<f64>,
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub f1_score: Option<f64>,
    pub auc_roc: Option<f64>,
    pub loss: Option<f64>,
    pub mae: Option<f64>,
    pub mse: Option<f64>,
    pub rmse: Option<f64>,
    pub r_squared: Option<f64>,
    pub custom_metrics: HashMap<String, f64>,
}

impl Default for ModelMetrics {
    fn default() -> Self {
        Self {
            accuracy: None,
            precision: None,
            recall: None,
            f1_score: None,
            auc_roc: None,
            loss: None,
            mae: None,
            mse: None,
            rmse: None,
            r_squared: None,
            custom_metrics: HashMap::new(),
        }
    }
}

/// Model artifact information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactInfo {
    pub artifact_type: ArtifactType,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub checksum: String,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    Model,          // Trained model file
    Config,         // Model configuration
    Weights,        // Model weights/parameters
    Metadata,       // Model metadata
    Tokenizer,      // Tokenizer for NLP models
    Schema,         // Input/output schema
    Documentation,  // Model documentation
    Visualization,  // Model visualization
    Custom(String), // Custom artifact type
}

/// Model search criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSearchCriteria {
    pub name_pattern: Option<String>,
    pub model_type: Option<ModelType>,
    pub status: Option<ModelStatus>,
    pub tags: Vec<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub min_accuracy: Option<f64>,
    pub max_loss: Option<f64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for ModelSearchCriteria {
    fn default() -> Self {
        Self {
            name_pattern: None,
            model_type: None,
            status: None,
            tags: Vec::new(),
            created_after: None,
            created_before: None,
            min_accuracy: None,
            max_loss: None,
            limit: Some(50),
            offset: None,
        }
    }
}

/// Model registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRegistryConfig {
    pub storage_path: PathBuf,
    pub max_versions_per_model: usize,
    pub enable_compression: bool,
    pub enable_encryption: bool,
    pub backup_enabled: bool,
    pub backup_interval_hours: u32,
    pub cleanup_interval_hours: u32,
}

impl Default for ModelRegistryConfig {
    fn default() -> Self {
        Self {
            storage_path: PathBuf::from("./models"),
            max_versions_per_model: 10,
            enable_compression: true,
            enable_encryption: false,
            backup_enabled: true,
            backup_interval_hours: 24,
            cleanup_interval_hours: 168, // 1 week
        }
    }
}

/// Model deployment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDeploymentConfig {
    pub deployment_id: String,
    pub model_id: String,
    pub model_version: String,
    pub environment: DeploymentEnvironment,
    pub resources: ResourceRequirements,
    pub auto_scaling: AutoScalingConfig,
    pub health_check: HealthCheckConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentEnvironment {
    Development,
    Staging,
    Production,
    Custom(String),
}

/// Resource requirements for model deployment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: f32,
    pub memory_gb: f32,
    pub gpu_count: u32,
    pub disk_gb: f32,
    pub max_request_size_mb: u32,
    pub timeout_seconds: u32,
}

impl Default for ResourceRequirements {
    fn default() -> Self {
        Self {
            cpu_cores: 1.0,
            memory_gb: 2.0,
            gpu_count: 0,
            disk_gb: 10.0,
            max_request_size_mb: 100,
            timeout_seconds: 30,
        }
    }
}

/// Auto-scaling configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoScalingConfig {
    pub enabled: bool,
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub target_cpu_utilization: f32,
    pub target_memory_utilization: f32,
    pub scale_up_cooldown_seconds: u32,
    pub scale_down_cooldown_seconds: u32,
}

impl Default for AutoScalingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            min_replicas: 1,
            max_replicas: 10,
            target_cpu_utilization: 70.0,
            target_memory_utilization: 80.0,
            scale_up_cooldown_seconds: 300,
            scale_down_cooldown_seconds: 300,
        }
    }
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    pub enabled: bool,
    pub path: String,
    pub interval_seconds: u32,
    pub timeout_seconds: u32,
    pub healthy_threshold: u32,
    pub unhealthy_threshold: u32,
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            path: "/health".to_string(),
            interval_seconds: 30,
            timeout_seconds: 5,
            healthy_threshold: 2,
            unhealthy_threshold: 3,
        }
    }
}

/// Model comparison result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelComparison {
    pub baseline_model: ModelInfo,
    pub candidate_model: ModelInfo,
    pub metric_differences: HashMap<String, f64>,
    pub improvement_percentage: f64,
    pub statistical_significance: f64,
    pub recommendation: ComparisonRecommendation,
    pub comparison_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonRecommendation {
    PromoteCandidate,
    KeepBaseline,
    RequireMoreData,
    InconclusivedResults,
}

/// Model lineage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLineage {
    pub model_id: String,
    pub parent_models: Vec<String>,
    pub child_models: Vec<String>,
    pub training_job_id: Option<Uuid>,
    pub dataset_versions: Vec<String>,
    pub feature_versions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Model access control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAccessControl {
    pub model_id: String,
    pub owner: String,
    pub permissions: HashMap<String, Vec<Permission>>, // user/role -> permissions
    pub public_access: bool,
    pub access_log: Vec<AccessLogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Permission {
    Read,
    Write,
    Delete,
    Deploy,
    Share,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessLogEntry {
    pub user: String,
    pub action: String,
    pub timestamp: DateTime<Utc>,
    pub success: bool,
    pub details: Option<String>,
}

/// Model trait for different model types
#[async_trait::async_trait]
pub trait Model: Send + Sync {
    /// Get model information
    fn get_info(&self) -> &ModelInfo;
    
    /// Predict using the model
    async fn predict(&self, input: &[f64]) -> Result<Vec<f64>>;
    
    /// Get model size in bytes
    fn get_size(&self) -> usize;
    
    /// Validate model integrity
    async fn validate(&self) -> Result<bool>;
    
    /// Export model to specified format
    async fn export(&self, path: &PathBuf, format: ExportFormat) -> Result<()>;
    
    /// Get model artifacts
    fn get_artifacts(&self) -> &HashMap<String, ArtifactInfo>;
}

/// Model export formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Native,     // Original format
    ONNX,       // Open Neural Network Exchange
    TensorFlow,
    PyTorch,
    JSON,       // JSON representation
    Binary,     // Binary format
}

/// Model registry trait for different storage backends
#[async_trait::async_trait]
pub trait ModelRegistryTrait: Send + Sync {
    /// Register a new model
    async fn register_model(&self, model_info: ModelInfo) -> Result<String>;
    
    /// Update existing model
    async fn update_model(&self, model_info: ModelInfo) -> Result<()>;
    
    /// Get model by ID
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>>;
    
    /// List models matching criteria
    async fn list_models(&self, criteria: Option<ModelSearchCriteria>) -> Result<Vec<ModelInfo>>;
    
    /// Delete model
    async fn delete_model(&self, model_id: &str) -> Result<()>;
    
    /// Add model version
    async fn add_version(&self, model_id: &str, version_info: ModelVersion) -> Result<()>;
    
    /// Get model versions
    async fn get_versions(&self, model_id: &str) -> Result<Vec<ModelVersion>>;
    
    /// Store model artifact
    async fn store_artifact(
        &self,
        model_id: &str,
        artifact_type: ArtifactType,
        data: &[u8],
    ) -> Result<String>;
    
    /// Retrieve model artifact
    async fn retrieve_artifact(&self, model_id: &str, artifact_id: &str) -> Result<Vec<u8>>;
    
    /// Get registry statistics
    async fn get_stats(&self) -> Result<RegistryStats>;
}

/// Registry statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryStats {
    pub total_models: usize,
    pub models_by_type: HashMap<String, usize>,
    pub models_by_status: HashMap<String, usize>,
    pub total_size_bytes: u64,
    pub average_model_size_bytes: f64,
    pub most_recent_model: Option<DateTime<Utc>>,
    pub oldest_model: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_model_info_serialization() {
        let model_info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            version: "1.0.0".to_string(),
            model_type: ModelType::NeuralNetwork,
            status: ModelStatus::Trained,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: Some("test-user".to_string()),
            description: Some("Test model description".to_string()),
            tags: vec!["test".to_string(), "neural".to_string()],
            metrics: ModelMetrics::default(),
            artifacts: HashMap::new(),
        };
        
        let json = serde_json::to_string(&model_info).unwrap();
        let deserialized: ModelInfo = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, model_info.id);
        assert_eq!(deserialized.name, model_info.name);
    }
    
    #[test]
    fn test_model_search_criteria_default() {
        let criteria = ModelSearchCriteria::default();
        assert_eq!(criteria.limit, Some(50));
        assert!(criteria.tags.is_empty());
    }
}