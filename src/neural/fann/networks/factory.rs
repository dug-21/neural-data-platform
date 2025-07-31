//! Network Factory for FANN predictor
//!
//! This module handles the creation of different types of neural networks
//! with architecture-specific optimizations and configurations.

use anyhow::{Context, Result};
use tracing::{debug, info, warn};

use super::{FannModelConfig, NetworkArchitecture};
use ::ruv_fann::{ActivationFunction, Network, NetworkBuilder};

/// Factory for creating neural networks with different architectures
pub struct NetworkFactory {
    /// Default activation function
    default_activation: ActivationFunction,
}

impl NetworkFactory {
    /// Create a new network factory
    pub fn new() -> Self {
        Self {
            default_activation: ActivationFunction::SigmoidSymmetric,
        }
    }

    /// Create a network for the specified model and configuration
    pub async fn create_network(&self, model_name: &str, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating network for model: {} with config: {:?}", model_name, config);

        // Parse the model name to determine architecture
        let architecture = model_name.parse::<NetworkArchitecture>()
            .unwrap_or_else(|_| {
                warn!("Unknown architecture '{}', defaulting to MLP", model_name);
                NetworkArchitecture::MLP
            });

        let network = match architecture {
            NetworkArchitecture::MLP => self.create_mlp_network(config)?,
            NetworkArchitecture::LSTM => self.create_lstm_network(config)?,
            NetworkArchitecture::GRU => self.create_gru_network(config)?,
            NetworkArchitecture::DeepAR => self.create_deepar_network(config)?,
            NetworkArchitecture::TCN => self.create_tcn_network(config)?,
            NetworkArchitecture::NHITS => self.create_nhits_network(config)?,
            NetworkArchitecture::Transformer => self.create_transformer_network(config)?,
        };

        info!("Successfully created {} network with {} layers", architecture, config.layers.len());
        Ok(network)
    }

    /// Create a Multi-Layer Perceptron network
    fn create_mlp_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating MLP network with layers: {:?}", config.layers);

        // Build network using layers_from_sizes - much simpler
        let network = NetworkBuilder::new()
            .layers_from_sizes(&config.layers)
            .build();

