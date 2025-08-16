//! Sector Hierarchy Manager - Core Two-Layer Architecture
//!
//! This module implements the unified interface for the two-layer sector-based
//! neural architecture, coordinating between sector models (Layer 1) and 
//! symbol specializations (Layer 2).

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

use crate::data::{TimeSeriesData, RedisCache};
use crate::data::sector_mapper::{SectorId, SectorMapper, SectorInfo};
use crate::config::sector_models::{SectorModelsConfig, SectorConfig, ModelConfig};
use crate::neural::emergency_model::BaseModel;

/// Prediction result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub value: f64,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Training phase enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingPhase {
    Phase1SectorModels {
        active_sectors: Vec<SectorId>,
        completion_status: HashMap<SectorId, TrainingStatus>,
        started_at: DateTime<Utc>,
    },
    Phase2Specializations {
        completed_sectors: Vec<SectorId>,
        active_specializations: Vec<String>,
        started_at: DateTime<Utc>,
    },
    OnlineUpdates {
        update_frequency: Duration,
        last_update: DateTime<Utc>,
    },
}

/// Training status for models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingStatus {
    Pending,
    InProgress { started_at: DateTime<Utc> },
    Completed { completed_at: DateTime<Utc>, accuracy: f64 },
    Failed { error_message: String, failed_at: DateTime<Utc> },
}

/// Training results enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingResults {
    SectorModelsComplete { 
        trained_sectors: Vec<SectorId>,
        total_accuracy: f64,
        training_duration: Duration,
    },
    SpecializationsComplete { 
        trained_symbols: Vec<String>,
        average_accuracy: f64,
        training_duration: Duration,
    },
    OnlineUpdateComplete {
        updated_models: usize,
        performance_improvement: f64,
    },
}

/// Layer 1: Sector Model Implementation
#[derive(Debug, Clone)]
pub struct SectorModel {
    pub sector_id: SectorId,
    pub etf_symbol: String,
    pub model_type: String,
    pub memory_allocation: u32,
    pub training_data_window: Duration,
    pub last_trained: Option<DateTime<Utc>>,
    pub accuracy: f64,
    pub is_frozen: bool,
    
    // Model state (simplified - in production this would be the actual model)
    pub model_parameters: HashMap<String, f64>,
    pub feature_weights: HashMap<String, f64>,
}

impl SectorModel {
    pub fn new(sector_id: SectorId, config: &SectorConfig, model_config: &ModelConfig) -> Self {
        let etf_symbol = config.etf_representative.clone();
        
        Self {
            sector_id,
            etf_symbol,
            model_type: model_config.model_type.clone(),
            memory_allocation: config.shared_memory_mb,
            training_data_window: Duration::days(30), // Default window
            last_trained: None,
            accuracy: 0.0,
            is_frozen: false,
            model_parameters: HashMap::new(),
            feature_weights: HashMap::new(),
        }
    }
    
    pub async fn predict(&self, data: &TimeSeriesData) -> Result<Prediction> {
        // Simulate sector-level prediction
        // In production, this would use the actual trained model
        
        let base_value = data.close;
        let volatility = self.calculate_sector_volatility(data)?;
        
        // Apply sector-specific adjustments
        let sector_adjustment = self.get_sector_adjustment();
        let predicted_value = base_value * (1.0 + sector_adjustment * volatility);
        
        // Calculate confidence based on model accuracy and data quality
        let confidence = self.accuracy * self.calculate_data_quality(data);
        
        Ok(Prediction {
            value: predicted_value,
            confidence: confidence.min(1.0).max(0.0),
            timestamp: Utc::now(),
            metadata: HashMap::from([
                ("model_type".to_string(), self.model_type.clone().into()),
                ("sector".to_string(), self.sector_id.as_str().into()),
                ("etf_symbol".to_string(), self.etf_symbol.clone().into()),
                ("layer".to_string(), "sector_model".into()),
                ("memory_mb".to_string(), self.memory_allocation.into()),
            ]),
        })
    }
    
    fn calculate_sector_volatility(&self, data: &TimeSeriesData) -> Result<f64> {
        if data.values.len() < 2 {
            return Ok(0.01); // Default volatility
        }
        
        let returns: Vec<f64> = data.values.windows(2)
            .map(|window| (window[1] - window[0]) / window[0])
            .collect();
        
        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        Ok(variance.sqrt())
    }
    
