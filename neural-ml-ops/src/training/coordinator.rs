//! Training Coordinator Implementation
//!
//! Extracted and refactored from trading-specific training coordinator to be domain agnostic.
//! Manages training workflows, job scheduling, and resource coordination.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{timeout, Duration as TokioDuration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{
    JobStatus, ScheduledTask, TrainingConfig, TrainingMetrics, TrainingResult, TrainingStatus,
    ValidationResult, WorkflowConfig, WorkflowStep,
};
use crate::events::{EventPublisher, MLEvent, MLEventType};

/// Main training coordinator - orchestrates all training activities
#[derive(Clone)]
pub struct TrainingCoordinator {
    config: TrainingConfig,
    active_jobs: Arc<DashMap<Uuid, TrainingJob>>,
    workflow_configs: Arc<DashMap<String, WorkflowConfig>>,
    training_semaphore: Arc<Semaphore>,
    event_publisher: Arc<EventPublisher>,
}

/// Internal representation of a training job
#[derive(Debug, Clone)]
struct TrainingJob {
    id: Uuid,
    workflow_id: String,
    status: JobStatus,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    current_step: Option<String>,
    progress: f64,
    metrics: Option<TrainingMetrics>,
    error: Option<String>,
    output_dir: PathBuf,
}

impl TrainingCoordinator {
    /// Create a new training coordinator
    pub async fn new(config: TrainingConfig) -> Result<Self> {
        info!("Initializing Training Coordinator");
        
        // Ensure output directory exists
        tokio::fs::create_dir_all(&config.output_dir).await?;
        
        let training_semaphore = Arc::new(Semaphore::new(config.max_concurrent_jobs));
        let active_jobs = Arc::new(DashMap::new());
        let workflow_configs = Arc::new(DashMap::new());
        
        let event_publisher = Arc::new(EventPublisher::new(crate::events::EventConfig::default()).await?);
        
        let coordinator = Self {
            config,
            active_jobs,
            workflow_configs,
            training_semaphore,
            event_publisher,
        };
        
        // Load predefined workflows
        coordinator.load_default_workflows().await?;
        
        info!("Training Coordinator initialized with {} max concurrent jobs", 
              coordinator.config.max_concurrent_jobs);
        
        Ok(coordinator)
    }
    
    /// Register a workflow configuration
    pub async fn register_workflow(&self, workflow: WorkflowConfig) -> Result<()> {
        info!("Registering workflow: {} ({})", workflow.name, workflow.id);
        self.workflow_configs.insert(workflow.id.clone(), workflow);
        Ok(())
    }
    
    /// Get workflow configuration by ID
    pub fn get_workflow_config(&self, workflow_id: &str) -> Result<WorkflowConfig> {
        self.workflow_configs
            .get(workflow_id)
            .map(|entry| entry.value().clone())
            .ok_or_else(|| anyhow!("Workflow not found: {}", workflow_id))
    }
    
    /// List available workflows
    pub async fn list_workflows(&self) -> Result<Vec<String>> {
        let workflows: Vec<String> = self.workflow_configs
            .iter()
            .map(|entry| entry.key().clone())
            .collect();
        Ok(workflows)
    }
    
