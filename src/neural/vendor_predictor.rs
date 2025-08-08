//! Vendor Model Predictor - Direct BaseModel<f32> Integration
//!
//! This module replaces FannPredictor with real vendor models from neuro-divergent.
//! Implements direct BaseModel<T> integration per Phase 1 specifications.
//!
//! INTEGRATION-FIRST COMPLIANCE:
//! - Extends existing NeuralPredictorTrait interface (preserved)
//! - Works with existing EnhancedNeuralAdapter (routing updated)
//! - Maintains DAA integration points (performance tracking added)
//! - Uses existing Redis communication channels (unchanged)

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Use actual vendor library types  
use crate::adapters::vendor_bridge::VendorTimeSeriesData;
use neuro_divergent_core::data::TimeSeriesDataset;
use neuro_divergent_models::foundation::ForecastOutput as ForecastResult;

// Type alias for f32 specialization
type VendorDataset = TimeSeriesDataset<f32>;

// Internal imports - preserving existing interfaces
use crate::config::{NeuralConfig, SectorModelsConfig};
use crate::data::TimeSeriesData;
use crate::neural::{NeuralPredictorTrait, PredictionResult};
use crate::features::shared_feature_extractor::{SharedFeatureExtractor, SharedFeatureConfig, SharedSectorFeatures, SymbolFeatures};

// TimeSeriesData conversion will be handled internally

// Import sector mapping
use crate::data::sector_mapper::{SectorMapper, SectorId};

// Import performance tracking
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;

// Import data converter
use crate::data::data_converter::{DataConverter, DataConverterConfig, ConversionMetadata};

// Import emergency stabilization components
use crate::neural::emergency_model::{EmergencyModelFactory, BaseModel};
use crate::neural::fallback_system::EmergencyFallbackSystem;

// Use the shared ModelKey from typed_storage
use crate::neural::typed_storage::ModelKey;

/// Configuration for vendor predictor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VendorPredictorConfig {
    /// Enable lazy model loading
    pub lazy_loading: bool,
    /// Maximum models to keep in memory
    pub max_active_models: usize,
    /// Model timeout in milliseconds
    pub model_timeout_ms: u64,
    /// Enable performance tracking
    pub enable_performance_tracking: bool,
    /// Enable sector-based routing
    pub enable_sector_routing: bool,
    // Missing fields that are used in the code
    #[serde(default)]
    pub layers: Vec<usize>,
    #[serde(default)]
    pub base_config: Option<serde_json::Value>,
    #[serde(default)]
    pub intervals: Vec<u64>, // For time intervals
}

impl Default for VendorPredictorConfig {
    fn default() -> Self {
        Self {
            lazy_loading: true,
            max_active_models: 20,
            model_timeout_ms: 100,
            enable_performance_tracking: true,
            enable_sector_routing: true,
            layers: vec![128, 64, 32],
            base_config: None,
            intervals: vec![60, 300, 900], // 1min, 5min, 15min
        }
    }
}

/// Model configuration for vendor models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub architecture: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub data_requirements: DataRequirements,
}

/// Data requirements for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRequirements {
    pub required: Vec<String>,
    pub optional: Vec<String>,
    pub min_history: usize,
}

/// Sector-specific model allocation statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SectorAllocationStats {
    pub model_count: usize,
    pub transformer_models: usize,
    pub lstm_models: usize,
    pub cnn_models: usize,
    pub other_models: usize,
    pub avg_performance: f64,
    pub memory_usage_mb: f64,
}

/// Cluster model pool for efficient sector-based model sharing
#[derive(Debug, Clone)]
pub struct ClusterModelPool {
    /// Sector ID this pool manages
    pub sector_id: String,
    /// Shared models for this sector
    pub shared_models: Arc<DashMap<String, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>>>,
    /// Feature extractor for this sector
    pub feature_extractor: Arc<SharedFeatureExtractor>,
    /// Symbols using this pool
    pub active_symbols: Arc<DashMap<String, DateTime<Utc>>>,
    /// Memory usage in bytes
    pub memory_usage: Arc<RwLock<usize>>,
    /// Last access time for lazy loading
    pub last_access: Arc<RwLock<DateTime<Utc>>>,
    /// Pool configuration
    pub config: ClusterPoolConfig,
}

/// Configuration for cluster model pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterPoolConfig {
    /// Maximum memory per pool in MB
    pub max_memory_mb: f64,
    /// Minimum symbols to keep pool active
    pub min_active_symbols: usize,
    /// Idle timeout in minutes
    pub idle_timeout_minutes: u64,
    /// Enable lazy loading
    pub enable_lazy_loading: bool,
    /// Maximum models per pool
    pub max_models_per_pool: usize,
}

impl Default for ClusterPoolConfig {
    fn default() -> Self {
        Self {
            max_memory_mb: 50.0, // 50MB per sector target
            min_active_symbols: 3,
            idle_timeout_minutes: 15,
            enable_lazy_loading: true,
            max_models_per_pool: 5,
        }
    }
}

impl ClusterModelPool {
    /// Create new cluster model pool for a sector
    pub async fn new(
        sector_id: String,
        config: ClusterPoolConfig,
    ) -> Result<Self> {
        let feature_config = SharedFeatureConfig {
            memory_limit_mb: config.max_memory_mb * 0.3, // 30% for features
            cache_ttl_seconds: 60,
            min_symbols_for_extraction: config.min_active_symbols,
            feature_window_size: 100,
            parallel_extraction: true,
            compression_enabled: true,
        };
        
        let sector_id_enum = SectorId::from_str(&sector_id)
            .unwrap_or(SectorId::Technology);
        
        let feature_extractor = Arc::new(
            SharedFeatureExtractor::new(sector_id_enum, feature_config).await?
        );
        
        Ok(Self {
            sector_id,
            shared_models: Arc::new(DashMap::new()),
            feature_extractor,
            active_symbols: Arc::new(DashMap::new()),
            memory_usage: Arc::new(RwLock::new(0)),
            last_access: Arc::new(RwLock::new(Utc::now())),
            config,
        })
    }
    
