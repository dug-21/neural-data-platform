//! Model Performance Tracking for DAA Integration
//!
//! Tracks individual model performance metrics and feeds data to the DAA
//! autonomous training system for informed training decisions.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::neural::PredictionResult;

/// Market regime for performance context
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarketRegime {
    Bullish,
    Bearish,
    Sideways,
    HighVolatility,
    LowVolatility,
}

/// Model performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    pub model_id: String,
    pub symbol: String,
    
    // Accuracy metrics
    pub prediction_accuracy: f64,
    pub mape: f64, // Mean Absolute Percentage Error
    pub rmse: f64, // Root Mean Square Error
    pub mae: f64,  // Mean Absolute Error
    pub r_squared: f64,
    
    // Trading performance
    pub sharpe_ratio: f64,
    pub win_rate: f64,
    pub max_drawdown: f64,
    pub profit_factor: f64,
    pub calmar_ratio: f64,
    
    // Reliability metrics
    pub prediction_count: u64,
    pub consecutive_failures: u32,
    pub confidence_calibration: f64,
    pub prediction_latency_ms: f64,
    
    // Time-based performance
    pub performance_trend_30d: f64,
    pub performance_by_time_of_day: HashMap<u8, f64>,
    pub performance_by_market_regime: HashMap<MarketRegime, f64>,
    
    // Resource usage
    pub memory_usage_mb: f64,
    pub cpu_usage_percent: f64,
    pub inference_cost_per_prediction: f64,
    
    // Timestamps
    pub last_updated: DateTime<Utc>,
    pub first_prediction: DateTime<Utc>,
    pub last_successful_prediction: DateTime<Utc>,
}

impl Default for ModelMetrics {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            model_id: String::new(),
            symbol: String::new(),
            prediction_accuracy: 0.0,
            mape: 100.0,
            rmse: f64::MAX,
            mae: f64::MAX,
            r_squared: 0.0,
            sharpe_ratio: 0.0,
            win_rate: 0.0,
            max_drawdown: 0.0,
            profit_factor: 0.0,
            calmar_ratio: 0.0,
            prediction_count: 0,
            consecutive_failures: 0,
            confidence_calibration: 0.0,
            prediction_latency_ms: 0.0,
            performance_trend_30d: 0.0,
            performance_by_time_of_day: HashMap::new(),
            performance_by_market_regime: HashMap::new(),
            memory_usage_mb: 0.0,
            cpu_usage_percent: 0.0,
            inference_cost_per_prediction: 0.0,
            last_updated: now,
            first_prediction: now,
            last_successful_prediction: now,
        }
    }
}

/// DAA performance input for autonomous training decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAAPerformanceInput {
    pub prediction_accuracy: f64,
    pub consecutive_failures: u32,
    pub confidence_calibration: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub win_rate: f64,
    pub performance_trend_30d: f64,
    pub performance_by_market_regime: HashMap<MarketRegime, f64>,
    pub memory_usage_mb: f64,
    pub prediction_latency_ms: f64,
    pub training_history: Vec<TrainingRecord>,
    pub last_training_date: DateTime<Utc>,
}

/// Training record for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRecord {
    pub date: DateTime<Utc>,
    pub accuracy_before: f64,
    pub accuracy_after: f64,
    pub training_duration_seconds: u64,
    pub reason: String,
}

/// Model value report for optimization decisions
#[derive(Debug, Clone)]
pub struct ModelValueReport {
    pub symbol: String,
    pub total_models: usize,
    pub top_performers: Vec<ModelRanking>,
    pub underperformers: Vec<ModelRanking>,
    pub recommendations: OptimizationRecommendations,
    pub resource_savings_potential: ResourceSavings,
}

/// Model ranking information
#[derive(Debug, Clone)]
pub struct ModelRanking {
    pub model_id: String,
    pub value_score: f64,
    pub metrics: ModelMetrics,
    pub recommendation: ModelRecommendation,
}

/// Model recommendation
#[derive(Debug, Clone)]
pub enum ModelRecommendation {
    Keep { reason: String },
    Optimize { changes: Vec<String> },
    Deactivate { reason: String, savings: ResourceSavings },
    Retrain { urgency: TrainingUrgency },
}

