//! Training Data Service
//!
//! This service bridges the DataAccessLayer to neural models, providing
//! efficient data transformation, caching, and format conversion for
//! different model types (LSTM, MLP, etc.).

use crate::data::{RedisCache, TimeSeriesData, TimescaleDBStorage};
use crate::integration::data_access::{DataAccessLayer, Timeframe};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// Configuration for training data loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataConfig {
    pub batch_size: usize,
    pub sequence_length: usize, // For LSTM models
    pub feature_window: usize,  // Lookback window for features
    pub normalize: bool,
    pub include_volume: bool,
    pub include_indicators: bool,
    pub cache_enabled: bool,
    pub cache_ttl_seconds: u64,
}

impl Default for TrainingDataConfig {
    fn default() -> Self {
        Self {
            batch_size: 32,
            sequence_length: 50, // 50 timesteps for LSTM
            feature_window: 20,  // 20 periods for technical indicators
            normalize: true,
            include_volume: true,
            include_indicators: true,
            cache_enabled: true,
            cache_ttl_seconds: 3600, // 1 hour
        }
    }
}

/// Model type for data preparation
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelType {
    MLP,      // Multi-Layer Perceptron (flat features)
    LSTM,     // Long Short-Term Memory (sequences)
    GRU,      // Gated Recurrent Unit (sequences)
    CNN,      // Convolutional Neural Network (2D features)
    Ensemble, // Multiple models combined
    DeepAR,   // DeepAR-style network
    TCN,      // Temporal Convolutional Network
    NHITS,    // N-HiTS network
}

/// Prepared training data ready for neural models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedTrainingData {
    pub model_type: ModelType,
    pub symbol: String,
    pub features: Vec<Vec<f64>>, // Shape depends on model type
    pub targets: Vec<f64>,       // Prediction targets
    pub timestamps: Vec<DateTime<Utc>>,
    pub feature_names: Vec<String>,
    pub normalization_params: Option<NormalizationParams>,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Normalization parameters for data scaling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationParams {
    pub feature_means: Vec<f64>,
    pub feature_stds: Vec<f64>,
    pub target_mean: f64,
    pub target_std: f64,
}

/// Validation error types
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Insufficient data: got {got}, need at least {need}")]
    InsufficientData { got: usize, need: usize },

    #[error("Invalid data values: {0}")]
    InvalidValues(String),

    #[error("Missing required features: {0:?}")]
    MissingFeatures(Vec<String>),

    #[error("Data quality issue: {0}")]
    QualityIssue(String),
}

/// Performance metrics for monitoring
#[derive(Debug, Clone, Default)]
struct ServiceMetrics {
    total_batches_loaded: u64,
    cache_hits: u64,
    cache_misses: u64,
    total_data_points: u64,
    average_preparation_time_ms: f64,
    last_error: Option<String>,
}

/// Main training data service
pub struct TrainingDataService {
    data_access: Arc<DataAccessLayer>,
    cache: Arc<RedisCache>,
    metrics: Arc<RwLock<ServiceMetrics>>,
    preparation_semaphore: Arc<Semaphore>,
}

impl TrainingDataService {
    /// Create a new TrainingDataService
    pub async fn new(storage: Arc<TimescaleDBStorage>, cache: Arc<RedisCache>) -> Result<Self> {
        let data_access = Arc::new(DataAccessLayer::new(storage, cache.clone()).await?);

        Ok(Self {
            data_access,
            cache,
            metrics: Arc::new(RwLock::new(ServiceMetrics::default())),
            preparation_semaphore: Arc::new(Semaphore::new(4)), // Limit concurrent preparations
        })
    }