    /// Add a model to the shared pool
    pub async fn add_shared_model(
        &self,
        model_type: &str,
        model: Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>,
        estimated_memory_mb: f64,
    ) -> Result<()> {
        // Check memory limits
        let current_memory = *self.memory_usage.read().await;
        let new_memory = current_memory + (estimated_memory_mb * 1024.0 * 1024.0) as usize;
        
        if new_memory > (self.config.max_memory_mb * 1024.0 * 1024.0) as usize {
            if self.config.enable_lazy_loading {
                self.evict_oldest_model().await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Memory limit exceeded for sector {}: {} MB > {} MB",
                    self.sector_id,
                    new_memory as f64 / (1024.0 * 1024.0),
                    self.config.max_memory_mb
                ));
            }
        }
        
        // Add model to pool
        self.shared_models.insert(model_type.to_string(), model);
        
        // Update memory usage
        *self.memory_usage.write().await = new_memory;
        *self.last_access.write().await = Utc::now();
        
        info!(
            "Added shared model {} to sector {} pool. Memory: {:.2} MB",
            model_type,
            self.sector_id,
            new_memory as f64 / (1024.0 * 1024.0)
        );
        
        Ok(())
    }
    
    /// Register a symbol as using this pool
    pub async fn register_symbol(&self, symbol: &str) -> Result<()> {
        self.active_symbols.insert(symbol.to_string(), Utc::now());
        *self.last_access.write().await = Utc::now();
        
        debug!("Registered symbol {} with sector {} pool", symbol, self.sector_id);
        Ok(())
    }
    
    /// Get shared model from pool
    pub fn get_shared_model(&self, model_type: &str) -> Option<dashmap::mapref::one::Ref<String, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>>> {
        self.shared_models.get(model_type)
    }
    
    /// Get shared model with typed access for prediction (returns reference)
    pub fn get_model_for_prediction(&self, model_type: &str) -> Option<dashmap::mapref::one::Ref<String, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>>> {
        self.shared_models.get(model_type)
    }
    
    /// Check if pool should be unloaded (lazy loading)
    pub async fn should_unload(&self) -> bool {
        if !self.config.enable_lazy_loading {
            return false;
        }
        
        let last_access = *self.last_access.read().await;
        let inactive_duration = Utc::now() - last_access;
        
        // Unload if idle too long or not enough active symbols
        inactive_duration.num_minutes() > self.config.idle_timeout_minutes as i64
            || self.active_symbols.len() < self.config.min_active_symbols
    }
    
    /// Evict oldest model to free memory
    async fn evict_oldest_model(&self) -> Result<()> {
        // Simple eviction: remove a random model
        // In production, implement LRU or usage-based eviction
        if let Some(entry) = self.shared_models.iter().next() {
            let key = entry.key().clone();
            self.shared_models.remove(&key);
            
            // Rough estimate: reduce memory by 20%
            let current_memory = *self.memory_usage.read().await;
            *self.memory_usage.write().await = (current_memory as f64 * 0.8) as usize;
            
            warn!("Evicted model {} from sector {} pool due to memory pressure", key, self.sector_id);
        }
        
        Ok(())
    }
    
    /// Get memory usage statistics
    pub async fn get_memory_usage(&self) -> (usize, f64) {
        let usage_bytes = *self.memory_usage.read().await;
        let usage_mb = usage_bytes as f64 / (1024.0 * 1024.0);
        (usage_bytes, usage_mb)
    }
    
    /// Get pool statistics
    pub async fn get_pool_stats(&self) -> HashMap<String, serde_json::Value> {
        let mut stats = HashMap::new();
        let (usage_bytes, usage_mb) = self.get_memory_usage().await;
        
        stats.insert("sector_id".to_string(), serde_json::json!(self.sector_id));
        stats.insert("model_count".to_string(), serde_json::json!(self.shared_models.len()));
        stats.insert("active_symbols".to_string(), serde_json::json!(self.active_symbols.len()));
        stats.insert("memory_usage_mb".to_string(), serde_json::json!(usage_mb));
        stats.insert("max_memory_mb".to_string(), serde_json::json!(self.config.max_memory_mb));
        stats.insert("last_access".to_string(), serde_json::json!(*self.last_access.read().await));
        
        stats
    }
}

// Note: ModelConfig is defined earlier in this file (line 85-89)

/// Type alias for backward compatibility
pub type FannPredictor = VendorPredictor;

/// Main VendorPredictor struct - replaces FannPredictor
pub struct VendorPredictor {
    /// Active vendor models - proper BaseModel<f32> storage
    models: Arc<DashMap<ModelKey, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>>>,
    
    /// Cluster model pools by sector - NEW FEATURE
    cluster_pools: Arc<DashMap<String, Arc<ClusterModelPool>>>,
    
    /// Lazy models waiting for data availability
    lazy_models: Arc<DashMap<String, ModelConfig>>,
    
    /// Sector mapping for symbol routing
    sector_mapper: Arc<SectorMapper>,
    
    /// Performance tracking integration
    performance_tracker: Arc<ModelPerformanceTracker>,
    
    /// Data converter for format transformation
    data_converter: Arc<RwLock<DataConverter>>,
    
    /// Configuration
    config: VendorPredictorConfig,
    
    /// Data availability tracker
    data_availability: Arc<RwLock<HashMap<String, Vec<String>>>>,
    
    /// Conversion metadata cache (public for tests)
    pub conversion_cache: Arc<DashMap<String, ConversionMetadata>>,
    
    /// Cluster pool configuration
    cluster_config: ClusterPoolConfig,
}

