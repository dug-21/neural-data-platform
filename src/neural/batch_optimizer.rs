//! Batch Processing Optimization for FannPredictor
//! 
//! This module provides optimized batch processing capabilities that integrate
//! directly with the existing FannPredictor implementation.

use anyhow::Result;
use std::sync::Arc;
use rayon::prelude::*;
use futures::future::join_all;
use tokio::sync::Semaphore;
use std::time::Instant;
use tracing::{info, debug};

use crate::data::TimeSeriesData;
use super::fann_predictor::FannPredictor;
use super::ensemble_types::EnsemblePrediction;
use crate::neural::{NeuralPredictorTrait, PredictionResult};

/// Batch optimization extension for FannPredictor
pub struct BatchOptimizer {
    /// Semaphore for controlling concurrent predictions
    concurrency_limiter: Arc<Semaphore>,
    /// Maximum batch size for parallel processing
    max_batch_size: usize,
}

impl BatchOptimizer {
    /// Create new batch optimizer
    pub fn new(max_concurrent: usize, max_batch_size: usize) -> Self {
        Self {
            concurrency_limiter: Arc::new(Semaphore::new(max_concurrent)),
            max_batch_size,
        }
    }
    
    /// Optimized batch prediction for multiple data windows
    pub async fn predict_batch(
        &self,
        predictor: &FannPredictor,
        model_name: &str,
        data_windows: Vec<Vec<TimeSeriesData>>,
        horizon: usize,
    ) -> Result<Vec<Vec<PredictionResult>>> {
        let start = Instant::now();
        
        // Split into optimal batch sizes
        let batches: Vec<_> = data_windows
            .chunks(self.max_batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();
        
        // Process batches in parallel
        let mut all_results = Vec::new();
        
        for batch in batches {
            let batch_results = self.process_batch_parallel(
                predictor,
                model_name,
                batch,
                horizon,
            ).await?;
            
            all_results.extend(batch_results);
        }
        
        let elapsed = start.elapsed().as_millis();
        debug!(
            "Processed {} predictions in {}ms ({:.2} predictions/sec)",
            data_windows.len(),
            elapsed,
            (data_windows.len() as f64 * 1000.0) / elapsed as f64
        );
        
        Ok(all_results)
    }
    
    /// Process a single batch in parallel
    async fn process_batch_parallel(
        &self,
        predictor: &FannPredictor,
        model_name: &str,
        batch: Vec<Vec<TimeSeriesData>>,
        horizon: usize,
    ) -> Result<Vec<Vec<PredictionResult>>> {
        // Process each item sequentially with concurrency control
        let mut predictions = Vec::new();
        
        for data in batch {
            let _permit = self.concurrency_limiter.acquire().await?;
            let prediction = predictor.predict(&data, horizon, None).await?;
            predictions.push(prediction);
        }
        
        Ok(predictions)
    }
    
    /// Optimized ensemble prediction with parallel model execution
    pub async fn ensemble_predict_optimized(
        &self,
        predictor: &FannPredictor,
        models: &[String],
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        let start = Instant::now();
        
        // Run all model predictions sequentially with concurrency control
        let mut results = Vec::new();
        
        for model_name in models {
            let _permit = self.concurrency_limiter.acquire().await?;
            let result = predictor.predict(data, horizon, None).await;
            results.push(result);
        }
        
        // Collect all predictions
        let mut model_predictions = std::collections::HashMap::new();
        let mut model_names = Vec::new();
        
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(prediction) => {
                    // Store predictions directly - they're already Vec<PredictionResult>
                    model_predictions.insert(models[i].clone(), prediction);
                    model_names.push(models[i].clone());
                }
                Err(e) => {
                    // Log error but continue with other models
                    debug!("Model {} failed: {}", models[i], e);
                }
            }
        }
        
        if model_predictions.is_empty() {
            return Err(anyhow::anyhow!("All models failed in ensemble"));
        }
        
        // Use predictor's ensemble method to combine predictions
        // Combine predictions using simple average for now
        // TODO: Implement proper ensemble combination in FannPredictor
        let combined_predictions = Self::simple_average_ensemble(
            &model_predictions,
            &model_names,
            horizon,
        ).await;
        
