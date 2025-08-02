//! Advanced Feature Engineering Module for Neural Trading
//! 
//! This module provides comprehensive feature engineering capabilities
//! specifically designed for high-frequency trading and market microstructure analysis.

pub mod technical_indicators;
pub mod market_microstructure;
pub mod cross_asset;
pub mod regime_detection;
pub mod feature_store;
pub mod realtime_pipeline;
pub mod feature_selection;
pub mod training_features;
pub mod shared_feature_extractor;
pub mod symbol_specialization;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export shared feature extractor types for easy access
pub use shared_feature_extractor::{
    SharedFeatureExtractor, SharedSectorFeatures, SymbolFeatures,
    SharedFeatureConfig, CachedSectorFeatures,
    MarketRegimeFeatures, VolatilityFeatures, TechnicalFeatures,
    CorrelationFeatures, MomentumFeatures
};

// Re-export symbol specialization types for easy access  
pub use symbol_specialization::{
    SymbolSpecializationLayer, SymbolSpecializationWeights, SymbolPerformanceMetrics,
    SymbolSpecializationConfig, SymbolSpecificSignals, PricePattern, VolumeProfile, OrderFlowSignals
};
use crate::data::TimeSeriesData;
use crate::data::sector_mapper::SectorId;

/// Feature metadata for tracking and versioning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureMetadata {
    pub name: String,
    pub category: FeatureCategory,
    pub computation_time_ms: f64,
    pub memory_usage_mb: f64,
    pub importance_score: Option<f64>,
    pub dependencies: Vec<String>,
    pub version: String,
    pub last_updated: DateTime<Utc>,
}

/// Feature categories for organization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeatureCategory {
    Price,
    Volume,
    Volatility,
    Momentum,
    MeanReversion,
    MarketMicrostructure,
    OrderFlow,
    CrossAsset,
    Sentiment,
    Regime,
    Custom,
}

/// Feature computation result with metadata
#[derive(Debug, Clone)]
pub struct FeatureResult {
    pub values: Vec<f64>,
    pub metadata: FeatureMetadata,
    pub computation_stats: ComputationStats,
}

/// Statistics about feature computation
#[derive(Debug, Clone)]
pub struct ComputationStats {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub records_processed: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Main feature engineering pipeline
pub struct FeatureEngineeringPipeline {
    /// Technical indicators calculator
    technical_indicators: Arc<technical_indicators::TechnicalIndicatorEngine>,
    
    /// Market microstructure analyzer
    microstructure: Arc<market_microstructure::MicrostructureAnalyzer>,
    
    /// Cross-asset correlation tracker
    cross_asset: Arc<cross_asset::CrossAssetCorrelationEngine>,
    
    /// Market regime detector
    regime_detector: Arc<regime_detection::RegimeDetector>,
    
    /// Feature importance tracker
    feature_selector: Arc<RwLock<feature_selection::AdaptiveFeatureSelector>>,
    
    /// Feature storage and versioning
    feature_store: Arc<feature_store::FeatureStore>,
    
    /// Shared feature extractors by sector for memory efficiency
    shared_extractors: Arc<RwLock<HashMap<SectorId, Arc<SharedFeatureExtractor>>>>,
    
    /// Pipeline configuration
    config: FeaturePipelineConfig,
}

/// Configuration for the feature engineering pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturePipelineConfig {
    /// Enable real-time feature computation
    pub enable_realtime: bool,
    
    /// Maximum features to compute
    pub max_features: usize,
    
    /// Feature importance threshold
    pub importance_threshold: f64,
    
    /// Enable feature caching
    pub enable_caching: bool,
    
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    
    /// Enable parallel computation
    pub enable_parallel: bool,
    
    /// Number of worker threads
    pub num_workers: usize,
    
    /// Memory limit in MB
    pub memory_limit_mb: f64,
    