    fn get_sector_adjustment(&self) -> f64 {
        // Sector-specific bias based on sector characteristics
        match self.sector_id {
            SectorId::Technology => 0.05,      // Higher growth potential
            SectorId::Financial => 0.02,       // Moderate growth
            SectorId::Healthcare => 0.03,      // Stable growth
            SectorId::Energy => -0.01,         // Volatile sector
            SectorId::ConsumerDiscretionary => 0.04,
            SectorId::ConsumerStaples => 0.01, // Defensive
            SectorId::Industrials => 0.02,
            SectorId::Materials => -0.005,
            SectorId::Utilities => 0.005,      // Very defensive
            SectorId::RealEstate => 0.015,
        }
    }
    
    fn calculate_data_quality(&self, data: &TimeSeriesData) -> f64 {
        let completeness = if data.values.is_empty() { 0.0 } else { 1.0 };
        let recency = {
            let age = Utc::now() - data.timestamp;
            if age < Duration::minutes(5) { 1.0 }
            else if age < Duration::hours(1) { 0.9 }
            else if age < Duration::days(1) { 0.7 }
            else { 0.5 }
        };
        
        (completeness + recency) / 2.0
    }
    
    pub fn freeze_parameters(&mut self) {
        self.is_frozen = true;
        debug!("Frozen parameters for sector model: {:?}", self.sector_id);
    }
    
    pub fn unfreeze_parameters(&mut self) {
        self.is_frozen = false;
        debug!("Unfrozen parameters for sector model: {:?}", self.sector_id);
    }
}

/// Layer 2: Symbol Specialization Implementation
#[derive(Debug, Clone)]
pub struct SymbolSpecialization {
    pub symbol: String,
    pub sector_reference: SectorId,
    pub memory_allocation: u32,
    pub adaptation_rate: f64,
    pub last_trained: Option<DateTime<Utc>>,
    pub accuracy: f64,
    
    // Specialization parameters
    pub deviation_patterns: HashMap<String, f64>,
    pub attention_weights: HashMap<String, f64>,
    pub residual_layers: Vec<HashMap<String, f64>>,
}

impl SymbolSpecialization {
    pub fn new(symbol: String, sector_reference: SectorId, memory_mb: u32) -> Self {
        Self {
            symbol,
            sector_reference,
            memory_allocation: memory_mb,
            adaptation_rate: 0.001,
            last_trained: None,
            accuracy: 0.0,
            deviation_patterns: HashMap::new(),
            attention_weights: HashMap::new(),
            residual_layers: Vec::new(),
        }
    }
    
    pub async fn predict_deviation(&self, data: &TimeSeriesData) -> Result<Prediction> {
        // Predict deviation from sector baseline
        let symbol_specific_factor = self.get_symbol_specific_factor();
        let market_condition_adjustment = self.assess_market_conditions(data);
        
        let deviation = symbol_specific_factor * market_condition_adjustment;
        let confidence = self.accuracy * 0.8; // Specializations typically less confident
        
        Ok(Prediction {
            value: deviation,
            confidence: confidence.min(1.0).max(0.0),
            timestamp: Utc::now(),
            metadata: HashMap::from([
                ("symbol".to_string(), self.symbol.clone().into()),
                ("sector_reference".to_string(), self.sector_reference.as_str().into()),
                ("layer".to_string(), "specialization".into()),
                ("memory_mb".to_string(), self.memory_allocation.into()),
                ("adaptation_rate".to_string(), self.adaptation_rate.into()),
            ]),
        })
    }
    
    fn get_symbol_specific_factor(&self) -> f64 {
        // Symbol-specific characteristics (simplified)
        match self.symbol.as_str() {
            "AAPL" => 0.08,   // High innovation premium
            "MSFT" => 0.05,   // Stable growth
            "GOOGL" => 0.07,  // Strong fundamentals
            "META" => 0.06,   // Social media leader
            "NVDA" => 0.12,   // AI/GPU leader
            "TSLA" => 0.15,   // High volatility/growth
            "JPM" => 0.03,    // Banking leader
            "BAC" => 0.02,    // Traditional banking
            _ => 0.01,        // Default modest adjustment
        }
    }
    
