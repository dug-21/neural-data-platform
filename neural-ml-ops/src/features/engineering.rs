//! Feature Engineering Implementation
//!
//! Extracted and refactored from trading-specific feature engineering to be domain agnostic.
//! Provides statistical, frequency, and custom feature extraction capabilities.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use super::{
    BatchFeatureResult, Feature, FeatureConfig, FeatureExtractor, FeatureQuality, FeatureRequest,
    FeatureType, NormalizationMethod, ProcessingStats,
};

/// Main feature engineering engine
pub struct FeatureEngine {
    config: FeatureConfig,
    extractors: Vec<Box<dyn FeatureExtractor>>,
    normalizers: HashMap<String, Box<dyn FeatureNormalizer>>,
}

impl std::fmt::Debug for FeatureEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureEngine")
            .field("config", &self.config)
            .field("extractors_count", &self.extractors.len())
            .field("normalizers_count", &self.normalizers.len())
            .finish()
    }
}

/// Configuration for individual feature extractors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureExtractorConfig {
    pub name: String,
    pub enabled: bool,
    pub parameters: HashMap<String, serde_json::Value>,
}

impl FeatureEngine {
    /// Create a new feature engine with the given configuration
    pub fn new(config: FeatureConfig) -> Self {
        let mut engine = Self {
            config: config.clone(),
            extractors: Vec::new(),
            normalizers: HashMap::new(),
        };
        
        engine.initialize_extractors();
        engine.initialize_normalizers();
        
        engine
    }
    
    /// Extract features from input data
    pub async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        if data.is_empty() {
            return Err(anyhow!("Input data is empty"));
        }
        
        info!("Extracting features from {} data points", data.len());
        let start_time = Instant::now();
        
        let mut all_features = Vec::new();
        
        // Extract features from each configured extractor
        for extractor in &self.extractors {
            match extractor.extract_features(data).await {
                Ok(mut features) => {
                    all_features.append(&mut features);
                }
                Err(e) => {
                    warn!("Feature extraction failed for extractor: {}", e);
                }
            }
        }
        
        // Apply normalization if configured
        if let Some(norm_config) = &self.config.normalization {
            all_features = self.apply_normalization(all_features, norm_config)?;
        }
        
        let duration = start_time.elapsed();
        info!("Feature extraction completed: {} features in {:?}", 
              all_features.len(), duration);
        