    /// Start a training workflow
    pub async fn start_workflow(
        &self,
        workflow_id: &str,
        parameters: serde_json::Value,
    ) -> Result<String> {
        let workflow = self.get_workflow_config(workflow_id)?;
        let job_id = Uuid::new_v4();
        
        info!("Starting training workflow: {} (job: {})", workflow_id, job_id);
        
        // Create job directory
        let job_dir = self.config.output_dir.join(job_id.to_string());
        tokio::fs::create_dir_all(&job_dir).await?;
        
        // Create training job
        let training_job = TrainingJob {
            id: job_id,
            workflow_id: workflow_id.to_string(),
            status: JobStatus::Queued,
            started_at: Utc::now(),
            completed_at: None,
            current_step: None,
            progress: 0.0,
            metrics: None,
            error: None,
            output_dir: job_dir,
        };
        
        // Store job
        self.active_jobs.insert(job_id, training_job);
        
        // Publish event
        self.event_publisher.publish(MLEvent {
            id: Uuid::new_v4(),
            event_type: MLEventType::TrainingStarted,
            job_id: Some(job_id),
            workflow_id: Some(workflow_id.to_string()),
            timestamp: Utc::now(),
            payload: parameters.clone(),
        }).await?;
        
        // Start execution asynchronously
        let coordinator_clone = self.clone();
        let workflow_clone = workflow.clone();
        tokio::spawn(async move {
            if let Err(e) = coordinator_clone.execute_workflow_async(job_id, workflow_clone, parameters).await {
                error!("Workflow execution failed for job {}: {}", job_id, e);
            }
        });
        
        Ok(job_id.to_string())
    }
    
    /// Execute workflow with input/output paths (for CLI)
    pub async fn execute_workflow(
        &self,
        workflow: &WorkflowConfig,
        input_path: &Path,
        output_path: &Path,
    ) -> Result<TrainingResult> {
        let job_id = Uuid::new_v4();
        let parameters = serde_json::json!({
            "input_path": input_path,
            "output_path": output_path
        });
        
        info!("Executing workflow: {} (job: {})", workflow.id, job_id);
        
        // Create training job
        let training_job = TrainingJob {
            id: job_id,
            workflow_id: workflow.id.clone(),
            status: JobStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            current_step: None,
            progress: 0.0,
            metrics: None,
            error: None,
            output_dir: output_path.to_path_buf(),
        };
        
        self.active_jobs.insert(job_id, training_job);
        
        // Execute workflow steps
        match self.execute_workflow_steps(job_id, workflow, &parameters).await {
            Ok(result) => {
                self.update_job_status(job_id, JobStatus::Completed, None).await;
                Ok(result)
            }
            Err(e) => {
                self.update_job_status(job_id, JobStatus::Failed, Some(e.to_string())).await;
                Err(e)
            }
        }
    }
    
    /// Get training job status
    pub async fn get_status(&self, job_id: &str) -> Result<TrainingStatus> {
        let job_uuid = Uuid::parse_str(job_id)?;
        
        if let Some(job_ref) = self.active_jobs.get(&job_uuid) {
            let job = job_ref.value();
            Ok(TrainingStatus {
                job_id: job.id,
                workflow_id: job.workflow_id.clone(),
                status: job.status.clone(),
                progress: job.progress,
                started_at: job.started_at,
                completed_at: job.completed_at,
                current_step: job.current_step.clone(),
                metrics: job.metrics.clone(),
                error: job.error.clone(),
            })
        } else {
            Err(anyhow!("Job not found: {}", job_id))
        }
    }
    
    /// Cancel a training job
    pub async fn cancel_job(&self, job_id: &str) -> Result<()> {
        let job_uuid = Uuid::parse_str(job_id)?;
        
        if let Some(mut job_ref) = self.active_jobs.get_mut(&job_uuid) {
            let job = job_ref.value_mut();
            if matches!(job.status, JobStatus::Queued | JobStatus::Running) {
                job.status = JobStatus::Cancelled;
                job.completed_at = Some(Utc::now());
                
                // Publish cancellation event
                self.event_publisher.publish(MLEvent {
                    id: Uuid::new_v4(),
                    event_type: MLEventType::TrainingCancelled,
                    job_id: Some(job_uuid),
                    workflow_id: Some(job.workflow_id.clone()),
                    timestamp: Utc::now(),
                    payload: serde_json::json!({}),
                }).await?;
                
                info!("Job {} cancelled", job_id);
                Ok(())
            } else {
                Err(anyhow!("Cannot cancel job in status: {:?}", job.status))
            }
        } else {
            Err(anyhow!("Job not found: {}", job_id))
        }
    }
    