    /// Enable adaptive feature selection
    pub enable_adaptive_selection: bool,
    
    /// Feature update frequency in seconds
    pub update_frequency_seconds: u64,
    
    /// Enable shared feature extraction for memory efficiency
    pub enable_shared_features: bool,
    
    /// Configuration for shared feature extraction
    pub shared_feature_config: SharedFeatureConfig,
}

impl Default for FeaturePipelineConfig {
    fn default() -> Self {
        Self {
            enable_realtime: true,
            max_features: 500,
            importance_threshold: 0.01,
            enable_caching: true,
            cache_ttl_seconds: 300,
            enable_parallel: true,
            num_workers: 4,
            memory_limit_mb: 1024.0,
            enable_adaptive_selection: true,
            update_frequency_seconds: 60,
            enable_shared_features: true,
            shared_feature_config: SharedFeatureConfig::default(),
        }
    }
}

impl FeatureEngineeringPipeline {
    /// Create a new feature engineering pipeline
    pub async fn new(config: FeaturePipelineConfig) -> Result<Self> {
        let technical_indicators = Arc::new(
            technical_indicators::TechnicalIndicatorEngine::new()
        );
        
        let microstructure = Arc::new(
            market_microstructure::MicrostructureAnalyzer::new()
        );
        
        let cross_asset = Arc::new(
            cross_asset::CrossAssetCorrelationEngine::new()
        );
        
        let regime_detector = Arc::new(
            regime_detection::RegimeDetector::new()
        );
        
        let feature_selector = Arc::new(RwLock::new(
            feature_selection::AdaptiveFeatureSelector::new(
                config.importance_threshold
            )
        ));
        
        let feature_store = Arc::new(
            feature_store::FeatureStore::new(&config).await?
        );
        
        let shared_extractors = Arc::new(RwLock::new(HashMap::new()));
        
        Ok(Self {
            technical_indicators,
            microstructure,
            cross_asset,
            regime_detector,
            feature_selector,
            feature_store,
            shared_extractors,
            config,
        })
    }
    
    /// Compute all features for given market data
    pub async fn compute_features(
        &self,
        data: &crate::data::TimeSeriesData,
        historical_data: &[crate::data::TimeSeriesData],
        market_context: &HashMap<String, Vec<crate::data::TimeSeriesData>>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        let start_time = Utc::now();
        
        // Compute technical indicators
        let technical_features = self.technical_indicators
            .compute_all(data, historical_data)
            .await?;
        features.extend(technical_features);
        
        // Compute market microstructure features
        if self.config.enable_realtime {
            let microstructure_features = self.microstructure
                .analyze(data, historical_data)
                .await?;
            features.extend(microstructure_features);
        }
        
        // Compute cross-asset correlations
        if !market_context.is_empty() {
            let cross_asset_features = self.cross_asset
                .compute_correlations(&data.symbol, market_context)
                .await?;
            features.extend(cross_asset_features);
        }
        
        // Detect market regime
        let regime = self.regime_detector
            .detect_regime(historical_data)
            .await?;
        features.insert("market_regime".to_string(), regime as i32 as f64);
        
        // Apply adaptive feature selection
        if self.config.enable_adaptive_selection {
            let selected_features = self.feature_selector
                .read()
                .await
                .select_features(&features)
                .await?;
            features = selected_features;
        }
        
        // Store features with versioning
        self.feature_store
            .store_features(&data.symbol, &data.timestamp, &features)
            .await?;
        
        // Track computation time
        let computation_time = (Utc::now() - start_time).num_milliseconds() as f64;
        features.insert("_computation_time_ms".to_string(), computation_time);
        
        Ok(features)
    }
    
