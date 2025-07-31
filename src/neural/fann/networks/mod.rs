//! Network management modules for FANN predictor
//!
//! This module provides network creation, management, and configuration
//! functionality for the FANN-based neural networks.

pub mod manager;
pub mod factory;

// Re-export commonly used types
pub use manager::NetworkManager;
pub use factory::NetworkFactory;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ::ruv_fann::ActivationFunction;

/// Model configuration for network creation
#[derive(Debug, Clone, PartialEq)]
pub struct ModelConfig {
    pub input_size: usize,
    pub output_size: usize,
    pub hidden_layers: Vec<usize>,
    pub learning_rate: f32,
    pub horizon: usize,
    pub hidden_activation: ActivationFunction,
    pub output_activation: ActivationFunction,
}

impl std::hash::Hash for ModelConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.input_size.hash(state);
        self.output_size.hash(state);
        self.hidden_layers.hash(state);
        self.learning_rate.to_bits().hash(state);
        self.horizon.hash(state);
        std::mem::discriminant(&self.hidden_activation).hash(state);
        std::mem::discriminant(&self.output_activation).hash(state);
    }
}

impl ModelConfig {
    pub fn default() -> Self {
        Self {
            input_size: 24,
            output_size: 1,
            hidden_layers: vec![64, 32],
            learning_rate: 0.001,
            horizon: 1,
            hidden_activation: ActivationFunction::SigmoidSymmetric,
            output_activation: ActivationFunction::Linear,
        }
    }
}

/// Unique identifier for models based on configuration
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ModelKey {
    pub model_type: String,
    pub config_hash: u64,
}

impl ModelKey {
    pub fn new(model_type: String, config: &ModelConfig) -> Self {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        config.hash(&mut hasher);
        
        Self {
            model_type,
            config_hash: hasher.finish(),
        }
    }

    pub fn from_name_and_input_size(model_type: String, input_size: usize) -> Self {
        let config = ModelConfig {
            input_size,
            output_size: 1,
            hidden_layers: vec![64, 32],
            learning_rate: 0.001,
            horizon: 1,
            hidden_activation: ActivationFunction::SigmoidSymmetric,
            output_activation: ActivationFunction::Linear,
        };
        Self::new(model_type, &config)
    }
}

/// FANN model configuration with network parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FannModelConfig {
    /// Network layer sizes (input, hidden layers, output)
    pub layers: Vec<usize>,
    /// Activation function for the network
    pub activation: ActivationFunction,
    /// Learning rate for training
    pub learning_rate: f32,
    /// Number of training epochs
    pub epochs: usize,
    /// Desired training error threshold
    pub desired_error: f32,
    /// Maximum number of epochs before stopping
    pub max_epochs: usize,
    /// Frequency of training progress reports
    pub epochs_between_reports: usize,
}

impl Default for FannModelConfig {
    fn default() -> Self {
        Self {
            layers: vec![24, 64, 32, 1],
            activation: ActivationFunction::SigmoidSymmetric,
            learning_rate: 0.001,
            epochs: 1000,
            desired_error: 0.001,
            max_epochs: 5000,
            epochs_between_reports: 100,
        }
    }
}

/// Training result information
#[derive(Debug, Clone)]
pub struct TrainingResult {
    /// Final training error achieved
    pub final_error: f32,
    /// Number of epochs completed
    pub epochs_completed: usize,
    /// Training duration in milliseconds
    pub training_duration_ms: u64,
    /// Whether training converged successfully
    pub converged: bool,
}

/// Training algorithm options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingAlgorithm {
    /// Incremental training with backpropagation
    Incremental,
    /// Batch training with backpropagation
    Batch,
    /// Resilient backpropagation (RPROP)
    Rprop,
    /// Quick propagation
    Quickprop,
    /// Scaled conjugate gradient
    Sarprop,
}

impl Default for TrainingAlgorithm {
    fn default() -> Self {
        TrainingAlgorithm::Rprop
    }
}

