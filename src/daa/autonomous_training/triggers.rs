//! Autonomous Training Triggers Module
//!
//! Contains logic for evaluating when training should be triggered based on performance metrics.

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::config::{
    ResourceRequirements, TrainingDecisionType, TrainingPriority, TrainingTriggerConfig,
};
use super::metrics::{
    MetricsAnalyzer, PerformanceSnapshot, PerformanceTrendAnalysis, TrainingDecision,
};

/// Training trigger evaluator that analyzes performance and makes training decisions
pub struct TrainingTriggerEvaluator {
    config: TrainingTriggerConfig,
    performance_history: Arc<RwLock<VecDeque<PerformanceSnapshot>>>,
    last_training_time: Arc<RwLock<DateTime<Utc>>>,
    consecutive_failure_count: Arc<AtomicUsize>,
    max_history_size: usize,
}

impl TrainingTriggerEvaluator {
    /// Create new training trigger evaluator
    pub fn new(config: TrainingTriggerConfig) -> Self {
        Self {
            config,
            performance_history: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            last_training_time: Arc::new(RwLock::new(Utc::now() - Duration::hours(24))),
            consecutive_failure_count: Arc::new(AtomicUsize::new(0)),
            max_history_size: 1000,
        }
    }

    /// Add new performance data and evaluate training needs
    pub async fn evaluate_training_need(
        &self,
        performance: PerformanceSnapshot,
    ) -> Result<TrainingDecision> {
        // Add to performance history
        {
            let mut history = self.performance_history.write().await;
            history.push_back(performance.clone());
            if history.len() > self.max_history_size {
                history.pop_front();
            }
        }

        // Update consecutive failure count
        if performance.accuracy < self.config.accuracy_threshold {
            self.consecutive_failure_count
                .fetch_add(1, Ordering::Relaxed);
        } else {
            self.consecutive_failure_count.store(0, Ordering::Relaxed);
        }

        // Analyze current state and make decision
        self.make_training_decision(&performance).await
    }