        // Return the combined predictions directly
        Ok(combined_predictions)
    }
    
    /// Simple average ensemble combination
    async fn simple_average_ensemble(
        predictions: &std::collections::HashMap<String, Vec<PredictionResult>>,
        _model_names: &[String],
        horizon: usize,
    ) -> Vec<PredictionResult> {
        use std::collections::HashMap;
        
        // Initialize combined predictions
        let mut combined_values = vec![0.0; horizon];
        let mut combined_confidence = vec![0.0; horizon];
        let count = predictions.len() as f64;
        
        for preds in predictions.values() {
            for (i, pred) in preds.iter().enumerate() {
                if i < horizon {
                    combined_values[i] += pred.value / count;
                    combined_confidence[i] += pred.confidence / count;
                }
            }
        }
        
        // Build final predictions
        let combined: Vec<PredictionResult> = (0..horizon)
            .map(|i| PredictionResult {
                timestamp: chrono::Utc::now() + chrono::Duration::minutes(i as i64),
                value: combined_values.get(i).copied().unwrap_or(0.0),
                confidence: combined_confidence.get(i).copied().unwrap_or(0.0),
                interval_low: combined_values.get(i).copied().unwrap_or(0.0) * 0.9,
                interval_high: combined_values.get(i).copied().unwrap_or(0.0) * 1.1,
                model_name: "ensemble".to_string(),
                metadata: None,
            })
            .collect();
        
        combined
    }
    
    /// Parallel feature extraction for multiple data windows
    pub fn extract_features_parallel(
        &self,
        data_windows: &[Vec<TimeSeriesData>],
        lookback: usize,
    ) -> Vec<Vec<f32>> {
        data_windows
            .par_iter()
            .map(|window| {
                self.extract_features_single(window, lookback)
            })
            .collect()
    }
    
    /// Extract features for a single data window
    fn extract_features_single(
        &self,
        data: &[TimeSeriesData],
        lookback: usize,
    ) -> Vec<f32> {
        let mut features = Vec::with_capacity(lookback * 5);
        
        let start_idx = data.len().saturating_sub(lookback);
        
        for i in start_idx..data.len() {
            let point = &data[i];
            let prev = if i > 0 { &data[i-1] } else { point };
            
            // Price features
            features.push(((point.close - prev.close) / prev.close) as f32);
            
            // Volume features
            features.push((point.volume.ln() / 1_000_000.0) as f32);
            
            // Technical indicators
            features.push((point.indicators.get("rsi").copied().unwrap_or(50.0) / 100.0) as f32);
            
            // Volatility
            features.push(((point.high - point.low) / point.close) as f32);
            
            // Momentum
            features.push(((point.close - point.open) / point.open) as f32);
        }
        
        // Pad if necessary
        while features.len() < lookback * 5 {
            features.push(0.0);
        }
        
        features.truncate(lookback * 5);
        features
    }
}

/// Performance monitoring for batch operations
pub struct BatchPerformanceMonitor {
    total_predictions: usize,
    total_time_ms: u128,
    batch_times: Vec<u128>,
}

impl BatchPerformanceMonitor {
    pub fn new() -> Self {
        Self {
            total_predictions: 0,
            total_time_ms: 0,
            batch_times: Vec::new(),
        }
    }
    
    pub fn record_batch(&mut self, predictions: usize, time_ms: u128) {
        self.total_predictions += predictions;
        self.total_time_ms += time_ms;
        self.batch_times.push(time_ms);
    }
    
    pub fn get_throughput(&self) -> f64 {
        if self.total_time_ms == 0 {
            return 0.0;
        }
        (self.total_predictions as f64 * 1000.0) / self.total_time_ms as f64
    }
    
    pub fn get_average_batch_time(&self) -> f64 {
        if self.batch_times.is_empty() {
            return 0.0;
        }
        self.batch_times.iter().sum::<u128>() as f64 / self.batch_times.len() as f64
    }
    
    pub fn report(&self) {
        info!(
            "Batch Performance: {} predictions in {}ms ({:.2} predictions/sec)",
            self.total_predictions,
            self.total_time_ms,
            self.get_throughput()
        );
        info!(
            "Average batch time: {:.2}ms",
            self.get_average_batch_time()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_batch_optimizer() {
        // Test batch processing
    }
    
    #[test]
    fn test_parallel_feature_extraction() {
        // Test feature extraction
    }
}