    fn assess_market_conditions(&self, data: &TimeSeriesData) -> f64 {
        // Assess current market conditions for specialization weighting
        let volume_factor = if data.volume_value > 1000000.0 { 1.2 } else { 0.8 };
        let price_momentum = self.calculate_price_momentum(data);
        
        volume_factor * price_momentum
    }
    
    fn calculate_price_momentum(&self, data: &TimeSeriesData) -> f64 {
        if data.values.len() < 3 {
            return 1.0;
        }
        
        let recent_change = (data.close - data.values[data.values.len() - 2]) / data.values[data.values.len() - 2];
        
        if recent_change > 0.02 { 1.3 }      // Strong upward momentum
        else if recent_change > 0.005 { 1.1 } // Moderate upward momentum
        else if recent_change < -0.02 { 0.7 } // Strong downward momentum
        else if recent_change < -0.005 { 0.9 } // Moderate downward momentum
        else { 1.0 }                         // Neutral momentum
    }
}

/// Main Sector Hierarchy Manager
pub struct SectorHierarchyManager {
    // Layer 1: Sector Models
    sector_models: Arc<DashMap<SectorId, SectorModel>>,
    
    // Layer 2: Symbol Specializations
    symbol_specializations: Arc<DashMap<String, SymbolSpecialization>>,
    
    // Integration components
    sector_mapper: Arc<SectorMapper>,
    redis_cache: Option<Arc<RedisCache>>,
    
    // Training coordination
    training_phase: Arc<RwLock<TrainingPhase>>,
    
    // Configuration
    config: SectorModelsConfig,
    
    // Performance tracking
    prediction_history: Arc<RwLock<Vec<(String, Prediction, DateTime<Utc>)>>>,
}

impl SectorHierarchyManager {
    pub fn new(
        sector_mapper: Arc<SectorMapper>,
        config: SectorModelsConfig,
        redis_cache: Option<Arc<RedisCache>>,
    ) -> Self {
        info!("🏗️ Initializing SectorHierarchyManager with two-layer architecture");
        
        let sector_models = Arc::new(DashMap::with_capacity(10));
        let symbol_specializations = Arc::new(DashMap::with_capacity(1000));
        
        // Initialize sector models
        for (sector_name, sector_config) in &config.sectors {
            if let Some(sector_id) = SectorId::from_str(sector_name) {
                // Find matching model configuration
                if let Some((_, model_config)) = config.models.iter()
                    .find(|(_, m)| m.sector == *sector_name) {
                    
                    let sector_model = SectorModel::new(sector_id, sector_config, model_config);
                    sector_models.insert(sector_id, sector_model);
                    
                    info!("Initialized sector model for {}: {} ({}MB)", 
                          sector_name, model_config.model_type, sector_config.shared_memory_mb);
                }
            }
        }
        
        let training_phase = Arc::new(RwLock::new(TrainingPhase::Phase1SectorModels {
            active_sectors: SectorId::all_sectors(),
            completion_status: HashMap::new(),
            started_at: Utc::now(),
        }));
        
        Self {
            sector_models,
            symbol_specializations,
            sector_mapper,
            redis_cache,
            training_phase,
            config,
            prediction_history: Arc::new(RwLock::new(Vec::with_capacity(10000))),
        }
    }
    
    /// Main prediction interface - combines Layer 1 and Layer 2
    pub async fn predict(&self, symbol: &str, data: &TimeSeriesData) -> Result<Prediction> {
        // 1. Get sector information for the symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        
        // 2. Get sector model prediction (Layer 1)
        let sector_prediction = self.get_sector_prediction(&sector_info.sector_id, data).await?;
        
        // 3. Get symbol specialization prediction (Layer 2)
        let specialization_prediction = self.get_specialization_prediction(symbol, data).await?;
        
        // 4. Combine predictions intelligently
        let combined_prediction = self.combine_predictions(
            sector_prediction, 
            specialization_prediction,
            &sector_info
        )?;
        
        // 5. Track prediction for performance analysis
        self.track_prediction(symbol, &combined_prediction).await;
        
        Ok(combined_prediction)
    }
    
