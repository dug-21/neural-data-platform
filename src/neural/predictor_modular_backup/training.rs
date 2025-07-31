//! Online training and performance monitoring module
//!
//! Handles model training, concept drift detection, and performance tracking.

use anyhow::{Context, Result};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

/// Market regime detector for adaptive training
#[derive(Debug, Clone, PartialEq)]
pub enum MarketRegime {
    Trending,
    Ranging,
    Volatile,
    Unknown,
}

/// Model performance tracking
#[derive(Debug, Clone)]
pub struct ModelPerformance {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub mae: f64,
    pub rmse: f64,
    pub directional_accuracy: f64,
    pub sharpe_ratio: Option<f64>,
    pub max_drawdown: Option<f64>,
    pub sample_count: u64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Concept drift detection system
pub struct ConceptDriftDetector {
    window_size: usize,
    drift_threshold: f64,
    performance_history: VecDeque<f64>,
    current_performance: f64,
    drift_detected: bool,
}

impl ConceptDriftDetector {
    pub fn new(window_size: usize, drift_threshold: f64) -> Self {
        Self {
            window_size,
            drift_threshold,
            performance_history: VecDeque::with_capacity(window_size),
            current_performance: 0.0,
            drift_detected: false,
        }
    }

    pub fn update_performance(&mut self, performance: f64) {
        self.performance_history.push_back(performance);
        if self.performance_history.len() > self.window_size {
            self.performance_history.pop_front();
        }

        // Detect drift using sliding window comparison
        if self.performance_history.len() >= self.window_size / 2 {
            let recent_avg: f64 = self.performance_history
                .iter()
                .rev()
                .take(self.window_size / 2)
                .sum::<f64>() / (self.window_size / 2) as f64;

            let historical_avg: f64 = self.performance_history
                .iter()
                .take(self.window_size / 2)
                .sum::<f64>() / (self.window_size / 2) as f64;

            let performance_drop = (historical_avg - recent_avg) / historical_avg.abs();
            self.drift_detected = performance_drop > self.drift_threshold;
        }

        self.current_performance = performance;
    }

    pub fn is_drift_detected(&self) -> bool {
        self.drift_detected
    }

    pub fn reset(&mut self) {
        self.performance_history.clear();
        self.current_performance = 0.0;
        self.drift_detected = false;
    }
}

/// Online performance metrics tracker
pub struct OnlinePerformanceMetrics {
    model_performances: HashMap<String, ModelPerformance>,
    prediction_errors: VecDeque<f64>,
    rolling_window_size: usize,
    total_predictions: u64,
    successful_predictions: u64,
}

impl OnlinePerformanceMetrics {
    pub fn new(rolling_window_size: usize) -> Self {
        Self {
            model_performances: HashMap::new(),
            prediction_errors: VecDeque::with_capacity(rolling_window_size),
            rolling_window_size,
            total_predictions: 0,
            successful_predictions: 0,
        }
    }

    pub fn update_model_performance(&mut self, model_name: &str, performance: ModelPerformance) {
        self.model_performances.insert(model_name.to_string(), performance);
    }

    pub fn get_model_performance(&self, model_name: &str) -> Option<&ModelPerformance> {
        self.model_performances.get(model_name)
    }

    pub fn add_prediction_error(&mut self, error: f64) {
        self.prediction_errors.push_back(error);
        if self.prediction_errors.len() > self.rolling_window_size {
            self.prediction_errors.pop_front();
        }
        self.total_predictions += 1;
        if error.abs() < 0.1 { // Threshold for "successful" prediction
            self.successful_predictions += 1;
        }
    }

    pub fn get_rolling_mae(&self) -> f64 {
        if self.prediction_errors.is_empty() {
            return 0.0;
        }
        self.prediction_errors.iter().map(|e| e.abs()).sum::<f64>() / self.prediction_errors.len() as f64
    }

