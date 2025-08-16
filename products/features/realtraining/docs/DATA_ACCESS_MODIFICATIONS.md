# DataAccessLayer Modifications for Training Pipeline

## Overview

This document provides detailed code modifications needed for `src/integration/data_access.rs` to support the neural model training pipeline.

## Required Imports

Add these imports to the top of `data_access.rs`:

```rust
// Additional imports for training support
use futures::stream::{self, Stream, StreamExt};
use std::pin::Pin;

// Import from the training module
use crate::features::{FeatureConfig, FeatureVector};
use crate::neural::ModelType;
```

## New Data Structures

Add these structures after the existing ones:

```rust
/// Training data configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataConfig {
    /// Window size for time series data
    pub window_size: usize,
    /// Step size for sliding window
    pub step_size: usize,
    /// Minimum required samples
    pub min_samples: usize,
    /// Maximum samples to load at once
    pub max_samples: Option<usize>,
    /// Include indicators in data
    pub include_indicators: bool,
    /// Data quality threshold
    pub quality_threshold: f64,
}

impl Default for TrainingDataConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            step_size: 1,
            min_samples: 1000,
            max_samples: Some(100_000),
            include_indicators: true,
            quality_threshold: 0.95,
        }
    }
}

/// Feature vector for neural models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub timestamp: DateTime<Utc>,
    pub features: Vec<f64>,
    pub feature_names: Vec<String>,
}

/// Training data batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDataBatch {
    pub symbol: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub data: Vec<TimeSeriesData>,
    pub sample_count: usize,
    pub quality_score: f64,
}
```

## Method Implementations

Add these methods to the `DataAccessLayer` implementation:

