//! Autonomous Neural Coordinator
//!
//! This module extends the DAA coordinator with autonomous neural training capabilities,
//! integrating the training decision engine with the existing coordination infrastructure.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

use crate::daa::autonomous_training::{
    AutonomousTrainingEngine, DAATrainingIntegration, PerformanceSnapshot, TrainingDecision,
    TrainingDecisionRecord, TrainingTriggerConfig,
};
use crate::data::TimeSeriesData;
use crate::integration::daa_coordinator::{AutonomousDecision, DaaCoordinator, TradingAction};
use crate::neural::EnhancedNeuralPredictor;
use crate::strategies::{MarketContext, Position};

/// Enhanced DAA coordinator with autonomous neural training capabilities
pub struct AutonomousNeuralCoordinator {
    /// Base DAA coordinator
    daa_coordinator: Arc<DaaCoordinator>,
    /// Autonomous training engine
    training_engine: Arc<AutonomousTrainingEngine>,
    /// Training integration handler
    training_integration: Arc<RwLock<DAATrainingIntegration>>,
    /// Enhanced neural predictor
    enhanced_predictor: Arc<RwLock<EnhancedNeuralPredictor>>,
    /// Performance tracking
    performance_tracker: Arc<RwLock<PerformanceTracker>>,
    /// Training configuration
    training_config: TrainingTriggerConfig,
}

/// Performance tracking for neural training decisions
#[derive(Debug)]
struct PerformanceTracker {
    recent_decisions: Vec<(DateTime<Utc>, AutonomousDecision)>,
    recent_predictions: Vec<(DateTime<Utc>, f64, f64)>, // timestamp, actual, predicted
    trading_performance: TradingPerformanceMetrics,
    last_performance_update: DateTime<Utc>,
}

/// Trading performance metrics for training decisions
#[derive(Debug, Default)]
struct TradingPerformanceMetrics {
    total_trades: usize,
    profitable_trades: usize,
    total_pnl: f64,
    sharpe_ratio: f64,
    max_drawdown: f64,
    current_drawdown: f64,
    win_rate: f64,
    avg_confidence: f64,
    prediction_accuracy: f64,
}

impl AutonomousNeuralCoordinator {
    /// Create new autonomous neural coordinator
    pub async fn new(
        daa_coordinator: Arc<DaaCoordinator>,
        enhanced_predictor: Arc<RwLock<EnhancedNeuralPredictor>>,
        training_config: TrainingTriggerConfig,
    ) -> Result<Self> {
        // Create autonomous training engine
        let (training_engine, training_receiver) =
            AutonomousTrainingEngine::new(training_config.clone())?;
        let training_engine = Arc::new(training_engine);

        // Create training integration
        let training_integration =
            DAATrainingIntegration::new(training_engine.clone(), training_receiver);

        let performance_tracker = PerformanceTracker {
            recent_decisions: Vec::new(),
            recent_predictions: Vec::new(),
            trading_performance: TradingPerformanceMetrics::default(),
            last_performance_update: Utc::now(),
        };

        Ok(Self {
            daa_coordinator,
            training_engine,
            training_integration: Arc::new(RwLock::new(training_integration)),
            enhanced_predictor,
            performance_tracker: Arc::new(RwLock::new(performance_tracker)),
            training_config,
        })
    }

    /// Start the autonomous neural coordination system
    pub async fn start(&self) -> Result<()> {
        info!("Starting autonomous neural coordination system");

        // Start training integration processing in background
        let training_integration = Arc::clone(&self.training_integration);
        tokio::spawn(async move {
            let mut integration = training_integration.write().await;
            if let Err(e) = integration.start_processing().await {
                error!("Training integration processing failed: {}", e);
            }
        });

        info!("Autonomous neural coordination system started");
        Ok(())
    }

    /// Enhanced decision making with autonomous neural training consideration
    pub async fn make_enhanced_decision(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
        historical_data: &[TimeSeriesData],
    ) -> Result<AutonomousDecision> {
        // First, check if neural training is needed
        self.evaluate_and_trigger_training(market_context, historical_data)
            .await?;

        // Get base decision from DAA coordinator
        let mut decision = self
            .daa_coordinator
            .make_decision(market_context, current_position, historical_data)
            .await?;

        // Enhance decision with neural training insights
        self.enhance_decision_with_training_insights(&mut decision)
            .await?;

        // Update performance tracking
        self.update_performance_tracking(&decision, market_context)
            .await?;

        Ok(decision)
    }