    /// Core decision-making logic
    async fn make_training_decision(
        &self,
        current_performance: &PerformanceSnapshot,
    ) -> Result<TrainingDecision> {
        let decision_id = uuid::Uuid::new_v4().to_string();
        let mut reasoning = Vec::new();
        let mut confidence: f64 = 1.0;

        // Check time-based constraints
        let last_training = *self.last_training_time.read().await;
        let hours_since_training = (Utc::now() - last_training).num_hours();

        if hours_since_training < self.config.min_training_interval_hours {
            reasoning.push(format!(
                "Too soon since last training ({} hours < {} minimum)",
                hours_since_training, self.config.min_training_interval_hours
            ));
            return Ok(TrainingDecision {
                decision_id,
                timestamp: Utc::now(),
                decision_type: TrainingDecisionType::NoTraining {
                    reason: "Minimum training interval not met".to_string(),
                },
                confidence: 1.0,
                reasoning,
                performance_snapshot: current_performance.clone(),
                resource_requirements: ResourceRequirements::minimal(),
                estimated_duration: Duration::zero(),
                priority: TrainingPriority::Low,
                affected_models: Vec::new(),
            });
        }

        // Analyze performance trends
        let _performance_analysis = self.analyze_performance_trends().await?;
        let consecutive_failures = self.consecutive_failure_count.load(Ordering::Relaxed);

        // Emergency conditions
        if current_performance.accuracy < self.config.accuracy_threshold * 0.5
            || consecutive_failures >= self.config.consecutive_failures_threshold * 2
            || current_performance.max_drawdown > self.config.max_drawdown_threshold * 1.5
        {
            reasoning.push("Emergency: Severe performance degradation detected".to_string());
            reasoning.push(format!(
                "Accuracy: {:.3} (critical threshold: {:.3})",
                current_performance.accuracy,
                self.config.accuracy_threshold * 0.5
            ));
            reasoning.push(format!(
                "Consecutive failures: {} (emergency threshold: {})",
                consecutive_failures,
                self.config.consecutive_failures_threshold * 2
            ));

            return Ok(TrainingDecision {
                decision_id,
                timestamp: Utc::now(),
                decision_type: TrainingDecisionType::Emergency {
                    reason: "Critical performance degradation".to_string(),
                    urgency_score: 1.0,
                },
                confidence: 0.95,
                reasoning,
                performance_snapshot: current_performance.clone(),
                resource_requirements: ResourceRequirements::high_priority(),
                estimated_duration: Duration::hours(2),
                priority: TrainingPriority::Emergency,
                affected_models: vec!["all".to_string()],
            });
        }

        // Check individual trigger conditions
        let mut trigger_score = 0.0;
        let mut triggered_conditions = Vec::new();

        // Accuracy trigger
        if current_performance.accuracy < self.config.accuracy_threshold {
            let severity = (self.config.accuracy_threshold - current_performance.accuracy)
                / self.config.accuracy_threshold;
            trigger_score += severity * 0.3;
            triggered_conditions.push(format!(
                "Accuracy below threshold: {:.3} < {:.3}",
                current_performance.accuracy, self.config.accuracy_threshold
            ));
            confidence *= 0.95;
        }

        // Sharpe ratio trigger
        if current_performance.sharpe_ratio < self.config.sharpe_ratio_threshold {
            let severity = (self.config.sharpe_ratio_threshold - current_performance.sharpe_ratio)
                / self.config.sharpe_ratio_threshold;
            trigger_score += severity * 0.2;
            triggered_conditions.push(format!(
                "Sharpe ratio below threshold: {:.3} < {:.3}",
                current_performance.sharpe_ratio, self.config.sharpe_ratio_threshold
            ));
        }

        // Drawdown trigger
        if current_performance.max_drawdown > self.config.max_drawdown_threshold {
            let severity = (current_performance.max_drawdown - self.config.max_drawdown_threshold)
                / self.config.max_drawdown_threshold;
            trigger_score += severity * 0.25;
            triggered_conditions.push(format!(
                "Drawdown exceeds threshold: {:.3} > {:.3}",
                current_performance.max_drawdown, self.config.max_drawdown_threshold
            ));
        }

        // Consecutive failures trigger
        if consecutive_failures >= self.config.consecutive_failures_threshold {
            trigger_score += 0.4;
            triggered_conditions.push(format!(
                "Consecutive failures: {} >= {}",
                consecutive_failures, self.config.consecutive_failures_threshold
            ));
            confidence *= 0.9;
        }

        // Model disagreement trigger
        if current_performance.model_agreement < (1.0 - self.config.model_disagreement_threshold) {
            trigger_score += 0.15;
            triggered_conditions.push(format!(
                "High model disagreement: agreement {:.3} < {:.3}",
                current_performance.model_agreement,
                1.0 - self.config.model_disagreement_threshold
            ));
        }

        // Time-based trigger
        if hours_since_training > self.config.max_training_interval_hours {
            trigger_score += 0.3;
            triggered_conditions.push(format!(
                "Maximum training interval exceeded: {} hours > {}",
                hours_since_training, self.config.max_training_interval_hours
            ));
        }

        // Volatility trigger (market conditions changed)
        if current_performance.volatility > self.config.volatility_threshold {
            trigger_score += 0.1;
            triggered_conditions.push(format!(
                "High market volatility: {:.3} > {:.3}",
                current_performance.volatility, self.config.volatility_threshold
            ));
        }

        reasoning.extend(triggered_conditions);

        // Make decision based on trigger score
        let decision_type = if trigger_score >= 0.8 {
            TrainingDecisionType::FullRetraining {
                reason: "Multiple severe performance issues detected".to_string(),
                expected_improvement: trigger_score * 0.15, // Estimate 15% improvement per point
            }
        } else if trigger_score >= 0.5 {
            TrainingDecisionType::IncrementalTraining {
                reason: "Moderate performance degradation detected".to_string(),
                scope: "primary_models".to_string(),
            }
        } else if trigger_score >= 0.3 {
            TrainingDecisionType::FineTuning {
                reason: "Minor adjustments needed".to_string(),
                target_regime: MetricsAnalyzer::detect_market_regime(current_performance),
            }
        } else {
            TrainingDecisionType::NoTraining {
                reason: "Performance within acceptable ranges".to_string(),
            }
        };

        let (priority, resource_requirements, estimated_duration, affected_models) =
            match &decision_type {
                TrainingDecisionType::FullRetraining { .. } => (
                    TrainingPriority::High,
                    ResourceRequirements::full_training(),
                    Duration::hours(6),
                    vec!["NHITS".to_string(), "DeepAR".to_string(), "TCN".to_string()],
                ),
                TrainingDecisionType::IncrementalTraining { .. } => (
                    TrainingPriority::Medium,
                    ResourceRequirements::incremental(),
                    Duration::hours(2),
                    vec!["primary".to_string()],
                ),
                TrainingDecisionType::FineTuning { .. } => (
                    TrainingPriority::Low,
                    ResourceRequirements::fine_tuning(),
                    Duration::hours(1),
                    vec!["target_model".to_string()],
                ),
                _ => (
                    TrainingPriority::Low,
                    ResourceRequirements::minimal(),
                    Duration::zero(),
                    Vec::new(),
                ),
            };

        Ok(TrainingDecision {
            decision_id,
            timestamp: Utc::now(),
            decision_type,
            confidence: confidence.max(0.1).min(1.0),
            reasoning,
            performance_snapshot: current_performance.clone(),
            resource_requirements,
            estimated_duration,
            priority,
            affected_models,
        })
    }