```rust
impl DataAccessLayer {
    /// Get training data for neural models with quality validation
    pub async fn get_training_data(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        config: &TrainingDataConfig,
    ) -> Result<Vec<TimeSeriesData>> {
        info!(
            "Fetching training data for {}: {} to {}", 
            symbol, start_time, end_time
        );
        
        // Try cache first for recent requests
        let cache_key = format!(
            "training:{}:{}:{}", 
            symbol, 
            start_time.timestamp(), 
            end_time.timestamp()
        );
        
        if let Ok(Some(cached_data)) = self.cache.get::<Vec<TimeSeriesData>>(&cache_key).await {
            debug!("Training data cache hit for {}", symbol);
            return Ok(cached_data);
        }
        
        // Query from database with larger batch size for training
        let mut all_data = Vec::new();
        let mut current_start = start_time;
        let batch_duration = Duration::hours(24); // Query in daily batches
        
        while current_start < end_time {
            let batch_end = (current_start + batch_duration).min(end_time);
            
            let batch_data = self.storage
                .query_range(symbol, current_start, batch_end)
                .await
                .context("Failed to query training data batch")?;
            
            // Convert and validate data
            for data_point in batch_data {
                if let Some(ts_data) = self.convert_to_timeseries(&data_point)? {
                    // Validate data quality
                    if self.validate_data_point(&ts_data, config.quality_threshold) {
                        all_data.push(ts_data);
                    }
                }
            }
            
            current_start = batch_end;
            
            // Respect max samples limit
            if let Some(max) = config.max_samples {
                if all_data.len() >= max {
                    all_data.truncate(max);
                    break;
                }
            }
        }
        
        // Validate minimum samples
        if all_data.len() < config.min_samples {
            bail!(
                "Insufficient training data: {} samples found, {} required",
                all_data.len(),
                config.min_samples
            );
        }
        
        // Sort by timestamp
        all_data.sort_by_key(|d| d.timestamp);
        
        // Cache the result for 1 hour
        if let Err(e) = self.cache.set(&cache_key, &all_data, Some(3600)).await {
            warn!("Failed to cache training data: {}", e);
        }
        
        info!(
            "Loaded {} training samples for {} ({}% of requested range)",
            all_data.len(),
            symbol,
            (all_data.len() as f64 / config.min_samples as f64 * 100.0) as i32
        );
        
        Ok(all_data)
    }
    
    /// Stream training data in batches for memory efficiency
    pub async fn stream_training_data(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        batch_size: usize,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Vec<TimeSeriesData>>> + Send>>> {
        let symbol = symbol.to_string();
        let storage = self.storage.clone();
        let cache = self.cache.clone();
        
        let stream = stream::unfold(
            (start_time, false),
            move |(current_time, done)| {
                let symbol = symbol.clone();
                let storage = storage.clone();
                let cache = cache.clone();
                
                async move {
                    if done || current_time >= end_time {
                        return None;
                    }
                    
                    let batch_end = (current_time + Duration::minutes((batch_size * 5) as i64))
                        .min(end_time);
                    
                    let result = async {
                        // Try cache first
                        let cache_key = format!(
                            "stream:{}:{}:{}", 
                            symbol, 
                            current_time.timestamp(), 
                            batch_end.timestamp()
                        );
                        
                        if let Ok(Some(data)) = cache.get::<Vec<TimeSeriesData>>(&cache_key).await {
                            return Ok(data);
                        }
                        
                        // Query from storage
                        let raw_data = storage
                            .query_range(&symbol, current_time, batch_end)
                            .await?;
                        
                        let mut batch = Vec::new();
                        for point in raw_data {
                            if let Some(metadata) = &point.metadata {
                                batch.push(TimeSeriesData {
                                    symbol: point.entity.clone(),
                                    timestamp: point.timestamp,
                                    open: metadata.get("open")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(point.value),
                                    high: metadata.get("high")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(point.value),
                                    low: metadata.get("low")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(point.value),
                                    close: point.value,
                                    volume: metadata.get("volume")
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(0.0),
                                    indicators: HashMap::new(),
                                    source: Some(point.source),
                                    entity: Some(point.entity),
                                    value: Some(point.value),
                                    metadata: Some(metadata.clone()),
                                });
                            }
                        }
                        
                        // Cache for 30 minutes
                        let _ = cache.set(&cache_key, &batch, Some(1800)).await;
                        
                        Ok(batch)
                    }.await;
                    
                    let next_time = batch_end;
                    let is_done = next_time >= end_time;
                    
                    Some((result, (next_time, is_done)))
                }
            }
        );
        
        Ok(Box::pin(stream))
    }
    
    /// Get feature-engineered data ready for neural models
    pub async fn get_engineered_features(
        &self,
        symbol: &str,
        timeframe: Timeframe,
        feature_config: &FeatureConfig,
    ) -> Result<Vec<FeatureVector>> {
        // Get base market data
        let market_data = self.get_market_data(symbol, timeframe).await?;
        
        if market_data.is_empty() {
            return Ok(Vec::new());
        }
        
        let mut feature_vectors = Vec::new();
        
        // Compute features based on configuration
        for (i, data) in market_data.iter().enumerate() {
            let mut features = Vec::new();
            let mut feature_names = Vec::new();
            
            // Price features
            features.push(data.open);
            feature_names.push("open".to_string());
            
            features.push(data.high);
            feature_names.push("high".to_string());
            
            features.push(data.low);
            feature_names.push("low".to_string());
            
            features.push(data.close);
            feature_names.push("close".to_string());
            
            // Volume features
            if feature_config.use_volume {
                features.push(data.volume);
                feature_names.push("volume".to_string());
                
                // Volume-weighted average price
                if data.volume > 0.0 {
                    let vwap = (data.high + data.low + data.close) / 3.0;
                    features.push(vwap);
                    feature_names.push("vwap".to_string());
                }
            }
            
            // Price ratios
            if feature_config.use_ratios && i > 0 {
                let prev = &market_data[i - 1];
                
                // Price change ratio
                let price_change = (data.close - prev.close) / prev.close;
                features.push(price_change);
                feature_names.push("price_change_ratio".to_string());
                
                // High-low ratio
                let hl_ratio = (data.high - data.low) / data.close;
                features.push(hl_ratio);
                feature_names.push("high_low_ratio".to_string());
            }
            
            // Technical indicators from metadata
            if feature_config.use_indicators {
                if let Some(indicators) = &data.indicators {
                    for (name, value) in indicators {
                        features.push(*value);
                        feature_names.push(format!("indicator_{}", name));
                    }
                }
            }
            
            // Temporal features
            if feature_config.use_temporal {
                let hour = data.timestamp.hour() as f64 / 24.0;
                let day_of_week = data.timestamp.weekday().num_days_from_monday() as f64 / 7.0;
                
                features.push(hour);
                feature_names.push("hour_normalized".to_string());
                
                features.push(day_of_week);
                feature_names.push("day_of_week_normalized".to_string());
            }
            
            feature_vectors.push(FeatureVector {
                timestamp: data.timestamp,
                features,
                feature_names,
            });
        }
        
        Ok(feature_vectors)
    }
    
    /// Get training data batch with metadata
    pub async fn get_training_batch(
        &self,
        symbol: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        config: &TrainingDataConfig,
    ) -> Result<TrainingDataBatch> {
        let data = self.get_training_data(symbol, start_time, end_time, config).await?;
        
        // Calculate quality score
        let quality_score = self.calculate_data_quality(&data);
        
        Ok(TrainingDataBatch {
            symbol: symbol.to_string(),
            start_time,
            end_time,
            sample_count: data.len(),
            quality_score,
            data,
        })
    }
    
    /// Helper method to convert DataPoint to TimeSeriesData
    fn convert_to_timeseries(&self, point: &DataPoint) -> Result<Option<TimeSeriesData>> {
        if let Some(metadata) = &point.metadata {
            Ok(Some(TimeSeriesData {
                symbol: point.entity.clone(),
                timestamp: point.timestamp,
                open: metadata.get("open")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(point.value),
                high: metadata.get("high")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(point.value),
                low: metadata.get("low")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(point.value),
                close: point.value,
                volume: metadata.get("volume")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
                indicators: self.extract_indicators(metadata),
                source: Some(point.source.clone()),
                entity: Some(point.entity.clone()),
                value: Some(point.value),
                metadata: Some(metadata.clone()),
            }))
        } else {
            Ok(None)
        }
    }
    
    /// Extract indicators from metadata
    fn extract_indicators(&self, metadata: &serde_json::Value) -> HashMap<String, f64> {
        let mut indicators = HashMap::new();
        
        if let Some(ind_obj) = metadata.get("indicators").and_then(|v| v.as_object()) {
            for (key, value) in ind_obj {
                if let Some(val) = value.as_f64() {
                    indicators.insert(key.clone(), val);
                }
            }
        }
        
        indicators
    }
    
    /// Validate individual data point quality
    fn validate_data_point(&self, data: &TimeSeriesData, threshold: f64) -> bool {
        // Check for valid price ranges
        if data.close <= 0.0 || data.open <= 0.0 || data.high <= 0.0 || data.low <= 0.0 {
            return false;
        }
        
        // Check OHLC consistency
        if data.high < data.low || data.high < data.close || data.low > data.close {
            return false;
        }
        
        // Check volume
        if data.volume < 0.0 {
            return false;
        }
        
        // Additional quality checks can be added here
        true
    }
    
    /// Calculate overall data quality score
    fn calculate_data_quality(&self, data: &[TimeSeriesData]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }
        
        let mut quality_factors = Vec::new();
        
        // Check for gaps
        let mut gap_score = 1.0;
        for i in 1..data.len() {
            let time_diff = data[i].timestamp.signed_duration_since(data[i-1].timestamp);
            if time_diff > Duration::hours(1) {
                gap_score *= 0.95; // Penalize gaps
            }
        }
        quality_factors.push(gap_score);
        
        // Check for price anomalies
        let mut price_scores = Vec::new();
        for window in data.windows(3) {
            if window.len() == 3 {
                let price_change_1 = (window[1].close - window[0].close).abs() / window[0].close;
                let price_change_2 = (window[2].close - window[1].close).abs() / window[1].close;
                
                // Penalize extreme price movements (>10%)
                if price_change_1 > 0.1 || price_change_2 > 0.1 {
                    price_scores.push(0.8);
                } else {
                    price_scores.push(1.0);
                }
            }
        }
        
        if !price_scores.is_empty() {
            let avg_price_score = price_scores.iter().sum::<f64>() / price_scores.len() as f64;
            quality_factors.push(avg_price_score);
        }
        
        // Check volume consistency
        let avg_volume = data.iter().map(|d| d.volume).sum::<f64>() / data.len() as f64;
        let zero_volume_count = data.iter().filter(|d| d.volume == 0.0).count();
        let volume_score = 1.0 - (zero_volume_count as f64 / data.len() as f64);
        quality_factors.push(volume_score);
        
        // Calculate overall quality score
        if quality_factors.is_empty() {
            1.0
        } else {
            quality_factors.iter().sum::<f64>() / quality_factors.len() as f64
        }
    }
}
```