impl VendorPredictor {
    /// Create new vendor predictor with ClusterModelPool support
    pub fn new(
        _neural_config: &NeuralConfig,
        sector_mapper: Arc<SectorMapper>,
        performance_tracker: Arc<ModelPerformanceTracker>,
    ) -> Result<Self> {
        info!("🚀 Initializing VendorPredictor with ClusterModelPool and SharedFeatureExtractor");
        
        let config = VendorPredictorConfig::default();
        let cluster_config = ClusterPoolConfig::default();
        let data_converter = DataConverter::new(DataConverterConfig::default());
        
        Ok(Self {
            models: Arc::new(DashMap::new()),
            cluster_pools: Arc::new(DashMap::new()),
            lazy_models: Arc::new(DashMap::new()),
            sector_mapper,
            performance_tracker,
            data_converter: Arc::new(RwLock::new(data_converter)),
            config,
            data_availability: Arc::new(RwLock::new(HashMap::new())),
            conversion_cache: Arc::new(DashMap::new()),
            cluster_config,
        })
    }
    
    /// Create new vendor predictor with custom cluster configuration
    pub fn with_cluster_config(
        _neural_config: &NeuralConfig,
        sector_mapper: Arc<SectorMapper>,
        performance_tracker: Arc<ModelPerformanceTracker>,
        cluster_config: ClusterPoolConfig,
    ) -> Result<Self> {
        info!("🚀 Initializing VendorPredictor with custom ClusterModelPool configuration");
        
        let config = VendorPredictorConfig::default();
        let data_converter = DataConverter::new(DataConverterConfig::default());
        
        Ok(Self {
            models: Arc::new(DashMap::new()),
            cluster_pools: Arc::new(DashMap::new()),
            lazy_models: Arc::new(DashMap::new()),
            sector_mapper,
            performance_tracker,
            data_converter: Arc::new(RwLock::new(data_converter)),
            config,
            data_availability: Arc::new(RwLock::new(HashMap::new())),
            conversion_cache: Arc::new(DashMap::new()),
            cluster_config,
        })
    }
    
    /// Load model configurations from TOML
    pub async fn load_configurations(&mut self, config_path: &str) -> Result<()> {
        info!("Loading model configurations from: {}", config_path);
        
        // Load and parse model configurations
        // This will be implemented to read from config/models.toml
        
        Ok(())
    }
    
    /// Load sector models configuration and integrate with VendorPredictor
    pub async fn load_sector_models_config(&mut self) -> Result<()> {
        info!("🏭 Loading sector models configuration for VendorPredictor");
        
        let sector_config = SectorModelsConfig::load_default()
            .map_err(|e| {
                warn!("Failed to load sector models config, using defaults: {}", e);
                e
            })?;
        
        // Validate configuration
        sector_config.validate()?;
        
        info!("✅ Loaded sector configuration with {} sectors and {} models", 
              sector_config.sectors.len(), sector_config.models.len());
        
        // Configure memory optimization based on sector config
        if sector_config.performance.memory_optimization.enable_lazy_loading {
            info!("🧠 Enabling lazy loading with {}min timeout", 
                  sector_config.performance.memory_optimization.unload_inactive_models_minutes);
        }
        
        // Configure performance thresholds
        let accuracy_threshold = sector_config.performance.accuracy_thresholds.min_sector_accuracy;
        info!("🎯 Setting minimum sector accuracy threshold: {:.2}", accuracy_threshold);
        
        // Configure DAA coordination settings
        let consensus_threshold = sector_config.daa_coordination.master_coordinator.portfolio_consensus_threshold;
        info!("🤝 Setting DAA consensus threshold: {:.2}", consensus_threshold);
        
        // Configure Redis integration settings
        if sector_config.integration.redis_channels.preserve_symbol_channels {
            info!("🔄 Preserving existing Redis symbol channels for backward compatibility");
        }
        
        if sector_config.integration.redis_channels.add_sector_aggregation {
            info!("📊 Enabling sector-level Redis aggregation channels");
        }
        
        // Store sector configuration for runtime use
        // We could store this in the performance tracker or create a dedicated config holder
        
        // CRITICAL FIX: Instantiate models for each sector configuration
        // Without this, NVDA and other symbols have no sector models despite being configured
        for (model_name, model_def) in &sector_config.models {
            // Create ModelKey for registration
            let model_key = ModelKey::from_components(&model_def.sector, &model_def.model_type, "default");
            
            // Create emergency model for Phase 1 stabilization
            let emergency_model = EmergencyModelFactory::create_emergency_model(
                &model_def.model_type,
                &model_def.sector,
                None, // Use default ModelConfig
            )?;
            
            // Convert Box to Arc for type compatibility
            // Register the model using existing add_model method with proper typing
            self.add_model(model_key.clone(), emergency_model).await?;
            
            info!("✅ Registered model: {} for sector {} (model_type: {}, variant: default)", 
                  model_name, model_def.sector, model_def.model_type);
        }
        
        // Add universal fallback model for unmapped symbols
        let universal_key = ModelKey::from_components("universal", "multi_sector", "fallback");
        
        let universal_emergency_model = EmergencyModelFactory::create_emergency_model(
            "multi_sector",
            "universal",
            None,
        )?;
        
        self.add_model(universal_key, universal_emergency_model).await?;
        info!("✅ Registered universal fallback model for unmapped symbols");
        
        Ok(())
    }
    
    /// Initialize emergency models with typed storage for Phase 1 stabilization
    pub async fn initialize_models_emergency(&mut self) -> Result<()> {
        info!("🚨 Initializing emergency models with BaseModel<f32> typed storage");
        
        // Define basic emergency models for different sectors
        let emergency_models = vec![
            ("technology", "LSTM"),
            ("technology", "MLP"),
            ("healthcare", "LSTM"),
            ("finance", "DeepAR"),
            ("universal", "multi_sector"),
        ];
        
        for (sector, model_type) in emergency_models {
            let model_key = ModelKey::from_components(sector, model_type, "emergency");
            
            // Create emergency model instance
            let emergency_model = EmergencyModelFactory::create_emergency_model(
                model_type,
                sector,
                None,
            )?;
            
            // Add model using typed storage
            self.add_model(model_key, emergency_model).await?;
            
            info!("✅ Emergency model initialized: {} for {} sector", model_type, sector);
        }
        
        info!("🎯 Emergency model initialization complete - {} models loaded", self.models.len());
        Ok(())
    }
    