        Ok(all_features)
    }
    
    /// Extract features in batch with comprehensive results
    pub async fn extract_features_batch(
        &self,
        data_batches: &[Vec<f64>],
    ) -> Result<BatchFeatureResult> {
        let start_time = Instant::now();
        let mut all_features = Vec::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let mut processed_records = 0;
        let mut failed_records = 0;
        
        info!("Processing {} data batches", data_batches.len());
        
        for (batch_idx, batch) in data_batches.iter().enumerate() {
            match self.extract_features(batch).await {
                Ok(mut features) => {
                    all_features.append(&mut features);
                    processed_records += 1;
                }
                Err(e) => {
                    errors.push(format!("Batch {}: {}", batch_idx, e));
                    failed_records += 1;
                }
            }
        }
        
        // Calculate quality metrics
        let quality_metrics = self.calculate_quality_metrics(&all_features).await?;
        
        let duration = start_time.elapsed();
        let processing_stats = ProcessingStats {
            total_records: data_batches.len(),
            processed_records,
            failed_records,
            processing_time_ms: duration.as_millis() as u64,
            features_generated: all_features.len(),
            memory_used_mb: self.estimate_memory_usage(&all_features),
        };
        
        Ok(BatchFeatureResult {
            features: all_features,
            processing_stats,
            quality_metrics,
            errors,
            warnings,
        })
    }
    
    /// Process feature request
    pub async fn process_feature_request(
        &self,
        request: FeatureRequest,
    ) -> Result<Vec<Feature>> {
        info!("Processing feature request for data source: {}", request.data_source);
        
        // In a real implementation, this would fetch data based on the request
        // For now, we'll simulate with sample data
        let sample_data = self.generate_sample_data(&request)?;
        
        // Extract only requested features
        let mut all_features = self.extract_features(&sample_data).await?;
        
        // Filter features based on request
        if !request.feature_names.is_empty() {
            all_features.retain(|f| request.feature_names.contains(&f.name));
        }
        
        // Apply time range filtering if specified
        if let Some((start_time, end_time)) = request.time_range {
            all_features.retain(|f| f.timestamp >= start_time && f.timestamp <= end_time);
        }
        
        Ok(all_features)
    }
    
    /// Get available feature names from all extractors
    pub fn get_available_features(&self) -> Vec<String> {
        self.extractors
            .iter()
            .flat_map(|e| e.get_feature_names())
            .collect()
    }
    
    /// Validate feature engineering configuration
    pub fn validate_config(&self) -> Result<()> {
        if self.config.window_size == 0 {
            return Err(anyhow!("Window size must be greater than 0"));
        }
        
        if !self.config.statistical_features
            && !self.config.frequency_features
            && !self.config.technical_features
            && self.config.custom_features.is_empty()
        {
            return Err(anyhow!("At least one feature type must be enabled"));
        }
        
        Ok(())
    }
    
    // Private methods
    
    fn initialize_extractors(&mut self) {
        if self.config.statistical_features {
            self.extractors.push(Box::new(StatisticalFeatureExtractor::new(&self.config)));
        }
        
        if self.config.frequency_features {
            self.extractors.push(Box::new(FrequencyFeatureExtractor::new(&self.config)));
        }
        
        if self.config.technical_features {
            self.extractors.push(Box::new(TechnicalFeatureExtractor::new(&self.config)));
        }
        
        if self.config.wavelet_features {
            self.extractors.push(Box::new(WaveletFeatureExtractor::new(&self.config)));
        }
        
        // Add custom feature extractors
        for custom_config in &self.config.custom_features {
            if let Ok(extractor) = CustomFeatureExtractor::new(custom_config) {
                self.extractors.push(Box::new(extractor));
            }
        }
        
        info!("Initialized {} feature extractors", self.extractors.len());
    }
    
    fn initialize_normalizers(&mut self) {
        if let Some(norm_config) = &self.config.normalization {
            match norm_config.method {
                NormalizationMethod::StandardScaler => {
                    self.normalizers.insert("standard".to_string(), Box::new(StandardScaler::new()));
                }
                NormalizationMethod::MinMaxScaler => {
                    self.normalizers.insert("minmax".to_string(), Box::new(MinMaxScaler::new()));
                }
                NormalizationMethod::RobustScaler => {
                    self.normalizers.insert("robust".to_string(), Box::new(RobustScaler::new()));
                }
                NormalizationMethod::MaxAbsScaler => {
                    self.normalizers.insert("maxabs".to_string(), Box::new(MaxAbsScaler::new()));
                }
                NormalizationMethod::None => {}
            }
        }
    }
    
    fn apply_normalization(
        &self,
        features: Vec<Feature>,
        norm_config: &super::NormalizationConfig,
    ) -> Result<Vec<Feature>> {
        if norm_config.method == NormalizationMethod::None {
            return Ok(features);
        }
        
        let normalizer_key = match norm_config.method {
            NormalizationMethod::StandardScaler => "standard",
            NormalizationMethod::MinMaxScaler => "minmax",
            NormalizationMethod::RobustScaler => "robust",
            NormalizationMethod::MaxAbsScaler => "maxabs",
            NormalizationMethod::None => return Ok(features),
        };
        
        if let Some(normalizer) = self.normalizers.get(normalizer_key) {
            normalizer.normalize(features)
        } else {
            warn!("Normalizer not found: {}", normalizer_key);
            Ok(features)
        }
    }
    
    async fn calculate_quality_metrics(&self, features: &[Feature]) -> Result<Vec<FeatureQuality>> {
        let mut metrics = Vec::new();
        let feature_groups = self.group_features_by_name(features);
        
        for (feature_name, feature_values) in feature_groups {
            let quality = FeatureQuality {
                feature_name: feature_name.clone(),
                completeness: self.calculate_completeness(&feature_values),
                uniqueness: self.calculate_uniqueness(&feature_values),
                consistency: self.calculate_consistency(&feature_values),
                timeliness: self.calculate_timeliness(&feature_values),
                accuracy_score: 0.9, // Would calculate based on validation data
                drift_score: 0.1, // Would calculate based on historical distribution
                importance_score: 0.7, // Would calculate based on target correlation
            };
            
            metrics.push(quality);
        }
        
        Ok(metrics)
    }
    
    fn group_features_by_name<'a>(&self, features: &'a [Feature]) -> HashMap<String, Vec<&'a Feature>> {
        let mut groups: HashMap<String, Vec<&Feature>> = HashMap::new();
        
        for feature in features {
            groups.entry(feature.name.clone()).or_default().push(feature);
        }
        
        groups
    }
    
    fn calculate_completeness(&self, features: &[&Feature]) -> f64 {
        if features.is_empty() {
            return 0.0;
        }
        
        let non_null_count = features
            .iter()
            .filter(|f| !f.value.is_nan() && f.value.is_finite())
            .count();
        
        (non_null_count as f64) / (features.len() as f64)
    }
    
    fn calculate_uniqueness(&self, features: &[&Feature]) -> f64 {
        if features.is_empty() {
            return 0.0;
        }
        
        let mut unique_values = std::collections::HashSet::new();
        for feature in features {
            unique_values.insert(feature.value.to_bits());
        }
        
        (unique_values.len() as f64) / (features.len() as f64)
    }
    
    fn calculate_consistency(&self, features: &[&Feature]) -> f64 {
        if features.len() < 2 {
            return 1.0;
        }
        
        // Simple consistency check - variance of values
        let values: Vec<f64> = features.iter().map(|f| f.value).collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        
        // Higher consistency = lower relative standard deviation
        if mean.abs() > 0.0 {
            (1.0 - (std_dev / mean.abs())).max(0.0).min(1.0)
        } else {
            0.5
        }
    }
    
    fn calculate_timeliness(&self, features: &[&Feature]) -> f64 {
        if features.is_empty() {
            return 0.0;
        }
        
        let now = Utc::now();
        let latest_time = features
            .iter()
            .map(|f| f.timestamp)
            .max()
            .unwrap_or(now);
        
        let age_hours = (now - latest_time).num_hours() as f64;
        
        // Timeliness decreases with age (exponential decay)
        (-age_hours / 24.0).exp() // Good for 24 hours, then decreases
    }
    
    fn estimate_memory_usage(&self, features: &[Feature]) -> f64 {
        // Rough estimate: each feature ~100 bytes (including metadata)
        (features.len() * 100) as f64 / (1024.0 * 1024.0) // Convert to MB
    }
    
    fn generate_sample_data(&self, request: &FeatureRequest) -> Result<Vec<f64>> {
        // Generate sample data based on request parameters
        let data_size = request.parameters
            .get("data_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;
        
        let mut data = Vec::with_capacity(data_size);
        let mut rng = rand::thread_rng();
        
        // Generate synthetic time series data
        for i in 0..data_size {
            let trend = 0.01 * (i as f64);
            let seasonal = (2.0 * std::f64::consts::PI * i as f64 / 50.0).sin() * 0.5;
            let noise = (rng.gen::<f64>() - 0.5) * 0.2;
            data.push(100.0 + trend + seasonal + noise);
        }
        
        Ok(data)
    }
}