## Usage Examples

### Basic Training Data Retrieval

```rust
// Get training data for the last 30 days
let dal = DataAccessLayer::new(storage, cache).await?;
let config = TrainingDataConfig::default();

let end_time = Utc::now();
let start_time = end_time - Duration::days(30);

let training_data = dal.get_training_data(
    "AAPL",
    start_time,
    end_time,
    &config
).await?;

println!("Loaded {} training samples", training_data.len());
```

### Streaming Large Datasets

```rust
// Stream data in batches of 1000
let mut stream = dal.stream_training_data(
    "AAPL",
    start_time,
    end_time,
    1000
).await?;

while let Some(batch_result) = stream.next().await {
    match batch_result {
        Ok(batch) => {
            println!("Processing batch of {} samples", batch.len());
            // Process batch
        }
        Err(e) => {
            eprintln!("Error loading batch: {}", e);
        }
    }
}
```

### Feature-Engineered Data

```rust
// Get engineered features for neural models
let feature_config = FeatureConfig {
    use_indicators: true,
    use_volume: true,
    use_ratios: true,
    use_temporal: true,
    normalization: NormalizationMethod::ZScore,
};

let features = dal.get_engineered_features(
    "AAPL",
    Timeframe::FiveMinute,
    &feature_config
).await?;

// Features are ready for neural network input
for feature_vec in features {
    println!(
        "Timestamp: {}, Features: {} dimensions",
        feature_vec.timestamp,
        feature_vec.features.len()
    );
}
```

