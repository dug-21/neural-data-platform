//! Enhanced Neural Predictor with Phase 6 Capabilities
//!
//! This module provides advanced neural prediction capabilities including:
//! - Confidence scoring with ensemble agreement
//! - Adaptive retraining based on performance metrics
//! - Model ensemble coordination
//! - Performance tracking with time-based decay

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::PredictionResult;
use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;
use crate::neural::fann_predictor::FannPredictor;
use crate::neural::NeuralPredictorTrait;

/// Enhanced prediction result with confidence breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedPredictionResult {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub confidence: f64,
    pub confidence_breakdown: ConfidenceBreakdown,
    pub models_agree: bool,
    pub model_agreement_score: f64,
    pub interval_low: f64,
    pub interval_high: f64,
    pub ensemble_size: usize,
    pub market_regime: String,
    pub volatility_adjustment: f64,
}

/// Detailed confidence score breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceBreakdown {
    /// Base model confidence (0.0 to 1.0)
    pub base_confidence: f64,
    /// Ensemble agreement bonus (0.0 to 0.3)
    pub ensemble_agreement: f64,
    /// Historical accuracy bonus (-0.2 to 0.2)
    pub historical_accuracy: f64,
    /// Market regime adjustment (-0.1 to 0.1)
    pub market_regime_adjustment: f64,
    /// Data quality factor (0.8 to 1.2)
    pub data_quality_factor: f64,
    /// Volatility penalty (-0.15 to 0.0)
    pub volatility_penalty: f64,
    /// Temporal distance penalty (-0.1 to 0.0 per step)
    pub temporal_distance_penalty: f64,
    /// Final combined confidence score (0.0 to 1.0)
    pub combined_confidence: f64,
}

/// Performance tracking metrics for retraining decisions
#[derive(Debug)]
pub struct PerformanceTracker {
    /// Recent prediction accuracy (exponentially weighted)
    recent_accuracy: f64,
    /// Total predictions made
    total_predictions: AtomicUsize,
    /// Successful predictions (within threshold)
    successful_predictions: AtomicUsize,
    /// Last training timestamp
    last_training_time: DateTime<Utc>,
    /// Training samples since last training
    new_samples_count: AtomicUsize,
    /// Accuracy threshold for retraining trigger
    accuracy_threshold: f64,
    /// Hours threshold for time-based retraining
    hours_threshold: i64,
    /// Sample count threshold for data-based retraining
    sample_threshold: usize,
    /// Recent prediction history for accuracy calculation
    prediction_history: Arc<RwLock<VecDeque<(f64, f64, DateTime<Utc>)>>>, // (actual, predicted, timestamp)
    /// Maximum history size
    max_history_size: usize,
    /// Performance decay factor for weighted accuracy
    decay_factor: f64,
}

/// Retraining decision metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrainingMetrics {
    pub should_retrain: bool,
    pub current_accuracy: f64,
    pub accuracy_threshold: f64,
    pub hours_since_training: i64,
    pub hours_threshold: i64,
    pub new_samples: usize,
    pub sample_threshold: usize,
    pub primary_trigger: String,
    pub urgency_score: f64,
    pub retrain_reasons: Vec<String>,
}

/// Main enhanced neural predictor with Phase 6 capabilities
pub struct EnhancedNeuralPredictor {
    /// Core FANN predictor for actual neural network operations
    fann_predictor: FannPredictor,
    /// Performance tracking for retraining decisions
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
    /// Configuration
    config: NeuralConfig,
    /// Model ensemble weights (dynamically adjusted)
    ensemble_weights: Arc<RwLock<HashMap<String, f64>>>,
    /// Recent market volatility for confidence adjustments
    recent_volatility: Arc<RwLock<f64>>,
    /// Market regime detection cache
    market_regime_cache: Arc<RwLock<(String, DateTime<Utc>)>>,
    /// Data quality metrics
    data_quality_tracker: Arc<RwLock<DataQualityTracker>>,
}

