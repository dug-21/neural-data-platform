//! Autonomous Training Metrics Module
//!
//! Contains performance tracking, analysis, and decision recording structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::config::{TrainingDecision, TrainingOutcome};

/// Performance snapshot for training decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: DateTime<Utc>,
    pub accuracy: f64,
    pub confidence: f64,
    pub price_error: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub volatility: f64,
    pub model_agreement: f64,
    pub consecutive_failures: usize,
    pub trading_volume: f64,
    pub profit_loss: f64,
}

/// Training decision with full context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDecision {
    pub decision_id: String,
    pub timestamp: DateTime<Utc>,
    pub decision_type: super::config::TrainingDecisionType,
    pub confidence: f64,
    pub reasoning: Vec<String>,
    pub performance_snapshot: PerformanceSnapshot,
    pub resource_requirements: super::config::ResourceRequirements,
    pub estimated_duration: chrono::Duration,
    pub priority: super::config::TrainingPriority,
    pub affected_models: Vec<String>,
}

/// Memory for storing training decisions and outcomes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingDecisionRecord {
    pub decision: TrainingDecision,
    pub execution_started: Option<DateTime<Utc>>,
    pub execution_completed: Option<DateTime<Utc>>,
    pub outcome: Option<TrainingOutcome>,
    pub performance_improvement: Option<f64>,
}

/// Information about individual models
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub accuracy: f64,
    pub confidence: f64,
    pub last_updated: DateTime<Utc>,
    pub training_count: usize,
    pub performance_trend: PerformanceTrend,
}

/// Performance trend analysis
#[derive(Debug, Clone)]
pub enum PerformanceTrend {
    Improving,
    Stable,
    Degrading,
    Volatile,
}

/// Performance trend analysis result
#[derive(Debug)]
pub struct PerformanceTrendAnalysis {
    pub accuracy_trend: PerformanceTrend,
    pub confidence_trend: PerformanceTrend,
    pub volatility_trend: PerformanceTrend,
    pub overall_trend: PerformanceTrend,
}

/// Metric analysis utilities
pub struct MetricsAnalyzer;

impl MetricsAnalyzer {
    /// Analyze trend for a specific metric
    pub fn analyze_metric_trend(values: &[f64]) -> PerformanceTrend {
        if values.len() < 3 {
            return PerformanceTrend::Stable;
        }

        // Calculate linear regression slope
        let n = values.len() as f64;
        let x_mean = (n - 1.0) / 2.0;
        let y_mean = values.iter().sum::<f64>() / n;

        let mut numerator = 0.0;
        let mut denominator = 0.0;

        for (i, &y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        let slope = if denominator != 0.0 {
            numerator / denominator
        } else {
            0.0
        };

        // Calculate volatility (coefficient of variation)
        let std_dev = {
            let variance = values.iter().map(|&y| (y - y_mean).powi(2)).sum::<f64>() / n;
            variance.sqrt()
        };

        let cv = if y_mean != 0.0 {
            std_dev / y_mean.abs()
        } else {
            0.0
        };

        // Classify trend
        if cv > 0.3 {
            PerformanceTrend::Volatile
        } else if slope > 0.05 {
            PerformanceTrend::Improving
        } else if slope < -0.05 {
            PerformanceTrend::Degrading
        } else {
            PerformanceTrend::Stable
        }
    }

    /// Detect current market regime
    pub fn detect_market_regime(performance: &PerformanceSnapshot) -> String {
        if performance.volatility > 0.04 {
            "high_volatility".to_string()
        } else if performance.volatility < 0.01 {
            "low_volatility".to_string()
        } else if performance.profit_loss > 0.0 {
            "bullish".to_string()
        } else if performance.profit_loss < -0.02 {
            "bearish".to_string()
        } else {
            "sideways".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metric_trend_analysis_stable() {
        let values = vec![0.7, 0.71, 0.69, 0.70, 0.72];
        let trend = MetricsAnalyzer::analyze_metric_trend(&values);
        match trend {
            PerformanceTrend::Stable => (),
            _ => panic!("Expected stable trend for small variations"),
        }
    }

    #[test]
    fn test_metric_trend_analysis_improving() {
        let values = vec![0.60, 0.65, 0.70, 0.75, 0.80];
        let trend = MetricsAnalyzer::analyze_metric_trend(&values);
        match trend {
            PerformanceTrend::Improving => (),
            _ => panic!("Expected improving trend for increasing values"),
        }
    }

    #[test]
    fn test_metric_trend_analysis_degrading() {
        let values = vec![0.80, 0.75, 0.70, 0.65, 0.60];
        let trend = MetricsAnalyzer::analyze_metric_trend(&values);
        match trend {
            PerformanceTrend::Degrading => (),
            _ => panic!("Expected degrading trend for decreasing values"),
        }
    }

    #[test]
    fn test_market_regime_detection() {
        let high_vol_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.7,
            confidence: 0.8,
            price_error: 0.05,
            sharpe_ratio: 0.6,
            max_drawdown: 0.1,
            volatility: 0.06, // High volatility
            model_agreement: 0.8,
            consecutive_failures: 0,
            trading_volume: 1000000.0,
            profit_loss: 0.01,
        };

        let regime = MetricsAnalyzer::detect_market_regime(&high_vol_performance);
        assert_eq!(regime, "high_volatility");
    }
}