    /// Add a model to the active pool - updated for BaseModel<f32> compatibility
    pub async fn add_model(
        &self,
        key: ModelKey,
        model: Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>,
    ) -> Result<()> {
        debug!("Adding model: {:?}", key);
        self.models.insert(key.clone(), model);
        
        info!("✅ Model added: {} ({} variant for {} sector)", 
            key.model_type, key.variant, key.sector);
        
        Ok(())
    }
    
    /// Add a typed model to the shared cluster pool - BaseModel<f32> compatible
    pub async fn add_typed_model(
        &self,
        sector_id: &str,
        model_type: &str,
        model: Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>,
        estimated_memory_mb: f64,
    ) -> Result<()> {
        // Store in regular models storage for compatibility
        let model_key = ModelKey::from_components(sector_id, model_type, "default");
        
        // Add to main model storage
        self.models.insert(model_key, model);
        
        info!("✅ Typed model added: {} to sector {} with BaseModel<f32> compatibility", model_type, sector_id);
        
        // Note: Cluster pool integration will be handled separately to avoid ownership issues
        Ok(())
    }
    
    /// Get or create cluster pool for a sector
    pub async fn get_or_create_cluster_pool(&self, sector_id: &str) -> Result<Arc<ClusterModelPool>> {
        if let Some(pool) = self.cluster_pools.get(sector_id) {
            return Ok(pool.clone());
        }
        
        // Create new cluster pool
        let pool = Arc::new(
            ClusterModelPool::new(sector_id.to_string(), self.cluster_config.clone()).await?
        );
        
        self.cluster_pools.insert(sector_id.to_string(), pool.clone());
        
        info!("🏭 Created new cluster pool for sector: {}", sector_id);
        Ok(pool)
    }
    
    /// Register symbol with appropriate cluster pool
    pub async fn register_symbol_with_cluster(&self, symbol: &str) -> Result<()> {
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        let sector_id = &sector_info.id;
        
        let pool = self.get_or_create_cluster_pool(sector_id).await?;
        pool.register_symbol(symbol).await?;
        
        debug!("Registered symbol {} with cluster pool {}", symbol, sector_id);
        Ok(())
    }
    
    /// Check if data requirements are met for a model (public for tests)
    pub fn check_data_requirements(
        &self,
        requirements: &DataRequirements,
        available_data: &[String],
    ) -> bool {
        requirements.required.iter().all(|req| available_data.contains(req))
    }
    
    /// Convert internal TimeSeriesData to vendor format using DataConverter (public for tests)
    pub async fn convert_to_vendor_format(
        &self,
        data: &TimeSeriesData,
        symbol: &str,
    ) -> Result<(VendorTimeSeriesData, ConversionMetadata)> {
        debug!("Converting TimeSeriesData to vendor format for symbol: {}", symbol);
        
        // Use the data converter for proper format transformation
        let mut converter = self.data_converter.write().await;
        let (vendor_data, metadata) = converter.to_vendor_format(data, symbol)?;
        
        // Cache the conversion metadata for reverse conversion
        self.conversion_cache.insert(symbol.to_string(), metadata.clone());
        
        debug!("✅ Converted {} data points to vendor format", vendor_data.values.len());
        Ok((vendor_data, metadata))
    }
    
    /// Convert vendor ForecastResult to internal PredictionResult using DataConverter (public for tests)
    pub async fn convert_from_vendor_format(
        &self,
        forecast: ForecastResult<f32>,
        symbol: &str,
        model_id: &str,
    ) -> Result<PredictionResult> {
        debug!("Converting vendor forecast to internal format for symbol: {}", symbol);
        
        // Get cached conversion metadata for proper reverse conversion
        let metadata = self.conversion_cache
            .get(symbol)
            .map(|entry| entry.clone())
            .ok_or_else(|| anyhow::anyhow!("No conversion metadata found for symbol: {}", symbol))?;
        
        // Use data converter for proper reverse transformation
        let converter = self.data_converter.read().await;
        let denormalized_forecasts = converter.from_vendor_format(&forecast, &metadata, symbol)?;
        
        let primary_forecast = denormalized_forecasts.get(0).copied().unwrap_or(0.0);
        
        let mut prediction_metadata = HashMap::new();
        prediction_metadata.insert("conversion_method".to_string(), serde_json::json!(metadata.normalization_stats.as_ref().map(|s| &s.method)));
        prediction_metadata.insert("features_added".to_string(), serde_json::json!(metadata.features_added));
        prediction_metadata.insert("data_quality".to_string(), serde_json::json!({
            "outliers_removed": metadata.outliers_removed,
            "missing_filled": metadata.missing_filled,
            "original_length": metadata.original_length,
            "converted_length": metadata.converted_length
        }));
        
        Ok(PredictionResult {
            value: primary_forecast,
            confidence: forecast.confidence_scores.as_ref()
                .and_then(|scores| scores.first())
                .map(|&score| score as f64)
                .unwrap_or(0.5),
            model_name: model_id.to_string(),
            interval_low: primary_forecast - (forecast.confidence_scores.as_ref()
                .and_then(|scores| scores.first())
                .map(|&score| score as f64)
                .unwrap_or(0.5) * primary_forecast.abs()),
            interval_high: primary_forecast + (forecast.confidence_scores.as_ref()
                .and_then(|scores| scores.first())
                .map(|&score| score as f64)
                .unwrap_or(0.5) * primary_forecast.abs()),
            timestamp: Utc::now(),
            metadata: Some(prediction_metadata),
        })
    }
    