    pub fn get_success_rate(&self) -> f64 {
        if self.total_predictions == 0 {
            return 0.0;
        }
        self.successful_predictions as f64 / self.total_predictions as f64
    }
}

/// Online training manager
pub struct OnlineTrainingManager {
    config: NeuralConfig,
    drift_detector: Arc<RwLock<ConceptDriftDetector>>,
    performance_metrics: Arc<RwLock<OnlinePerformanceMetrics>>,
    training_queue: Arc<RwLock<VecDeque<TimeSeriesData>>>,
    regime_detector: Arc<RwLock<MarketRegime>>,
}

impl OnlineTrainingManager {
    pub fn new(config: NeuralConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            drift_detector: Arc::new(RwLock::new(ConceptDriftDetector::new(100, 0.1))),
            performance_metrics: Arc::new(RwLock::new(OnlinePerformanceMetrics::new(1000))),
            training_queue: Arc::new(RwLock::new(VecDeque::with_capacity(10000))),
            regime_detector: Arc::new(RwLock::new(MarketRegime::Unknown)),
        })
    }

    /// Add training data to queue
    pub async fn add_training_data(&self, data: TimeSeriesData) -> Result<()> {
        let mut queue = self.training_queue.write().await;
        queue.push_back(data);
        
        // Keep queue size manageable
        if queue.len() > 10000 {
            queue.pop_front();
        }
        
        Ok(())
    }

    /// Check if retraining is needed
    pub async fn should_retrain(&self) -> bool {
        let drift_detector = self.drift_detector.read().await;
        drift_detector.is_drift_detected()
    }

    /// Update performance and detect drift
    pub async fn update_performance(&self, model_name: &str, actual: f64, predicted: f64) -> Result<()> {
        let error = actual - predicted;
        
        // Update performance metrics
        {
            let mut metrics = self.performance_metrics.write().await;
            metrics.add_prediction_error(error);
        }

        // Update drift detector
        {
            let mut drift_detector = self.drift_detector.write().await;
            drift_detector.update_performance(error.abs());
        }

        Ok(())
    }

    /// Get feature importance (placeholder implementation)
    pub async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        // In a real implementation, this would analyze model weights and gradients
        let mut importance = HashMap::new();
        importance.insert("price".to_string(), 0.4);
        importance.insert("volume".to_string(), 0.3);
        importance.insert("volatility".to_string(), 0.2);
        importance.insert("trend".to_string(), 0.1);
        Ok(importance)
    }

    /// Detect current market regime
    pub async fn detect_market_regime(&self, data: &[TimeSeriesData]) -> MarketRegime {
        if data.len() < 20 {
            return MarketRegime::Unknown;
        }

        // Simple regime detection based on price volatility and trend
        let prices: Vec<f64> = data.iter().map(|d| d.close).collect();
        let returns: Vec<f64> = prices.windows(2)
            .map(|w| (w[1] - w[0]) / w[0])
            .collect();

        let volatility = returns.iter()
            .map(|r| r.powi(2))
            .sum::<f64>() / returns.len() as f64;

        let trend_strength = returns.iter().sum::<f64>().abs() / returns.len() as f64;

        let regime = if volatility > 0.02 {
            MarketRegime::Volatile
        } else if trend_strength > 0.01 {
            MarketRegime::Trending
        } else {
            MarketRegime::Ranging
        };

        // Update stored regime
        {
            let mut stored_regime = self.regime_detector.write().await;
            *stored_regime = regime.clone();
        }

        regime
    }

    /// Get current performance metrics
    pub async fn get_performance_summary(&self) -> HashMap<String, f64> {
        let metrics = self.performance_metrics.read().await;
        let mut summary = HashMap::new();
        
        summary.insert("rolling_mae".to_string(), metrics.get_rolling_mae());
        summary.insert("success_rate".to_string(), metrics.get_success_rate());
        summary.insert("total_predictions".to_string(), metrics.total_predictions as f64);
        
        summary
    }
}