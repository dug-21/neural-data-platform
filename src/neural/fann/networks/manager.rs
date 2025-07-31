//! Network Manager for FANN predictor
//!
//! This module handles the lifecycle management of neural networks,
//! including creation, caching, and cleanup of network instances.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tracing::{debug, info, warn, error};
use dashmap::DashMap;

use super::{ModelKey, FannModelConfig, NetworkArchitecture};
use super::factory::NetworkFactory;
use ::ruv_fann::Network;

/// Network manager that handles the lifecycle of neural networks
pub struct NetworkManager {
    /// Active networks indexed by model name
    networks: Arc<RwLock<HashMap<String, Arc<Mutex<Network<f32>>>>>>,
    /// Network cache for quick access
    network_cache: Arc<DashMap<ModelKey, Arc<Network<f32>>>>,
    /// Model configurations
    model_configs: HashMap<String, FannModelConfig>,
    /// Network factory for creating new networks
    factory: NetworkFactory,
    /// Maximum number of cached networks
    max_cache_size: usize,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new(model_configs: HashMap<String, FannModelConfig>) -> Self {
        Self {
            networks: Arc::new(RwLock::new(HashMap::new())),
            network_cache: Arc::new(DashMap::new()),
            model_configs,
            factory: NetworkFactory::new(),
            max_cache_size: 100, // Reasonable default for network caching
        }
    }

    /// Ensure a model network exists, creating it if necessary
    pub async fn ensure_model(&self, model_name: &str) -> Result<()> {
        let networks = self.networks.read().await;
        if networks.contains_key(model_name) {
            debug!("Model {} already exists", model_name);
            return Ok(());
        }
        drop(networks);

        info!("Creating new model: {}", model_name);
        
        // Get or create default configuration
        let config = self.model_configs
            .get(model_name)
            .cloned()
            .unwrap_or_else(|| {
                warn!("No configuration found for model {}, using default", model_name);
                self.create_default_config(model_name)
            });

        // Create the network using the factory
        let network = self.factory.create_network(model_name, &config)
            .await
            .with_context(|| format!("Failed to create network for model: {}", model_name))?;

        // Store the network
        let mut networks = self.networks.write().await;
        networks.insert(model_name.to_string(), Arc::new(Mutex::new(network)));

        info!("Successfully created model: {}", model_name);
        Ok(())
    }

    /// Get a reference to a network by model name
    pub async fn get_network(&self, model_name: &str) -> Option<Arc<Mutex<Network<f32>>>> {
        let networks = self.networks.read().await;
        networks.get(model_name).cloned()
    }

    /// Get all available model names
    pub async fn get_model_names(&self) -> Vec<String> {
        let networks = self.networks.read().await;
        networks.keys().cloned().collect()
    }

    /// Check if a model exists
    pub async fn model_exists(&self, model_name: &str) -> bool {
        let networks = self.networks.read().await;
        networks.contains_key(model_name)
    }

    /// Remove a model from the manager
    pub async fn remove_model(&self, model_name: &str) -> Result<()> {
        let mut networks = self.networks.write().await;
        if networks.remove(model_name).is_some() {
            info!("Removed model: {}", model_name);
            
            // Also remove from cache
            let keys_to_remove: Vec<_> = self.network_cache
                .iter()
                .filter(|entry| entry.key().model_type == model_name)
                .map(|entry| entry.key().clone())
                .collect();
            
            for key in keys_to_remove {
                self.network_cache.remove(&key);
            }
            
            Ok(())
        } else {
            Err(anyhow::anyhow!("Model not found: {}", model_name))
        }
    }

    /// Get cached network by model key
    pub fn get_cached_network(&self, key: &ModelKey) -> Option<Arc<Network<f32>>> {
        self.network_cache.get(key).map(|entry| entry.value().clone())
    }

    /// Cache a network for reuse
    pub fn cache_network(&self, key: ModelKey, network: Arc<Network<f32>>) {
        // Check cache size limit
        if self.network_cache.len() >= self.max_cache_size {
            self.evict_oldest_cached_network();
        }

        self.network_cache.insert(key, network);
    }

    /// Get the current cache size
    pub fn cache_size(&self) -> usize {
        self.network_cache.len()
    }

    /// Clear the entire cache
    pub fn clear_cache(&self) {
        self.network_cache.clear();
        info!("Cleared network cache");
    }

    /// Get network statistics
    pub async fn get_network_stats(&self) -> NetworkStats {
        let networks = self.networks.read().await;
        NetworkStats {
            active_networks: networks.len(),
            cached_networks: self.network_cache.len(),
            total_models_configured: self.model_configs.len(),
        }
    }