    /// List active jobs
    pub async fn list_active_jobs(&self) -> Vec<TrainingStatus> {
        self.active_jobs
            .iter()
            .map(|entry| {
                let job = entry.value();
                TrainingStatus {
                    job_id: job.id,
                    workflow_id: job.workflow_id.clone(),
                    status: job.status.clone(),
                    progress: job.progress,
                    started_at: job.started_at,
                    completed_at: job.completed_at,
                    current_step: job.current_step.clone(),
                    metrics: job.metrics.clone(),
                    error: job.error.clone(),
                }
            })
            .collect()
    }
    
    /// Clean up completed jobs older than specified duration
    pub async fn cleanup_completed_jobs(&self, older_than: Duration) -> Result<usize> {
        let cutoff = Utc::now() - older_than;
        let mut removed_count = 0;
        
        let jobs_to_remove: Vec<Uuid> = self.active_jobs
            .iter()
            .filter_map(|entry| {
                let job = entry.value();
                if matches!(job.status, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled) {
                    if let Some(completed_at) = job.completed_at {
                        if completed_at < cutoff {
                            return Some(job.id);
                        }
                    }
                }
                None
            })
            .collect();
        
        for job_id in jobs_to_remove {
            self.active_jobs.remove(&job_id);
            removed_count += 1;
        }
        
        if removed_count > 0 {
            info!("Cleaned up {} completed jobs", removed_count);
        }
        
        Ok(removed_count)
    }
    
    // Private methods
    
    async fn execute_workflow_async(
        &self,
        job_id: Uuid,
        workflow: WorkflowConfig,
        parameters: serde_json::Value,
    ) -> Result<()> {
        // Acquire semaphore permit
        let _permit = self.training_semaphore.acquire().await
            .map_err(|e| anyhow!("Failed to acquire training permit: {}", e))?;
        
        // Update job status
        self.update_job_status(job_id, JobStatus::Running, None).await;
        
        // Execute with timeout
        let timeout_duration = TokioDuration::from_secs(workflow.timeout_secs);
        
        match timeout(timeout_duration, self.execute_workflow_steps(job_id, &workflow, &parameters)).await {
            Ok(Ok(result)) => {
                self.update_job_status(job_id, JobStatus::Completed, None).await;
                
                // Publish completion event
                self.event_publisher.publish(MLEvent {
                    id: Uuid::new_v4(),
                    event_type: MLEventType::TrainingCompleted,
                    job_id: Some(job_id),
                    workflow_id: Some(workflow.id),
                    timestamp: Utc::now(),
                    payload: serde_json::to_value(&result)?,
                }).await?;
                
                info!("Workflow completed successfully: {}", job_id);
            }
            Ok(Err(e)) => {
                self.update_job_status(job_id, JobStatus::Failed, Some(e.to_string())).await;
                error!("Workflow failed: {} - {}", job_id, e);
            }
            Err(_) => {
                self.update_job_status(job_id, JobStatus::Failed, Some("Timeout".to_string())).await;
                error!("Workflow timed out: {}", job_id);
            }
        }
        
        Ok(())
    }
    
    async fn execute_workflow_steps(
        &self,
        job_id: Uuid,
        workflow: &WorkflowConfig,
        parameters: &serde_json::Value,
    ) -> Result<TrainingResult> {
        let start_time = Utc::now();
        let total_steps = workflow.steps.len() as f64;
        let mut artifacts = HashMap::new();
        
        info!("Executing workflow steps for job {}: {} steps", job_id, total_steps);
        
        // Execute steps in dependency order
        let execution_order = self.resolve_step_dependencies(&workflow.steps)?;
        
        for (step_index, step) in execution_order.iter().enumerate() {
            let progress = (step_index as f64) / total_steps;
            
            // Update current step and progress
            self.update_job_progress(job_id, &step.name, progress).await;
            
            info!("Executing step: {} (job: {})", step.name, job_id);
            
            // Execute the step
            let step_result = self.execute_step(job_id, step, parameters, &artifacts).await?;
            
            // Store artifacts
            if let Some(artifact_path) = step_result.artifact_path {
                artifacts.insert(step.name.clone(), artifact_path);
            }
            
            // Update metrics if provided
            if let Some(step_metrics) = step_result.metrics {
                self.update_job_metrics(job_id, step_metrics).await;
            }
        }
        
        // Final progress update
        self.update_job_progress(job_id, "completed", 1.0).await;
        
        let duration = (Utc::now() - start_time).num_seconds() as u64;
        let final_metrics = self.get_job_metrics(job_id).await.unwrap_or_default();
        
        Ok(TrainingResult {
            job_id,
            success: true,
            metrics: final_metrics,
            model_path: artifacts.get("model").cloned(),
            artifacts,
            duration_secs: duration,
        })
    }
    