/// Training urgency levels
#[derive(Debug, Clone, Copy)]
pub enum TrainingUrgency {
    Emergency,
    Critical,
    High,
    Medium,
    Low,
}

/// Resource savings information
#[derive(Debug, Clone, Default)]
pub struct ResourceSavings {
    pub memory_mb: f64,
    pub cpu_percent: f64,
    pub cost_per_hour: f64,
}

/// Optimization recommendations
#[derive(Debug, Clone)]
pub struct OptimizationRecommendations {
    pub models_to_deactivate: Vec<String>,
    pub models_to_retrain: Vec<String>,
    pub ensemble_size_recommendation: usize,
    pub resource_optimization_potential: f64,
}

/// Main performance tracker
pub struct ModelPerformanceTracker {
    /// Individual model metrics per symbol
    model_metrics: Arc<DashMap<(String, String), ModelMetrics>>, // (symbol, model_id)
    
    /// Ensemble performance tracking
    ensemble_metrics: Arc<DashMap<String, EnsembleMetrics>>, // symbol -> metrics
    
    /// Training history
    training_history: Arc<RwLock<HashMap<String, Vec<TrainingRecord>>>>,
    
    /// DAA integration callback
    daa_callback: Option<Arc<dyn DAAIntegration>>,
}

/// Ensemble metrics
#[derive(Debug, Clone)]
pub struct EnsembleMetrics {
    pub symbol: String,
    pub total_models_active: usize,
    pub ensemble_accuracy: f64,
    pub ensemble_sharpe: f64,
    pub model_contributions: HashMap<String, f64>,
    pub top_performers: Vec<String>,
    pub underperformers: Vec<String>,
    pub redundant_models: Vec<String>,
    pub prediction_correlation_matrix: HashMap<(String, String), f64>,
    pub ensemble_diversity_score: f64,
    pub optimal_ensemble_size: usize,
}

/// DAA integration trait
#[async_trait::async_trait]
pub trait DAAIntegration: Send + Sync {
    async fn notify_performance_update(
        &self,
        symbol: &str,
        model_id: &str,
        metrics: &ModelMetrics,
    ) -> Result<()>;
}

impl ModelPerformanceTracker {
    /// Create new performance tracker
    pub fn new() -> Self {
        info!("Initializing ModelPerformanceTracker for DAA integration");
        
        Self {
            model_metrics: Arc::new(DashMap::new()),
            ensemble_metrics: Arc::new(DashMap::new()),
            training_history: Arc::new(RwLock::new(HashMap::new())),
            daa_callback: None,
        }
    }
    
    /// Set DAA integration callback
    pub fn set_daa_integration(&mut self, integration: Arc<dyn DAAIntegration>) {
        self.daa_callback = Some(integration);
        info!("DAA integration callback configured");
    }
    
    /// Record a prediction and track performance
    pub async fn record_prediction(
        &self,
        symbol: &str,
        model_id: &str,
        prediction: &PredictionResult,
        actual_outcome: Option<f64>,
    ) -> Result<()> {
        let key = (symbol.to_string(), model_id.to_string());
        
        // Get or create metrics
        let mut entry = self.model_metrics.entry(key.clone())
            .or_insert_with(|| {
                let mut metrics = ModelMetrics::default();
                metrics.symbol = symbol.to_string();
                metrics.model_id = model_id.to_string();
                metrics.first_prediction = Utc::now();
                metrics
            });
        
        // Update prediction count
        entry.prediction_count += 1;
        entry.last_updated = Utc::now();
        
        // If we have an actual outcome, calculate accuracy
        if let Some(actual) = actual_outcome {
            self.update_accuracy_metrics(&mut entry, prediction.value, actual);
        }
        
        // Update latency tracking (placeholder - would be measured in real implementation)
        entry.prediction_latency_ms = 10.0; // Example value
        
        // Notify DAA if configured
        if let Some(daa) = &self.daa_callback {
            daa.notify_performance_update(symbol, model_id, &entry).await?;
        }
        
        debug!("Recorded prediction for {} ({}): accuracy={:.2}%, count={}", 
            symbol, model_id, entry.prediction_accuracy * 100.0, entry.prediction_count);
        
        Ok(())
    }
    
