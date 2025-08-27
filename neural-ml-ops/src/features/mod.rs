//! Feature Engineering Module
//!
//! Domain-agnostic feature engineering and storage system extracted
//! from trading-specific feature engineering code.

pub mod engineering;
pub mod store;

pub use engineering::{FeatureEngine, FeatureExtractorConfig};
pub use store::{FeatureStore};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Feature data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feature {
    pub name: String,
    pub value: f64,
    pub feature_type: FeatureType,
    pub timestamp: DateTime<Utc>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Types of features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeatureType {
    Numerical,
    Categorical,
    Binary,
    Text,
    Time,
    Custom(String),
}

/// Feature extraction request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureRequest {
    pub data_source: String,
    pub feature_names: Vec<String>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Feature engineering configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub window_size: usize,
    pub statistical_features: bool,
    pub frequency_features: bool,
    pub wavelet_features: bool,
    pub technical_features: bool,
    pub custom_features: Vec<CustomFeatureConfig>,
    pub normalization: Option<NormalizationConfig>,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            statistical_features: true,
            frequency_features: true,
            wavelet_features: false,
            technical_features: true,
            custom_features: Vec::new(),
            normalization: Some(NormalizationConfig::default()),
        }
    }
}

/// Custom feature configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomFeatureConfig {
    pub name: String,
    pub feature_type: FeatureType,
    pub parameters: HashMap<String, serde_json::Value>,
    pub dependencies: Vec<String>,
}

/// Data normalization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationConfig {
    pub method: NormalizationMethod,
    pub per_feature: bool,
    pub preserve_zeros: bool,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            method: NormalizationMethod::StandardScaler,
            per_feature: true,
            preserve_zeros: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NormalizationMethod {
    StandardScaler,  // z-score normalization
    MinMaxScaler,    // min-max scaling to [0,1]
    RobustScaler,    // median and IQR
    MaxAbsScaler,    // max absolute scaling
    None,
}

/// Feature quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureQuality {
    pub feature_name: String,
    pub completeness: f64,        // Percentage of non-null values
    pub uniqueness: f64,          // Percentage of unique values
    pub consistency: f64,         // Consistency score
    pub timeliness: f64,          // Data freshness score
    pub accuracy_score: f64,      // Estimated accuracy
    pub drift_score: f64,         // Distribution drift score
    pub importance_score: f64,    // Feature importance
}

/// Batch feature processing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchFeatureResult {
    pub features: Vec<Feature>,
    pub processing_stats: ProcessingStats,
    pub quality_metrics: Vec<FeatureQuality>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Feature processing statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStats {
    pub total_records: usize,
    pub processed_records: usize,
    pub failed_records: usize,
    pub processing_time_ms: u64,
    pub features_generated: usize,
    pub memory_used_mb: f64,
}

/// Feature store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStoreConfig {
    pub storage_backend: StorageBackend,
    pub cache_size_mb: usize,
    pub enable_versioning: bool,
    pub compression: CompressionConfig,
    pub retention_days: u32,
}

impl Default for FeatureStoreConfig {
    fn default() -> Self {
        Self {
            storage_backend: StorageBackend::Memory,
            cache_size_mb: 512,
            enable_versioning: true,
            compression: CompressionConfig::default(),
            retention_days: 90,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageBackend {
    Memory,
    Redis { connection_string: String },
    Database { connection_string: String },
    FileSystem { base_path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub enabled: bool,
    pub algorithm: CompressionAlgorithm,
    pub level: u8, // 1-9, higher = more compression
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            algorithm: CompressionAlgorithm::Gzip,
            level: 6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    Gzip,
    Lz4,
    Zstd,
}

/// Feature engineering trait for extensibility
#[async_trait::async_trait]
pub trait FeatureExtractor: Send + Sync {
    /// Extract features from input data
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>>;
    
    /// Get feature names this extractor produces
    fn get_feature_names(&self) -> Vec<String>;
    
    /// Get configuration for this extractor
    fn get_config(&self) -> serde_json::Value;
    
    /// Validate input data before processing
    async fn validate_input(&self, data: &[f64]) -> Result<()>;
}

/// Feature store trait for different storage backends
#[async_trait::async_trait]
pub trait FeatureStoreTrait: Send + Sync {
    /// Store features with optional versioning
    async fn store_features(
        &self,
        namespace: &str,
        features: &[Feature],
        version: Option<&str>,
    ) -> Result<()>;
    
    /// Retrieve features by name and time range
    async fn retrieve_features(
        &self,
        namespace: &str,
        feature_names: &[String],
        time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
        version: Option<&str>,
    ) -> Result<Vec<Feature>>;
    
    /// List available feature names
    async fn list_features(&self, namespace: &str) -> Result<Vec<String>>;
    
    /// Delete features older than specified time
    async fn cleanup_old_features(&self, older_than: DateTime<Utc>) -> Result<usize>;
    
    /// Get storage statistics
    async fn get_stats(&self) -> Result<StorageStats>;
}

/// Storage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_features: usize,
    pub total_namespaces: usize,
    pub storage_size_mb: f64,
    pub oldest_feature: Option<DateTime<Utc>>,
    pub newest_feature: Option<DateTime<Utc>>,
    pub cache_hit_ratio: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_feature_serialization() {
        let feature = Feature {
            name: "test_feature".to_string(),
            value: 42.0,
            feature_type: FeatureType::Numerical,
            timestamp: Utc::now(),
            metadata: Some([("source".to_string(), "test".to_string())].into()),
        };
        
        let json = serde_json::to_string(&feature).unwrap();
        let deserialized: Feature = serde_json::from_str(&json).unwrap();
        
        assert_eq!(deserialized.name, feature.name);
        assert_eq!(deserialized.value, feature.value);
    }
    
    #[test]
    fn test_feature_config_default() {
        let config = FeatureConfig::default();
        assert_eq!(config.window_size, 50);
        assert!(config.statistical_features);
        assert!(config.normalization.is_some());
    }
}