    /// Get feature importance scores
    pub async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        self.feature_selector
            .read()
            .await
            .get_importance_scores()
            .await
    }
    
    /// Update feature importance based on model feedback
    pub async fn update_feature_importance(
        &self,
        importance_scores: HashMap<String, f64>,
    ) -> Result<()> {
        self.feature_selector
            .write()
            .await
            .update_importance(importance_scores)
            .await
    }
    
    /// Get feature computation statistics
    pub async fn get_computation_stats(&self) -> Result<ComputationStats> {
        self.feature_store.get_computation_stats().await
    }
    
    /// Optimize feature pipeline based on performance metrics
    pub async fn optimize_pipeline(&mut self) -> Result<()> {
        // Analyze feature computation times
        let stats = self.get_computation_stats().await?;
        
        // Adjust parallelism based on performance
        if stats.records_processed > 0 {
            let avg_time = (stats.end_time - stats.start_time).num_milliseconds() as f64
                / stats.records_processed as f64;
            
            if avg_time > 100.0 && self.config.num_workers < 8 {
                self.config.num_workers += 1;
            } else if avg_time < 10.0 && self.config.num_workers > 2 {
                self.config.num_workers -= 1;
            }
        }
        
        Ok(())
    }
    
    /// Compute features using shared extraction for memory efficiency
    pub async fn compute_features_with_shared_extraction(
        &self,
        data: &TimeSeriesData,
        sector_id: SectorId,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        let start_time = Utc::now();
        
        if self.config.enable_shared_features && sector_data.len() >= self.config.shared_feature_config.min_symbols_for_extraction {
            // Use shared feature extraction
            let shared_features = self.get_or_create_shared_features(sector_id, sector_data).await?;
            let symbol_features = self.get_symbol_specialization(&data.symbol, data, &shared_features, sector_data).await?;
            
            // Convert shared features to pipeline format
            features.extend(self.convert_shared_features(&shared_features)?);
            features.extend(self.convert_symbol_features(&symbol_features)?);
            
        } else {
            // Fall back to individual feature extraction
            features = self.compute_features(data, &[], &HashMap::new()).await?;
        }
        
        // Track computation time for memory efficiency
        let computation_time = (Utc::now() - start_time).num_milliseconds() as f64;
        features.insert("_shared_computation_time_ms".to_string(), computation_time);
        
        Ok(features)
    }
    
    /// Get or create shared features for a sector
    async fn get_or_create_shared_features(
        &self,
        sector_id: SectorId,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<SharedSectorFeatures> {
        // Check if we already have an extractor for this sector
        let extractors = self.shared_extractors.read().await;
        if let Some(extractor) = extractors.get(&sector_id) {
            return extractor.extract_sector_features(sector_data).await;
        }
        drop(extractors);
        
        // Create new extractor for this sector
        let extractor = Arc::new(
            SharedFeatureExtractor::new(sector_id, self.config.shared_feature_config.clone()).await?
        );
        
        // Store the extractor
        let mut extractors = self.shared_extractors.write().await;
        extractors.insert(sector_id, extractor.clone());
        drop(extractors);
        
        // Extract features
        extractor.extract_sector_features(sector_data).await
    }
    
    /// Get symbol-specific features layered on shared features
    async fn get_symbol_specialization(
        &self,
        symbol: &str,
        symbol_data: &TimeSeriesData,
        shared_features: &SharedSectorFeatures,
        sector_data: &HashMap<String, TimeSeriesData>,
    ) -> Result<SymbolFeatures> {
        let sector_id = self.get_sector_id_for_symbol(symbol)?;
        let extractors = self.shared_extractors.read().await;
        
        if let Some(extractor) = extractors.get(&sector_id) {
            extractor.get_symbol_specialization(symbol, symbol_data, shared_features, sector_data).await
        } else {
            Err(anyhow::anyhow!("No shared extractor found for sector {:?}", sector_id))
        }
    }
    
    /// Convert shared sector features to pipeline format
    fn convert_shared_features(&self, shared: &SharedSectorFeatures) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        // Market regime features
        features.insert("shared_market_regime".to_string(), shared.market_regime.regime_type as f64);
        features.insert("shared_regime_confidence".to_string(), shared.market_regime.regime_confidence);
        features.insert("shared_volatility_percentile".to_string(), shared.market_regime.volatility_percentile);
        features.insert("shared_trend_strength".to_string(), shared.market_regime.trend_strength);
        
        // Volatility features
        features.insert("shared_realized_volatility".to_string(), shared.volatility_features.realized_volatility);
        features.insert("shared_volatility_regime".to_string(), shared.volatility_features.volatility_regime as f64);
        features.insert("shared_garch_forecast".to_string(), shared.volatility_features.garch_forecast);
        
        // Technical features
        features.insert("shared_sector_rsi".to_string(), shared.technical_features.sector_rsi);
        features.insert("shared_advance_decline_ratio".to_string(), shared.technical_features.advance_decline_ratio);
        features.insert("shared_breadth_thrust".to_string(), shared.technical_features.breadth_thrust);
        
        // Correlation features
        features.insert("shared_avg_correlation".to_string(), shared.correlation_features.average_pairwise_correlation);
        features.insert("shared_correlation_dispersion".to_string(), shared.correlation_features.correlation_dispersion);
        
        // Momentum features
        features.insert("shared_sector_momentum_1d".to_string(), shared.momentum_features.sector_momentum_1d);
        features.insert("shared_sector_momentum_5d".to_string(), shared.momentum_features.sector_momentum_5d);
        features.insert("shared_momentum_dispersion".to_string(), shared.momentum_features.momentum_dispersion);
        
        Ok(features)
    }
    
    /// Convert symbol-specific features to pipeline format
    fn convert_symbol_features(&self, symbol: &SymbolFeatures) -> Result<HashMap<String, f64>> {
        let mut features = HashMap::new();
        
        features.insert("symbol_relative_strength".to_string(), symbol.relative_strength);
        features.insert("symbol_idiosyncratic_volatility".to_string(), symbol.idiosyncratic_volatility);
        features.insert("symbol_beta_to_sector".to_string(), symbol.beta_to_sector);
        features.insert("symbol_correlation_to_sector".to_string(), symbol.correlation_to_sector);
        features.insert("symbol_volume_relative".to_string(), symbol.volume_relative_to_sector);
        features.insert("symbol_price_relative".to_string(), symbol.price_relative_to_sector);
        
        // Add symbol-specific technical signals
        for (key, value) in &symbol.specific_technical_signals {
            features.insert(format!("symbol_{}", key), *value);
        }
        
        Ok(features)
    }
    
    /// Get sector ID for a symbol (placeholder - in production, use sector mapping)
    fn get_sector_id_for_symbol(&self, symbol: &str) -> Result<SectorId> {
        // This is a placeholder implementation
        // In production, use the sector mapper from the data module
        match symbol {
            s if s.starts_with("AAPL") || s.starts_with("MSFT") || s.starts_with("GOOGL") => Ok(SectorId::Technology),
            s if s.starts_with("JPM") || s.starts_with("BAC") || s.starts_with("WFC") => Ok(SectorId::Financial),
            s if s.starts_with("JNJ") || s.starts_with("PFE") || s.starts_with("MRK") => Ok(SectorId::Healthcare),
            s if s.starts_with("XOM") || s.starts_with("CVX") || s.starts_with("COP") => Ok(SectorId::Energy),
            _ => Ok(SectorId::Technology), // Default fallback
        }
    }
    
    /// Get memory usage statistics for shared feature extraction
    pub async fn get_shared_memory_stats(&self) -> Result<HashMap<SectorId, (usize, usize)>> {
        let mut stats = HashMap::new();
        let extractors = self.shared_extractors.read().await;
        
        for (sector_id, extractor) in extractors.iter() {
            let (used, total) = extractor.get_memory_stats().await?;
            stats.insert(*sector_id, (used, total));
        }
        
        Ok(stats)
    }
    
    /// Calculate memory reduction achieved through shared feature extraction
    pub async fn calculate_memory_reduction(&self, sector_symbol_counts: &HashMap<SectorId, usize>) -> Result<f64> {
        let shared_stats = self.get_shared_memory_stats().await?;
        let mut total_shared_memory = 0;
        let mut total_individual_memory = 0;
        
        for (sector_id, symbol_count) in sector_symbol_counts {
            if let Some((used, _)) = shared_stats.get(sector_id) {
                total_shared_memory += used;
                // Estimate individual memory would be 10x more (conservative estimate)
                total_individual_memory += used * symbol_count * 10;
            }
        }
        
        if total_individual_memory > 0 {
            Ok(1.0 - (total_shared_memory as f64 / total_individual_memory as f64))
        } else {
            Ok(0.0)
        }
    }
    
    /// Cleanup unused shared extractors to free memory
    pub async fn cleanup_unused_extractors(&self, active_sectors: &[SectorId]) -> Result<()> {
        let mut extractors = self.shared_extractors.write().await;
        let mut to_remove = Vec::new();
        
        for sector_id in extractors.keys() {
            if !active_sectors.contains(sector_id) {
                to_remove.push(*sector_id);
            }
        }
        
        for sector_id in to_remove {
            extractors.remove(&sector_id);
        }
        
        Ok(())
    }
}