    /// Update accuracy metrics
    fn update_accuracy_metrics(
        &self,
        metrics: &mut ModelMetrics,
        predicted: f64,
        actual: f64,
    ) {
        let error = (predicted - actual).abs();
        let percentage_error = if actual != 0.0 {
            error / actual.abs()
        } else {
            1.0
        };
        
        // Update MAPE (running average)
        let n = metrics.prediction_count as f64;
        metrics.mape = (metrics.mape * (n - 1.0) + percentage_error * 100.0) / n;
        
        // Update MAE (running average)
        metrics.mae = (metrics.mae * (n - 1.0) + error) / n;
        
        // Update accuracy (within 5% threshold)
        let is_accurate = percentage_error <= 0.05;
        if is_accurate {
            metrics.prediction_accuracy = 
                (metrics.prediction_accuracy * (n - 1.0) + 1.0) / n;
            metrics.consecutive_failures = 0;
            metrics.last_successful_prediction = Utc::now();
        } else {
            metrics.prediction_accuracy = 
                (metrics.prediction_accuracy * (n - 1.0)) / n;
            metrics.consecutive_failures += 1;
        }
        
        // Simple win rate (predicted direction correct)
        let direction_correct = (predicted > 0.0) == (actual > 0.0);
        if direction_correct {
            metrics.win_rate = (metrics.win_rate * (n - 1.0) + 1.0) / n;
        } else {
            metrics.win_rate = (metrics.win_rate * (n - 1.0)) / n;
        }
    }
    
    /// Get model metrics
    pub async fn get_model_metrics(
        &self,
        symbol: &str,
        model_id: &str,
    ) -> Option<ModelMetrics> {
        let key = (symbol.to_string(), model_id.to_string());
        self.model_metrics.get(&key).map(|entry| entry.clone())
    }
    
    /// Generate model value report
    pub async fn generate_model_value_report(&self, symbol: &str) -> Result<ModelValueReport> {
        let mut model_rankings = Vec::new();
        
        // Collect all models for this symbol
        for entry in self.model_metrics.iter() {
            if entry.key().0 == symbol {
                let value_score = self.calculate_model_value_score(&entry.value());
                model_rankings.push(ModelRanking {
                    model_id: entry.key().1.clone(),
                    value_score,
                    metrics: entry.value().clone(),
                    recommendation: self.get_model_recommendation(&entry.value(), value_score),
                });
            }
        }
        
        // Sort by value score
        model_rankings.sort_by(|a, b| b.value_score.partial_cmp(&a.value_score).unwrap());
        
        let top_performers = model_rankings.iter()
            .take(5)
            .cloned()
            .collect();
        
        let underperformers = model_rankings.iter()
            .rev()
            .take(5)
            .cloned()
            .collect();
        
        Ok(ModelValueReport {
            symbol: symbol.to_string(),
            total_models: model_rankings.len(),
            top_performers,
            underperformers,
            recommendations: self.generate_optimization_recommendations(&model_rankings),
            resource_savings_potential: self.calculate_resource_savings(&model_rankings),
        })
    }
    
    /// Calculate model value score
    fn calculate_model_value_score(&self, metrics: &ModelMetrics) -> f64 {
        let accuracy_weight = 0.3;
        let trading_weight = 0.3;
        let reliability_weight = 0.2;
        let efficiency_weight = 0.2;
        
        let accuracy_score = (metrics.prediction_accuracy * 0.4 + 
                             (1.0 - metrics.mape / 100.0).max(0.0) * 0.3 + 
                             metrics.r_squared * 0.3).max(0.0);
        
        let trading_score = ((metrics.sharpe_ratio / 3.0).min(1.0).max(0.0) * 0.4 +
                           metrics.win_rate * 0.3 +
                           (1.0 - metrics.max_drawdown) * 0.3).max(0.0);
        
        let reliability_score = (metrics.confidence_calibration * 0.4 +
                               (1.0 - metrics.consecutive_failures as f64 / 10.0).max(0.0) * 0.6);
        
        let efficiency_score = ((1.0 - (metrics.memory_usage_mb / 1000.0).min(1.0)) * 0.5 +
                              (1.0 - (metrics.prediction_latency_ms / 1000.0).min(1.0)) * 0.5).max(0.0);
        
        accuracy_score * accuracy_weight +
        trading_score * trading_weight +
        reliability_score * reliability_weight +
        efficiency_score * efficiency_weight
    }
    
