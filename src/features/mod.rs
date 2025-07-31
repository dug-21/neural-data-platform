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

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
        
        Ok(Self {
            technical_indicators,
            microstructure,
            cross_asset,
            regime_detector,
            feature_selector,
            feature_store,
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
    
    #[tokio::test]
    async fn test_feature_pipeline_creation() {
        let config = FeaturePipelineConfig::default();
        let pipeline = FeatureEngineeringPipeline::new(config).await;
        assert!(pipeline.is_ok());
    }
}

#[cfg(test)]
#[path = "integration_tests.rs"]
mod integration_tests;