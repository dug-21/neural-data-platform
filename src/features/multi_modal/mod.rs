//! Multi-Modal Feature Fusion System
//! 
//! This module implements a comprehensive feature fusion system that integrates
//! diverse data types for enhanced neural trading predictions.

pub mod data_types;  
pub mod fusion_engine;
pub mod feature_store;
pub mod temporal_alignment;
pub mod normalization;
pub mod dimensionality_reduction;
pub mod missing_data_handler;
pub mod model_mapping;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub use data_types::*;
pub use fusion_engine::MultiModalFusionEngine;
pub use feature_store::MultiModalFeatureStore;
pub use temporal_alignment::TemporalAlignmentEngine;
pub use normalization::DataNormalizer;
pub use dimensionality_reduction::DimensionalityReducer;
pub use missing_data_handler::MissingDataHandler;
pub use model_mapping::ModelFeatureMapper;

/// Multi-modal feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiModalConfig {
    /// Enable real-time fusion
    pub enable_realtime: bool,
    
    /// Maximum features per modality
    pub max_features_per_modality: usize,
    
    /// Feature importance threshold
    pub importance_threshold: f64,
    
    /// Enable cross-modal correlations
    pub enable_cross_modal_correlations: bool,
    
    /// Temporal alignment window in seconds
    pub alignment_window_seconds: u64,
    
    /// Missing data tolerance (0.0 to 1.0)
    pub missing_data_tolerance: f64,
    
    /// Enable dimensionality reduction
    pub enable_dimensionality_reduction: bool,
    
    /// Target feature count after reduction
    pub target_feature_count: usize,
    
    /// Normalization strategy
    pub normalization_strategy: NormalizationStrategy,
    
    /// Feature store configuration
    pub feature_store: FeatureStoreConfig,
    
    /// Model mapping configuration
    pub model_mapping: ModelMappingConfig,
}

/// Normalization strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NormalizationStrategy {
    /// Z-score normalization
    ZScore,
    /// Min-max normalization
    MinMax,
    /// Robust normalization using median and IQR
    Robust,
    /// Quantile normalization
    Quantile,
    /// Unit vector normalization
    UnitVector,
}

/// Feature store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStoreConfig {
    /// Enable feature versioning
    pub enable_versioning: bool,
    /// Cache size in MB
    pub cache_size_mb: usize,
    /// Compression enabled
    pub enable_compression: bool,
    /// Batch size for writes
    pub batch_size: usize,
}

/// Model mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMappingConfig {
    /// Enable adaptive mapping
    pub enable_adaptive_mapping: bool,
    /// Model-specific feature counts
    pub model_feature_counts: HashMap<String, usize>,
    /// Feature importance weights per model
    pub model_importance_weights: HashMap<String, HashMap<String, f64>>,
}

impl Default for MultiModalConfig {
    fn default() -> Self {
        Self {
            enable_realtime: true,
            max_features_per_modality: 200,
            importance_threshold: 0.01,
            enable_cross_modal_correlations: true,
            alignment_window_seconds: 300, // 5 minutes
            missing_data_tolerance: 0.15,  // 15% missing data allowed
            enable_dimensionality_reduction: true,
            target_feature_count: 128,
            normalization_strategy: NormalizationStrategy::Robust,
            feature_store: FeatureStoreConfig {
                enable_versioning: true,
                cache_size_mb: 512,
                enable_compression: true,
                batch_size: 1000,
            },
            model_mapping: ModelMappingConfig {
                enable_adaptive_mapping: true,
                model_feature_counts: HashMap::from([
                    ("MLP".to_string(), 64),
                    ("LSTM".to_string(), 96),
                    ("NHITS".to_string(), 128),
                    ("TCN".to_string(), 80),
                    ("DeepAR".to_string(), 72),
                ]),
                model_importance_weights: HashMap::new(),
            },
        }
    }
}

/// Multi-modal feature result
#[derive(Debug, Clone)]
pub struct MultiModalFeatureResult {
    /// Fused feature vector
    pub features: HashMap<String, f64>,
    /// Features by modality
    pub modality_features: HashMap<DataModality, HashMap<String, f64>>,
    /// Cross-modal correlations
    pub cross_modal_correlations: HashMap<String, f64>,
    /// Feature metadata
    pub metadata: MultiModalMetadata,
    /// Quality metrics
    pub quality_metrics: QualityMetrics,
}

/// Multi-modal metadata
#[derive(Debug, Clone)]
pub struct MultiModalMetadata {
    /// Processing timestamp
    pub timestamp: DateTime<Utc>,
    /// Source modalities
    pub modalities_used: Vec<DataModality>,
    /// Feature counts per modality
    pub feature_counts: HashMap<DataModality, usize>,
    /// Processing time in milliseconds
    pub processing_time_ms: f64,
    /// Data completeness by modality
    pub data_completeness: HashMap<DataModality, f64>,
    /// Alignment quality score
    pub alignment_quality: f64,
}

/// Quality metrics for multi-modal features
#[derive(Debug, Clone)]
pub struct QualityMetrics {
    /// Overall feature quality (0.0 to 1.0)
    pub overall_quality: f64,
    /// Data completeness across all modalities
    pub data_completeness: f64,
    /// Temporal alignment quality
    pub temporal_alignment_quality: f64,
    /// Cross-modal consistency
    pub cross_modal_consistency: f64,
    /// Feature importance distribution balance
    pub importance_balance: f64,
    /// Processing latency in milliseconds  
    pub processing_latency_ms: f64,
}

/// Error types for multi-modal processing
#[derive(Debug, thiserror::Error)]
pub enum MultiModalError {
    #[error("Insufficient data for modality {0}")]
    InsufficientData(DataModality),
    
    #[error("Temporal alignment failed: {0}")]
    AlignmentFailed(String),
    
    #[error("Normalization error: {0}")]
    NormalizationError(String),
    
    #[error("Dimensionality reduction failed: {0}")]
    DimensionalityReductionFailed(String),
    
    #[error("Missing data handler error: {0}")]
    MissingDataError(String),
    
    #[error("Feature store error: {0}")]
    FeatureStoreError(String),
    
    #[error("Model mapping error: {0}")]
    ModelMappingError(String),
    
    #[error("Configuration error: {0}")]
    ConfigurationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MultiModalConfig::default();
        assert!(config.enable_realtime);
        assert_eq!(config.max_features_per_modality, 200);
        assert!(config.enable_cross_modal_correlations);
    }

    #[test]
    fn test_model_feature_counts() {
        let config = MultiModalConfig::default();
        assert_eq!(config.model_mapping.model_feature_counts.get("MLP"), Some(&64));
        assert_eq!(config.model_mapping.model_feature_counts.get("LSTM"), Some(&96));
        assert_eq!(config.model_mapping.model_feature_counts.get("NHITS"), Some(&128));
    }
}