    /// Load a training batch for a specific model type
    pub async fn load_training_batch(
        &self,
        model_type: ModelType,
        symbol: &str,
        config: TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        let start_time = std::time::Instant::now();
        debug!(
            "Loading training batch for {:?} model, symbol: {}",
            model_type, symbol
        );

        // Try cache first if enabled
        if config.cache_enabled {
            let cache_key = format!(
                "training_data:{}:{}:{:?}",
                symbol,
                serde_json::to_string(&model_type)?,
                config.batch_size
            );

            if let Ok(Some(cached_data)) = self.cache.get::<PreparedTrainingData>(&cache_key).await
            {
                debug!("Cache hit for training data");
                let mut metrics = self.metrics.write().await;
                metrics.cache_hits += 1;
                return Ok(cached_data);
            }
        }

        // Acquire semaphore to limit concurrent preparations
        let _permit = self.preparation_semaphore.acquire().await?;

        // Load raw data
        let raw_data = self.load_raw_data(symbol, &config).await?;

        // Validate data
        self.validate_training_data(&raw_data)?;

        // Prepare data based on model type
        let prepared_data = match model_type {
            ModelType::MLP => self.prepare_mlp_data(symbol, raw_data, &config).await?,
            ModelType::LSTM | ModelType::GRU => {
                self.prepare_sequence_data(symbol, raw_data, &config, model_type.clone())
                    .await?
            }
            ModelType::CNN => self.prepare_cnn_data(symbol, raw_data, &config).await?,
            ModelType::Ensemble => {
                self.prepare_ensemble_data(symbol, raw_data, &config)
                    .await?
            }
            ModelType::DeepAR => {
                // DeepAR requires sequence data with probabilistic features
                self.prepare_sequence_data(symbol, raw_data, &config, model_type.clone())
                    .await?
            }
            ModelType::TCN => {
                // TCN uses sequence data with convolutional approach
                self.prepare_sequence_data(symbol, raw_data, &config, model_type.clone())
                    .await?
            }
            ModelType::NHITS => {
                // NHITS requires hierarchical sequence data
                self.prepare_sequence_data(symbol, raw_data, &config, model_type.clone())
                    .await?
            }
        };

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_batches_loaded += 1;
            metrics.cache_misses += 1;
            metrics.total_data_points += prepared_data.features.len() as u64;

            let prep_time = start_time.elapsed().as_millis() as f64;
            metrics.average_preparation_time_ms = (metrics.average_preparation_time_ms
                * (metrics.total_batches_loaded - 1) as f64
                + prep_time)
                / metrics.total_batches_loaded as f64;
        }

        // Cache the prepared data if enabled
        if config.cache_enabled {
            let cache_key = format!(
                "training_data:{}:{}:{:?}",
                symbol,
                serde_json::to_string(&model_type)?,
                config.batch_size
            );

            if let Err(e) = self
                .cache
                .set(&cache_key, &prepared_data, Some(config.cache_ttl_seconds))
                .await
            {
                warn!("Failed to cache prepared training data: {}", e);
            }
        }