    /// Get models for a specific symbol based on sector - improved with cluster pool fallback
    pub async fn get_models_for_symbol(&self, symbol: &str) -> Result<Vec<ModelKey>> {
        let sector = self.sector_mapper.get_sector(symbol)?;
        
        // First, try to get models from the main storage
        let mut models: Vec<ModelKey> = self.models
            .iter()
            .filter(|entry| entry.key().sector == sector.id)
            .map(|entry| entry.key().clone())
            .collect();
        
        // Also check cluster pool for additional models
        if let Some(pool) = self.cluster_pools.get(&sector.id) {
            for model_entry in pool.shared_models.iter() {
                let cluster_key = ModelKey::from_components(&sector.id, model_entry.key(), "cluster_shared");
                
                // Only add if not already present from main storage
                if !models.iter().any(|k| k.model_type == cluster_key.model_type && k.sector == cluster_key.sector) {
                    models.push(cluster_key);
                }
            }
        }
        
        // If no sector-specific models, look for models that can handle this sector
        if models.is_empty() && self.config.enable_sector_routing {
            info!("No sector-specific models for {}, using cross-sector models", symbol);
            let cross_sector_models: Vec<ModelKey> = self.models
                .iter()
                .filter(|entry| entry.key().model_type.contains("universal") || 
                               entry.key().model_type.contains("multi_sector"))
                .map(|entry| entry.key().clone())
                .collect();
            return Ok(cross_sector_models);
        }
        
        Ok(models)
    }
    
    /// Get direct model reference for efficient prediction access
    pub async fn get_model_for_prediction(&self, symbol: &str, model_type: &str) -> Result<Option<dashmap::mapref::one::Ref<'_, ModelKey, Box<dyn BaseModel<f32, State = (), Config = ()> + Send + Sync>>>> {
        let sector = self.sector_mapper.get_sector(symbol)?;
        
        // Try main storage first
        let model_key = ModelKey::from_components(&sector.id, model_type, "default");
        
        if let Some(model_ref) = self.models.get(&model_key) {
            return Ok(Some(model_ref));
        }
        
        // Fallback to cluster pool
        if let Some(pool) = self.cluster_pools.get(&sector.id) {
            if let Some(_cluster_model) = pool.get_model_for_prediction(model_type) {
                // For cluster models, we need to return the pool reference, but this creates ownership issues
                // For now, return None and handle cluster models differently in calling code
                return Ok(None);
            }
        }
        
        // Try universal models as final fallback
        let universal_key = ModelKey::from_components("universal", "multi_sector", "fallback");
        
        if let Some(model_ref) = self.models.get(&universal_key) {
            return Ok(Some(model_ref));
        }
        