    async fn get_sector_prediction(&self, sector_id: &SectorId, data: &TimeSeriesData) -> Result<Prediction> {
        let sector_model = self.sector_models.get(sector_id)
            .ok_or_else(|| anyhow!("Sector model not found: {:?}", sector_id))?;
        
        sector_model.predict(data).await
    }
    
    async fn get_specialization_prediction(&self, symbol: &str, data: &TimeSeriesData) -> Result<Option<Prediction>> {
        if let Some(specialization) = self.symbol_specializations.get(symbol) {
            Ok(Some(specialization.predict_deviation(data).await?))
        } else {
            Ok(None)
        }
    }
    
    fn combine_predictions(
        &self,
        sector: Prediction,
        specialization: Option<Prediction>,
        sector_info: &SectorInfo
    ) -> Result<Prediction> {
        match specialization {
            Some(spec) => {
                // Intelligent ensemble of sector + specialization
                let sector_weight = 0.7; // Could be made configurable
                let spec_weight = 0.3;
                
                // Apply specialization as a deviation from sector baseline
                let combined_value = sector.value * (1.0 + spec.value);
                
                // Weighted confidence combination
                let combined_confidence = (sector.confidence * sector_weight + spec.confidence * spec_weight).min(1.0);
                
                let mut combined_metadata = sector.metadata.clone();
                combined_metadata.insert("specialization_applied".to_string(), true.into());
                combined_metadata.insert("specialization_deviation".to_string(), spec.value.into());
                combined_metadata.insert("sector_weight".to_string(), sector_weight.into());
                combined_metadata.insert("specialization_weight".to_string(), spec_weight.into());
                
                Ok(Prediction {
                    value: combined_value,
                    confidence: combined_confidence,
                    timestamp: Utc::now(),
                    metadata: combined_metadata,
                })
            },
            None => {
                // Use sector prediction only
                let mut metadata = sector.metadata.clone();
                metadata.insert("specialization_applied".to_string(), false.into());
                metadata.insert("fallback_reason".to_string(), "no_specialization_available".into());
                
                Ok(Prediction {
                    value: sector.value,
                    confidence: sector.confidence * 0.9, // Slight confidence reduction for missing specialization
                    timestamp: Utc::now(),
                    metadata,
                })
            }
        }
    }
    
    async fn track_prediction(&self, symbol: &str, prediction: &Prediction) {
        let mut history = self.prediction_history.write().await;
        history.push((symbol.to_string(), prediction.clone(), Utc::now()));
        
        // Keep only recent predictions for memory efficiency
        if history.len() > 10000 {
            history.drain(0..5000);
        }
    }
    
    /// Initialize symbol specialization
    pub async fn create_specialization(&self, symbol: &str) -> Result<()> {
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        let memory_mb = self.config.sectors
            .get(sector_info.sector_id.as_str())
            .map(|s| s.specialization_memory_mb)
            .unwrap_or(8);
        
        let specialization = SymbolSpecialization::new(
            symbol.to_string(),
            sector_info.sector_id,
            memory_mb
        );
        
        self.symbol_specializations.insert(symbol.to_string(), specialization);
        info!("Created specialization for {} in sector {:?} ({}MB)", 
              symbol, sector_info.sector_id, memory_mb);
        
        Ok(())
    }
    
    /// Get current training phase
    pub async fn get_training_phase(&self) -> TrainingPhase {
        self.training_phase.read().await.clone()
    }
    
    /// Set training phase
    pub async fn set_training_phase(&self, phase: TrainingPhase) {
        let mut current_phase = self.training_phase.write().await;
        *current_phase = phase;
    }
    
    /// Get sector model
    pub fn get_sector_model(&self, sector_id: &SectorId) -> Option<SectorModel> {
        self.sector_models.get(sector_id).map(|entry| entry.clone())
    }
    