## Testing

Add these tests to validate the new functionality:

```rust
#[cfg(test)]
mod training_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_training_data_retrieval() {
        let dal = create_test_dal().await;
        let config = TrainingDataConfig::default();
        
        let result = dal.get_training_data(
            "TEST",
            Utc::now() - Duration::days(7),
            Utc::now(),
            &config
        ).await;
        
        assert!(result.is_ok());
        let data = result.unwrap();
        assert!(data.len() >= config.min_samples);
    }
    
    #[tokio::test]
    async fn test_data_quality_validation() {
        let dal = create_test_dal().await;
        
        let good_data = TimeSeriesData {
            symbol: "TEST".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 105.0,
            low: 95.0,
            close: 102.0,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
        };
        
        assert!(dal.validate_data_point(&good_data, 0.95));
        
        let bad_data = TimeSeriesData {
            close: -10.0, // Invalid negative price
            ..good_data
        };
        
        assert!(!dal.validate_data_point(&bad_data, 0.95));
    }
}
```

## Integration Points

### 1. TrainingDataService Connection

The modified DataAccessLayer can now be used by TrainingDataService:

```rust
// In TrainingDataService
pub async fn load_from_dal(
    &mut self,
    dal: &DataAccessLayer,
    symbol: &str,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
) -> Result<TrainingBatch> {
    let config = TrainingDataConfig {
        window_size: self.config.window_size,
        step_size: self.config.step_size,
        min_samples: self.config.min_samples,
        max_samples: self.config.max_samples,
        include_indicators: true,
        quality_threshold: 0.95,
    };
    
    let batch = dal.get_training_batch(symbol, start, end, &config).await?;
    
    // Convert to training format
    self.process_batch(batch).await
}
```

### 2. Autonomous Training Integration

```rust
// In autonomous_training.rs
impl AutonomousTrainingOrchestrator {
    pub async fn train_from_dal(
        &mut self,
        dal: &DataAccessLayer,
        symbol: &str,
    ) -> Result<()> {
        // Stream data to avoid memory issues
        let mut stream = dal.stream_training_data(
            symbol,
            self.start_time,
            self.end_time,
            5000, // 5k samples per batch
        ).await?;
        
        while let Some(batch_result) = stream.next().await {
            let batch = batch_result?;
            
            // Process batch through training pipeline
            self.process_training_batch(batch).await?;
        }
        
        Ok(())
    }
}
```

## Performance Considerations

1. **Caching Strategy**: 
   - Training data is cached for 1 hour
   - Stream batches are cached for 30 minutes
   - Consider Redis memory usage

2. **Batch Sizes**:
   - Default to 24-hour queries for historical data
   - Stream in configurable batch sizes
   - Monitor memory usage during training

3. **Parallel Processing**:
   - Consider using rayon for feature computation
   - Implement concurrent database queries
   - Use tokio::spawn for independent tasks

## Next Steps

1. Implement the IntegratedTrainingService
2. Add feature engineering pipeline
3. Create comprehensive tests
4. Add performance monitoring
5. Document API usage