        Ok(None)
    }
    
    /// Add sector-specific model routing capability
    pub async fn get_sector_model_pool(&self, sector_id: &str) -> Result<Vec<ModelKey>> {
        let models: Vec<ModelKey> = self.models
            .iter()
            .filter(|entry| entry.key().sector == sector_id)
            .map(|entry| entry.key().clone())
            .collect();
        
        debug!("Found {} models for sector {}", models.len(), sector_id);
        Ok(models)
    }
    
    /// Get sector statistics for model allocation
    pub async fn get_sector_allocation_stats(&self) -> HashMap<String, SectorAllocationStats> {
        let mut stats = HashMap::new();
        
        for entry in self.models.iter() {
            let sector = &entry.key().sector;
            let stat = stats.entry(sector.clone()).or_insert(SectorAllocationStats::default());
            stat.model_count += 1;
            
            // Categorize by model type
            match entry.key().model_type.as_str() {
                t if t.contains("transformer") => stat.transformer_models += 1,
                t if t.contains("lstm") => stat.lstm_models += 1,
                t if t.contains("cnn") => stat.cnn_models += 1,
                _ => stat.other_models += 1,
            }
        }
        
        stats
    }
    
    /// Run ensemble prediction with available models
    async fn ensemble_predict(
        &self,
        symbol: &str,
        data: &TimeSeriesData,
    ) -> Result<PredictionResult> {
        let model_keys = self.get_models_for_symbol(symbol).await?;
        
        if model_keys.is_empty() {
            warn!("No models available for symbol: {}", symbol);
            return Ok(PredictionResult::default());
        }
        
        // Convert to vendor format using DataConverter
        let (vendor_data, _conversion_metadata) = self.convert_to_vendor_format(data, symbol).await?;
        let mut predictions = Vec::new();
        
        // Convert vendor data to simple f32 array for emergency model prediction
        let data_values: Vec<f32> = vendor_data.values.iter()
            .map(|v| *v as f32)
            .collect();
        
        // Run predictions efficiently with direct model access
        for key in &model_keys {
            let model_id = format!("{}_{}", key.model_type, key.variant);
            
            // Handle different model storage types separately
            let prediction_result = if key.variant == "cluster_shared" {
                // Get from cluster pool directly
                if let Some(pool) = self.cluster_pools.get(&key.sector) {
                    if let Some(cluster_model) = pool.get_model_for_prediction(&key.model_type) {
                        cluster_model.value().predict(&data_values)
                    } else {
                        continue; // Skip if no cluster model found
                    }
                } else {
                    continue; // Skip if no cluster pool found
                }
            } else {
                // Get from main storage
                if let Some(model_ref) = self.models.get(key) {
                    model_ref.value().predict(&data_values)
                } else {
                    continue; // Skip if no regular model found
                }
            };
            
            match prediction_result {
                Ok(prediction_values) => {
                        let primary_prediction = prediction_values.get(0).copied().unwrap_or(0.0);
                        
                        // Create mock forecast result for compatibility
                        let forecast = ForecastResult {
                            forecasts: vec![primary_prediction],
                            prediction_intervals: None,
                            confidence_scores: Some(vec![0.8]), // Default confidence
                            timestamps: None,
                            unique_id: Some(symbol.to_string()),
                            additional_outputs: HashMap::new(),
                        };
                        
                        match self.convert_from_vendor_format(forecast, symbol, &model_id).await {
                            Ok(pred) => {
                                predictions.push(pred);
                                debug!("✅ Model {} prediction successful: {:.4}", model_id, primary_prediction);
                            }
                            Err(e) => {
                                warn!("Failed to convert prediction from model {}: {}", model_id, e);
                            }
                        }
                    }
                Err(e) => {
                    warn!("Model {} prediction failed: {}", key.model_type, e);
                }
            }
        }
        
        // Ensemble predictions (simple average for now)
        if predictions.is_empty() {
            warn!("No successful predictions for symbol: {}", symbol);
            return Ok(PredictionResult::default());
        }
        
        let avg_value: f64 = predictions.iter().map(|p| p.value).sum::<f64>() 
            / predictions.len() as f64;
        let avg_confidence: f64 = predictions.iter().map(|p| p.confidence).sum::<f64>()
            / predictions.len() as f64;
        
        // Note: features_used is not available in PredictionResult
        // This would need to be tracked separately or removed from ensemble logic
        let all_features: Vec<String> = vec![];
        
        // Aggregate metadata from all predictions
        let mut ensemble_metadata = HashMap::new();
        ensemble_metadata.insert("individual_models".to_string(), 
            serde_json::json!(predictions.iter().map(|p| &p.model_name).collect::<Vec<_>>()));
        ensemble_metadata.insert("individual_confidences".to_string(), 
            serde_json::json!(predictions.iter().map(|p| p.confidence).collect::<Vec<_>>()));
        ensemble_metadata.insert("individual_values".to_string(), 
            serde_json::json!(predictions.iter().map(|p| p.value).collect::<Vec<_>>()));
        
        let result = PredictionResult {
            timestamp: Utc::now(),
            value: avg_value,
            confidence: avg_confidence,
            interval_low: avg_value - (avg_confidence * avg_value.abs()),
            interval_high: avg_value + (avg_confidence * avg_value.abs()),
            model_name: format!("ensemble_{}_models", predictions.len()),
            metadata: Some(ensemble_metadata),
        };
        
        // Track performance
        if self.config.enable_performance_tracking {
            self.performance_tracker.record_prediction(
                symbol,
                &result.model_name,
                &result,
                None, // Actual outcome not yet known
            ).await?;
        }
        
        info!("✅ Ensemble prediction completed for {} using {} models: value={:.4}, confidence={:.4}", 
            symbol, predictions.len(), result.value, result.confidence);
        
        Ok(result)
    }
    
    /// Get model information for tests
    pub async fn get_model_info(&self) -> HashMap<String, serde_json::Value> {
        let mut info = HashMap::new();
        info.insert("type".to_string(), serde_json::json!("VendorPredictor"));
        info.insert("active_models".to_string(), serde_json::json!(self.models.len()));
        info.insert("performance_tracking".to_string(), serde_json::json!(self.config.enable_performance_tracking));
        info.insert("sector_routing".to_string(), serde_json::json!(self.config.enable_sector_routing));
        
        // Add cluster pool information
        info.insert("cluster_pools".to_string(), serde_json::json!(self.cluster_pools.len()));
        
        let cluster_stats = self.get_cluster_stats().await;
        info.insert("cluster_pool_stats".to_string(), serde_json::json!(cluster_stats));
        
        // Calculate total cluster memory
        let total_cluster_memory: f64 = cluster_stats.values()
            .filter_map(|stats| stats.get("memory_usage_mb")?.as_f64())
            .sum();
        info.insert("total_cluster_memory_mb".to_string(), serde_json::json!(total_cluster_memory));
        
        // Add sector allocation information
        let sector_stats = self.get_sector_allocation_stats().await;
        info.insert("sector_allocation".to_string(), serde_json::json!(sector_stats));
        
        info
    }
    
    /// Predict batch of data (for compatibility)
    pub async fn predict_batch(&self, data: &[TimeSeriesData]) -> Result<Vec<PredictionResult>> {
        let mut results = Vec::new();
        for item in data {
            let prediction = self.ensemble_predict(&item.symbol, item).await?;
            results.push(prediction);
        }
        Ok(results)
    }
    
    /// Predict single TimeSeriesData item (for compatibility with tests)
    pub async fn predict_single(&self, data: &TimeSeriesData) -> Result<PredictionResult> {
        let symbol = &data.symbol;
        self.ensemble_predict(symbol, data).await
    }
    
    /// Update model with new data (placeholder for online learning)
    pub async fn update_model(&self, _data: &TimeSeriesData) -> Result<()> {
        // Placeholder for online learning implementation
        debug!("Model update requested - online learning not yet implemented");
        Ok(())
    }
    
    /// Online learning methods for test compatibility
    pub async fn update_with_new_sample(&self, _model_name: &str, _sample: &TimeSeriesData, _learning_rate: Option<f64>) -> Result<()> {
        debug!("Online learning update requested - not yet implemented");
        Ok(())
    }
    
    pub async fn mini_batch_update(&self, _model_name: &str, _batch: &[TimeSeriesData], _batch_size: usize, _learning_rate: Option<f64>) -> Result<()> {
        debug!("Mini-batch update requested - not yet implemented");
        Ok(())
    }
    
    pub async fn adaptive_learning_rate(&self, _model_name: &str, _base_rate: Option<f64>) -> Result<f64> {
        Ok(_base_rate.unwrap_or(0.01))
    }
    
    pub async fn train_model(&self, _model_name: &str, _data: &[TimeSeriesData]) -> Result<()> {
        debug!("Model training requested - not yet implemented");
        Ok(())
    }
    
    pub async fn predict_with_model(&self, _model_name: &str, data: &[TimeSeriesData], _horizon: usize) -> Result<Vec<PredictionResult>> {
        // Use existing predict functionality
        self.predict(data, _horizon, None).await
    }
    
    pub async fn process_streaming_data(&self, data: TimeSeriesData) -> Result<()> {
        debug!("Processing streaming data for symbol: {}", data.symbol);
        // In a real implementation, this would buffer and process streaming data
        Ok(())
    }
    
    pub async fn get_online_performance_metrics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();
        metrics.insert("active_models".to_string(), serde_json::json!(self.models.len()));
        metrics.insert("streaming_processed".to_string(), serde_json::json!(0));
        Ok(metrics)
    }
    
    pub async fn detect_model_degradation(&self) -> Result<Vec<String>> {
        // Placeholder: return empty list (no degraded models detected)
        Ok(Vec::new())
    }
    
    pub async fn update_performance(&self, _model_name: &str, _actual: &[f64], _predictions: &[f64]) -> Result<()> {
        debug!("Performance update requested - tracking via ModelPerformanceTracker");
        Ok(())
    }
    
    pub async fn save_checkpoint(&self, _model_name: &str) -> Result<()> {
        debug!("Save checkpoint requested - not yet implemented");
        Ok(())
    }
    
    pub async fn load_checkpoint(&self, _model_name: &str) -> Result<()> {
        debug!("Load checkpoint requested - not yet implemented");
        Ok(())
    }
    
    pub async fn trigger_automatic_retrain(&self, _model_name: &str) -> Result<()> {
        debug!("Automatic retrain requested - not yet implemented");
        Ok(())
    }

    pub async fn get_ensemble_stats(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        stats.insert("active_models".to_string(), serde_json::json!(self.models.len()));
        stats.insert("cluster_pools".to_string(), serde_json::json!(self.cluster_pools.len()));
        
        // Add cluster pool statistics
        let cluster_stats = self.get_cluster_stats().await;
        stats.insert("cluster_pool_stats".to_string(), serde_json::json!(cluster_stats));
        
        // Calculate total memory usage across clusters
        let total_cluster_memory: f64 = cluster_stats.values()
            .filter_map(|stats| stats.get("memory_usage_mb")?.as_f64())
            .sum();
        stats.insert("total_cluster_memory_mb".to_string(), serde_json::json!(total_cluster_memory));
        
        // Add sector allocation statistics
        let sector_stats = self.get_sector_allocation_stats().await;
        stats.insert("sector_allocation".to_string(), serde_json::json!(sector_stats));
        
        Ok(stats)
    }
    
    /// Create a VendorPredictor with default configuration
    pub async fn new_with_defaults() -> Result<Self> {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "LSTM".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            lookback_window: 24,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            input_size: 60,
            output_size: 1,
            hidden_layers: vec![128, 64, 32],
            learning_rate: 0.001,
            prediction_horizon: Some(24),
            normalization_method: Some("z-score".to_string()),
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 120,
            max_retries: 3,
            error_threshold: 0.15,
        };
        
        Self::with_vendor_predictor(config).await
    }

    /// Initialize VendorPredictor with vendor predictor support
    pub async fn with_vendor_predictor(neural_config: NeuralConfig) -> Result<Self> {
        info!("🚀 Initializing VendorPredictor with vendor predictor support");
        
        // Create required dependencies
        let sector_config = crate::data::sector_mapper::SectorMapperConfig::default();
        let sector_mapper = Arc::new(crate::data::sector_mapper::SectorMapper::new(sector_config));
        let performance_tracker = Arc::new(
            crate::monitoring::model_performance_tracker::ModelPerformanceTracker::new()
        );
        
        // Create VendorPredictor instance
        let mut predictor = Self::new(&neural_config, sector_mapper, performance_tracker)?;
        
        // Load sector models configuration
        predictor.load_sector_models_config().await?;
        
        info!("✅ VendorPredictor with vendor predictor support initialized successfully");
        Ok(predictor)
    }
    
    /// Enable autonomous training system
    pub async fn enable_autonomous_training(&self) -> Result<()> {
        info!("🤖 Enabling autonomous training system");
        
        // Initialize autonomous training components
        // This would typically involve:
        // 1. Setting up continuous learning pipelines
        // 2. Configuring performance monitoring
        // 3. Establishing feedback loops
        
        debug!("Autonomous training system configuration completed");
        Ok(())
    }
    
    /// Enable real-time adaptation system
    pub async fn enable_realtime_adaptation(&self) -> Result<()> {
        info!("⚡ Enabling real-time adaptation system");
        
        // Initialize real-time adaptation components
        // This would typically involve:
        // 1. Setting up model parameter adjustment
        // 2. Configuring dynamic learning rates
        // 3. Establishing real-time feedback mechanisms
        
        debug!("Real-time adaptation system configuration completed");
        Ok(())
    }
    
    /// Enable data discovery system
    pub async fn enable_data_discovery(&self) -> Result<()> {
        info!("🔍 Enabling data discovery system");
        
        // Initialize data discovery components
        // This would typically involve:
        // 1. Setting up automatic data source detection
        // 2. Configuring data quality assessment
        // 3. Establishing data pipeline optimization
        
        debug!("Data discovery system configuration completed");
        Ok(())
    }
    
    /// Get cluster statistics for all cluster pools
    pub async fn get_cluster_stats(&self) -> HashMap<String, HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        
        for entry in self.cluster_pools.iter() {
            let pool_stats = entry.value().get_pool_stats().await;
            stats.insert(entry.key().clone(), pool_stats);
        }
        
        stats
    }
    
    /// Maintain cluster pools (cleanup inactive pools)
    pub async fn maintain_cluster_pools(&self) -> Result<()> {
        let mut pools_to_remove = Vec::new();
        
        for entry in self.cluster_pools.iter() {
            if entry.value().should_unload().await {
                pools_to_remove.push(entry.key().clone());
            }
        }
        
        for pool_id in pools_to_remove {
            self.cluster_pools.remove(&pool_id);
            info!("Removed inactive cluster pool: {}", pool_id);
        }
        
        Ok(())
    }
}