    /// Analyze performance trends over time
    async fn analyze_performance_trends(&self) -> Result<PerformanceTrendAnalysis> {
        let history = self.performance_history.read().await;

        if history.len() < 5 {
            return Ok(PerformanceTrendAnalysis {
                accuracy_trend: super::metrics::PerformanceTrend::Stable,
                confidence_trend: super::metrics::PerformanceTrend::Stable,
                volatility_trend: super::metrics::PerformanceTrend::Stable,
                overall_trend: super::metrics::PerformanceTrend::Stable,
            });
        }

        let recent_window = 10;
        let recent_start = history.len().saturating_sub(recent_window);
        let recent_performance: Vec<&PerformanceSnapshot> =
            history.iter().skip(recent_start).collect();

        let accuracy_trend = MetricsAnalyzer::analyze_metric_trend(
            &recent_performance
                .iter()
                .map(|p| p.accuracy)
                .collect::<Vec<f64>>(),
        );

        let confidence_trend = MetricsAnalyzer::analyze_metric_trend(
            &recent_performance
                .iter()
                .map(|p| p.confidence)
                .collect::<Vec<f64>>(),
        );

        let volatility_trend = MetricsAnalyzer::analyze_metric_trend(
            &recent_performance
                .iter()
                .map(|p| p.volatility)
                .collect::<Vec<f64>>(),
        );

        let overall_trend = match (&accuracy_trend, &confidence_trend) {
            (super::metrics::PerformanceTrend::Degrading, _) | (_, super::metrics::PerformanceTrend::Degrading) => {
                super::metrics::PerformanceTrend::Degrading
            }
            (super::metrics::PerformanceTrend::Improving, super::metrics::PerformanceTrend::Improving) => {
                super::metrics::PerformanceTrend::Improving
            }
            (super::metrics::PerformanceTrend::Volatile, _) | (_, super::metrics::PerformanceTrend::Volatile) => {
                super::metrics::PerformanceTrend::Volatile
            }
            _ => super::metrics::PerformanceTrend::Stable,
        };

        Ok(PerformanceTrendAnalysis {
            accuracy_trend,
            confidence_trend,
            volatility_trend,
            overall_trend,
        })
    }

    /// Update last training time (called after successful training)
    pub async fn update_last_training_time(&self) {
        *self.last_training_time.write().await = Utc::now();
        self.consecutive_failure_count.store(0, Ordering::Relaxed);
    }

    /// Get current failure count
    pub fn get_consecutive_failure_count(&self) -> usize {
        self.consecutive_failure_count.load(Ordering::Relaxed)
    }

    /// Reset failure count
    pub fn reset_consecutive_failure_count(&self) {
        self.consecutive_failure_count.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_training_decision_logic() {
        let config = TrainingTriggerConfig::default();
        let evaluator = TrainingTriggerEvaluator::new(config);

        // Test performance that should trigger training
        let poor_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.6, // Below 0.7 threshold
            confidence: 0.5,
            price_error: 0.15,
            sharpe_ratio: 0.3, // Below 0.5 threshold
            max_drawdown: 0.2, // Above 0.15 threshold
            volatility: 0.03,
            model_agreement: 0.6,
            consecutive_failures: 6, // Above 5 threshold
            trading_volume: 1000000.0,
            profit_loss: -0.05,
        };

        let decision = evaluator
            .evaluate_training_need(poor_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::FullRetraining { .. }
            | TrainingDecisionType::Emergency { .. } => {
                assert!(decision.confidence > 0.8);
                assert!(!decision.reasoning.is_empty());
            }
            _ => panic!("Expected training to be triggered for poor performance"),
        }
    }

    #[tokio::test]
    async fn test_no_training_decision() {
        let config = TrainingTriggerConfig::default();
        let evaluator = TrainingTriggerEvaluator::new(config);

        // Test good performance that should not trigger training
        let good_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.85, // Above threshold
            confidence: 0.9,
            price_error: 0.05,
            sharpe_ratio: 0.8,  // Above threshold
            max_drawdown: 0.08, // Below threshold
            volatility: 0.02,
            model_agreement: 0.9,
            consecutive_failures: 1, // Below threshold
            trading_volume: 1000000.0,
            profit_loss: 0.03,
        };

        let decision = evaluator
            .evaluate_training_need(good_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::NoTraining { .. } => {
                assert!(decision
                    .reasoning
                    .iter()
                    .any(|r| r.contains("within acceptable ranges")));
            }
            _ => panic!("Expected no training for good performance"),
        }
    }

    #[tokio::test]
    async fn test_emergency_training_conditions() {
        let config = TrainingTriggerConfig::default();
        let evaluator = TrainingTriggerEvaluator::new(config);

        // Set up multiple failures to trigger emergency
        for _ in 0..12 {
            evaluator
                .consecutive_failure_count
                .fetch_add(1, Ordering::Relaxed);
        }

        let critical_performance = PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: 0.3, // Far below threshold
            confidence: 0.2,
            price_error: 0.25,
            sharpe_ratio: -0.5,
            max_drawdown: 0.4, // Very high
            volatility: 0.08,
            model_agreement: 0.3,
            consecutive_failures: 12,
            trading_volume: 1000000.0,
            profit_loss: -0.15,
        };

        let decision = evaluator
            .evaluate_training_need(critical_performance)
            .await
            .unwrap();

        match decision.decision_type {
            TrainingDecisionType::Emergency { urgency_score, .. } => {
                assert_eq!(urgency_score, 1.0);
                assert_eq!(decision.priority, TrainingPriority::Emergency);
            }
            _ => panic!("Expected emergency training for critical performance"),
        }
    }
}