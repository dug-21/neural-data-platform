//! DAA Agent Management and Coordination
//!
//! Manages autonomous agents and their coordination within the DAA system.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot};
use crate::data::TimeSeriesData;
use crate::strategies::MarketContext;

use super::config::{DaaConfig, PerformanceMetrics};
use super::decisions::AutonomousDecision;

/// Agent management functionality for DAA Coordinator
pub struct AgentManager {
    config: DaaConfig,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    autonomous_training: Option<Arc<AutonomousTrainingEngine>>,
    last_retrain_check: Arc<RwLock<DateTime<Utc>>>,
    autonomous_retraining_enabled: bool,
}

impl AgentManager {
    pub fn new(config: DaaConfig) -> Self {
        Self {
            config,
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            autonomous_training: None,
            last_retrain_check: Arc::new(RwLock::new(Utc::now())),
            autonomous_retraining_enabled: true,
        }
    }

    /// Update performance metrics and trigger retraining evaluation
    pub async fn update_metrics(&self, decision: &AutonomousDecision) {
        let mut metrics = self.performance_metrics.write().await;

        metrics.total_decisions += 1;
        metrics.avg_confidence = (metrics.avg_confidence * (metrics.total_decisions - 1) as f64
            + decision.confidence)
            / metrics.total_decisions as f64;

        // Update model accuracy tracking
        for (model, signal) in &decision.neural_consensus {
            let current_accuracy = metrics.model_accuracy.get(model).unwrap_or(&0.5);
            // Simple exponential moving average for accuracy
            let updated_accuracy = current_accuracy * 0.9 + signal.abs() * 0.1;
            metrics
                .model_accuracy
                .insert(model.clone(), updated_accuracy);
        }
    }

    /// Check if retraining is needed and trigger autonomously if enabled
    pub async fn check_and_trigger_retraining(&self) -> Result<()> {
        if !self.autonomous_retraining_enabled {
            return Ok(());
        }

        let now = Utc::now();
        let mut last_check = self.last_retrain_check.write().await;

        // Only check every hour to avoid excessive overhead
        if now - *last_check < chrono::Duration::hours(1) {
            return Ok(());
        }

        *last_check = now;
        drop(last_check);

        // Check if retraining is needed based on performance metrics
        let metrics = self.performance_metrics.read().await;
        let should_retrain = metrics.model_accuracy.values()
            .any(|&accuracy| accuracy < 0.7) || metrics.win_rate < 0.45;
        drop(metrics);

        if should_retrain {
            info!(
                "DAA triggering autonomous retraining due to low performance"
            );

            // Execute autonomous retraining
            Self::execute_autonomous_retraining(1.0).await?;
        }

        Ok(())
    }

    /// Execute the actual retraining process
    pub async fn execute_autonomous_retraining(urgency_score: f64) -> Result<()> {
        // Record training start
        let training_start = std::time::Instant::now();

        // Simulate training process based on urgency
        let training_duration = if urgency_score > 0.8 {
            // High urgency - quick training
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            "fast_retrain"
        } else if urgency_score > 0.5 {
            // Medium urgency - standard training
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            "standard_retrain"
        } else {
            // Low urgency - comprehensive training
            tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
            "comprehensive_retrain"
        };

        info!(
            "Completed {} in {:.2}s",
            training_duration,
            training_start.elapsed().as_secs_f64()
        );

        Ok(())
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }

    /// Enable or disable autonomous retraining
    pub fn set_autonomous_retraining(&mut self, enabled: bool) {
        self.autonomous_retraining_enabled = enabled;
        info!(
            "Autonomous retraining {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Force manual retraining (for testing or manual intervention)
    pub async fn force_retraining(&self) -> Result<()> {
        info!("Manual retraining triggered");
        Self::execute_autonomous_retraining(1.0).await?;
        Ok(())
    }

    /// Set autonomous training engine for enhanced training decisions
    pub fn set_autonomous_training(&mut self, training_engine: Arc<AutonomousTrainingEngine>) {
        self.autonomous_training = Some(training_engine);
        info!("Autonomous training engine integrated with DAA coordinator");
    }

    /// Evaluate training need using autonomous training engine
    pub async fn evaluate_autonomous_training(
        &self,
        market_context: &MarketContext,
        _historical_data: &[TimeSeriesData],
    ) -> Result<()> {
        if let Some(training_engine) = &self.autonomous_training {
            // Calculate performance snapshot from current state
            let metrics = self.performance_metrics.read().await;

            let performance_snapshot = PerformanceSnapshot {
                timestamp: Utc::now(),
                accuracy: metrics.avg_confidence, // Use average confidence as proxy for accuracy
                confidence: metrics.avg_confidence,
                price_error: 1.0 - metrics.avg_confidence, // Convert confidence to error
                sharpe_ratio: metrics.sharpe_ratio,
                max_drawdown: metrics.max_drawdown,
                volatility: market_context.volatility,
                model_agreement: 0.8, // Default value - would be calculated from ensemble
                consecutive_failures: 0, // Would be tracked separately
                trading_volume: market_context.volume_24h,
                profit_loss: metrics.total_pnl,
            };

            // Evaluate training need
            let _training_decision = training_engine
                .evaluate_training_need(performance_snapshot)
                .await?;

            // Training decision is automatically sent to DAA via the engine's channel
        }

        Ok(())
    }

    /// Adapt parameters based on decision and market context
    pub async fn adapt_parameters(
        &self,
        decision: &AutonomousDecision,
        market_context: &MarketContext,
    ) -> Result<HashMap<String, f64>> {
        let mut adapted_params = HashMap::new();

        // Adapt confidence threshold based on recent performance
        let metrics = self.performance_metrics.read().await;
        let base_confidence = self.config.min_confidence;
        
        // Increase confidence threshold if recent accuracy is low
        let accuracy_penalty = if metrics.avg_confidence < 0.6 { 0.1 } else { 0.0 };
        let adapted_confidence = (base_confidence + accuracy_penalty).min(0.95);
        adapted_params.insert("min_confidence".to_string(), adapted_confidence);

        // Adapt position size based on volatility
        let volatility_factor = 1.0 / (1.0 + market_context.volatility * 5.0);
        let adapted_size = self.config.max_risk_per_trade * volatility_factor;
        adapted_params.insert("max_risk_per_trade".to_string(), adapted_size);

        // Adapt model weights based on recent performance
        for (model, accuracy) in &metrics.model_accuracy {
            let current_weight = self.config.model_weights.get(model).unwrap_or(&1.0);
            let performance_factor = if *accuracy > 0.8 { 1.1 } else { 0.9 };
            let adapted_weight = current_weight * performance_factor;
            adapted_params.insert(format!("model_weight_{}", model), adapted_weight);
        }

        info!("Adapted {} parameters based on performance", adapted_params.len());
        Ok(adapted_params)
    }

    /// Get enhanced predictor performance metrics
    pub async fn get_enhanced_performance_metrics(
        &self,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut metrics = HashMap::new();
        let perf_metrics = self.performance_metrics.read().await;
        
        metrics.insert("recent_accuracy".to_string(), serde_json::Value::from(perf_metrics.avg_confidence));
        metrics.insert("prediction_count".to_string(), serde_json::Value::from(perf_metrics.total_decisions));
        metrics.insert("average_confidence".to_string(), serde_json::Value::from(perf_metrics.avg_confidence));
        metrics.insert("win_rate".to_string(), serde_json::Value::from(perf_metrics.win_rate));
        metrics.insert("sharpe_ratio".to_string(), serde_json::Value::from(perf_metrics.sharpe_ratio));
        
        Ok(metrics)
    }
}