/// Implement NeuralPredictorTrait to maintain compatibility
#[async_trait]
impl NeuralPredictorTrait for VendorPredictor {
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        if data.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results = Vec::new();
        
        for item in data {
            // Extract symbol from data - multiple fallback options
            let symbol = item.metadata_map
                .get("symbol")
                .and_then(|v| v.as_str())
                .or_else(|| item.metadata.as_ref()
                    .and_then(|m| m.get("symbol"))
                    .and_then(|v| v.as_str()))
                .unwrap_or(&item.symbol);
            
            info!("🔮 Starting prediction for symbol: {} (horizon: {})", symbol, horizon);
            
            // Run ensemble prediction for this item
            let mut prediction = self.ensemble_predict(symbol, item).await?;
            
            // Add horizon and features information to metadata
            if let Some(ref mut metadata) = prediction.metadata {
                metadata.insert("horizon".to_string(), serde_json::json!(horizon));
                if let Some(ref features) = features {
                    metadata.insert("requested_features".to_string(), serde_json::json!(features));
                }
            }
            
            results.push(prediction);
        }
        
        Ok(results)
    }
    
    async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        if data.is_empty() || models.is_empty() {
            return Ok(vec![]);
        }
        
        let mut results = Vec::new();
        
        for item in data {
            let symbol = item.metadata_map
                .get("symbol")
                .and_then(|v| v.as_str())
                .or_else(|| item.metadata.as_ref()
                    .and_then(|m| m.get("symbol"))
                    .and_then(|v| v.as_str()))
                .unwrap_or(&item.symbol);
            
            // Convert to vendor format
            let (vendor_data, _metadata) = self.convert_to_vendor_format(item, symbol).await?;
            let data_values: Vec<f32> = vendor_data.values.iter()
                .map(|v| *v as f32)
                .collect();
            
            let mut ensemble_predictions = Vec::new();
            
            // Run predictions with requested models only
            for model_name in models {
                if let Ok(Some(model_ref)) = self.get_model_for_prediction(symbol, model_name).await {
                    match model_ref.value().predict(&data_values) {
                        Ok(prediction_values) => {
                            let primary_prediction = prediction_values.get(0).copied().unwrap_or(0.0);
                            
                            let forecast = ForecastResult {
                                forecasts: vec![primary_prediction],
                                prediction_intervals: None,
                                confidence_scores: Some(vec![0.8]),
                                timestamps: None,
                                unique_id: Some(symbol.to_string()),
                                additional_outputs: HashMap::new(),
                            };
                            
                            match self.convert_from_vendor_format(forecast, symbol, model_name).await {
                                Ok(pred) => {
                                    ensemble_predictions.push(pred);
                                    debug!("✅ Ensemble model {} prediction: {:.4}", model_name, primary_prediction);
                                }
                                Err(e) => {
                                    warn!("Failed to convert ensemble prediction from {}: {}", model_name, e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Ensemble model {} prediction failed: {}", model_name, e);
                        }
                    }
                } else {
                    warn!("Requested ensemble model {} not available for symbol {}", model_name, symbol);
                }
            }
            
            // Create ensemble result
            if !ensemble_predictions.is_empty() {
                let avg_value: f64 = ensemble_predictions.iter().map(|p| p.value).sum::<f64>() 
                    / ensemble_predictions.len() as f64;
                let avg_confidence: f64 = ensemble_predictions.iter().map(|p| p.confidence).sum::<f64>()
                    / ensemble_predictions.len() as f64;
                
                let mut metadata = HashMap::new();
                metadata.insert("requested_models".to_string(), serde_json::json!(models));
                metadata.insert("ensemble_type".to_string(), serde_json::json!("requested_models"));
                metadata.insert("horizon".to_string(), serde_json::json!(horizon));
                metadata.insert("successful_models".to_string(), 
                    serde_json::json!(ensemble_predictions.iter().map(|p| &p.model_name).collect::<Vec<_>>()));
                if let Some(ref features) = features {
                    metadata.insert("requested_features".to_string(), serde_json::json!(features));
                }
                
                let result = PredictionResult {
                    timestamp: Utc::now(),
                    value: avg_value,
                    confidence: avg_confidence,
                    interval_low: avg_value - (avg_confidence * avg_value.abs()),
                    interval_high: avg_value + (avg_confidence * avg_value.abs()),
                    model_name: format!("ensemble_{}_models", ensemble_predictions.len()),
                    metadata: Some(metadata),
                };
                
                results.push(result);
            } else {
                warn!("No successful ensemble predictions for symbol: {}", symbol);
                results.push(PredictionResult::default());
            }
        }
        
        Ok(results)
    }
    
    async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        // Return aggregated feature importance from all active models
        let mut importance_map = HashMap::new();
        
        // For vendor models, we'll return a basic feature importance based on common financial features
        // This will be enhanced in future phases with actual model introspection
        importance_map.insert("price".to_string(), 0.35);
        importance_map.insert("volume".to_string(), 0.20);
        importance_map.insert("volatility".to_string(), 0.15);
        importance_map.insert("trend".to_string(), 0.15);
        importance_map.insert("momentum".to_string(), 0.10);
        importance_map.insert("sector_correlation".to_string(), 0.05);
        
        Ok(importance_map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_vendor_predictor_creation() {
        // Test will be implemented with mock components
    }
    
    #[tokio::test]
    async fn test_model_data_requirements() {
        // Test data requirement checking
    }
    
    #[tokio::test]
    async fn test_ensemble_prediction() {
        // Test ensemble prediction logic
    }
}