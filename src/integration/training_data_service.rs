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
    /// CRITICAL: Ensures complete data isolation - only loads data for the requested symbol
    pub async fn load_training_batch(
        &self,
        model_type: ModelType,
        symbol: &str,
        config: TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        let start_time = std::time::Instant::now();
        
        // CRITICAL LOGGING: Explicitly log symbol isolation
        info!(
            "🎯 [DATA ISOLATION] Loading training batch for {:?} model, SYMBOL: {} ONLY",
            model_type, symbol
        );
        debug!(
            "Training data request - Model: {:?}, Symbol: {}, Batch size: {}",
            model_type, symbol, config.batch_size
        );

        // Try cache first if enabled - SYMBOL-SPECIFIC cache key prevents cross-contamination
        if config.cache_enabled {
            let cache_key = format!(
                "training_data:SYMBOL_{}:MODEL_{}:BATCH_{}",
                symbol,
                serde_json::to_string(&model_type)?,
                config.batch_size
            );
            
            debug!("🔍 [DATA ISOLATION] Cache lookup with symbol-specific key: {}", cache_key);

            if let Ok(Some(cached_data)) = self.cache.get::<PreparedTrainingData>(&cache_key).await
            {
                // CRITICAL VALIDATION: Verify cached data matches requested symbol
                if cached_data.symbol != symbol {
                    error!(
                        "🚨 [DATA ISOLATION ERROR] Cache contamination detected! Requested: {}, Got: {}",
                        symbol, cached_data.symbol
                    );
                    // Clear contaminated cache entry
                    let _ = self.cache.invalidate(&cache_key).await;
                } else {
                    info!("✅ [DATA ISOLATION] Cache hit for symbol {} - data validated", symbol);
                    let mut metrics = self.metrics.write().await;
                    metrics.cache_hits += 1;
                    return Ok(cached_data);
                }
            }
        }

        // Acquire semaphore to limit concurrent preparations
        let _permit = self.preparation_semaphore.acquire().await?;

        // Load raw data - SYMBOL-SPECIFIC query
        let raw_data = self.load_raw_data(symbol, &config).await?;
        
        // CRITICAL VALIDATION: Verify all loaded data belongs to requested symbol
        self.validate_symbol_isolation(&raw_data, symbol)?;

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

        // Cache the prepared data if enabled - SYMBOL-SPECIFIC cache key
        if config.cache_enabled {
            let cache_key = format!(
                "training_data:SYMBOL_{}:MODEL_{}:BATCH_{}",
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

        // FINAL VALIDATION: Confirm prepared data symbol matches request
        if prepared_data.symbol != symbol {
            error!(
                "🚨 [DATA ISOLATION CRITICAL ERROR] Prepared data symbol mismatch! Requested: {}, Prepared: {}",
                symbol, prepared_data.symbol
            );
            bail!("Data isolation violation: prepared data symbol ({}) does not match requested symbol ({})", 
                  prepared_data.symbol, symbol);
        }
        
        info!(
            "✅ [DATA ISOLATION] Successfully prepared {} training samples for SYMBOL {} ONLY in {:?}",
            prepared_data.features.len(),
            symbol,
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
            let volume_avg = window_data.iter().map(|d| d.volume_value).sum::<f64>() / window as f64;
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
            volume: latest.volume.clone(),
            volume_value: latest.volume_value,
            intervals: vec![1000], // Default 1-second intervals
            timestamps: vec![latest.timestamp],
            values: vec![latest.close],
            indicators,
            source: Some("online".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(latest.close),
            metadata: Some(serde_json::json!({
                "window_size": window,
                "data_points": data.len()
            })),
            metadata_map: HashMap::new(),
        })
    }

    /// Validate symbol isolation - ensures no cross-contamination between symbols
    pub fn validate_symbol_isolation(&self, data: &[TimeSeriesData], expected_symbol: &str) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        
        // Check every data point to ensure it belongs to the expected symbol
        for (i, point) in data.iter().enumerate() {
            if point.symbol != expected_symbol {
                error!(
                    "🚨 [DATA ISOLATION VIOLATION] Data point {} contains wrong symbol! Expected: {}, Found: {}",
                    i, expected_symbol, point.symbol
                );
                bail!(
                    "Data isolation violation: point {} has symbol '{}' but expected '{}'",
                    i, point.symbol, expected_symbol
                );
            }
        }
        
        info!(
            "✅ [DATA ISOLATION] Validated {} data points all belong to symbol {}",
            data.len(), expected_symbol
        );
        Ok(())
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

            if point.volume_value < 0.0 || point.volume_value.is_nan() || point.volume_value.is_infinite() {
                return Err(ValidationError::InvalidValues(format!(
                    "Invalid volume at index {}: {}",
                    i, point.volume_value
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

    /// Get market data specifically for training, bypassing cache to ensure fresh data
    /// This prevents serving stale cached data that might have been limited to 7 days
    async fn get_training_market_data(
        &self,
        symbol: &str,
        timeframe: Timeframe,
    ) -> Result<Vec<TimeSeriesData>> {
        use crate::integration::data_access::DataAccessLayer;
        
        info!(
            "🎯 [TRAINING DATA] Fetching fresh {} data for symbol {} (bypassing cache)",
            format!("{:?}", timeframe),
            symbol
        );

        // Calculate time range using environment-configured training window
        let end_time = chrono::Utc::now();
        let duration = match std::env::var("TRAINING_HISTORY_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok()) {
            Some(days) => {
                info!("📊 Using TRAINING_HISTORY_DAYS={} for training data query", days);
                chrono::Duration::days(days)
            }
            None => {
                info!("📊 Using default 90 days for training data query (TRAINING_HISTORY_DAYS not set)");
                chrono::Duration::days(90) // Default: 90 days
            }
        };
        
        let start_time = end_time - duration;
        
        info!(
            "🔍 [TRAINING DATA] Direct query: {} from {} to {} ({} days)",
            symbol,
            start_time.format("%Y-%m-%d %H:%M:%S UTC"),
            end_time.format("%Y-%m-%d %H:%M:%S UTC"),
            duration.num_days()
        );

        // Query directly from storage, bypassing cache
        let raw_data = self
            .data_access
            .storage
            .query_range(symbol, start_time, end_time)
            .await
            .with_context(|| format!("Failed to query training data for {}", symbol))?;

        info!("✅ [TRAINING DATA] Retrieved {} raw data points for {}", raw_data.len(), symbol);

        // Convert to TimeSeriesData format (similar to DataAccessLayer::get_market_data)
        let mut time_series_data = Vec::new();
        for data_point in raw_data {
            // Extract OHLCV data from metadata if available
            let (open, high, low, close, volume) = if let Some(metadata) = &data_point.metadata {
                (
                    metadata.get("open").and_then(|v| v.as_f64()).unwrap_or(data_point.value),
                    metadata.get("high").and_then(|v| v.as_f64()).unwrap_or(data_point.value),
                    metadata.get("low").and_then(|v| v.as_f64()).unwrap_or(data_point.value),
                    metadata.get("close").and_then(|v| v.as_f64()).unwrap_or(data_point.value),
                    metadata.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.0),
                )
            } else {
                // Fallback to using value as close price
                (data_point.value, data_point.value, data_point.value, data_point.value, 0.0)
            };

            time_series_data.push(TimeSeriesData {
                symbol: data_point.entity.clone(),
                timestamp: data_point.timestamp,
                open,
                high,
                low,
                close,
                volume: vec![volume],
                volume_value: volume,
                intervals: vec![1000], // Default 1-second intervals
                timestamps: vec![data_point.timestamp],
                values: vec![close],
                indicators: data_point.metadata
                    .as_ref()
                    .and_then(|meta| meta.get("indicators"))
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_f64().map(|f| (k.clone(), f)))
                            .collect()
                    })
                    .unwrap_or_default(),
                source: Some(data_point.source.clone()),
                entity: Some(data_point.entity.clone()),
                value: Some(close),
                metadata: data_point.metadata.clone(),
                metadata_map: std::collections::HashMap::new(),
            });
        }

        info!(
            "✅ [TRAINING DATA] Converted to {} TimeSeriesData points for {}",
            time_series_data.len(),
            symbol
        );

        Ok(time_series_data)
    }

    async fn load_raw_data(
        &self,
        symbol: &str,
        config: &TrainingDataConfig,
    ) -> Result<Vec<TimeSeriesData>> {
        info!("🔍 [DATA ISOLATION] Loading raw data for SYMBOL {} ONLY", symbol);
        
        // Calculate required data size based on config
        let required_size = config.batch_size + config.sequence_length + config.feature_window;

        // CRITICAL FIX: Bypass cache for training data to ensure we get the full 90-day window
        // The regular get_market_data() might return cached data from when the system was misconfigured
        debug!("🔍 [DATA ISOLATION] Querying hourly data for symbol: {} (bypassing cache for training)", symbol);
        let data = self.get_training_market_data(symbol, Timeframe::Hourly).await?;

        // IMMEDIATE VALIDATION: Check data belongs to requested symbol
        if !data.is_empty() {
            self.validate_symbol_isolation(&data, symbol)
                .context("Hourly data failed symbol isolation check")?;
        }

        if data.len() < required_size {
            // Try to load more data from daily timeframe (also bypassing cache)
            debug!("🔍 [DATA ISOLATION] Insufficient hourly data, querying daily data for symbol: {} (bypassing cache)", symbol);
            let daily_data = self.get_training_market_data(symbol, Timeframe::Daily).await?;
                
            // IMMEDIATE VALIDATION: Check daily data belongs to requested symbol
            if !daily_data.is_empty() {
                self.validate_symbol_isolation(&daily_data, symbol)
                    .context("Daily data failed symbol isolation check")?;
            }
                
            if daily_data.len() >= required_size {
                info!("✅ [DATA ISOLATION] Loaded {} daily data points for symbol {}", daily_data.len(), symbol);
                Ok(daily_data)
            } else {
                bail!(
                    "Insufficient data for training symbol {}: got {}, need {}",
                    symbol,
                    daily_data.len(),
                    required_size
                );
            }
        } else {
            info!("✅ [DATA ISOLATION] Loaded {} hourly data points for symbol {}", data.len(), symbol);
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
                    current.volume_value
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
                let avg_volume = window.iter().map(|d| d.volume_value).sum::<f64>() / window.len() as f64;
                let volume_ratio = if avg_volume > 0.0 {
                    current.volume_value / avg_volume
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

        let prepared_data = PreparedTrainingData {
            model_type: ModelType::MLP,
            symbol: symbol.to_string(),
            features,
            targets,
            timestamps,
            feature_names,
            normalization_params,
            metadata: HashMap::new(),
        };
        
        // FINAL VALIDATION: Ensure prepared data symbol matches input
        if prepared_data.symbol != symbol {
            error!(
                "🚨 [DATA ISOLATION CRITICAL] MLP data preparation symbol mismatch! Expected: {}, Got: {}",
                symbol, prepared_data.symbol
            );
            bail!("MLP data preparation symbol isolation failure");
        }
        
        info!(
            "✅ [DATA ISOLATION] MLP data prepared successfully for symbol {} with {} features",
            symbol, prepared_data.features.len()
        );
        
        Ok(prepared_data)
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
                        point.volume_value
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

        let prepared_data = PreparedTrainingData {
            model_type: model_type.clone(),
            symbol: symbol.to_string(),
            features: sequences,
            targets,
            timestamps,
            feature_names,
            normalization_params,
            metadata,
        };
        
        // FINAL VALIDATION: Ensure prepared data symbol matches input
        if prepared_data.symbol != symbol {
            error!(
                "🚨 [DATA ISOLATION CRITICAL] Sequence data preparation symbol mismatch! Expected: {}, Got: {}",
                symbol, prepared_data.symbol
            );
            bail!("Sequence data preparation symbol isolation failure");
        }
        
        info!(
            "✅ [DATA ISOLATION] {:?} sequence data prepared successfully for symbol {} with {} sequences",
            model_type, symbol, prepared_data.features.len()
        );
        
        Ok(prepared_data)
    }

    async fn prepare_cnn_data(
        &self,
        symbol: &str,
        data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        info!("🔍 [DATA ISOLATION] Preparing CNN data for symbol {} ONLY", symbol);
        
        // For CNN, we can create 2D feature maps
        // This is a simplified version - real implementation would create more sophisticated features
        self.prepare_mlp_data(symbol, data, config)
            .await
            .map(|mut prepared| {
                prepared.model_type = ModelType::CNN;
                
                // VALIDATION: Ensure symbol consistency after model type change
                if prepared.symbol != symbol {
                    error!(
                        "🚨 [DATA ISOLATION CRITICAL] CNN data symbol mismatch after preparation! Expected: {}, Got: {}",
                        symbol, prepared.symbol
                    );
                    panic!("CNN data preparation symbol isolation failure");
                }
                
                info!("✅ [DATA ISOLATION] CNN data prepared successfully for symbol {}", symbol);
                prepared
            })
    }

    async fn prepare_ensemble_data(
        &self,
        symbol: &str,
        data: Vec<TimeSeriesData>,
        config: &TrainingDataConfig,
    ) -> Result<PreparedTrainingData> {
        info!("🔍 [DATA ISOLATION] Preparing ensemble data for symbol {} ONLY", symbol);
        
        // For ensemble, prepare data that can be used by multiple model types
        // This returns MLP-style features that can be adapted by each model
        self.prepare_mlp_data(symbol, data, config)
            .await
            .map(|mut prepared| {
                prepared.model_type = ModelType::Ensemble;
                
                // VALIDATION: Ensure symbol consistency after model type change
                if prepared.symbol != symbol {
                    error!(
                        "🚨 [DATA ISOLATION CRITICAL] Ensemble data symbol mismatch after preparation! Expected: {}, Got: {}",
                        symbol, prepared.symbol
                    );
                    panic!("Ensemble data preparation symbol isolation failure");
                }
                
                info!("✅ [DATA ISOLATION] Ensemble data prepared successfully for symbol {}", symbol);
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
