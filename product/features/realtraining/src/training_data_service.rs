//! Training Data Service
//! 
//! Bridges TimescaleDB historical market data to neural model training.
//! Provides efficient data access and transformation for various model types.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;

use crate::adapters::timescale::{TimescaleAdapter, TimescaleConfig};
use crate::data::TimeSeriesData;
use crate::neural::{FannModelConfig, ModelType};

/// Training data configuration
#[derive(Debug, Clone)]
pub struct TrainingDataConfig {
    /// Window size for time series data (number of candles)
    pub window_size: usize,
    /// Step size for sliding window (1 = no overlap)
    pub step_size: usize,
    /// Minimum samples required for training
    pub min_samples: usize,
    /// Maximum samples to load (for memory management)
    pub max_samples: Option<usize>,
    /// Feature engineering configuration
    pub feature_config: FeatureConfig,
    /// Data validation settings
    pub validation_config: ValidationConfig,
}

impl Default for TrainingDataConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            step_size: 1,
            min_samples: 1000,
            max_samples: Some(100_000),
            feature_config: FeatureConfig::default(),
            validation_config: ValidationConfig::default(),
        }
    }
}

/// Feature engineering configuration
#[derive(Debug, Clone)]
pub struct FeatureConfig {
    /// Include technical indicators
    pub use_indicators: bool,
    /// Include volume features
    pub use_volume: bool,
    /// Include price ratios (high/low, open/close)
    pub use_ratios: bool,
    /// Include time-based features (hour, day of week)
    pub use_temporal: bool,
    /// Normalization method
    pub normalization: NormalizationMethod,
}

impl Default for FeatureConfig {
    fn default() -> Self {
        Self {
            use_indicators: true,
            use_volume: true,
            use_ratios: true,
            use_temporal: true,
            normalization: NormalizationMethod::MinMax,
        }
    }
}

/// Data normalization methods
#[derive(Debug, Clone)]
pub enum NormalizationMethod {
    /// Min-max scaling to [0, 1]
    MinMax,
    /// Z-score normalization (mean=0, std=1)
    ZScore,
    /// Percentage change from previous
    PercentChange,
    /// Log returns
    LogReturns,
}

/// Data validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Check for missing data
    pub check_gaps: bool,
    /// Maximum allowed gap in minutes
    pub max_gap_minutes: i64,
    /// Remove outliers beyond N standard deviations
    pub outlier_threshold: Option<f64>,
    /// Minimum required data quality score
    pub min_quality_score: f64,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            check_gaps: true,
            max_gap_minutes: 60,
            outlier_threshold: Some(5.0),
            min_quality_score: 0.95,
        }
    }
}

/// Training data batch
#[derive(Debug, Clone)]
pub struct TrainingBatch {
    /// Input features for each sample
    pub features: Vec<Vec<f64>>,
    /// Target values for each sample
    pub targets: Vec<Vec<f64>>,
    /// Timestamps for each sample (for validation)
    pub timestamps: Vec<DateTime<Utc>>,
    /// Symbol being trained
    pub symbol: String,
    /// Batch metadata
    pub metadata: BatchMetadata,
}

/// Batch metadata for tracking
#[derive(Debug, Clone)]
pub struct BatchMetadata {
    /// Start time of data in batch
    pub start_time: DateTime<Utc>,
    /// End time of data in batch
    pub end_time: DateTime<Utc>,
    /// Number of samples
    pub sample_count: usize,
    /// Data quality metrics
    pub quality_score: f64,
    /// Feature statistics
    pub feature_stats: HashMap<String, FeatureStats>,
}

/// Feature statistics for monitoring
#[derive(Debug, Clone)]
pub struct FeatureStats {
    pub mean: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

/// Training Data Service
pub struct TrainingDataService {
    timescale: Arc<TimescaleAdapter>,
    config: TrainingDataConfig,
    feature_cache: HashMap<String, Vec<f64>>,
}

impl TrainingDataService {
    /// Create new training data service
    pub fn new(timescale: Arc<TimescaleAdapter>, config: TrainingDataConfig) -> Self {
        Self {
            timescale,
            config,
            feature_cache: HashMap::new(),
        }
    }