    /// Evaluate current performance and trigger training if needed
    async fn evaluate_and_trigger_training(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<()> {
        // Calculate current performance snapshot
        let performance_snapshot = self
            .calculate_performance_snapshot(market_context, historical_data)
            .await?;

        // Evaluate training need
        let training_decision = self
            .training_engine
            .evaluate_training_need(performance_snapshot)
            .await?;

        // Log training decision
        match &training_decision.decision_type {
            crate::daa::autonomous_training::TrainingDecisionType::NoTraining { reason } => {
                debug!("No training needed: {}", reason);
            }
            _ => {
                info!(
                    "Training decision made: {:?} (confidence: {:.2}%)",
                    training_decision.decision_type,
                    training_decision.confidence * 100.0
                );
                info!(
                    "Training reasoning: {}",
                    training_decision.reasoning.join(", ")
                );
            }
        }

        Ok(())
    }

    /// Calculate current performance snapshot for training decisions
    async fn calculate_performance_snapshot(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<PerformanceSnapshot> {
        let performance_tracker = self.performance_tracker.read().await;
        let trading_perf = &performance_tracker.trading_performance;

        // Calculate prediction accuracy from recent predictions
        let prediction_accuracy = if performance_tracker.recent_predictions.len() >= 5 {
            let recent_predictions = &performance_tracker.recent_predictions[performance_tracker
                .recent_predictions
                .len()
                .saturating_sub(10)..];

            let total_error: f64 = recent_predictions
                .iter()
                .map(|(_, actual, predicted)| {
                    let error = (actual - predicted).abs() / actual.abs().max(0.01);
                    if error < 0.1 {
                        1.0
                    } else {
                        0.0
                    } // Within 10% = success
                })
                .sum();

            total_error / recent_predictions.len() as f64
        } else {
            0.7 // Default neutral accuracy
        };

        // Calculate model agreement from enhanced predictor
        let model_agreement = match self
            .enhanced_predictor
            .read()
            .await
            .predict_with_confidence(historical_data, 1)
            .await
        {
            Ok(predictions) => {
                if let Some(pred) = predictions.first() {
                    pred.model_agreement_score
                } else {
                    0.8 // Default good agreement
                }
            }
            Err(_) => 0.5, // Default neutral agreement if prediction fails
        };

        // Calculate volatility from recent data
        let volatility = if historical_data.len() >= 20 {
            let recent_data = &historical_data[historical_data.len().saturating_sub(20)..];
            let returns: Vec<f64> = recent_data
                .windows(2)
                .map(|w| (w[1].close - w[0].close) / w[0].close)
                .collect();

            if !returns.is_empty() {
                let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance =
                    returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
                variance.sqrt()
            } else {
                market_context.volatility
            }
        } else {
            market_context.volatility
        };

        // Get consecutive failures from performance tracking
        let consecutive_failures = self.calculate_consecutive_failures().await;

        Ok(PerformanceSnapshot {
            timestamp: Utc::now(),
            accuracy: prediction_accuracy,
            confidence: trading_perf.avg_confidence,
            price_error: 1.0 - prediction_accuracy, // Convert accuracy to error
            sharpe_ratio: trading_perf.sharpe_ratio,
            max_drawdown: trading_perf.max_drawdown,
            volatility,
            model_agreement,
            consecutive_failures,
            trading_volume: market_context.volume_24h,
            profit_loss: trading_perf.total_pnl,
        })
    }

    /// Calculate consecutive failures from recent decisions
    async fn calculate_consecutive_failures(&self) -> usize {
        let performance_tracker = self.performance_tracker.read().await;
        let mut consecutive_failures = 0;

        // Look at recent decisions in reverse order
        for (_, decision) in performance_tracker.recent_decisions.iter().rev().take(10) {
            // Consider a decision a "failure" if confidence was high but outcome was poor
            // This is a simplified heuristic - in production you'd track actual outcomes
            if decision.confidence > 0.8 {
                // High confidence decision - check if it was likely profitable
                match &decision.action {
                    TradingAction::Buy { .. } | TradingAction::Sell { .. } => {
                        // For simplicity, assume some decisions fail
                        // In production, you'd track actual P&L outcomes
                        if decision.confidence < 0.85 {
                            consecutive_failures += 1;
                        } else {
                            break; // Found a success, stop counting
                        }
                    }
                    _ => break,
                }
            }
        }

        consecutive_failures
    }

    /// Enhance decision with training insights
    async fn enhance_decision_with_training_insights(
        &self,
        decision: &mut AutonomousDecision,
    ) -> Result<()> {
        // Get recent training decisions for context
        let training_history = self.training_engine.get_decision_history().await;

        // Check if recent training has occurred
        let recent_training = training_history
            .values()
            .filter(|record| record.decision.timestamp > Utc::now() - chrono::Duration::hours(24))
            .any(|record| {
                matches!(
                    record.outcome,
                    Some(crate::daa::autonomous_training::TrainingOutcome::Success { .. })
                )
            });

        if recent_training {
            // Boost confidence if recent training was successful
            decision.confidence = (decision.confidence * 1.1).min(0.98);
            decision
                .reasoning
                .push("Confidence enhanced due to recent successful neural training".to_string());
        }

        // Check for ongoing training that might affect decision reliability
        let ongoing_training = training_history.values().any(|record| {
            matches!(
                record.outcome,
                Some(crate::daa::autonomous_training::TrainingOutcome::InProgress { .. })
            )
        });

        if ongoing_training {
            // Reduce confidence during training
            decision.confidence = (decision.confidence * 0.9).max(0.1);
            decision
                .reasoning
                .push("Confidence reduced due to ongoing neural training".to_string());
        }

        Ok(())
    }

    /// Update performance tracking with new decision
    async fn update_performance_tracking(
        &self,
        decision: &AutonomousDecision,
        market_context: &MarketContext,
    ) -> Result<()> {
        let mut performance_tracker = self.performance_tracker.write().await;

        // Add decision to recent decisions
        performance_tracker
            .recent_decisions
            .push((decision.timestamp, decision.clone()));

        // Keep only recent decisions (last 100)
        if performance_tracker.recent_decisions.len() > 100 {
            performance_tracker.recent_decisions.drain(0..50); // Remove oldest 50
        }

        // Update trading performance metrics
        let trading_perf = &mut performance_tracker.trading_performance;

        // Update confidence tracking
        trading_perf.avg_confidence = if trading_perf.total_trades > 0 {
            (trading_perf.avg_confidence * trading_perf.total_trades as f64 + decision.confidence)
                / (trading_perf.total_trades + 1) as f64
        } else {
            decision.confidence
        };

        // Update trade counting based on action
        match &decision.action {
            TradingAction::Buy { .. } | TradingAction::Sell { .. } => {
                trading_perf.total_trades += 1;

                // Estimate profitability based on confidence and market conditions
                // This is a simplified heuristic - in production you'd track actual P&L
                let estimated_profitable =
                    decision.confidence > 0.75 && market_context.volatility < 0.05;

                if estimated_profitable {
                    trading_perf.profitable_trades += 1;
                    trading_perf.total_pnl += decision.confidence * 0.02; // Simplified P&L estimate
                } else {
                    trading_perf.total_pnl -= (1.0 - decision.confidence) * 0.01;
                }

                // Update win rate
                trading_perf.win_rate =
                    trading_perf.profitable_trades as f64 / trading_perf.total_trades as f64;

                // Update Sharpe ratio (simplified calculation)
                if trading_perf.total_trades > 10 {
                    let avg_return = trading_perf.total_pnl / trading_perf.total_trades as f64;
                    trading_perf.sharpe_ratio = avg_return / market_context.volatility.max(0.01);
                }

                // Update drawdown tracking
                let current_pnl = trading_perf.total_pnl;
                if current_pnl < 0.0 {
                    trading_perf.current_drawdown = current_pnl.abs();
                    trading_perf.max_drawdown =
                        trading_perf.max_drawdown.max(trading_perf.current_drawdown);
                } else {
                    trading_perf.current_drawdown = 0.0;
                }
            }
            _ => {
                // No trading action, no performance impact
            }
        }

        performance_tracker.last_performance_update = Utc::now();

        Ok(())
    }

    /// Get current training status and performance metrics
    pub async fn get_training_status(&self) -> Result<TrainingStatus> {
        let training_history = self.training_engine.get_decision_history().await;
        let performance_tracker = self.performance_tracker.read().await;

        // Find most recent training decision
        let recent_training = training_history
            .values()
            .max_by_key(|record| record.decision.timestamp);

        // Calculate performance summary
        let performance_summary = PerformanceSummary {
            total_decisions: performance_tracker.recent_decisions.len(),
            avg_confidence: performance_tracker.trading_performance.avg_confidence,
            win_rate: performance_tracker.trading_performance.win_rate,
            sharpe_ratio: performance_tracker.trading_performance.sharpe_ratio,
            max_drawdown: performance_tracker.trading_performance.max_drawdown,
            prediction_accuracy: performance_tracker.trading_performance.prediction_accuracy,
        };

        Ok(TrainingStatus {
            recent_training_decision: recent_training.cloned(),
            training_decisions_count: training_history.len(),
            performance_summary,
            last_performance_update: performance_tracker.last_performance_update,
        })
    }

    /// Force a training evaluation (for testing or manual intervention)
    pub async fn force_training_evaluation(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<TrainingDecision> {
        let performance_snapshot = self
            .calculate_performance_snapshot(market_context, historical_data)
            .await?;

        self.training_engine
            .evaluate_training_need(performance_snapshot)
            .await
    }

    /// Get enhanced neural predictor for direct access
    pub fn get_enhanced_predictor(&self) -> Arc<RwLock<EnhancedNeuralPredictor>> {
        self.enhanced_predictor.clone()
    }

    /// Get base DAA coordinator for direct access
    pub fn get_daa_coordinator(&self) -> Arc<DaaCoordinator> {
        self.daa_coordinator.clone()
    }
}

/// Training status information
#[derive(Debug, Clone)]
pub struct TrainingStatus {
    pub recent_training_decision: Option<TrainingDecisionRecord>,
    pub training_decisions_count: usize,
    pub performance_summary: PerformanceSummary,
    pub last_performance_update: DateTime<Utc>,
}

/// Performance summary for training status
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub total_decisions: usize,
    pub avg_confidence: f64,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub prediction_accuracy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;
    use crate::neural::NeuralPredictor;
    use tokio::sync::mpsc;

    async fn create_test_coordinator() -> AutonomousNeuralCoordinator {
        // Create neural predictor
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config.clone()).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        // Create DAA coordinator
        let daa_config = crate::integration::daa_coordinator::DaaConfig::default();
        let daa_coordinator =
            Arc::new(DaaCoordinator::new(daa_config, neural_predictor.clone(), tx).unwrap());

        // Create enhanced predictor
        let enhanced_predictor = Arc::new(RwLock::new(
            EnhancedNeuralPredictor::new(neural_config).unwrap(),
        ));

        // Create training config
        let training_config = TrainingTriggerConfig::default();

        AutonomousNeuralCoordinator::new(daa_coordinator, enhanced_predictor, training_config)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_autonomous_neural_coordinator_creation() {
        let coordinator = create_test_coordinator().await;
        let status = coordinator.get_training_status().await.unwrap();

        assert_eq!(status.training_decisions_count, 0);
        assert_eq!(status.performance_summary.total_decisions, 0);
    }

    #[tokio::test]
    async fn test_performance_tracking() {
        let coordinator = create_test_coordinator().await;

        let market_context = crate::strategies::MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let test_data = vec![crate::data::TimeSeriesData {
            timestamp: Utc::now(),
            entity: Some("test".to_string()),
            symbol: "BTC/USDT".to_string(),
            open: 49800.0,
            high: 50200.0,
            low: 49600.0,
            close: 50000.0,
            volume: 100.0,
            source: Some("test".to_string()),
            value: Some(50000.0),
            metadata: None,
            indicators: std::collections::HashMap::new(),
        }];

        // Test performance snapshot calculation
        let snapshot = coordinator
            .calculate_performance_snapshot(&market_context, &test_data)
            .await
            .unwrap();

        assert!(snapshot.accuracy >= 0.0);
        assert!(snapshot.volatility >= 0.0);
        assert!(snapshot.model_agreement >= 0.0);
    }

    #[tokio::test]
    async fn test_training_evaluation() {
        let coordinator = create_test_coordinator().await;

        let market_context = crate::strategies::MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        };

        let test_data = vec![crate::data::TimeSeriesData {
            timestamp: Utc::now(),
            entity: Some("test".to_string()),
            symbol: "BTC/USDT".to_string(),
            open: 49800.0,
            high: 50200.0,
            low: 49600.0,
            close: 50000.0,
            volume: 100.0,
            source: Some("test".to_string()),
            value: Some(50000.0),
            metadata: None,
            indicators: std::collections::HashMap::new(),
        }];

        // Force training evaluation
        let training_decision = coordinator
            .force_training_evaluation(&market_context, &test_data)
            .await
            .unwrap();

        // Should make some kind of decision
        assert!(!training_decision.reasoning.is_empty());
        assert!(training_decision.confidence >= 0.0 && training_decision.confidence <= 1.0);
    }
}