    /// Initialize all configured models
    pub async fn initialize_all_models(&self) -> Result<()> {
        info!("Initializing all configured models");
        
        let model_names: Vec<String> = self.model_configs.keys().cloned().collect();
        
        for model_name in model_names {
            if let Err(e) = self.ensure_model(&model_name).await {
                error!("Failed to initialize model {}: {}", model_name, e);
                // Continue with other models rather than failing completely
            }
        }

        let stats = self.get_network_stats().await;
        info!("Model initialization complete: {} active networks", stats.active_networks);
        
        Ok(())
    }

    /// Shutdown all networks gracefully
    pub async fn shutdown(&self) {
        info!("Shutting down network manager");
        
        let mut networks = self.networks.write().await;
        networks.clear();
        self.network_cache.clear();
        
        info!("Network manager shutdown complete");
    }

    /// Create a default configuration for unknown models
    fn create_default_config(&self, model_name: &str) -> FannModelConfig {
        // Try to parse as known architecture
        if let Ok(arch) = model_name.parse::<NetworkArchitecture>() {
            arch.default_config(24, 1) // Default input/output sizes
        } else {
            // Fallback to basic MLP configuration
            FannModelConfig::default()
        }
    }

    /// Evict the oldest cached network (simple LRU approximation)
    fn evict_oldest_cached_network(&self) {
        // Simple eviction strategy - remove first found entry
        // In a production system, you might want proper LRU tracking
        if let Some(entry) = self.network_cache.iter().next() {
            let key = entry.key().clone();
            drop(entry);
            self.network_cache.remove(&key);
            debug!("Evicted cached network: {:?}", key);
        }
    }

    /// Validate network configuration
    pub fn validate_config(&self, model_name: &str, config: &FannModelConfig) -> Result<()> {
        // Basic validation
        if config.layers.len() < 2 {
            return Err(anyhow::anyhow!(
                "Network {} must have at least 2 layers (input and output)", 
                model_name
            ));
        }

        if config.learning_rate <= 0.0 || config.learning_rate > 1.0 {
            return Err(anyhow::anyhow!(
                "Learning rate for {} must be between 0 and 1, got: {}", 
                model_name, 
                config.learning_rate
            ));
        }

        if config.layers.iter().any(|&size| size == 0) {
            return Err(anyhow::anyhow!(
                "All layer sizes for {} must be greater than 0", 
                model_name
            ));
        }

        Ok(())
    }

    /// Update model configuration
    pub async fn update_model_config(&mut self, model_name: &str, config: FannModelConfig) -> Result<()> {
        // Validate the new configuration
        self.validate_config(model_name, &config)?;

        // Remove existing network if it exists
        if self.model_exists(model_name).await {
            self.remove_model(model_name).await?;
        }

        // Update configuration
        self.model_configs.insert(model_name.to_string(), config);

        // Recreate the network with new configuration
        self.ensure_model(model_name).await?;

        info!("Updated configuration for model: {}", model_name);
        Ok(())
    }

    /// Get model configuration
    pub fn get_model_config(&self, model_name: &str) -> Option<&FannModelConfig> {
        self.model_configs.get(model_name)
    }

    /// List all configured models
    pub fn list_configured_models(&self) -> Vec<&String> {
        self.model_configs.keys().collect()
    }
}

/// Network statistics for monitoring
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Number of active networks
    pub active_networks: usize,
    /// Number of cached networks
    pub cached_networks: usize,
    /// Total models configured
    pub total_models_configured: usize,
}

impl NetworkStats {
    /// Calculate cache hit ratio (approximation)
    pub fn cache_efficiency(&self) -> f64 {
        if self.total_models_configured == 0 {
            return 0.0;
        }
        
        self.cached_networks as f64 / self.total_models_configured as f64
    }

    /// Check if all configured models are active
    pub fn all_models_active(&self) -> bool {
        self.active_networks == self.total_models_configured
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_network_manager_creation() {
        let configs = HashMap::new();
        let manager = NetworkManager::new(configs);
        
        let stats = manager.get_network_stats().await;
        assert_eq!(stats.active_networks, 0);
        assert_eq!(stats.cached_networks, 0);
    }

    #[tokio::test]
    async fn test_model_existence_check() {
        let configs = HashMap::new();
        let manager = NetworkManager::new(configs);
        
        assert!(!manager.model_exists("test_model").await);
        
        // This should fail since no config exists
        let result = manager.ensure_model("test_model").await;
        assert!(result.is_ok()); // Should succeed with default config
        
        assert!(manager.model_exists("test_model").await);
    }
}