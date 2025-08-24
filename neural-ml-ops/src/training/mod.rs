//! Training Coordination Module
//!
//! Domain-agnostic training coordination and scheduling system extracted
//! from trading-specific training coordinator.

pub mod coordinator;
pub mod scheduler;
pub mod metrics;

pub use coordinator::TrainingCoordinator;
pub use scheduler::{TrainingScheduler, ScheduledTask};
pub use metrics::{TrainingMetrics, MetricsCollector};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Maximum concurrent training jobs
    pub max_concurrent_jobs: usize,
    /// Default timeout for training jobs (seconds)
    pub default_timeout_secs: u64,
    /// Checkpoint frequency (epochs)
    pub checkpoint_frequency: u32,
    /// Enable automatic model validation
    pub enable_validation: bool,
    /// Base directory for training outputs
    pub output_dir: PathBuf,
    /// Training scheduler configuration
    pub scheduler: scheduler::SchedulerConfig,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 4,
            default_timeout_secs: 3600, // 1 hour
            checkpoint_frequency: 100,
            enable_validation: true,
            output_dir: PathBuf::from("./models"),
            scheduler: scheduler::SchedulerConfig::default(),
        }
    }
}

/// Training workflow configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub timeout_secs: u64,
    pub retry_count: u32,
}

/// Individual workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub step_type: StepType,
    pub parameters: HashMap<String, serde_json::Value>,
    pub depends_on: Vec<String>,
}

/// Types of workflow steps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepType {
    DataPreparation,
    FeatureEngineering,
    ModelTraining {
        model_type: String,
        hyperparameters: HashMap<String, serde_json::Value>,
    },
    ModelValidation,
    ModelSaving,
    Custom {
        command: String,
        args: Vec<String>,
    },
}

/// Training job status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingStatus {
    pub job_id: Uuid,
    pub workflow_id: String,
    pub status: JobStatus,
    pub progress: f64, // 0.0 to 1.0
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub current_step: Option<String>,
    pub metrics: Option<TrainingMetrics>,
    pub error: Option<String>,
}

/// Job execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Training execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingResult {
    pub job_id: Uuid,
    pub success: bool,
    pub metrics: TrainingMetrics,
    pub model_path: Option<PathBuf>,
    pub artifacts: HashMap<String, PathBuf>,
    pub duration_secs: u64,
}

/// Data preparation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    pub input_format: DataFormat,
    pub validation_split: f64,
    pub test_split: f64,
    pub shuffle: bool,
    pub normalization: Option<NormalizationConfig>,
}

/// Supported data formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    CSV { separator: char },
    JSON,
    Parquet,
    Binary,
}

/// Data normalization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    pub method: NormalizationMethod,
    pub feature_range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationMethod {
    StandardScaler,  // z-score normalization
    MinMaxScaler,    // min-max scaling
    RobustScaler,    // median and IQR
    None,
}

/// Feature engineering pipeline step
pub trait FeaturePipelineStep: Send + Sync {
    fn name(&self) -> &str;
    fn transform(&self, data: &[f64]) -> Result<Vec<f64>>;
    fn get_output_size(&self, input_size: usize) -> usize;
}

/// Model training trait for different model types
#[async_trait::async_trait]
pub trait ModelTrainer: Send + Sync {
    type Model;
    type Config;
    
    /// Train a model with the given configuration and data
    async fn train(
        &self,
        config: &Self::Config,
        training_data: &[f64],
        validation_data: &[f64],
        metrics_callback: Option<Box<dyn Fn(TrainingMetrics) + Send + Sync>>,
    ) -> Result<Self::Model>;
    
    /// Validate a trained model
    async fn validate(&self, model: &Self::Model, test_data: &[f64]) -> Result<ValidationResult>;
    
    /// Save model to storage
    async fn save(&self, model: &Self::Model, path: &PathBuf) -> Result<()>;
    
    /// Load model from storage
    async fn load(&self, path: &PathBuf) -> Result<Self::Model>;
}

/// Model validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub accuracy: f64,
    pub loss: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub confusion_matrix: Option<Vec<Vec<u32>>>,
    pub predictions: Vec<f64>,
    pub actuals: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_training_config_default() {
        let config = TrainingConfig::default();
        assert_eq!(config.max_concurrent_jobs, 4);
        assert!(config.enable_validation);
    }
    
    #[test]
    fn test_workflow_serialization() {
        let workflow = WorkflowConfig {
            id: "test-workflow".to_string(),
            name: "Test Workflow".to_string(),
            description: "Test workflow description".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "data-prep".to_string(),
                    step_type: StepType::DataPreparation,
                    parameters: HashMap::new(),
                    depends_on: vec![],
                },
                WorkflowStep {
                    name: "training".to_string(),
                    step_type: StepType::ModelTraining {
                        model_type: "neural".to_string(),
                        hyperparameters: HashMap::new(),
                    },
                    parameters: HashMap::new(),
                    depends_on: vec!["data-prep".to_string()],
                },
            ],
            timeout_secs: 3600,
            retry_count: 3,
        };
        
        let json = serde_json::to_string(&workflow).unwrap();
        let deserialized: WorkflowConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.id, workflow.id);
        assert_eq!(deserialized.steps.len(), 2);
    }
}