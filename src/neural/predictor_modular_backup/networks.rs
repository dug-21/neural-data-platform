//! Network management module
//!
//! Handles FANN network lifecycle, caching, and state management.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use dashmap::DashMap;
use ::ruv_fann::Network;

use super::{NetworkFactory, NetworkCache};
use crate::neural::PredictionResult;

/// Key for identifying network models
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ModelKey {
    pub model_name: String,
    pub input_size: usize,
    pub horizon: usize,
}

impl ModelKey {
    pub fn new(model_name: String, input_size: usize, horizon: usize) -> Self {
        Self {
            model_name,
            input_size,
            horizon,
        }
    }
}

/// Recurrent state for LSTM/GRU models
#[derive(Debug, Clone)]
pub struct RecurrentState {
    pub hidden_state: Vec<f32>,
    pub cell_state: Option<Vec<f32>>, // For LSTM
    pub last_update: chrono::DateTime<chrono::Utc>,
}

impl RecurrentState {
    pub fn new(hidden_size: usize, has_cell_state: bool) -> Self {
        Self {
            hidden_state: vec![0.0; hidden_size],
            cell_state: if has_cell_state {
                Some(vec![0.0; hidden_size])
            } else {
                None
            },
            last_update: chrono::Utc::now(),
        }
    }
}

/// Network manager coordinating network lifecycle
pub struct NetworkManager {
    factory: Arc<NetworkFactory>,
    cache: Arc<NetworkCache>,
    recurrent_states: Arc<DashMap<String, RecurrentState>>,
    active_networks: Arc<RwLock<HashMap<ModelKey, Arc<Network>>>>,
}

impl NetworkManager {
    pub fn new(factory: Arc<NetworkFactory>, cache: Arc<NetworkCache>) -> Result<Self> {
        Ok(Self {
            factory,
            cache,
            recurrent_states: Arc::new(DashMap::new()),
            active_networks: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get or create network for given model and configuration
    pub async fn get_network(&self, model_key: &ModelKey) -> Result<Arc<Network>> {
        // Check active networks first
        {
            let networks = self.active_networks.read().await;
            if let Some(network) = networks.get(model_key) {
                return Ok(Arc::clone(network));
            }
        }

        // Check cache
        if let Some(network) = self.cache.get_network(model_key).await? {
            let mut networks = self.active_networks.write().await;
            networks.insert(model_key.clone(), Arc::clone(&network));
            return Ok(network);
        }

        // Create new network
        let network = self.factory.create_network(&model_key.model_name, model_key.input_size, model_key.horizon).await?;
        let network_arc = Arc::new(network);

        // Store in cache and active networks
        self.cache.store_network(model_key, Arc::clone(&network_arc)).await?;
        let mut networks = self.active_networks.write().await;
        networks.insert(model_key.clone(), Arc::clone(&network_arc));

        Ok(network_arc)
    }

    /// Predict using network manager
    pub async fn predict(&self, data: &[f32], horizon: usize) -> Result<Vec<PredictionResult>> {
        let model_key = ModelKey::new("MLP".to_string(), data.len(), horizon);
        let network = self.get_network(&model_key).await?;

        let mut results = Vec::new();
        for i in 0..horizon {
            let output = network.run(data)?;
            
            // Create prediction result
            let result = PredictionResult {
                timestamp: chrono::Utc::now() + chrono::Duration::hours(i as i64),
                predicted_value: output[0] as f64,
                confidence: 0.85, // Base confidence for FANN models
                model_name: "FANN_MLP".to_string(),
                horizon_step: i + 1,
                lower_bound: Some(output[0] as f64 * 0.95),
                upper_bound: Some(output[0] as f64 * 1.05),
                features: None,
            };
            results.push(result);
        }

        Ok(results)
    }

    /// Update recurrent state for LSTM/GRU models
    pub fn update_recurrent_state(&self, model_name: &str, state: RecurrentState) {
        self.recurrent_states.insert(model_name.to_string(), state);
    }

    /// Get recurrent state for model
    pub fn get_recurrent_state(&self, model_name: &str) -> Option<RecurrentState> {
        self.recurrent_states.get(model_name).map(|entry| entry.clone())
    }

    /// Clean up inactive networks
    pub async fn cleanup_inactive_networks(&self) {
        let mut networks = self.active_networks.write().await;
        // In a real implementation, you'd check last access time and remove old networks
        // For now, we'll keep all networks for simplicity
    }
}