    async fn execute_step(
        &self,
        job_id: Uuid,
        step: &WorkflowStep,
        parameters: &serde_json::Value,
        artifacts: &HashMap<String, PathBuf>,
    ) -> Result<StepResult> {
        debug!("Executing step: {} for job {}", step.name, job_id);
        
        match &step.step_type {
            super::StepType::DataPreparation => {
                self.execute_data_preparation_step(job_id, step, parameters).await
            }
            super::StepType::FeatureEngineering => {
                self.execute_feature_engineering_step(job_id, step, parameters, artifacts).await
            }
            super::StepType::ModelTraining { model_type, hyperparameters } => {
                self.execute_model_training_step(job_id, step, model_type, hyperparameters, artifacts).await
            }
            super::StepType::ModelValidation => {
                self.execute_model_validation_step(job_id, step, artifacts).await
            }
            super::StepType::ModelSaving => {
                self.execute_model_saving_step(job_id, step, artifacts).await
            }
            super::StepType::Custom { command, args } => {
                self.execute_custom_step(job_id, step, command, args).await
            }
        }
    }
    
    async fn execute_data_preparation_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        parameters: &serde_json::Value,
    ) -> Result<StepResult> {
        info!("Executing data preparation for job {}", job_id);
        
        // Simulate data preparation
        tokio::time::sleep(TokioDuration::from_secs(2)).await;
        
        // Create output path for prepared data
        let job_dir = self.get_job_output_dir(job_id)?;
        let prepared_data_path = job_dir.join("prepared_data.json");
        
        // Simulate data preparation by copying/processing input
        if let Some(input_path) = parameters.get("input_path") {
            if let Some(input_str) = input_path.as_str() {
                let input_path = PathBuf::from(input_str);
                if input_path.exists() {
                    tokio::fs::copy(&input_path, &prepared_data_path).await?;
                    info!("Data preparation completed: {:?}", prepared_data_path);
                }
            }
        }
        
        Ok(StepResult {
            success: true,
            artifact_path: Some(prepared_data_path),
            metrics: None,
        })
    }
    
    async fn execute_feature_engineering_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        _parameters: &serde_json::Value,
        artifacts: &HashMap<String, PathBuf>,
    ) -> Result<StepResult> {
        info!("Executing feature engineering for job {}", job_id);
        
        // Simulate feature engineering
        tokio::time::sleep(TokioDuration::from_secs(3)).await;
        
        let job_dir = self.get_job_output_dir(job_id)?;
        let features_path = job_dir.join("features.json");
        
        // Use prepared data if available
        if let Some(data_path) = artifacts.get("data-prep") {
            debug!("Using prepared data from: {:?}", data_path);
        }
        
        // Create dummy features file
        let features = serde_json::json!({
            "feature_count": 42,
            "feature_names": ["feature_1", "feature_2", "feature_3"],
            "samples": 1000
        });
        
        tokio::fs::write(&features_path, serde_json::to_string_pretty(&features)?).await?;
        
        Ok(StepResult {
            success: true,
            artifact_path: Some(features_path),
            metrics: None,
        })
    }
    
    async fn execute_model_training_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        model_type: &str,
        hyperparameters: &HashMap<String, serde_json::Value>,
        artifacts: &HashMap<String, PathBuf>,
    ) -> Result<StepResult> {
        info!("Executing model training for job {} with model type: {}", job_id, model_type);
        
        // Simulate model training with progress updates
        let training_duration = 10; // seconds
        for epoch in 1..=training_duration {
            tokio::time::sleep(TokioDuration::from_secs(1)).await;
            
            let progress = (epoch as f64) / (training_duration as f64);
            let metrics = TrainingMetrics {
                epoch: Some(epoch as u32),
                training_loss: Some(1.0 - progress * 0.8), // Decreasing loss
                validation_loss: Some(1.0 - progress * 0.7),
                accuracy: Some(progress * 0.95),
                learning_rate: Some(0.001),
                timestamp: Utc::now(),
            };
            
            self.update_job_metrics(job_id, metrics).await;
        }
        
        let job_dir = self.get_job_output_dir(job_id)?;
        let model_path = job_dir.join(format!("model_{}.bin", model_type));
        
        // Create dummy model file
        let model_data = serde_json::json!({
            "model_type": model_type,
            "hyperparameters": hyperparameters,
            "trained_at": Utc::now(),
            "accuracy": 0.95
        });
        
        tokio::fs::write(&model_path, serde_json::to_string_pretty(&model_data)?).await?;
        
        let final_metrics = TrainingMetrics {
            epoch: Some(training_duration),
            training_loss: Some(0.2),
            validation_loss: Some(0.25),
            accuracy: Some(0.95),
            learning_rate: Some(0.001),
            timestamp: Utc::now(),
        };
        
        Ok(StepResult {
            success: true,
            artifact_path: Some(model_path),
            metrics: Some(final_metrics),
        })
    }
    
    async fn execute_model_validation_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        artifacts: &HashMap<String, PathBuf>,
    ) -> Result<StepResult> {
        info!("Executing model validation for job {}", job_id);
        
        // Simulate validation
        tokio::time::sleep(TokioDuration::from_secs(2)).await;
        
        // Check if model exists
        if !artifacts.contains_key("model") {
            return Err(anyhow!("Model not found for validation"));
        }
        
        let validation_metrics = TrainingMetrics {
            epoch: None,
            training_loss: None,
            validation_loss: Some(0.25),
            accuracy: Some(0.93),
            learning_rate: None,
            timestamp: Utc::now(),
        };
        
        Ok(StepResult {
            success: true,
            artifact_path: None,
            metrics: Some(validation_metrics),
        })
    }
    
    async fn execute_model_saving_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        artifacts: &HashMap<String, PathBuf>,
    ) -> Result<StepResult> {
        info!("Executing model saving for job {}", job_id);
        
        if let Some(model_path) = artifacts.get("model") {
            // Model is already saved in the training step
            info!("Model already saved at: {:?}", model_path);
            
            Ok(StepResult {
                success: true,
                artifact_path: Some(model_path.clone()),
                metrics: None,
            })
        } else {
            Err(anyhow!("No model found to save"))
        }
    }
    
    async fn execute_custom_step(
        &self,
        job_id: Uuid,
        _step: &WorkflowStep,
        command: &str,
        args: &[String],
    ) -> Result<StepResult> {
        info!("Executing custom command for job {}: {} {:?}", job_id, command, args);
        
        // For safety, we'll just simulate custom step execution
        tokio::time::sleep(TokioDuration::from_secs(1)).await;
        
        warn!("Custom step execution is simulated for security reasons");
        
        Ok(StepResult {
            success: true,
            artifact_path: None,
            metrics: None,
        })
    }
    
    fn resolve_step_dependencies<'a>(&self, steps: &'a [WorkflowStep]) -> Result<Vec<&'a WorkflowStep>> {
        let mut execution_order = Vec::new();
        let mut completed = std::collections::HashSet::new();
        let mut remaining: Vec<&WorkflowStep> = steps.iter().collect();
        
        while !remaining.is_empty() {
            let mut progress_made = false;
            
            remaining.retain(|step| {
                let dependencies_met = step.depends_on.iter().all(|dep| completed.contains(dep));
                
                if dependencies_met {
                    execution_order.push(*step);
                    completed.insert(step.name.clone());
                    progress_made = true;
                    false // Remove from remaining
                } else {
                    true // Keep in remaining
                }
            });
            
            if !progress_made && !remaining.is_empty() {
                return Err(anyhow!("Circular dependency detected in workflow steps"));
            }
        }
        
        Ok(execution_order)
    }
    
    async fn update_job_status(&self, job_id: Uuid, status: JobStatus, error: Option<String>) {
        if let Some(mut job_ref) = self.active_jobs.get_mut(&job_id) {
            let job = job_ref.value_mut();
            job.status = status;
            job.error = error;
            
            if matches!(job.status, JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled) {
                job.completed_at = Some(Utc::now());
            }
        }
    }
    
    async fn update_job_progress(&self, job_id: Uuid, current_step: &str, progress: f64) {
        if let Some(mut job_ref) = self.active_jobs.get_mut(&job_id) {
            let job = job_ref.value_mut();
            job.current_step = Some(current_step.to_string());
            job.progress = progress;
        }
    }
    
    async fn update_job_metrics(&self, job_id: Uuid, metrics: TrainingMetrics) {
        if let Some(mut job_ref) = self.active_jobs.get_mut(&job_id) {
            let job = job_ref.value_mut();
            job.metrics = Some(metrics);
        }
    }
    
    async fn get_job_metrics(&self, job_id: Uuid) -> Option<TrainingMetrics> {
        self.active_jobs.get(&job_id).and_then(|job_ref| job_ref.value().metrics.clone())
    }
    
    fn get_job_output_dir(&self, job_id: Uuid) -> Result<PathBuf> {
        if let Some(job_ref) = self.active_jobs.get(&job_id) {
            Ok(job_ref.value().output_dir.clone())
        } else {
            Err(anyhow!("Job not found: {}", job_id))
        }
    }
    
    async fn load_default_workflows(&self) -> Result<()> {
        // Load built-in workflow templates
        let basic_training_workflow = WorkflowConfig {
            id: "basic-training".to_string(),
            name: "Basic Training Workflow".to_string(),
            description: "Standard machine learning training pipeline".to_string(),
            steps: vec![
                WorkflowStep {
                    name: "data-prep".to_string(),
                    step_type: super::StepType::DataPreparation,
                    parameters: HashMap::new(),
                    depends_on: vec![],
                },
                WorkflowStep {
                    name: "feature-engineering".to_string(),
                    step_type: super::StepType::FeatureEngineering,
                    parameters: HashMap::new(),
                    depends_on: vec!["data-prep".to_string()],
                },
                WorkflowStep {
                    name: "model-training".to_string(),
                    step_type: super::StepType::ModelTraining {
                        model_type: "neural".to_string(),
                        hyperparameters: HashMap::new(),
                    },
                    parameters: HashMap::new(),
                    depends_on: vec!["feature-engineering".to_string()],
                },
                WorkflowStep {
                    name: "model-validation".to_string(),
                    step_type: super::StepType::ModelValidation,
                    parameters: HashMap::new(),
                    depends_on: vec!["model-training".to_string()],
                },
                WorkflowStep {
                    name: "model-saving".to_string(),
                    step_type: super::StepType::ModelSaving,
                    parameters: HashMap::new(),
                    depends_on: vec!["model-validation".to_string()],
                },
            ],
            timeout_secs: 3600,
            retry_count: 3,
        };
        
        self.register_workflow(basic_training_workflow).await?;
        
        info!("Loaded default workflows");
        Ok(())
    }
}

/// Result of executing a workflow step
#[derive(Debug)]
struct StepResult {
    success: bool,
    artifact_path: Option<PathBuf>,
    metrics: Option<TrainingMetrics>,
}