/// Network architecture types supported by FANN
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkArchitecture {
    /// Multi-layer perceptron
    MLP,
    /// Simulated LSTM (using MLP with state management)
    LSTM,
    /// Simulated GRU (using MLP with simplified gating)
    GRU,
    /// Simulated DeepAR (probabilistic forecasting)
    DeepAR,
    /// Simulated TCN (temporal convolutional networks)
    TCN,
    /// Simulated NHITS (neural hierarchical interpolation)
    NHITS,
    /// Simulated Transformer (attention mechanism)
    Transformer,
}

impl NetworkArchitecture {
    /// Get the default configuration for this architecture
    pub fn default_config(&self, input_size: usize, output_size: usize) -> FannModelConfig {
        match self {
            NetworkArchitecture::MLP => FannModelConfig {
                layers: vec![input_size, 64, 32, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.001,
                epochs: 1000,
                desired_error: 0.001,
                max_epochs: 5000,
                epochs_between_reports: 100,
            },
            NetworkArchitecture::LSTM => FannModelConfig {
                layers: vec![input_size, 128, 64, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.001,
                epochs: 1500,
                desired_error: 0.001,
                max_epochs: 7000,
                epochs_between_reports: 150,
            },
            NetworkArchitecture::GRU => FannModelConfig {
                layers: vec![input_size, 96, 48, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.001,
                epochs: 1200,
                desired_error: 0.001,
                max_epochs: 6000,
                epochs_between_reports: 120,
            },
            NetworkArchitecture::DeepAR => FannModelConfig {
                layers: vec![input_size, 128, 96, 64, output_size * 2], // *2 for mean and variance
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.0005,
                epochs: 2000,
                desired_error: 0.0005,
                max_epochs: 8000,
                epochs_between_reports: 200,
            },
            NetworkArchitecture::TCN => FannModelConfig {
                layers: vec![input_size, 128, 96, 64, 32, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.001,
                epochs: 1800,
                desired_error: 0.001,
                max_epochs: 7500,
                epochs_between_reports: 180,
            },
            NetworkArchitecture::NHITS => FannModelConfig {
                layers: vec![input_size, 256, 128, 64, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.0008,
                epochs: 1600,
                desired_error: 0.0008,
                max_epochs: 7000,
                epochs_between_reports: 160,
            },
            NetworkArchitecture::Transformer => FannModelConfig {
                layers: vec![input_size, 256, 128, 64, 32, output_size],
                activation: ActivationFunction::SigmoidSymmetric,
                learning_rate: 0.0005,
                epochs: 2500,
                desired_error: 0.0005,
                max_epochs: 10000,
                epochs_between_reports: 250,
            },
        }
    }

    /// Check if this architecture requires special handling
    pub fn is_recurrent(&self) -> bool {
        matches!(self, NetworkArchitecture::LSTM | NetworkArchitecture::GRU)
    }

    /// Check if this architecture supports attention mechanisms
    pub fn supports_attention(&self) -> bool {
        matches!(self, NetworkArchitecture::Transformer)
    }

    /// Check if this architecture is probabilistic
    pub fn is_probabilistic(&self) -> bool {
        matches!(self, NetworkArchitecture::DeepAR)
    }
}

impl std::fmt::Display for NetworkArchitecture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkArchitecture::MLP => write!(f, "MLP"),
            NetworkArchitecture::LSTM => write!(f, "LSTM"),
            NetworkArchitecture::GRU => write!(f, "GRU"),
            NetworkArchitecture::DeepAR => write!(f, "DeepAR"),
            NetworkArchitecture::TCN => write!(f, "TCN"),
            NetworkArchitecture::NHITS => write!(f, "NHITS"),
            NetworkArchitecture::Transformer => write!(f, "Transformer"),
        }
    }
}

impl std::str::FromStr for NetworkArchitecture {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "MLP" => Ok(NetworkArchitecture::MLP),
            "LSTM" => Ok(NetworkArchitecture::LSTM),
            "GRU" => Ok(NetworkArchitecture::GRU),
            "DEEPAR" => Ok(NetworkArchitecture::DeepAR),
            "TCN" => Ok(NetworkArchitecture::TCN),
            "NHITS" => Ok(NetworkArchitecture::NHITS),
            "TRANSFORMER" => Ok(NetworkArchitecture::Transformer),
            _ => Err(format!("Unknown network architecture: {}", s)),
        }
    }
}