// Feature extractor implementations

/// Statistical feature extractor
pub struct StatisticalFeatureExtractor {
    window_size: usize,
}

impl StatisticalFeatureExtractor {
    fn new(config: &FeatureConfig) -> Self {
        Self {
            window_size: config.window_size,
        }
    }
}

#[async_trait::async_trait]
impl FeatureExtractor for StatisticalFeatureExtractor {
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        let mut features = Vec::new();
        let timestamp = Utc::now();
        
        if data.len() < self.window_size {
            return Err(anyhow!("Insufficient data for window size {}", self.window_size));
        }
        
        let window = &data[data.len() - self.window_size..];
        
        // Mean
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        features.push(Feature {
            name: "statistical_mean".to_string(),
            value: mean,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Standard deviation
        let variance = window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / window.len() as f64;
        let std_dev = variance.sqrt();
        features.push(Feature {
            name: "statistical_std".to_string(),
            value: std_dev,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Min and Max
        let min_val = window.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max_val = window.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        
        features.push(Feature {
            name: "statistical_min".to_string(),
            value: min_val,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        features.push(Feature {
            name: "statistical_max".to_string(),
            value: max_val,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Range
        features.push(Feature {
            name: "statistical_range".to_string(),
            value: max_val - min_val,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Skewness (simplified)
        let skewness = if std_dev > 0.0 {
            window.iter().map(|x| ((x - mean) / std_dev).powi(3)).sum::<f64>() / window.len() as f64
        } else {
            0.0
        };
        
        features.push(Feature {
            name: "statistical_skewness".to_string(),
            value: skewness,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Kurtosis (simplified)
        let kurtosis = if std_dev > 0.0 {
            window.iter().map(|x| ((x - mean) / std_dev).powi(4)).sum::<f64>() / window.len() as f64 - 3.0
        } else {
            0.0
        };
        
        features.push(Feature {
            name: "statistical_kurtosis".to_string(),
            value: kurtosis,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        Ok(features)
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec![
            "statistical_mean".to_string(),
            "statistical_std".to_string(),
            "statistical_min".to_string(),
            "statistical_max".to_string(),
            "statistical_range".to_string(),
            "statistical_skewness".to_string(),
            "statistical_kurtosis".to_string(),
        ]
    }
    
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "window_size": self.window_size,
            "features": ["mean", "std", "min", "max", "range", "skewness", "kurtosis"]
        })
    }
    
    async fn validate_input(&self, data: &[f64]) -> Result<()> {
        if data.len() < self.window_size {
            return Err(anyhow!("Data length {} is less than window size {}", 
                              data.len(), self.window_size));
        }
        Ok(())
    }
}

/// Frequency domain feature extractor
pub struct FrequencyFeatureExtractor {
    window_size: usize,
}

impl FrequencyFeatureExtractor {
    fn new(config: &FeatureConfig) -> Self {
        Self {
            window_size: config.window_size,
        }
    }
}

#[async_trait::async_trait]
impl FeatureExtractor for FrequencyFeatureExtractor {
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        let mut features = Vec::new();
        let timestamp = Utc::now();
        
        if data.len() < self.window_size {
            return Err(anyhow!("Insufficient data for FFT analysis"));
        }
        
        let window = &data[data.len() - self.window_size..];
        
        // Simplified frequency analysis (would use proper FFT in practice)
        // For now, calculate some basic frequency-related metrics
        
        // Zero crossing rate
        let zero_crossings = window.windows(2)
            .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
            .count();
        
        let zero_crossing_rate = zero_crossings as f64 / (window.len() - 1) as f64;
        
        features.push(Feature {
            name: "frequency_zero_crossing_rate".to_string(),
            value: zero_crossing_rate,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Spectral centroid (simplified)
        let mean = window.iter().sum::<f64>() / window.len() as f64;
        let spectral_centroid = window.iter().enumerate()
            .map(|(i, &x)| i as f64 * (x - mean).abs())
            .sum::<f64>() / window.iter().map(|&x| (x - mean).abs()).sum::<f64>();
        
        features.push(Feature {
            name: "frequency_spectral_centroid".to_string(),
            value: spectral_centroid,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        Ok(features)
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec![
            "frequency_zero_crossing_rate".to_string(),
            "frequency_spectral_centroid".to_string(),
        ]
    }
    
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "window_size": self.window_size,
            "features": ["zero_crossing_rate", "spectral_centroid"]
        })
    }
    
    async fn validate_input(&self, data: &[f64]) -> Result<()> {
        if data.len() < 4 {
            return Err(anyhow!("Insufficient data for frequency analysis"));
        }
        Ok(())
    }
}

/// Technical analysis feature extractor
pub struct TechnicalFeatureExtractor {
    window_size: usize,
}

impl TechnicalFeatureExtractor {
    fn new(config: &FeatureConfig) -> Self {
        Self {
            window_size: config.window_size,
        }
    }
}

#[async_trait::async_trait]
impl FeatureExtractor for TechnicalFeatureExtractor {
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        let mut features = Vec::new();
        let timestamp = Utc::now();
        
        if data.len() < self.window_size {
            return Err(anyhow!("Insufficient data for technical analysis"));
        }
        
        let window = &data[data.len() - self.window_size..];
        
        // Simple Moving Average
        let sma = window.iter().sum::<f64>() / window.len() as f64;
        features.push(Feature {
            name: "technical_sma".to_string(),
            value: sma,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Exponential Moving Average (simplified)
        let alpha = 2.0 / (self.window_size as f64 + 1.0);
        let mut ema = window[0];
        for &value in &window[1..] {
            ema = alpha * value + (1.0 - alpha) * ema;
        }
        
        features.push(Feature {
            name: "technical_ema".to_string(),
            value: ema,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        });
        
        // Rate of Change
        if window.len() >= 2 {
            let roc = (window.last().unwrap() - window.first().unwrap()) / window.first().unwrap();
            features.push(Feature {
                name: "technical_roc".to_string(),
                value: roc,
                feature_type: FeatureType::Numerical,
                timestamp,
                metadata: None,
            });
        }
        
        // Momentum
        let momentum_period = (self.window_size / 4).max(1);
        if window.len() > momentum_period {
            let momentum = window.last().unwrap() - window[window.len() - momentum_period - 1];
            features.push(Feature {
                name: "technical_momentum".to_string(),
                value: momentum,
                feature_type: FeatureType::Numerical,
                timestamp,
                metadata: None,
            });
        }
        
        Ok(features)
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec![
            "technical_sma".to_string(),
            "technical_ema".to_string(),
            "technical_roc".to_string(),
            "technical_momentum".to_string(),
        ]
    }
    
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "window_size": self.window_size,
            "features": ["sma", "ema", "roc", "momentum"]
        })
    }
    
    async fn validate_input(&self, data: &[f64]) -> Result<()> {
        if data.iter().any(|&x| x <= 0.0) {
            warn!("Technical analysis works best with positive values");
        }
        Ok(())
    }
}

/// Wavelet feature extractor (placeholder)
pub struct WaveletFeatureExtractor {
    window_size: usize,
}

impl WaveletFeatureExtractor {
    fn new(config: &FeatureConfig) -> Self {
        Self {
            window_size: config.window_size,
        }
    }
}

#[async_trait::async_trait]
impl FeatureExtractor for WaveletFeatureExtractor {
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        let timestamp = Utc::now();
        
        // Placeholder implementation - would use proper wavelet transform
        let energy = data.iter().map(|x| x * x).sum::<f64>() / data.len() as f64;
        
        Ok(vec![Feature {
            name: "wavelet_energy".to_string(),
            value: energy,
            feature_type: FeatureType::Numerical,
            timestamp,
            metadata: None,
        }])
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec!["wavelet_energy".to_string()]
    }
    
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "window_size": self.window_size,
            "features": ["energy"]
        })
    }
    
    async fn validate_input(&self, _data: &[f64]) -> Result<()> {
        Ok(())
    }
}

/// Custom feature extractor
pub struct CustomFeatureExtractor {
    name: String,
    parameters: HashMap<String, serde_json::Value>,
}

impl CustomFeatureExtractor {
    fn new(config: &super::CustomFeatureConfig) -> Result<Self> {
        Ok(Self {
            name: config.name.clone(),
            parameters: config.parameters.clone(),
        })
    }
}

#[async_trait::async_trait]
impl FeatureExtractor for CustomFeatureExtractor {
    async fn extract_features(&self, data: &[f64]) -> Result<Vec<Feature>> {
        let timestamp = Utc::now();
        
        // Placeholder for custom feature extraction logic
        // In practice, this would implement user-defined feature calculations
        
        let custom_value = data.iter().sum::<f64>() / data.len() as f64;
        
        Ok(vec![Feature {
            name: format!("custom_{}", self.name),
            value: custom_value,
            feature_type: FeatureType::Custom(self.name.clone()),
            timestamp,
            metadata: Some([("extractor".to_string(), "custom".to_string())].into()),
        }])
    }
    
    fn get_feature_names(&self) -> Vec<String> {
        vec![format!("custom_{}", self.name)]
    }
    
    fn get_config(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "parameters": self.parameters
        })
    }
    
    async fn validate_input(&self, _data: &[f64]) -> Result<()> {
        Ok(())
    }
}

// Feature normalization implementations

trait FeatureNormalizer: Send + Sync {
    fn normalize(&self, features: Vec<Feature>) -> Result<Vec<Feature>>;
}

struct StandardScaler;

impl StandardScaler {
    fn new() -> Self {
        Self
    }
}

impl FeatureNormalizer for StandardScaler {
    fn normalize(&self, features: Vec<Feature>) -> Result<Vec<Feature>> {
        let mut normalized = features;
        
        // Group by feature name for per-feature normalization
        let mut feature_groups: HashMap<String, Vec<f64>> = HashMap::new();
        for feature in &normalized {
            feature_groups.entry(feature.name.clone()).or_default().push(feature.value);
        }
        
        // Calculate mean and std for each feature
        let mut stats: HashMap<String, (f64, f64)> = HashMap::new();
        for (name, values) in &feature_groups {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;
            let std_dev = variance.sqrt();
            stats.insert(name.clone(), (mean, std_dev));
        }
        
        // Apply normalization
        for feature in &mut normalized {
            if let Some((mean, std_dev)) = stats.get(&feature.name) {
                if *std_dev > 1e-8 {
                    feature.value = (feature.value - mean) / std_dev;
                }
            }
        }
        
        Ok(normalized)
    }
}

struct MinMaxScaler;

impl MinMaxScaler {
    fn new() -> Self {
        Self
    }
}

impl FeatureNormalizer for MinMaxScaler {
    fn normalize(&self, features: Vec<Feature>) -> Result<Vec<Feature>> {
        let mut normalized = features;
        
        // Group by feature name
        let mut feature_groups: HashMap<String, Vec<f64>> = HashMap::new();
        for feature in &normalized {
            feature_groups.entry(feature.name.clone()).or_default().push(feature.value);
        }
        
        // Calculate min and max for each feature
        let mut stats: HashMap<String, (f64, f64)> = HashMap::new();
        for (name, values) in &feature_groups {
            let min_val = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max_val = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            stats.insert(name.clone(), (min_val, max_val));
        }
        
        // Apply normalization
        for feature in &mut normalized {
            if let Some((min_val, max_val)) = stats.get(&feature.name) {
                if (max_val - min_val).abs() > 1e-8 {
                    feature.value = (feature.value - min_val) / (max_val - min_val);
                }
            }
        }
        
        Ok(normalized)
    }
}

struct RobustScaler;

impl RobustScaler {
    fn new() -> Self {
        Self
    }
}

impl FeatureNormalizer for RobustScaler {
    fn normalize(&self, features: Vec<Feature>) -> Result<Vec<Feature>> {
        let mut normalized = features;
        
        // Group by feature name
        let mut feature_groups: HashMap<String, Vec<f64>> = HashMap::new();
        for feature in &normalized {
            feature_groups.entry(feature.name.clone()).or_default().push(feature.value);
        }
        
        // Calculate median and IQR for each feature
        let mut stats: HashMap<String, (f64, f64)> = HashMap::new();
        for (name, mut values) in feature_groups {
            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = values.len();
            
            let median = if n % 2 == 0 {
                (values[n / 2 - 1] + values[n / 2]) / 2.0
            } else {
                values[n / 2]
            };
            
            let q1 = values[n / 4];
            let q3 = values[(3 * n) / 4];
            let iqr = q3 - q1;
            
            stats.insert(name, (median, iqr));
        }
        
        // Apply normalization
        for feature in &mut normalized {
            if let Some((median, iqr)) = stats.get(&feature.name) {
                if iqr.abs() > 1e-8 {
                    feature.value = (feature.value - median) / iqr;
                }
            }
        }
        
        Ok(normalized)
    }
}

struct MaxAbsScaler;

impl MaxAbsScaler {
    fn new() -> Self {
        Self
    }
}

impl FeatureNormalizer for MaxAbsScaler {
    fn normalize(&self, features: Vec<Feature>) -> Result<Vec<Feature>> {
        let mut normalized = features;
        
        // Group by feature name
        let mut feature_groups: HashMap<String, Vec<f64>> = HashMap::new();
        for feature in &normalized {
            feature_groups.entry(feature.name.clone()).or_default().push(feature.value);
        }
        
        // Calculate max absolute value for each feature
        let mut max_abs: HashMap<String, f64> = HashMap::new();
        for (name, values) in &feature_groups {
            let max_abs_val = values.iter().map(|x| x.abs()).fold(0.0_f64, |a, b| a.max(b));
            max_abs.insert(name.clone(), max_abs_val);
        }
        
        // Apply normalization
        for feature in &mut normalized {
            if let Some(max_abs_val) = max_abs.get(&feature.name) {
                if *max_abs_val > 1e-8 {
                    feature.value = feature.value / max_abs_val;
                }
            }
        }
        
        Ok(normalized)
    }
}

// Add this to resolve compilation issues
use rand::Rng;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_feature_engine() {
        let config = FeatureConfig::default();
        let engine = FeatureEngine::new(config);
        
        let data: Vec<f64> = (0..100).map(|i| (i as f64) + rand::random::<f64>()).collect();
        let features = engine.extract_features(&data).await.unwrap();
        
        assert!(!features.is_empty());
        assert!(features.iter().any(|f| f.name.starts_with("statistical")));
    }
    
    #[tokio::test]
    async fn test_statistical_extractor() {
        let config = FeatureConfig::default();
        let extractor = StatisticalFeatureExtractor::new(&config);
        
        let data: Vec<f64> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let features = extractor.extract_features(&data).await.unwrap();
        
        assert!(features.len() >= 5);
        assert!(features.iter().any(|f| f.name == "statistical_mean"));
    }
}