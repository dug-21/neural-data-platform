//! Core predictor implementation
//!
//! Contains the main FannPredictor struct and its primary prediction logic,
//! coordinating with other modules for specialized functionality.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info, warn};

use super::{NetworkManager, OnlineTrainingManager, DataConverter, NetworkCache, NetworkFactory, ModelPersistence};
use crate::adapters::enhanced_neural_adapter::EnhancedNeuralAdapter;
use crate::adapters::DataAdapter;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::{PredictionResult, NeuralPredictorTrait, PerformanceChannel, PerformanceEvent};

/// Model configuration for network creation
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ModelConfig {
    pub input_size: usize,
    pub output_size: usize,
    pub hidden_layers: Vec<usize>,
    pub learning_rate: f32,
    pub horizon: usize,
}

impl ModelConfig {
    pub fn default() -> Self {
        Self {
            input_size: 24,
            output_size: 1,
            hidden_layers: vec![64, 32],
            learning_rate: 0.001,
            horizon: 1,
        }
    }
}

/// Training result with performance metrics
#[derive(Debug, Clone)]
pub struct TrainingResult {
    pub epochs_trained: u32,
    pub final_error: f32,
    pub training_time: std::time::Duration,
    pub converged: bool,
}

/// Main FANN predictor with modular architecture
pub struct FannPredictor {
    config: NeuralConfig,
    network_manager: Arc<NetworkManager>,
    training_manager: Arc<OnlineTrainingManager>,
    data_converter: Arc<DataConverter>,
    network_cache: Arc<NetworkCache>,
    network_factory: Arc<NetworkFactory>,
    model_persistence: Arc<ModelPersistence>,
    enhanced_adapter: Option<Arc<EnhancedNeuralAdapter>>,
    performance_channel: Option<PerformanceChannel>,
}

impl FannPredictor {
    /// Create new FannPredictor with modular components
    pub fn new(mut config: NeuralConfig) -> Result<Self> {
        // Always respect the environment variable if set
        if let Ok(env_value) = std::env::var("NEURAL_USE_REAL_MODELS") {
            match env_value.to_lowercase().as_str() {
                "true" | "1" | "yes" => config.use_real_models = true,
                "false" | "0" | "no" => config.use_real_models = false,
                _ => {} // Keep the config value if env var is invalid
            }
            info!("🔧 FannPredictor: Overriding use_real_models from env: {}", config.use_real_models);
        }

        // Initialize modular components
        let network_factory = Arc::new(NetworkFactory::new(config.clone())?);
        let network_cache = Arc::new(NetworkCache::new());
        let data_converter = Arc::new(DataConverter::new());
        let network_manager = Arc::new(NetworkManager::new(
            Arc::clone(&network_factory),
            Arc::clone(&network_cache),
        )?);
        let training_manager = Arc::new(OnlineTrainingManager::new(config.clone())?);
        let model_persistence = Arc::new(ModelPersistence::new(config.clone())?);

        // Initialize enhanced adapter if real models are enabled
        let enhanced_adapter = if config.use_real_models {
            match EnhancedNeuralAdapter::new(crate::adapters::enhanced_neural_adapter::EnhancedNeuralConfig {
                neural: config.clone(),
                use_real_models: true,
                enable_health_monitoring: config.enable_health_checks,
                enable_fallback: config.enable_fallback,
                enable_caching: true,
                enable_circuit_breakers: config.enable_circuit_breakers,
                ..Default::default()
            }).await {
                Ok(adapter) => {
                    info!("✅ Enhanced neural adapter initialized successfully");
                    Some(Arc::new(adapter))
                }
                Err(e) => {
                    warn!("⚠️ Failed to initialize enhanced adapter, falling back to FANN-only: {}", e);
                    None
                }
            }
        } else {
            info!("🔧 Enhanced adapter disabled - using FANN-only mode");
            None
        };

        Ok(Self {
            config,
            network_manager,
            training_manager,
            data_converter,
            network_cache,
            network_factory,
            model_persistence,
            enhanced_adapter,
            performance_channel: None,
        })
    }

    /// Get model configurations
    pub fn get_model_configs(&self) -> HashMap<String, crate::neural::predictor::factory::FannModelConfig> {
        self.network_factory.get_model_configs()
    }

    /// Set performance channel for feedback loop
    pub fn set_performance_channel(&mut self, channel: PerformanceChannel) {
        self.performance_channel = Some(channel);
    }
}

#[async_trait::async_trait]
impl NeuralPredictorTrait for FannPredictor {
    /// Main prediction method coordinating all modules
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        debug!("🔮 Starting prediction with horizon: {}", horizon);

        // Convert data using the data converter
        let converted_data = self.data_converter.convert_to_fann_input(data, horizon)?;

        // Try enhanced adapter first if available
        if let Some(ref enhanced_adapter) = self.enhanced_adapter {
            match enhanced_adapter.predict(data, horizon, features.clone()).await {
                Ok(results) => {
                    info!("✅ Enhanced adapter prediction successful");
                    return Ok(results);
                }
                Err(e) => {
                    warn!("⚠️ Enhanced adapter failed, falling back to FANN: {}", e);
                }
            }
        }

        // Fallback to FANN-based prediction using network manager
        self.predict_with_fann(&converted_data, horizon).await
    }

    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        // Use model persistence for ensemble coordination
        self.model_persistence.predict_ensemble(data, horizon, models, features).await
    }

    async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        // Delegate to training manager
        self.training_manager.get_feature_importance().await
    }
}

impl FannPredictor {
    /// Internal FANN prediction method
    async fn predict_with_fann(
        &self,
        converted_data: &[f32],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        // Use network manager for FANN-based predictions
        self.network_manager.predict(converted_data, horizon).await
    }
}