/// Data quality tracking for confidence adjustments
#[derive(Debug, Clone)]
struct DataQualityTracker {
    /// Recent data completeness (0.0 to 1.0)
    completeness_score: f64,
    /// Data freshness score (0.0 to 1.0)
    freshness_score: f64,
    /// Outlier detection score (0.0 to 1.0, higher = fewer outliers)
    outlier_score: f64,
    /// Volume consistency score (0.0 to 1.0)
    volume_consistency: f64,
    /// Last quality update
    last_update: DateTime<Utc>,
}

impl EnhancedNeuralPredictor {
    /// Create a new enhanced neural predictor
    pub fn new(config: NeuralConfig) -> Result<Self> {
        let fann_predictor = FannPredictor::new(config.clone())?;

        let performance_tracker = PerformanceTracker {
            recent_accuracy: 0.5, // Start neutral
            total_predictions: AtomicUsize::new(0),
            successful_predictions: AtomicUsize::new(0),
            last_training_time: Utc::now(),
            new_samples_count: AtomicUsize::new(0),
            accuracy_threshold: config.accuracy_threshold.max(0.7), // Ensure minimum 70%
            hours_threshold: 24,                                    // Retrain after 24 hours
            sample_threshold: 10000,                                // Retrain after 10k new samples
            prediction_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            max_history_size: 1000,
            decay_factor: 0.95, // 5% decay per prediction
        };

        let data_quality_tracker = DataQualityTracker {
            completeness_score: 1.0,
            freshness_score: 1.0,
            outlier_score: 1.0,
            volume_consistency: 1.0,
            last_update: Utc::now(),
        };

        // Initialize ensemble weights equally
        let mut ensemble_weights = HashMap::new();
        for model in &config.models {
            ensemble_weights.insert(model.clone(), 1.0);
        }

        Ok(Self {
            fann_predictor,
            performance_tracker: Arc::new(RwLock::new(performance_tracker)),
            config,
            ensemble_weights: Arc::new(RwLock::new(ensemble_weights)),
            recent_volatility: Arc::new(RwLock::new(0.02)), // Default 2% volatility
            market_regime_cache: Arc::new(RwLock::new((
                "unknown".to_string(),
                Utc::now() - Duration::hours(1),
            ))),
            data_quality_tracker: Arc::new(RwLock::new(data_quality_tracker)),
        })
    }