        info!(
            "Prepared {} training samples in {:?}",
            prepared_data.features.len(),
            start_time.elapsed()
        );
        Ok(prepared_data)
    }

    /// Prepare online data for real-time predictions
    pub async fn prepare_online_data(&self, symbol: &str, window: usize) -> Result<TimeSeriesData> {
        debug!(
            "Preparing online data for symbol: {}, window: {}",
            symbol, window
        );

        // Get recent data
        let data = self
            .data_access
            .get_market_data(symbol, Timeframe::Minute)
            .await?;

        if data.is_empty() {
            bail!("No data available for symbol: {}", symbol);
        }

        // Get the most recent data point
        let latest = data
            .last()
            .ok_or_else(|| anyhow::anyhow!("No data available"))?;

        // Calculate indicators from the window
        let mut indicators = HashMap::new();
        if data.len() >= window {
            let window_data = &data[data.len() - window..];

            // Simple Moving Average
            let sma = window_data.iter().map(|d| d.close).sum::<f64>() / window as f64;
            indicators.insert("sma".to_string(), sma);

            // Price change
            let price_change = (latest.close - window_data[0].close) / window_data[0].close;
            indicators.insert("price_change".to_string(), price_change);

            // Volume average
            let volume_avg = window_data.iter().map(|d| d.volume).sum::<f64>() / window as f64;
            indicators.insert("volume_avg".to_string(), volume_avg);

            // Volatility (simple standard deviation)
            let mean = sma;
            let variance = window_data
                .iter()
                .map(|d| (d.close - mean).powi(2))
                .sum::<f64>()
                / window as f64;
            let volatility = variance.sqrt();
            indicators.insert("volatility".to_string(), volatility);
        }

        Ok(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: latest.timestamp,
            open: latest.open,
            high: latest.high,
            low: latest.low,
            close: latest.close,
            volume: latest.volume,
            indicators,
            source: Some("online".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(latest.close),
            metadata: Some(serde_json::json!({
                "window_size": window,
                "data_points": data.len()
            })),
            // Required fields for vendor model integration
            values: vec![latest.close],
            timestamps: vec![latest.timestamp],
            metadata_map: HashMap::new(),
        })
    }

    /// Validate training data
    pub fn validate_training_data(&self, data: &[TimeSeriesData]) -> Result<(), ValidationError> {
        // Check minimum data requirements
        if data.len() < 100 {
            return Err(ValidationError::InsufficientData {
                got: data.len(),
                need: 100,
            });
        }

        // Check for invalid values
        for (i, point) in data.iter().enumerate() {
            if point.close <= 0.0 || point.close.is_nan() || point.close.is_infinite() {
                return Err(ValidationError::InvalidValues(format!(
                    "Invalid close price at index {}: {}",
                    i, point.close
                )));
            }

            if point.volume < 0.0 || point.volume.is_nan() || point.volume.is_infinite() {
                return Err(ValidationError::InvalidValues(format!(
                    "Invalid volume at index {}: {}",
                    i, point.volume
                )));
            }

            if point.high < point.low {
                return Err(ValidationError::QualityIssue(format!(
                    "High < Low at index {}: {} < {}",
                    i, point.high, point.low
                )));
            }
        }

        // Check data continuity (no large gaps)
        for i in 1..data.len() {
            let time_diff = data[i].timestamp - data[i - 1].timestamp;
            if time_diff > Duration::hours(24) {
                return Err(ValidationError::QualityIssue(format!(
                    "Large time gap detected: {} hours between indices {} and {}",
                    time_diff.num_hours(),
                    i - 1,
                    i
                )));
            }
        }

        Ok(())
    }

    /// Get service metrics
    pub async fn get_metrics(&self) -> ServiceMetrics {
        self.metrics.read().await.clone()
    }

    // Private helper methods

    async fn load_raw_data(
        &self,
        symbol: &str,
        config: &TrainingDataConfig,
    ) -> Result<Vec<TimeSeriesData>> {
        // Calculate required data size based on config
        let required_size = config.batch_size + config.sequence_length + config.feature_window;

        // Load more data than needed to account for feature calculation
        let data = self
            .data_access
            .get_market_data(symbol, Timeframe::Hourly)
            .await?;

        if data.len() < required_size {
            // Try to load more data from daily timeframe
            let daily_data = self
                .data_access
                .get_market_data(symbol, Timeframe::Daily)
                .await?;
            if daily_data.len() >= required_size {
                Ok(daily_data)
            } else {
                bail!(
                    "Insufficient data for training: got {}, need {}",
                    daily_data.len(),
                    required_size
                );
            }
        } else {
            Ok(data)
        }
    }

    async fn prepare_mlp_data(
        &self,
        symbol: &str,
        mut data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        // Sort data by timestamp
        data.sort_by_key(|d| d.timestamp);

        let mut features = Vec::new();
        let mut targets = Vec::new();
        let mut timestamps = Vec::new();
        let mut feature_names = vec![
            "close".to_string(),
            "volume".to_string(),
            "high_low_ratio".to_string(),
            "close_open_ratio".to_string(),
        ];

        // Add indicator names
        if config.include_indicators {
            feature_names.extend(vec![
                "sma_5".to_string(),
                "sma_20".to_string(),
                "rsi".to_string(),
                "volume_ratio".to_string(),
            ]);
        }

        // Calculate features for each data point
        for i in config.feature_window..data.len() - 1 {
            let window = &data[i - config.feature_window..i];
            let current = &data[i];
            let next = &data[i + 1];

            // Basic features
            let mut feature_vec = vec![
                current.close,
                if config.include_volume {
                    current.volume
                } else {
                    0.0
                },
                current.high / current.low,
                current.close / current.open,
            ];

            // Technical indicators
            if config.include_indicators {
                // SMA 5
                let sma_5 = window.iter().rev().take(5).map(|d| d.close).sum::<f64>() / 5.0;

                // SMA 20
                let sma_20 = window
                    .iter()
                    .rev()
                    .take(20.min(window.len()))
                    .map(|d| d.close)
                    .sum::<f64>()
                    / 20.0_f64.min(window.len() as f64);

                // Simple RSI approximation
                let gains: f64 = window
                    .windows(2)
                    .map(|w| (w[1].close - w[0].close).max(0.0))
                    .sum();
                let losses: f64 = window
                    .windows(2)
                    .map(|w| (w[0].close - w[1].close).max(0.0))
                    .sum();
                let rsi = if losses > 0.0 {
                    100.0 - (100.0 / (1.0 + gains / losses))
                } else {
                    100.0
                };

                // Volume ratio
                let avg_volume = window.iter().map(|d| d.volume).sum::<f64>() / window.len() as f64;
                let volume_ratio = if avg_volume > 0.0 {
                    current.volume / avg_volume
                } else {
                    1.0
                };

                feature_vec.extend(vec![sma_5, sma_20, rsi, volume_ratio]);
            }

            features.push(feature_vec);
            targets.push(next.close);
            timestamps.push(current.timestamp);

            if features.len() >= config.batch_size {
                break;
            }
        }

        // Normalize if requested
        let normalization_params = if config.normalize {
            Some(self.normalize_features(&mut features, &mut targets)?)
        } else {
            None
        };

        Ok(PreparedTrainingData {
            model_type: ModelType::MLP,
            symbol: symbol.to_string(),
            features,
            targets,
            timestamps,
            feature_names,
            normalization_params,
            metadata: HashMap::new(),
        })
    }

    async fn prepare_sequence_data(
        &self,
        symbol: &str,
        mut data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
        model_type: ModelType,
    ) -> Result<PreparedTrainingData> {
        // Sort data by timestamp
        data.sort_by_key(|d| d.timestamp);

        let mut sequences = Vec::new();
        let mut targets = Vec::new();
        let mut timestamps = Vec::new();
        let feature_names = vec![
            "close".to_string(),
            "volume".to_string(),
            "high".to_string(),
            "low".to_string(),
            "open".to_string(),
        ];

        // Create sequences
        for i in config.sequence_length..data.len() - 1 {
            let sequence_data = &data[i - config.sequence_length..i];
            let next = &data[i + 1];

            // Create sequence of features
            let mut sequence = Vec::new();
            for point in sequence_data {
                sequence.push(vec![
                    point.close,
                    if config.include_volume {
                        point.volume
                    } else {
                        0.0
                    },
                    point.high,
                    point.low,
                    point.open,
                ]);
            }

            // Flatten sequence for storage (will be reshaped by model)
            let flattened: Vec<f64> = sequence.into_iter().flatten().collect();
            sequences.push(flattened);
            targets.push(next.close);
            timestamps.push(data[i].timestamp);

            if sequences.len() >= config.batch_size {
                break;
            }
        }

        // Normalize if requested
        let normalization_params = if config.normalize {
            Some(self.normalize_features(&mut sequences, &mut targets)?)
        } else {
            None
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "sequence_length".to_string(),
            serde_json::json!(config.sequence_length),
        );
        metadata.insert(
            "features_per_step".to_string(),
            serde_json::json!(feature_names.len()),
        );

        Ok(PreparedTrainingData {
            model_type,
            symbol: symbol.to_string(),
            features: sequences,
            targets,
            timestamps,
            feature_names,
            normalization_params,
            metadata,
        })
    }

    async fn prepare_cnn_data(
        &self,
        symbol: &str,
        data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        // For CNN, we can create 2D feature maps
        // This is a simplified version - real implementation would create more sophisticated features
        self.prepare_mlp_data(symbol, data, config)
            .await
            .map(|mut prepared| {
                prepared.model_type = ModelType::CNN;
                prepared
            })
    }

    async fn prepare_ensemble_data(
        &self,
        symbol: &str,
        data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        // For ensemble, prepare data that can be used by multiple model types
        // This returns MLP-style features that can be adapted by each model
        self.prepare_mlp_data(symbol, data, config)
            .await
            .map(|mut prepared| {
                prepared.model_type = ModelType::Ensemble;
                prepared
            })
    }

    fn normalize_features(
        &self,
        features: &mut Vec<Vec<f64>>,
        targets: &mut Vec<f64>,
    ) -> Result<NormalizationParams> {
        if features.is_empty() {
            bail!("Cannot normalize empty features");
        }

        let num_features = features[0].len();
        let mut feature_means = vec![0.0; num_features];
        let mut feature_stds = vec![0.0; num_features];

        // Calculate means
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                feature_means[i] += value;
            }
        }
        for mean in &mut feature_means {
            *mean /= features.len() as f64;
        }

        // Calculate standard deviations
        for feature_vec in features.iter() {
            for (i, &value) in feature_vec.iter().enumerate() {
                feature_stds[i] += (value - feature_means[i]).powi(2);
            }
        }
        for std in &mut feature_stds {
            *std = (*std / features.len() as f64).sqrt();
            if *std < 1e-8 {
                *std = 1.0; // Avoid division by zero
            }
        }

        // Normalize features
        for feature_vec in features.iter_mut() {
            for (i, value) in feature_vec.iter_mut().enumerate() {
                *value = (*value - feature_means[i]) / feature_stds[i];
            }
        }

        // Normalize targets
        let target_mean = targets.iter().sum::<f64>() / targets.len() as f64;
        let target_std = (targets
            .iter()
            .map(|&t| (t - target_mean).powi(2))
            .sum::<f64>()
            / targets.len() as f64)
            .sqrt()
            .max(1e-8);

        for target in targets.iter_mut() {
            *target = (*target - target_mean) / target_std;
        }

        Ok(NormalizationParams {
            feature_means,
            feature_stds,
            target_mean,
            target_std,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_display() {
        let err = ValidationError::InsufficientData { got: 50, need: 100 };
        assert_eq!(
            err.to_string(),
            "Insufficient data: got 50, need at least 100"
        );

        let err = ValidationError::InvalidValues("NaN detected".to_string());
        assert_eq!(err.to_string(), "Invalid data values: NaN detected");
    }

    #[test]
    fn test_training_config_default() {
        let config = TrainingDataConfig::default();
        assert_eq!(config.batch_size, 32);
        assert_eq!(config.sequence_length, 50);
        assert_eq!(config.feature_window, 20);
        assert!(config.normalize);
        assert!(config.include_volume);
        assert!(config.include_indicators);
        assert!(config.cache_enabled);
        assert_eq!(config.cache_ttl_seconds, 3600);
    }

    #[test]
    fn test_model_type_equality() {
        assert_eq!(ModelType::MLP, ModelType::MLP);
        assert_ne!(ModelType::MLP, ModelType::LSTM);
        assert_ne!(ModelType::LSTM, ModelType::GRU);
    }
}