        Ok(network)
    }

    /// Create a simulated LSTM network (using MLP with larger hidden layers)
    fn create_lstm_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated LSTM network");

        // LSTM simulation uses larger hidden layers to approximate memory cells
        let mut enhanced_layers = config.layers.clone();
        
        // Enhance hidden layers for LSTM simulation
        let layers_len = enhanced_layers.len();
        for layer in enhanced_layers.iter_mut().skip(1).take(layers_len - 2) {
            *layer = (*layer * 3) / 2; // 1.5x the original size for memory simulation
        }

        // Build network using layers_from_sizes for LSTM simulation
        let network = NetworkBuilder::new()
            .layers_from_sizes(&enhanced_layers)
            .build();

        Ok(network)
    }

    /// Create a simulated GRU network (using MLP with optimized hidden layers)
    fn create_gru_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated GRU network");

        // GRU simulation uses moderately enhanced hidden layers
        let mut enhanced_layers = config.layers.clone();
        
        // Enhance hidden layers for GRU simulation (less than LSTM)
        let layers_len = enhanced_layers.len();
        for layer in enhanced_layers.iter_mut().skip(1).take(layers_len - 2) {
            *layer = (*layer * 5) / 4; // 1.25x the original size
        }

        // Build network using layers_from_sizes for GRU simulation
        let network = NetworkBuilder::new()
            .layers_from_sizes(&enhanced_layers)
            .build();

        Ok(network)
    }

    /// Create a simulated DeepAR network (probabilistic forecasting)
    fn create_deepar_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated DeepAR network");

        // DeepAR needs to output both mean and variance for probabilistic predictions
        let output_size = config.layers.last().unwrap() * 2; // Double output for mean + variance
        
        // Build layers for DeepAR with enhanced capacity for probabilistic modeling
        let mut deepar_layers = Vec::new();
        deepar_layers.push(config.layers[0]); // Input layer
        for &size in config.layers.iter().skip(1).take(config.layers.len() - 2) {
            deepar_layers.push(size + size / 4); // Enhanced hidden layers
        }
        deepar_layers.push(output_size); // Double output for mean + variance
        
        let network = NetworkBuilder::new()
            .layers_from_sizes(&deepar_layers)
            .build();

        Ok(network)
    }

    /// Create a simulated TCN network (Temporal Convolutional Network)
    fn create_tcn_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated TCN network");

        // TCN simulation uses multiple layers with decreasing sizes (similar to dilated convolutions)
        let mut tcn_layers = Vec::new();
        tcn_layers.push(config.layers[0]); // Input layer

        // Create a series of decreasing hidden layers to simulate temporal convolutions
        let mut current_size = config.layers[0] * 2; // Start larger
        for _ in 0..4 { // 4 hidden layers for temporal modeling
            tcn_layers.push(current_size);
            current_size = (current_size * 3) / 4; // Gradually decrease
        }
        tcn_layers.push(*config.layers.last().unwrap()); // Output layer

        // Build network using layers_from_sizes for TCN simulation
        let network = NetworkBuilder::new()
            .layers_from_sizes(&tcn_layers)
            .build();

        Ok(network)
    }

    /// Create a simulated NHITS network (Neural Hierarchical Interpolation)
    fn create_nhits_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated NHITS network");

        // NHITS simulation uses hierarchical structure
        let mut nhits_layers = Vec::new();
        nhits_layers.push(config.layers[0]); // Input layer

        // Create hierarchical layers (large -> medium -> small -> output)
        let base_size = config.layers[0];
        nhits_layers.push(base_size * 4);      // Large representation
        nhits_layers.push(base_size * 2);      // Medium representation  
        nhits_layers.push(base_size);          // Original size
        nhits_layers.push(base_size / 2);      // Compressed representation
        nhits_layers.push(*config.layers.last().unwrap()); // Output

        // Build network using layers_from_sizes for NHITS simulation
        let network = NetworkBuilder::new()
            .layers_from_sizes(&nhits_layers)
            .build();

        Ok(network)
    }

    /// Create a simulated Transformer network (attention-based)
    fn create_transformer_network(&self, config: &FannModelConfig) -> Result<Network<f32>> {
        debug!("Creating simulated Transformer network");

        // Transformer simulation uses large hidden layers to approximate attention mechanisms
        let mut transformer_layers = Vec::new();
        transformer_layers.push(config.layers[0]); // Input layer

        // Multi-layer structure for attention simulation
        let attention_size = config.layers[0] * 4; // Large for attention heads
        transformer_layers.push(attention_size);
        transformer_layers.push(attention_size * 3 / 4);
        transformer_layers.push(attention_size / 2);
        transformer_layers.push(attention_size / 4);
        transformer_layers.push(*config.layers.last().unwrap()); // Output

        // Build network using layers_from_sizes for Transformer simulation
        let network = NetworkBuilder::new()
            .layers_from_sizes(&transformer_layers)
            .build();

        Ok(network)
    }

    /// Validate network configuration before creation
    pub fn validate_config(&self, config: &FannModelConfig) -> Result<()> {
        if config.layers.len() < 2 {
            return Err(anyhow::anyhow!("Network must have at least input and output layers"));
        }

        if config.layers.iter().any(|&size| size == 0) {
            return Err(anyhow::anyhow!("All layers must have size > 0"));
        }

        if config.learning_rate <= 0.0 || config.learning_rate > 1.0 {
            return Err(anyhow::anyhow!("Learning rate must be between 0 and 1"));
        }

        if config.epochs == 0 {
            return Err(anyhow::anyhow!("Epochs must be > 0"));
        }

        Ok(())
    }

    /// Get recommended configuration for an architecture
    pub fn get_recommended_config(&self, architecture: NetworkArchitecture, input_size: usize, output_size: usize) -> FannModelConfig {
        let mut config = architecture.default_config(input_size, output_size);
        
        // Apply factory-specific optimizations
        match architecture {
            NetworkArchitecture::LSTM | NetworkArchitecture::GRU => {
                // Recurrent networks benefit from lower learning rates
                config.learning_rate *= 0.8;
                config.max_epochs = (config.max_epochs * 3) / 2; // More training time
            },
            NetworkArchitecture::Transformer => {
                // Transformers need very careful learning rate tuning
                config.learning_rate *= 0.5;
                config.max_epochs *= 2;
            },
            NetworkArchitecture::DeepAR => {
                // Probabilistic models need more stable training
                config.learning_rate *= 0.7;
                config.desired_error *= 0.5; // Tighter convergence
            },
            _ => {
                // Default optimizations for other architectures
            }
        }

        config
    }

    /// Set the default activation function
    pub fn set_default_activation(&mut self, activation: ActivationFunction) {
        self.default_activation = activation;
    }

    /// Get the current default activation function
    pub fn default_activation(&self) -> ActivationFunction {
        self.default_activation
    }
}

impl Default for NetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mlp_creation() {
        let factory = NetworkFactory::new();
        let config = FannModelConfig {
            layers: vec![10, 20, 5, 1],
            activation: ActivationFunction::SigmoidSymmetric,
            learning_rate: 0.01,
            epochs: 100,
            desired_error: 0.01,
            max_epochs: 1000,
            epochs_between_reports: 10,
        };

        let result = factory.create_network("MLP", &config).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_validation() {
        let factory = NetworkFactory::new();
        
        // Valid config
        let valid_config = FannModelConfig::default();
        assert!(factory.validate_config(&valid_config).is_ok());

        // Invalid config - no layers
        let invalid_config = FannModelConfig {
            layers: vec![],
            ..FannModelConfig::default()
        };
        assert!(factory.validate_config(&invalid_config).is_err());

        // Invalid config - zero layer size
        let invalid_config = FannModelConfig {
            layers: vec![10, 0, 1],
            ..FannModelConfig::default()
        };
        assert!(factory.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_recommended_configs() {
        let factory = NetworkFactory::new();
        
        let lstm_config = factory.get_recommended_config(NetworkArchitecture::LSTM, 24, 1);
        let mlp_config = factory.get_recommended_config(NetworkArchitecture::MLP, 24, 1);
        
        // LSTM should have lower learning rate than MLP
        assert!(lstm_config.learning_rate < mlp_config.learning_rate);
    }
}