    /// Get symbol specialization
    pub fn get_symbol_specialization(&self, symbol: &str) -> Option<SymbolSpecialization> {
        self.symbol_specializations.get(symbol).map(|entry| entry.clone())
    }
    
    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> HashMap<String, serde_json::Value> {
        let mut metrics = HashMap::new();
        
        // Sector model metrics
        metrics.insert("sector_models_count".to_string(), self.sector_models.len().into());
        metrics.insert("specializations_count".to_string(), self.symbol_specializations.len().into());
        
        // Memory usage estimation
        let sector_memory: u32 = self.sector_models.iter()
            .map(|entry| entry.value().memory_allocation)
            .sum();
        let specialization_memory: u32 = self.symbol_specializations.iter()
            .map(|entry| entry.value().memory_allocation)
            .sum();
        let total_memory_mb = sector_memory + specialization_memory;
        
        metrics.insert("sector_models_memory_mb".to_string(), sector_memory.into());
        metrics.insert("specializations_memory_mb".to_string(), specialization_memory.into());
        metrics.insert("total_memory_mb".to_string(), total_memory_mb.into());
        metrics.insert("memory_target_mb".to_string(), 4096.into()); // 4GB target
        
        // Performance metrics
        let history = self.prediction_history.read().await;
        if !history.is_empty() {
            let avg_confidence = history.iter()
                .map(|(_, pred, _)| pred.confidence)
                .sum::<f64>() / history.len() as f64;
            
            metrics.insert("average_confidence".to_string(), avg_confidence.into());
            metrics.insert("predictions_count".to_string(), history.len().into());
        }
        
        metrics
    }
    
    /// Batch prediction for multiple symbols
    pub async fn batch_predict(
        &self,
        requests: Vec<(String, TimeSeriesData)>
    ) -> Vec<Result<Prediction>> {
        use futures::stream::{self, StreamExt};
        
        // Process each request asynchronously without lifetime issues
        let results = stream::iter(requests)
            .then(|(symbol, data)| async move {
                self.predict(&symbol, &data).await
            })
            .collect::<Vec<_>>()
            .await;
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sector_mapper::SectorMapperConfig;
    
    fn create_test_config() -> SectorModelsConfig {
        SectorModelsConfig::default()
    }
    
    fn create_test_time_series_data(symbol: &str) -> TimeSeriesData {
        TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 105.0,
            low: 98.0,
            close: 102.0,
            volume: Some(1000000),
            volume_value: 1000000.0,
            values: vec![98.0, 99.0, 101.0, 102.0],
            indicators: HashMap::new(),
        }
    }
    
    #[tokio::test]
    async fn test_sector_hierarchy_manager_creation() {
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let config = create_test_config();
        
        let hierarchy_manager = SectorHierarchyManager::new(sector_mapper, config, None);
        
        assert_eq!(hierarchy_manager.sector_models.len(), 0); // Would be populated with real config
        assert_eq!(hierarchy_manager.symbol_specializations.len(), 0);
    }
    
    #[tokio::test]
    async fn test_sector_model_prediction() {
        let sector_config = crate::config::sector_models::SectorConfig {
            etf_representative: "XLK".to_string(),
            sector_name: "Technology".to_string(),
            description: "Technology sector".to_string(),
            symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
            shared_memory_mb: 512,
            specialization_memory_mb: 8,
            max_symbols: 15,
            correlation_threshold: 0.65,
            sector_weight: 0.25,
        };
        
        let model_config = crate::config::sector_models::ModelConfig {
            model_type: "LSTM".to_string(),
            sector: "technology".to_string(),
            description: "Test model".to_string(),
            required_data: vec!["price".to_string()],
            optional_data: vec![],
            preferred_data: vec![],
            max_memory_mb: 256,
            min_accuracy: 0.7,
            max_latency_ms: 100,
            ensemble_weight: 1.0,
            lazy_load_conditions: vec![],
            specialization_layers: 2,
        };
        
        let sector_model = SectorModel::new(SectorId::Technology, &sector_config, &model_config);
        let test_data = create_test_time_series_data("XLK");
        
        let prediction = sector_model.predict(&test_data).await.unwrap();
        
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.value.is_finite());
        assert!(prediction.metadata.contains_key("model_type"));
    }
    
    #[tokio::test]
    async fn test_symbol_specialization_prediction() {
        let specialization = SymbolSpecialization::new(
            "AAPL".to_string(),
            SectorId::Technology,
            8
        );
        
        let test_data = create_test_time_series_data("AAPL");
        let prediction = specialization.predict_deviation(&test_data).await.unwrap();
        
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.value.is_finite());
        assert_eq!(prediction.metadata.get("symbol").unwrap().as_str().unwrap(), "AAPL");
    }
}