    /// Get model recommendation
    fn get_model_recommendation(&self, metrics: &ModelMetrics, value_score: f64) -> ModelRecommendation {
        if value_score < 0.3 {
            ModelRecommendation::Deactivate {
                reason: format!("Low value score: {:.2}", value_score),
                savings: ResourceSavings {
                    memory_mb: metrics.memory_usage_mb,
                    cpu_percent: metrics.cpu_usage_percent,
                    cost_per_hour: metrics.inference_cost_per_prediction * 3600.0,
                },
            }
        } else if metrics.consecutive_failures >= 5 {
            ModelRecommendation::Retrain {
                urgency: TrainingUrgency::High,
            }
        } else if value_score < 0.6 {
            ModelRecommendation::Optimize {
                changes: vec![
                    "Increase training data".to_string(),
                    "Adjust hyperparameters".to_string(),
                ],
            }
        } else {
            ModelRecommendation::Keep {
                reason: format!("Good performance: {:.2}", value_score),
            }
        }
    }
    
    /// Generate optimization recommendations
    fn generate_optimization_recommendations(&self, rankings: &[ModelRanking]) -> OptimizationRecommendations {
        let models_to_deactivate = rankings.iter()
            .filter(|r| r.value_score < 0.3)
            .map(|r| r.model_id.clone())
            .collect();
        
        let models_to_retrain = rankings.iter()
            .filter(|r| r.metrics.consecutive_failures >= 3 || r.value_score < 0.5)
            .map(|r| r.model_id.clone())
            .collect();
        
        let optimal_count = rankings.iter()
            .filter(|r| r.value_score >= 0.6)
            .count()
            .max(3);
        
        OptimizationRecommendations {
            models_to_deactivate,
            models_to_retrain,
            ensemble_size_recommendation: optimal_count,
            resource_optimization_potential: 0.3, // Placeholder
        }
    }
    
    /// Calculate resource savings
    fn calculate_resource_savings(&self, rankings: &[ModelRanking]) -> ResourceSavings {
        rankings.iter()
            .filter(|r| r.value_score < 0.3)
            .fold(ResourceSavings::default(), |mut acc, r| {
                acc.memory_mb += r.metrics.memory_usage_mb;
                acc.cpu_percent += r.metrics.cpu_usage_percent;
                acc.cost_per_hour += r.metrics.inference_cost_per_prediction * 3600.0;
                acc
            })
    }
    
    /// Convert metrics to DAA performance input
    pub fn to_daa_performance_input(&self, metrics: &ModelMetrics) -> DAAPerformanceInput {
        DAAPerformanceInput {
            prediction_accuracy: metrics.prediction_accuracy,
            consecutive_failures: metrics.consecutive_failures,
            confidence_calibration: metrics.confidence_calibration,
            sharpe_ratio: metrics.sharpe_ratio,
            max_drawdown: metrics.max_drawdown,
            win_rate: metrics.win_rate,
            performance_trend_30d: metrics.performance_trend_30d,
            performance_by_market_regime: metrics.performance_by_market_regime.clone(),
            memory_usage_mb: metrics.memory_usage_mb,
            prediction_latency_ms: metrics.prediction_latency_ms,
            training_history: Vec::new(), // Would be populated from training_history
            last_training_date: metrics.last_updated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_performance_tracker_creation() {
        let tracker = ModelPerformanceTracker::new();
        assert_eq!(tracker.model_metrics.len(), 0);
    }
    
    #[tokio::test]
    async fn test_record_prediction() {
        let tracker = ModelPerformanceTracker::new();
        
        let prediction = PredictionResult {
            value: 100.0,
            confidence: 0.8,
            model_type: "LSTM".to_string(),
            features_used: vec![],
            timestamp: Utc::now(),
            metadata: Some(HashMap::new()),
        };
        
        tracker.record_prediction("AAPL", "LSTM_test", &prediction, Some(105.0))
            .await
            .unwrap();
        
        let metrics = tracker.get_model_metrics("AAPL", "LSTM_test").await;
        assert!(metrics.is_some());
        
        let metrics = metrics.unwrap();
        assert_eq!(metrics.prediction_count, 1);
        assert!(metrics.prediction_accuracy > 0.0);
    }
}