    /// Load training data for a specific time range
    pub async fn load_training_data(
        &mut self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        model_type: &ModelType,
    ) -> Result<TrainingBatch> {
        // Query raw market data from TimescaleDB
        let raw_data = self.query_market_data(symbol, start_time, end_time).await?;
        
        // Validate data quality
        self.validate_data(&raw_data)?;
        
        // Convert to time series format
        let time_series = self.convert_to_time_series(raw_data)?;
        
        // Apply feature engineering
        let engineered_data = self.apply_feature_engineering(&time_series, model_type)?;
        
        // Create sliding windows
        let windows = self.create_sliding_windows(&engineered_data)?;
        
        // Split into features and targets
        let batch = self.prepare_training_batch(windows, symbol)?;
        
        Ok(batch)
    }

    /// Load incremental update data (for online learning)
    pub async fn load_incremental_data(
        &mut self,
        symbol: &str,
        last_timestamp: DateTime<Utc>,
        model_type: &ModelType,
    ) -> Result<Option<TrainingBatch>> {
        let end_time = Utc::now();
        let start_time = last_timestamp - Duration::minutes(5); // Small overlap for continuity
        
        // Query recent data
        let raw_data = self.query_market_data(symbol, start_time, end_time).await?;
        
        if raw_data.len() < self.config.window_size {
            return Ok(None); // Not enough new data
        }
        
        // Process as normal
        let time_series = self.convert_to_time_series(raw_data)?;
        let engineered_data = self.apply_feature_engineering(&time_series, model_type)?;
        let windows = self.create_sliding_windows(&engineered_data)?;
        let batch = self.prepare_training_batch(windows, symbol)?;
        
        Ok(Some(batch))
    }

