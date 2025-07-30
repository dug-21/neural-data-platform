//! Autonomous Training Configuration Module
//!
//! Contains configuration structures and defaults for autonomous training triggers and decisions.

use chrono::Duration;
use serde::{Deserialize, Serialize};

/// Autonomous training trigger thresholds and conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingTriggerConfig {
    /// Performance accuracy threshold (below this triggers retraining)
    pub accuracy_threshold: f64,
    /// Sharpe ratio threshold for trading performance
    pub sharpe_ratio_threshold: f64,
    /// Maximum drawdown threshold
    pub max_drawdown_threshold: f64,
    /// Price prediction error threshold (percentage)
    pub price_error_threshold: f64,
    /// Confidence drop threshold
    pub confidence_drop_threshold: f64,
    /// Minimum time between training sessions (hours)
    pub min_training_interval_hours: i64,
    /// Maximum time without training (hours)
    pub max_training_interval_hours: i64,
    /// Consecutive poor predictions threshold
    pub consecutive_failures_threshold: usize,
    /// Market volatility threshold for emergency retraining
    pub volatility_threshold: f64,
    /// Model agreement threshold (when models disagree significantly)
    pub model_disagreement_threshold: f64,
}

impl Default for TrainingTriggerConfig {
    fn default() -> Self {
        Self {
            accuracy_threshold: 0.7,
            sharpe_ratio_threshold: 0.5,
            max_drawdown_threshold: 0.15,
            price_error_threshold: 0.1,     // 10% error
            confidence_drop_threshold: 0.2, // 20% drop in confidence
            min_training_interval_hours: 6,
            max_training_interval_hours: 72,
            consecutive_failures_threshold: 5,
            volatility_threshold: 0.05,        // 5% volatility
            model_disagreement_threshold: 0.3, // 30% disagreement
        }
    }
}

/// Training decision types based on urgency and scope
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TrainingDecisionType {
    /// Emergency retraining due to severe performance degradation
    Emergency { reason: String, urgency_score: f64 },
    /// Full model retraining for significant improvements
    FullRetraining {
        reason: String,
        expected_improvement: f64,
    },
    /// Incremental training for minor adjustments
    IncrementalTraining { reason: String, scope: String },
    /// Fine-tuning for specific market conditions
    FineTuning {
        reason: String,
        target_regime: String,
    },
    /// No training needed
    NoTraining { reason: String },
}

/// Training priority levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrainingPriority {
    Low,
    Medium,
    High,
    Critical,
    Emergency,
}

/// Resource requirements for training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: usize,
    pub memory_gb: f64,
    pub gpu_required: bool,
    pub disk_space_gb: f64,
    pub network_bandwidth_mbps: f64,
}

impl ResourceRequirements {
    pub fn minimal() -> Self {
        Self {
            cpu_cores: 1,
            memory_gb: 1.0,
            gpu_required: false,
            disk_space_gb: 1.0,
            network_bandwidth_mbps: 10.0,
        }
    }

    pub fn fine_tuning() -> Self {
        Self {
            cpu_cores: 2,
            memory_gb: 4.0,
            gpu_required: false,
            disk_space_gb: 5.0,
            network_bandwidth_mbps: 50.0,
        }
    }

    pub fn incremental() -> Self {
        Self {
            cpu_cores: 4,
            memory_gb: 8.0,
            gpu_required: true,
            disk_space_gb: 10.0,
            network_bandwidth_mbps: 100.0,
        }
    }

    pub fn full_training() -> Self {
        Self {
            cpu_cores: 8,
            memory_gb: 16.0,
            gpu_required: true,
            disk_space_gb: 50.0,
            network_bandwidth_mbps: 500.0,
        }
    }

    pub fn high_priority() -> Self {
        Self {
            cpu_cores: 12,
            memory_gb: 32.0,
            gpu_required: true,
            disk_space_gb: 100.0,
            network_bandwidth_mbps: 1000.0,
        }
    }
}

/// Training execution outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingOutcome {
    Success {
        improvement_percentage: f64,
        new_accuracy: f64,
    },
    Failure {
        error_message: String,
        retry_recommended: bool,
    },
    Cancelled {
        reason: String,
    },
    InProgress {
        completion_percentage: f64,
    },
}