    /// Enhanced prediction with confidence scoring
    pub async fn predict_with_confidence(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<EnhancedPredictionResult>> {
        // Update data quality metrics
        self.update_data_quality(data).await?;

        // Get ensemble predictions from all models
        let ensemble_predictions = self
            .fann_predictor
            .predict_ensemble(data, horizon, &self.config.models, None)
            .await
            .context("Failed to get ensemble predictions")?;

        if ensemble_predictions.is_empty() {
            return Err(anyhow::anyhow!("No predictions returned from ensemble"));
        }

        // Get current volatility and market regime
        let volatility = self.calculate_current_volatility(data).await?;
        let market_regime = self.detect_market_regime(data).await?;

        // Calculate ensemble agreement and enhanced confidence
        let mut enhanced_results = Vec::new();

        // Group predictions by timestamp for ensemble analysis
        let mut predictions_by_timestamp: HashMap<DateTime<Utc>, Vec<&PredictionResult>> =
            HashMap::new();
        for prediction in &ensemble_predictions {
            predictions_by_timestamp
                .entry(prediction.timestamp)
                .or_default()
                .push(prediction);
        }

        // Process each timestamp
        for i in 0..horizon {
            if let Some(timestamp_predictions) = predictions_by_timestamp.values().find(|preds| {
                preds.len() > i
                    && preds[0].timestamp
                        == ensemble_predictions
                            .get(i * self.config.models.len())
                            .map(|p| p.timestamp)
                            .unwrap_or(Utc::now())
            }) {
                let enhanced_result = self
                    .calculate_enhanced_prediction(
                        timestamp_predictions,
                        i,
                        volatility,
                        &market_regime,
                        horizon,
                    )
                    .await?;

                enhanced_results.push(enhanced_result);
            }
        }

        // If we couldn't process by timestamp, fall back to sequential processing
        if enhanced_results.is_empty() {
            for (i, chunk) in ensemble_predictions
                .chunks(self.config.models.len())
                .enumerate()
            {
                if i >= horizon {
                    break;
                }

                let chunk_refs: Vec<&PredictionResult> = chunk.iter().collect();
                let enhanced_result = self
                    .calculate_enhanced_prediction(
                        &chunk_refs,
                        i,
                        volatility,
                        &market_regime,
                        horizon,
                    )
                    .await?;

                enhanced_results.push(enhanced_result);
            }
        }

        Ok(enhanced_results)
    }

    /// Calculate enhanced prediction with detailed confidence breakdown
    async fn calculate_enhanced_prediction(
        &self,
        predictions: &[&PredictionResult],
        step: usize,
        volatility: f64,
        market_regime: &str,
        horizon: usize,
    ) -> Result<EnhancedPredictionResult> {
        if predictions.is_empty() {
            return Err(anyhow::anyhow!("No predictions provided"));
        }

        // Calculate weighted ensemble prediction
        let ensemble_weights = self.ensemble_weights.read().await;
        let mut weighted_sum = 0.0;
        let mut total_weight = 0.0;

        for prediction in predictions {
            let weight = ensemble_weights
                .get(&prediction.model_name)
                .copied()
                .unwrap_or(1.0);
            weighted_sum += prediction.value * weight;
            total_weight += weight;
        }

        let ensemble_value = if total_weight > 0.0 {
            weighted_sum / total_weight
        } else {
            predictions.iter().map(|p| p.value).sum::<f64>() / predictions.len() as f64
        };

        // Calculate model agreement
        let values: Vec<f64> = predictions.iter().map(|p| p.value).collect();
        let mean_value = values.iter().sum::<f64>() / values.len() as f64;
        let variance =
            values.iter().map(|v| (v - mean_value).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        let agreement_score = if mean_value.abs() > 0.0 {
            1.0 - (std_dev / mean_value.abs()).min(1.0)
        } else {
            1.0 - std_dev.min(1.0)
        };

        let models_agree = std_dev < (mean_value.abs() * 0.1).max(0.01); // 10% or 1% absolute

        // Calculate base confidence (weighted average of individual confidences)
        let base_confidence = predictions
            .iter()
            .zip(
                predictions
                    .iter()
                    .map(|p| ensemble_weights.get(&p.model_name).copied().unwrap_or(1.0)),
            )
            .map(|(p, w)| p.confidence * w)
            .sum::<f64>()
            / total_weight.max(1.0);

        // Calculate confidence breakdown components
        let confidence_breakdown = self
            .calculate_confidence_breakdown(
                base_confidence,
                agreement_score,
                step,
                volatility,
                market_regime,
                horizon,
            )
            .await?;

        // Calculate final confidence
        let final_confidence = (confidence_breakdown.base_confidence
            + confidence_breakdown.ensemble_agreement
            + confidence_breakdown.historical_accuracy
            + confidence_breakdown.market_regime_adjustment
            + confidence_breakdown.volatility_penalty
            + confidence_breakdown.temporal_distance_penalty)
            * confidence_breakdown.data_quality_factor;

        let clamped_confidence = final_confidence.max(0.0).min(1.0);

        // Calculate prediction intervals with volatility adjustment
        let volatility_adjustment = 1.0 + volatility * 2.0; // Scale with volatility
        let interval_width = volatility * volatility_adjustment * (1.0 + 0.1 * step as f64);

        Ok(EnhancedPredictionResult {
            timestamp: predictions[0].timestamp,
            value: ensemble_value,
            confidence: clamped_confidence,
            confidence_breakdown,
            models_agree,
            model_agreement_score: agreement_score,
            interval_low: ensemble_value * (1.0 - interval_width),
            interval_high: ensemble_value * (1.0 + interval_width),
            ensemble_size: predictions.len(),
            market_regime: market_regime.to_string(),
            volatility_adjustment,
        })
    }

    /// Calculate detailed confidence breakdown
    async fn calculate_confidence_breakdown(
        &self,
        base_confidence: f64,
        agreement_score: f64,
        step: usize,
        volatility: f64,
        market_regime: &str,
        horizon: usize,
    ) -> Result<ConfidenceBreakdown> {
        let performance_tracker = self.performance_tracker.read().await;
        let data_quality = self.data_quality_tracker.read().await;

        // Ensemble agreement bonus (up to 30% boost)
        let ensemble_agreement = (agreement_score - 0.5).max(0.0) * 0.6; // 0.0 to 0.3

        // Historical accuracy adjustment (-20% to +20%)
        let historical_accuracy = (performance_tracker.recent_accuracy - 0.7) * 0.4; // -0.2 to 0.2

        // Market regime adjustment
        let market_regime_adjustment = match market_regime {
            "bullish" => 0.05,
            "bearish" => 0.02,
            "sideways" => 0.08,
            "high_volatility" => -0.05,
            "low_volatility" => 0.05,
            _ => 0.0,
        };

        // Data quality factor (80% to 120%)
        let avg_quality = (data_quality.completeness_score
            + data_quality.freshness_score
            + data_quality.outlier_score
            + data_quality.volume_consistency)
            / 4.0;
        let data_quality_factor = 0.8 + (avg_quality * 0.4); // 0.8 to 1.2

        // Volatility penalty (higher volatility = lower confidence)
        let volatility_penalty = -(volatility * 3.0).min(0.15); // 0 to -15%

        // Temporal distance penalty (further predictions = lower confidence)
        let temporal_distance_penalty = -(step as f64 * 0.02).min(0.1 * horizon as f64); // -2% per step

        // Calculate combined confidence from all components
        let combined_confidence = (base_confidence
            + ensemble_agreement
            + historical_accuracy
            + market_regime_adjustment
            + volatility_penalty
            + temporal_distance_penalty)
            * data_quality_factor;

        Ok(ConfidenceBreakdown {
            base_confidence,
            ensemble_agreement,
            historical_accuracy,
            market_regime_adjustment,
            data_quality_factor,
            volatility_penalty,
            temporal_distance_penalty,
            combined_confidence: combined_confidence.max(0.0).min(1.0),
        })
    }

    /// Determine if the model should be retrained
    pub async fn should_retrain(&self) -> Result<RetrainingMetrics> {
        let performance_tracker = self.performance_tracker.read().await;

        let current_accuracy = performance_tracker.recent_accuracy;
        let hours_since_training =
            (Utc::now() - performance_tracker.last_training_time).num_hours();
        let new_samples = performance_tracker
            .new_samples_count
            .load(Ordering::Relaxed);

        // Check individual triggers
        let accuracy_trigger = current_accuracy < performance_tracker.accuracy_threshold;
        let time_trigger = hours_since_training > performance_tracker.hours_threshold;
        let samples_trigger = new_samples > performance_tracker.sample_threshold;

        let should_retrain = accuracy_trigger || time_trigger || samples_trigger;

        // Determine primary trigger and urgency
        // Collect reasons for retraining
        let mut retrain_reasons = Vec::new();
        if accuracy_trigger {
            retrain_reasons.push(format!(
                "Accuracy below threshold: {:.3} < {:.3}",
                current_accuracy, performance_tracker.accuracy_threshold
            ));
        }
        if time_trigger {
            retrain_reasons.push(format!(
                "Time since training: {} hours > {} threshold",
                hours_since_training, performance_tracker.hours_threshold
            ));
        }
        if samples_trigger {
            retrain_reasons.push(format!(
                "New samples available: {} > {} threshold",
                new_samples, performance_tracker.sample_threshold
            ));
        }

        let (primary_trigger, urgency_score) = if accuracy_trigger {
            let urgency = (performance_tracker.accuracy_threshold - current_accuracy) * 5.0; // 0-5 scale
            ("accuracy_degradation".to_string(), urgency)
        } else if time_trigger {
            let urgency =
                (hours_since_training as f64 / performance_tracker.hours_threshold as f64) - 1.0;
            ("time_based".to_string(), urgency.min(3.0))
        } else if samples_trigger {
            let urgency = (new_samples as f64 / performance_tracker.sample_threshold as f64) - 1.0;
            ("data_volume".to_string(), urgency.min(2.0))
        } else {
            ("none".to_string(), 0.0)
        };

        Ok(RetrainingMetrics {
            should_retrain,
            current_accuracy,
            accuracy_threshold: performance_tracker.accuracy_threshold,
            hours_since_training,
            hours_threshold: performance_tracker.hours_threshold,
            new_samples,
            sample_threshold: performance_tracker.sample_threshold,
            primary_trigger,
            urgency_score,
            retrain_reasons: retrain_reasons,
        })
    }

    /// Update performance tracking with actual results
    pub async fn update_performance(
        &self,
        actual_values: &[f64],
        predicted_results: &[EnhancedPredictionResult],
    ) -> Result<()> {
        for (actual, predicted) in actual_values.iter().zip(predicted_results.iter()) {
            let error = (actual - predicted.value).abs() / actual.abs().max(0.01);
            let is_successful = error < 0.1; // Within 10% threshold

            // Update performance tracker
            {
                let performance_tracker = self.performance_tracker.read().await;
                performance_tracker
                    .total_predictions
                    .fetch_add(1, Ordering::Relaxed);
                if is_successful {
                    performance_tracker
                        .successful_predictions
                        .fetch_add(1, Ordering::Relaxed);
                }
            }

            // Update accuracy and history in separate scope
            {
                let mut performance_tracker = self.performance_tracker.write().await;
                let new_accuracy = if is_successful { 1.0 } else { 0.0 };
                performance_tracker.recent_accuracy = performance_tracker.recent_accuracy
                    * performance_tracker.decay_factor
                    + new_accuracy * (1.0 - performance_tracker.decay_factor);

                // Add to prediction history in nested scope
                {
                    let mut prediction_history =
                        performance_tracker.prediction_history.write().await;
                    prediction_history.push_back((*actual, predicted.value, predicted.timestamp));
                    if prediction_history.len() > performance_tracker.max_history_size {
                        prediction_history.pop_front();
                    }
                }

                debug!(
                    "Updated performance: accuracy={:.3}, error={:.3}, success={}",
                    performance_tracker.recent_accuracy, error, is_successful
                );
            }
        }

        // Update individual model performance for ensemble weighting
        for predicted in predicted_results {
            // This would typically update individual model weights
            // For now, we maintain equal weighting with small adjustments
            info!(
                "Performance update for ensemble size: {}",
                predicted.ensemble_size
            );
        }

        Ok(())
    }

    /// Add new training samples (for retraining threshold)
    pub async fn add_training_samples(&self, sample_count: usize) -> Result<()> {
        let performance_tracker = self.performance_tracker.read().await;
        performance_tracker
            .new_samples_count
            .fetch_add(sample_count, Ordering::Relaxed);

        debug!(
            "Added {} training samples, total new samples: {}",
            sample_count,
            performance_tracker
                .new_samples_count
                .load(Ordering::Relaxed)
        );

        Ok(())
    }

    /// Reset retraining counters after training
    pub async fn mark_retrained(&self) -> Result<()> {
        let mut performance_tracker = self.performance_tracker.write().await;
        performance_tracker.last_training_time = Utc::now();
        performance_tracker
            .new_samples_count
            .store(0, Ordering::Relaxed);

        info!(
            "Marked model as retrained at {}",
            performance_tracker.last_training_time
        );
        Ok(())
    }

    /// Get current performance metrics
    pub async fn get_performance_metrics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let performance_tracker = self.performance_tracker.read().await;
        let prediction_history = performance_tracker.prediction_history.read().await;

        let total_preds = performance_tracker
            .total_predictions
            .load(Ordering::Relaxed);
        let successful_preds = performance_tracker
            .successful_predictions
            .load(Ordering::Relaxed);
        let overall_accuracy = if total_preds > 0 {
            successful_preds as f64 / total_preds as f64
        } else {
            0.0
        };

        let mut metrics = HashMap::new();
        metrics.insert(
            "recent_accuracy".to_string(),
            serde_json::json!(performance_tracker.recent_accuracy),
        );
        metrics.insert(
            "overall_accuracy".to_string(),
            serde_json::json!(overall_accuracy),
        );
        metrics.insert(
            "total_predictions".to_string(),
            serde_json::json!(total_preds),
        );
        metrics.insert(
            "successful_predictions".to_string(),
            serde_json::json!(successful_preds),
        );
        metrics.insert(
            "hours_since_training".to_string(),
            serde_json::json!((Utc::now() - performance_tracker.last_training_time).num_hours()),
        );
        metrics.insert(
            "new_samples_count".to_string(),
            serde_json::json!(performance_tracker
                .new_samples_count
                .load(Ordering::Relaxed)),
        );
        metrics.insert(
            "prediction_history_size".to_string(),
            serde_json::json!(prediction_history.len()),
        );

        Ok(metrics)
    }

    /// Calculate current volatility from recent data
    pub(crate) async fn calculate_current_volatility(&self, data: &[TimeSeriesData]) -> Result<f64> {
        if data.len() < 20 {
            return Ok(0.02); // Default 2% volatility
        }

        let recent_data = &data[data.len().saturating_sub(20)..];
        let returns: Vec<f64> = recent_data
            .windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();

        if returns.is_empty() {
            return Ok(0.02);
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        let volatility = variance.sqrt();

        // Update cached volatility
        *self.recent_volatility.write().await = volatility;

        Ok(volatility)
    }

    /// Detect current market regime
    pub(crate) async fn detect_market_regime(&self, data: &[TimeSeriesData]) -> Result<String> {
        // Check cache first
        {
            let cache = self.market_regime_cache.read().await;
            if (Utc::now() - cache.1).num_minutes() < 30 {
                return Ok(cache.0.clone());
            }
        }

        if data.len() < 20 {
            return Ok("unknown".to_string());
        }

        let recent_data = &data[data.len().saturating_sub(20)..];
        let first_price = recent_data.first().unwrap().close;
        let last_price = recent_data.last().unwrap().close;
        let price_change = (last_price - first_price) / first_price;

        let volatility = *self.recent_volatility.read().await;

        let regime = if volatility > 0.05 {
            "high_volatility"
        } else if volatility < 0.01 {
            "low_volatility"
        } else if price_change > 0.05 {
            "bullish"
        } else if price_change < -0.05 {
            "bearish"
        } else {
            "sideways"
        };

        // Update cache
        *self.market_regime_cache.write().await = (regime.to_string(), Utc::now());

        Ok(regime.to_string())
    }

    /// Update data quality metrics
    pub(crate) async fn update_data_quality(&self, data: &[TimeSeriesData]) -> Result<()> {
        let mut quality_tracker = self.data_quality_tracker.write().await;

        if data.is_empty() {
            quality_tracker.completeness_score = 0.0;
            return Ok(());
        }

        // Calculate completeness (non-zero values)
        let non_zero_count = data
            .iter()
            .filter(|d| d.close > 0.0 && d.volume > 0.0)
            .count();
        quality_tracker.completeness_score = non_zero_count as f64 / data.len() as f64;

        // Calculate freshness (recent data)
        let latest_timestamp = data.iter().map(|d| d.timestamp).max().unwrap_or(Utc::now());
        let age_hours = (Utc::now() - latest_timestamp).num_hours();
        quality_tracker.freshness_score = (1.0 - (age_hours as f64 / 24.0)).max(0.0);

        // Calculate outlier score (fewer outliers = higher score)
        if data.len() > 10 {
            let prices: Vec<f64> = data.iter().map(|d| d.close).collect();
            let mean = prices.iter().sum::<f64>() / prices.len() as f64;
            let std_dev = {
                let variance =
                    prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
                variance.sqrt()
            };

            let outlier_count = prices
                .iter()
                .filter(|&&p| (p - mean).abs() > std_dev * 3.0)
                .count();
            quality_tracker.outlier_score = 1.0 - (outlier_count as f64 / prices.len() as f64);
        }

        // Calculate volume consistency
        if data.len() > 5 {
            let volumes: Vec<f64> = data.iter().map(|d| d.volume).collect();
            let volume_mean = volumes.iter().sum::<f64>() / volumes.len() as f64;
            let volume_cv = if volume_mean > 0.0 {
                let volume_std = {
                    let variance = volumes
                        .iter()
                        .map(|v| (v - volume_mean).powi(2))
                        .sum::<f64>()
                        / volumes.len() as f64;
                    variance.sqrt()
                };
                volume_std / volume_mean
            } else {
                1.0
            };
            quality_tracker.volume_consistency = (1.0 - volume_cv).max(0.0);
        }

        quality_tracker.last_update = Utc::now();

        debug!("Updated data quality: completeness={:.2}, freshness={:.2}, outliers={:.2}, volume={:.2}",
               quality_tracker.completeness_score, quality_tracker.freshness_score,
               quality_tracker.outlier_score, quality_tracker.volume_consistency);

        Ok(())
    }

    /// Get the underlying FANN predictor for direct access
    pub fn get_fann_predictor(&self) -> &FannPredictor {
        &self.fann_predictor
    }
}

impl Default for EnhancedNeuralPredictor {
    fn default() -> Self {
        let config = NeuralConfig {
            memory_gb: 2.0,
            models: vec![
                "DeepAR".to_string(),
                "LSTM".to_string(),
                "NHITS".to_string(),
            ],
            prediction_cache_ttl: 300,
            model_load_timeout: 300,
            max_concurrent_predictions: 50,
            enable_model_monitoring: true,
            accuracy_threshold: 0.75,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 300,
            max_retries: 3,
            error_threshold: 0.1,
            lookback_window: 24,
        };
        Self::new(config).expect("Failed to create default enhanced predictor")
    }
}

impl Default for ConfidenceBreakdown {
    fn default() -> Self {
        Self {
            base_confidence: 0.5,
            ensemble_agreement: 0.0,
            historical_accuracy: 0.0,
            market_regime_adjustment: 0.0,
            data_quality_factor: 1.0,
            volatility_penalty: 0.0,
            temporal_distance_penalty: 0.0,
            combined_confidence: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_data(count: usize) -> Vec<TimeSeriesData> {
        let mut data = Vec::new();
        let base_time = Utc::now();
        let mut price = 100.0;

        for i in 0..count {
            price *= 1.0 + (0.02 * (i as f64 * 0.1).sin()); // Synthetic price movement

            let mut indicators = HashMap::new();
            indicators.insert("rsi".to_string(), 50.0 + (i as f64 * 0.5));

            data.push(TimeSeriesData {
                timestamp: base_time + chrono::Duration::minutes(i as i64),
                entity: Some("TEST".to_string()),
                symbol: "TEST".to_string(),
                open: price * 0.999,
                high: price * 1.001,
                low: price * 0.998,
                close: price,
                volume: 1000000.0 + (i as f64 * 1000.0),
                source: Some("test".to_string()),
                value: Some(price),
                metadata: Some(serde_json::json!({})),
                indicators,
            });
        }

        data
    }

    #[tokio::test]
    async fn test_enhanced_predictor_creation() {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
            use_real_models: false,
            enable_health_checks: true,
            enable_fallback: true,
            enable_circuit_breakers: true,
            enable_graceful_degradation: false,
            enable_performance_monitoring: true,
            enable_adaptive_retry: true,
            enable_model_ensembles: false,
            model_timeout_seconds: 60,
            max_retries: 3,
            error_threshold: 0.1,
            lookback_window: 24,
        };

        let predictor = EnhancedNeuralPredictor::new(config).unwrap();
        assert!(predictor.config.models.contains(&"MLP".to_string()));
    }

    #[tokio::test]
    async fn test_should_retrain_logic() {
        let predictor = EnhancedNeuralPredictor::default();

        // Test initial state (should not retrain immediately)
        let metrics = predictor.should_retrain().await.unwrap();
        assert!(!metrics.should_retrain || metrics.primary_trigger == "accuracy_degradation");

        // Simulate performance degradation
        {
            let mut performance_tracker = predictor.performance_tracker.write().await;
            performance_tracker.recent_accuracy = 0.6; // Below 0.7 threshold
        }

        let metrics = predictor.should_retrain().await.unwrap();
        assert!(metrics.should_retrain);
        assert_eq!(metrics.primary_trigger, "accuracy_degradation");
        assert!(metrics.urgency_score > 0.0);
    }

    #[tokio::test]
    async fn test_confidence_calculation() {
        let predictor = EnhancedNeuralPredictor::default();
        let test_data = create_test_data(50);

        let predictions = predictor
            .predict_with_confidence(&test_data, 5)
            .await
            .unwrap();

        assert!(!predictions.is_empty());
        for prediction in predictions {
            assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
            assert!(prediction.confidence_breakdown.base_confidence >= 0.0);
            assert!(prediction.confidence_breakdown.data_quality_factor > 0.0);
            assert!(
                prediction.model_agreement_score >= 0.0 && prediction.model_agreement_score <= 1.0
            );
        }
    }

    #[tokio::test]
    async fn test_performance_tracking() {
        let predictor = EnhancedNeuralPredictor::default();

        // Create some mock predictions
        let predictions = vec![EnhancedPredictionResult {
            timestamp: Utc::now(),
            value: 100.0,
            confidence: 0.8,
            confidence_breakdown: ConfidenceBreakdown::default(),
            models_agree: true,
            model_agreement_score: 0.9,
            interval_low: 98.0,
            interval_high: 102.0,
            ensemble_size: 3,
            market_regime: "bullish".to_string(),
            volatility_adjustment: 1.1,
        }];

        let actual_values = vec![101.0]; // Close to prediction

        predictor
            .update_performance(&actual_values, &predictions)
            .await
            .unwrap();

        let metrics = predictor.get_performance_metrics().await.unwrap();
        assert!(metrics.contains_key("recent_accuracy"));
        assert!(metrics.contains_key("total_predictions"));

        let total_preds = metrics.get("total_predictions").unwrap().as_u64().unwrap();
        assert_eq!(total_preds, 1);
    }

    #[tokio::test]
    async fn test_volatility_calculation() {
        let predictor = EnhancedNeuralPredictor::default();

        // Create data with known volatility pattern
        let mut test_data = create_test_data(30);

        // Make prices more volatile
        for (i, data_point) in test_data.iter_mut().enumerate() {
            data_point.close *= 1.0 + 0.1 * (i as f64).sin(); // 10% volatility swings
        }

        let volatility = predictor
            .calculate_current_volatility(&test_data)
            .await
            .unwrap();

        // Should detect higher volatility
        assert!(volatility > 0.02); // Higher than default
        assert!(volatility < 1.0); // But reasonable
    }

    #[tokio::test]
    async fn test_market_regime_detection() {
        let predictor = EnhancedNeuralPredictor::default();

        // Create bullish market data
        let mut test_data = create_test_data(25);
        for (i, data_point) in test_data.iter_mut().enumerate() {
            data_point.close = 100.0 + i as f64 * 2.0; // Steady uptrend
        }

        let regime = predictor.detect_market_regime(&test_data).await.unwrap();
        assert_eq!(regime, "bullish");

        // Test caching by calling again immediately
        let regime2 = predictor.detect_market_regime(&test_data).await.unwrap();
        assert_eq!(regime2, "bullish");
    }

    #[tokio::test]
    async fn test_add_training_samples() {
        let predictor = EnhancedNeuralPredictor::default();

        predictor.add_training_samples(5000).await.unwrap();

        let metrics = predictor.get_performance_metrics().await.unwrap();
        let new_samples = metrics.get("new_samples_count").unwrap().as_u64().unwrap();
        assert_eq!(new_samples, 5000);

        // Add more samples
        predictor.add_training_samples(6000).await.unwrap();

        let metrics = predictor.get_performance_metrics().await.unwrap();
        let new_samples = metrics.get("new_samples_count").unwrap().as_u64().unwrap();
        assert_eq!(new_samples, 11000);

        // Test retraining trigger
        let retrain_metrics = predictor.should_retrain().await.unwrap();
        assert!(retrain_metrics.should_retrain);
        assert_eq!(retrain_metrics.primary_trigger, "data_volume");
    }

    #[tokio::test]
    async fn test_mark_retrained() {
        let predictor = EnhancedNeuralPredictor::default();

        // Add samples and verify retraining needed
        predictor.add_training_samples(15000).await.unwrap();
        let metrics_before = predictor.should_retrain().await.unwrap();
        assert!(metrics_before.should_retrain);

        // Mark as retrained
        predictor.mark_retrained().await.unwrap();

        // Should reset counters
        let performance_metrics = predictor.get_performance_metrics().await.unwrap();
        let new_samples = performance_metrics
            .get("new_samples_count")
            .unwrap()
            .as_u64()
            .unwrap();
        assert_eq!(new_samples, 0);

        let metrics_after = predictor.should_retrain().await.unwrap();
        assert!(
            !metrics_after.should_retrain
                || metrics_after.primary_trigger == "accuracy_degradation"
        );
    }
}