/// Feature engineering error types
#[derive(Debug, thiserror::Error)]
pub enum FeatureError {
    #[error("Insufficient data: {0}")]
    InsufficientData(String),
    
    #[error("Computation error: {0}")]
    ComputationError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
    
    #[error("Memory limit exceeded: {0}")]
    MemoryLimitExceeded(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{TimeSeriesData, SectorId};
    use chrono::Utc;
    
    #[tokio::test]
    async fn test_feature_pipeline_creation() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await;
        assert!(pipeline.is_ok());
    }
    
    #[tokio::test]
    async fn test_shared_feature_extraction() {
        let mut config = FeaturePipelineConfig::default();
        config.enable_shared_features = true;
        
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        // Create mock sector data
        let mut sector_data = HashMap::new();
        for i in 0..5 {
            let symbol = format!("AAPL{}", i);
            let data = TimeSeriesData {
                symbol: symbol.clone(),
                timestamp: Utc::now(),
                values: vec![100.0, 101.0, 102.0, 101.5, 103.0],
                volume: vec![1000.0, 1100.0, 1200.0, 1150.0, 1300.0],
                open: 100.0,
                high: 103.0,
                low: 99.0,
                close: 103.0,
                volume_value: 5850.0,
            };
            sector_data.insert(symbol, data);
        }
        
        // Test shared feature extraction
        let test_data = sector_data.values().next().unwrap();
        let features = pipeline.compute_features_with_shared_extraction(
            test_data,
            SectorId::Technology,
            &sector_data,
        ).await;
        
        assert!(features.is_ok());
        let features = features.unwrap();
        
        // Verify shared features are present
        assert!(features.contains_key("shared_market_regime"));
        assert!(features.contains_key("shared_realized_volatility"));
        assert!(features.contains_key("symbol_relative_strength"));
        assert!(features.contains_key("_shared_computation_time_ms"));
    }
    
    #[tokio::test]
    async fn test_memory_reduction_calculation() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await.unwrap();
        
        let mut sector_counts = HashMap::new();
        sector_counts.insert(SectorId::Technology, 10);
        sector_counts.insert(SectorId::Financial, 8);
        
        let reduction = pipeline.calculate_memory_reduction(&sector_counts).await;
        assert!(reduction.is_ok());
        // Should be 0.0 initially as no shared extractors are active
        assert_eq!(reduction.unwrap(), 0.0);
    }
}

#[cfg(test)]
#[path = "integration_tests.rs"]
mod integration_tests;