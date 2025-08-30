//! Neural network configuration module
//!
//! Handles neural network specific configuration settings.

use serde::{Deserialize, Serialize};

/// Neural model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralConfig {
    pub memory_gb: f32,
    pub models: Vec<String>,
    pub prediction_cache_ttl: u64,
    #[serde(default = "default_model_load_timeout")]
    pub model_load_timeout: u64,
    #[serde(default = "default_max_concurrent_predictions")]
    pub max_concurrent_predictions: u32,
    #[serde(default = "default_true")]
    pub enable_model_monitoring: bool,
    #[serde(default = "default_accuracy_threshold")]
    pub accuracy_threshold: f64,
    #[serde(default = "default_false")]
    pub use_real_models: bool,
    #[serde(default = "default_true")]
    pub enable_health_checks: bool,
    #[serde(default = "default_true")]
    pub enable_fallback: bool,
    #[serde(default = "default_true")]
    pub enable_circuit_breakers: bool,
    #[serde(default = "default_false")]
    pub enable_graceful_degradation: bool,
    #[serde(default = "default_true")]
    pub enable_performance_monitoring: bool,
    #[serde(default = "default_true")]
    pub enable_adaptive_retry: bool,
    #[serde(default = "default_false")]
    pub enable_model_ensembles: bool,
    #[serde(default = "default_model_timeout_seconds")]
    pub model_timeout_seconds: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_error_threshold")]
    pub error_threshold: f64,
    #[serde(default = "default_lookback_window")]
    pub lookback_window: usize,
    
    // Additional neural-specific configurations
    #[serde(default = "default_input_size")]
    pub input_size: usize,
    #[serde(default = "default_output_size")]
    pub output_size: usize,
    #[serde(default = "default_hidden_layers")]
    pub hidden_layers: Vec<usize>,
    #[serde(default = "default_learning_rate")]
    pub learning_rate: f32,
    #[serde(default)]
    pub prediction_horizon: Option<usize>,
    #[serde(default)]
    pub normalization_method: Option<String>,
}

impl Default for NeuralConfig {
    fn default() -> Self {
        Self {
            memory_gb: 2.0,
            models: vec!["MLP".to_string(), "NHITS".to_string(), "DeepAR".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: default_model_load_timeout(),
            max_concurrent_predictions: default_max_concurrent_predictions(),
            enable_model_monitoring: default_true(),
            accuracy_threshold: default_accuracy_threshold(),
            use_real_models: default_false(),
            enable_health_checks: default_true(),
            enable_fallback: default_true(),
            enable_circuit_breakers: default_true(),
            enable_graceful_degradation: default_false(),
            enable_performance_monitoring: default_true(),
            enable_adaptive_retry: default_true(),
            enable_model_ensembles: default_false(),
            model_timeout_seconds: default_model_timeout_seconds(),
            max_retries: default_max_retries(),
            error_threshold: default_error_threshold(),
            lookback_window: default_lookback_window(),
            input_size: default_input_size(),
            output_size: default_output_size(),
            hidden_layers: default_hidden_layers(),
            learning_rate: default_learning_rate(),
            prediction_horizon: None,
            normalization_method: None,
        }
    }
}

// Default value functions
fn default_model_load_timeout() -> u64 { 300 }
fn default_max_concurrent_predictions() -> u32 { 10 }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_accuracy_threshold() -> f64 { 0.85 }
fn default_model_timeout_seconds() -> u64 { 60 }
fn default_max_retries() -> u32 { 3 }
fn default_error_threshold() -> f64 { 0.1 }
fn default_lookback_window() -> usize { 24 }
fn default_input_size() -> usize { 24 }
fn default_output_size() -> usize { 1 }
fn default_hidden_layers() -> Vec<usize> { vec![64, 32] }
fn default_learning_rate() -> f32 { 0.001 }

/// Neural network training configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    #[serde(default = "default_epochs")]
    pub epochs: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_validation_split")]
    pub validation_split: f32,
    #[serde(default = "default_early_stopping")]
    pub early_stopping: bool,
    #[serde(default = "default_patience")]
    pub patience: usize,
    #[serde(default = "default_min_delta")]
    pub min_delta: f32,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            epochs: default_epochs(),
            batch_size: default_batch_size(),
            validation_split: default_validation_split(),
            early_stopping: default_early_stopping(),
            patience: default_patience(),
            min_delta: default_min_delta(),
        }
    }
}

fn default_epochs() -> usize { 1000 }
fn default_batch_size() -> usize { 32 }
fn default_validation_split() -> f32 { 0.2 }
fn default_early_stopping() -> bool { true }
fn default_patience() -> usize { 50 }
fn default_min_delta() -> f32 { 0.001 }

/// Model ensemble configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleConfig {
    #[serde(default = "default_ensemble_size")]
    pub ensemble_size: usize,
    #[serde(default = "default_voting_strategy")]
    pub voting_strategy: String,
    #[serde(default = "default_diversity_threshold")]
    pub diversity_threshold: f64,
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f64,
}

impl Default for EnsembleConfig {
    fn default() -> Self {
        Self {
            ensemble_size: default_ensemble_size(),
            voting_strategy: default_voting_strategy(),
            diversity_threshold: default_diversity_threshold(),
            confidence_threshold: default_confidence_threshold(),
        }
    }
}

fn default_ensemble_size() -> usize { 5 }
fn default_voting_strategy() -> String { "weighted_average".to_string() }
fn default_diversity_threshold() -> f64 { 0.3 }
fn default_confidence_threshold() -> f64 { 0.7 }