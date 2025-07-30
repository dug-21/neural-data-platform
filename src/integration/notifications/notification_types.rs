//! Training notification types and supporting structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Priority levels for training notifications
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Training metrics reported during and after training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingMetrics {
    pub loss: f64,
    pub accuracy: f64,
    pub learning_rate: f64,
    pub epoch_duration_ms: u64,
    pub samples_processed: usize,
}

/// Training notification variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingNotification {
    /// Training has been requested but not yet started
    TrainingRequested {
        model_id: String,
        reason: String,
        priority: Priority,
        timestamp: DateTime<Utc>,
    },
    
    /// Training has started
    TrainingStarted {
        job_id: String,
        model_id: String,
        resources: ResourceAllocation,
        timestamp: DateTime<Utc>,
    },
    
    /// Progress update during training
    TrainingProgress {
        job_id: String,
        epoch: u32,
        total_epochs: u32,
        metrics: TrainingMetrics,
        timestamp: DateTime<Utc>,
    },
    
    /// Training completed successfully
    TrainingCompleted {
        job_id: String,
        final_metrics: TrainingMetrics,
        model_path: String,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
    
    /// Training failed
    TrainingFailed {
        job_id: String,
        error: String,
        retry_count: u32,
        timestamp: DateTime<Utc>,
    },
}

/// Resource allocation for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub gpu_device: Option<String>,
}

impl TrainingNotification {
    /// Get the timestamp of this notification
    pub fn timestamp(&self) -> &DateTime<Utc> {
        match self {
            Self::TrainingRequested { timestamp, .. } => timestamp,
            Self::TrainingStarted { timestamp, .. } => timestamp,
            Self::TrainingProgress { timestamp, .. } => timestamp,
            Self::TrainingCompleted { timestamp, .. } => timestamp,
            Self::TrainingFailed { timestamp, .. } => timestamp,
        }
    }
    
    /// Get the job ID if this notification is job-specific
    pub fn job_id(&self) -> Option<&str> {
        match self {
            Self::TrainingRequested { .. } => None,
            Self::TrainingStarted { job_id, .. } => Some(job_id),
            Self::TrainingProgress { job_id, .. } => Some(job_id),
            Self::TrainingCompleted { job_id, .. } => Some(job_id),
            Self::TrainingFailed { job_id, .. } => Some(job_id),
        }
    }
    
    /// Get the priority of this notification
    pub fn priority(&self) -> Priority {
        match self {
            Self::TrainingRequested { priority, .. } => *priority,
            Self::TrainingStarted { .. } => Priority::High,
            Self::TrainingProgress { .. } => Priority::Low,
            Self::TrainingCompleted { .. } => Priority::High,
            Self::TrainingFailed { .. } => Priority::Critical,
        }
    }
}

impl fmt::Display for TrainingNotification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TrainingRequested { model_id, reason, priority, .. } => {
                write!(f, "Training requested for {} ({}): {}", model_id, priority, reason)
            }
            Self::TrainingStarted { job_id, model_id, .. } => {
                write!(f, "Training started [{}] for model {}", job_id, model_id)
            }
            Self::TrainingProgress { job_id, epoch, total_epochs, metrics, .. } => {
                write!(f, "Training progress [{}]: {}/{} - loss: {:.4}, accuracy: {:.2}%", 
                    job_id, epoch, total_epochs, metrics.loss, metrics.accuracy * 100.0)
            }
            Self::TrainingCompleted { job_id, final_metrics, duration_ms, .. } => {
                write!(f, "Training completed [{}] in {:.1}s - accuracy: {:.2}%", 
                    job_id, duration_ms as f64 / 1000.0, final_metrics.accuracy * 100.0)
            }
            Self::TrainingFailed { job_id, error, retry_count, .. } => {
                write!(f, "Training failed [{}] after {} retries: {}", job_id, retry_count, error)
            }
        }
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Priority::Low => write!(f, "LOW"),
            Priority::Medium => write!(f, "MEDIUM"),
            Priority::High => write!(f, "HIGH"),
            Priority::Critical => write!(f, "CRITICAL"),
        }
    }
}