    /// Query market data from TimescaleDB
    async fn query_market_data(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Vec<crate::adapters::MarketData>> {
        let start_ts = start_time.timestamp();
        let end_ts = end_time.timestamp();
        
        self.timescale.query_market_data(symbol, start_ts, end_ts).await
    }

    /// Validate data quality
    fn validate_data(&self, data: &[crate::adapters::MarketData]) -> Result<()> {
        if data.is_empty() {
            anyhow::bail!("No data found for training");
        }
        
        if data.len() < self.config.min_samples {
            anyhow::bail!(
                "Insufficient data: {} samples found, {} required",
                data.len(),
                self.config.min_samples
            );
        }
        
        // Check for gaps if enabled
        if self.config.validation_config.check_gaps {
            for i in 1..data.len() {
                let gap = data[i].timestamp - data[i - 1].timestamp;
                if gap > self.config.validation_config.max_gap_minutes * 60 {
                    log::warn!(
                        "Data gap detected: {} minutes between {} and {}",
                        gap / 60,
                        data[i - 1].timestamp,
                        data[i].timestamp
                    );
                }
            }
        }
        
        Ok(())
    }

    /// Convert market data to time series format
    fn convert_to_time_series(
        &self,
        data: Vec<crate::adapters::MarketData>,
    ) -> Result<Vec<TimeSeriesData>> {
        data.into_iter()
            .map(|d| {
                let timestamp = DateTime::<Utc>::from_timestamp(d.timestamp, 0)
                    .ok_or_else(|| anyhow::anyhow!("Invalid timestamp: {}", d.timestamp))?;
                
                Ok(TimeSeriesData {
                    symbol: d.symbol,
                    timestamp,
                    open: d.open,
                    high: d.high,
                    low: d.low,
                    close: d.close,
                    volume: d.volume,
                    indicators: HashMap::new(),
                    source: Some("timescaledb".to_string()),
                    entity: None,
                    value: Some(d.close),
                    metadata: None,
                })
            })
            .collect()
    }

    /// Apply feature engineering based on model type
    fn apply_feature_engineering(
        &mut self,
        data: &[TimeSeriesData],
        model_type: &ModelType,
    ) -> Result<Vec<Vec<f64>>> {
        // This will be implemented in feature_engineering.rs
        // For now, return basic OHLCV features
        let features: Vec<Vec<f64>> = data
            .iter()
            .map(|d| vec![d.open, d.high, d.low, d.close, d.volume])
            .collect();
        
        Ok(features)
    }

    /// Create sliding windows for time series
    fn create_sliding_windows(&self, data: &[Vec<f64>]) -> Result<Vec<Vec<Vec<f64>>>> {
        if data.len() < self.config.window_size {
            anyhow::bail!("Insufficient data for windowing");
        }
        
        let mut windows = Vec::new();
        let mut i = 0;
        
        while i + self.config.window_size <= data.len() {
            let window: Vec<Vec<f64>> = data[i..i + self.config.window_size].to_vec();
            windows.push(window);
            i += self.config.step_size;
        }
        
        Ok(windows)
    }

    /// Prepare final training batch
    fn prepare_training_batch(
        &self,
        windows: Vec<Vec<Vec<f64>>>,
        symbol: &str,
    ) -> Result<TrainingBatch> {
        let mut features = Vec::new();
        let mut targets = Vec::new();
        let timestamps = Vec::new(); // TODO: Track timestamps
        
        for window in windows {
            if window.len() < 2 {
                continue;
            }
            
            // Flatten all but last row as features
            let feature_vec: Vec<f64> = window[..window.len() - 1]
                .iter()
                .flatten()
                .cloned()
                .collect();
            
            // Last row close price as target (simple example)
            let target_vec = vec![window.last().unwrap()[3]]; // Close price
            
            features.push(feature_vec);
            targets.push(target_vec);
        }
        
        let metadata = BatchMetadata {
            start_time: Utc::now(), // TODO: Use actual times
            end_time: Utc::now(),
            sample_count: features.len(),
            quality_score: 1.0, // TODO: Calculate actual quality
            feature_stats: HashMap::new(), // TODO: Calculate stats
        };
        
        Ok(TrainingBatch {
            features,
            targets,
            timestamps,
            symbol: symbol.to_string(),
            metadata,
        })
    }

    /// Get feature statistics for monitoring
    pub fn get_feature_statistics(&self, batch: &TrainingBatch) -> HashMap<String, FeatureStats> {
        let mut stats = HashMap::new();
        
        // Calculate statistics for each feature dimension
        if !batch.features.is_empty() {
            let feature_dim = batch.features[0].len();
            
            for i in 0..feature_dim {
                let values: Vec<f64> = batch.features.iter().map(|f| f[i]).collect();
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                let std_dev = variance.sqrt();
                let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                
                stats.insert(
                    format!("feature_{}", i),
                    FeatureStats { mean, std_dev, min, max },
                );
            }
        }
        
        stats
    }
}

/// Training data iterator for batch processing
pub struct TrainingDataIterator {
    service: TrainingDataService,
    symbol: String,
    current_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    batch_duration: Duration,
    model_type: ModelType,
}

impl TrainingDataIterator {
    pub fn new(
        service: TrainingDataService,
        symbol: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        batch_duration: Duration,
        model_type: ModelType,
    ) -> Self {
        Self {
            service,
            symbol,
            current_time: start_time,
            end_time,
            batch_duration,
            model_type,
        }
    }

    /// Get next batch of training data
    pub async fn next_batch(&mut self) -> Result<Option<TrainingBatch>> {
        if self.current_time >= self.end_time {
            return Ok(None);
        }
        
        let batch_end = (self.current_time + self.batch_duration).min(self.end_time);
        let batch = self.service
            .load_training_data(&self.symbol, self.current_time, batch_end, &self.model_type)
            .await?;
        
        self.current_time = batch_end;
        Ok(Some(batch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_training_config_defaults() {
        let config = TrainingDataConfig::default();
        assert_eq!(config.window_size, 50);
        assert_eq!(config.step_size, 1);
        assert_eq!(config.min_samples, 1000);
    }

    #[test]
    fn test_feature_config_defaults() {
        let config = FeatureConfig::default();
        assert!(config.use_indicators);
        assert!(config.use_volume);
        assert!(config.use_ratios);
        assert!(config.use_temporal);
    }
}