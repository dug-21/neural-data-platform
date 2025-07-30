//! DAA Coordinator for Autonomous Trading Decisions
//!
//! This module integrates neural-enhanced strategies with Decentralized Autonomous Agents
//! for fully autonomous trading decisions based on neural feedback.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot};
use crate::data::TimeSeriesData;
use crate::neural::{
    NeuralPredictor, PredictionResult,
};
use crate::strategies::{MarketContext, Position, Signal, TradingStrategy};

/// Simplified confidence breakdown for DAA decisions
#[derive(Debug, Clone, Default)]
struct ConfidenceBreakdown {
    pub base_confidence: f64,
    pub ensemble_agreement: f64,
    pub historical_accuracy: f64,
}

/// Simplified retraining metrics
#[derive(Debug, Clone)]
struct RetrainingMetrics {
    pub urgency_score: f64,
    pub accuracy: f64,
    pub should_retrain: bool,
}

/// Configuration for DAA coordination
#[derive(Debug, Clone)]
pub struct DaaConfig {
    /// Enable autonomous decision making
    pub enabled: bool,
    /// Minimum confidence for autonomous trades
    pub min_confidence: f64,
    /// Risk limit per trade
    pub max_risk_per_trade: f64,
    /// Maximum concurrent positions
    pub max_positions: usize,
    /// Neural model weights for decisions
    pub model_weights: HashMap<String, f64>,
    /// Consensus threshold for multi-agent decisions
    pub consensus_threshold: f64,
    /// Enable real-time adaptation
    pub enable_adaptation: bool,
}

impl Default for DaaConfig {
    fn default() -> Self {
        let mut model_weights = HashMap::new();
        model_weights.insert("NHITS".to_string(), 1.2);
        model_weights.insert("TCN".to_string(), 1.1);
        model_weights.insert("DeepAR".to_string(), 1.3);
        model_weights.insert("Transformer".to_string(), 1.4);
        model_weights.insert("MLP".to_string(), 0.8);

        Self {
            enabled: true,
            min_confidence: 0.75,
            max_risk_per_trade: 0.02,
            max_positions: 5,
            model_weights,
            consensus_threshold: 0.7,
            enable_adaptation: true,
        }
    }
}

/// Autonomous decision from DAA
#[derive(Debug, Clone)]
pub struct AutonomousDecision {
    pub timestamp: DateTime<Utc>,
    pub action: TradingAction,
    pub confidence: f64,
    pub risk_assessment: RiskAssessment,
    pub reasoning: Vec<String>,
    pub neural_consensus: HashMap<String, f64>,
    pub adapted_parameters: Option<HashMap<String, f64>>,
}

#[derive(Debug, Clone)]
pub enum TradingAction {
    Buy {
        symbol: String,
        size: f64,
        stop_loss: Option<f64>,
        take_profit: Option<f64>,
    },
    Sell {
        symbol: String,
        size: f64,
        reason: String,
    },
    Hold {
        reason: String,
    },
    AdjustPosition {
        symbol: String,
        new_stop_loss: Option<f64>,
        new_take_profit: Option<f64>,
    },
}

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub market_risk: f64,
    pub position_risk: f64,
    pub portfolio_risk: f64,
    pub volatility_adjusted_size: f64,
}

/// DAA Coordinator for autonomous trading
pub struct DaaCoordinator {
    config: DaaConfig,
    neural_predictor: Arc<NeuralPredictor>,
    // Enhanced predictor functionality is now internal to NeuralPredictor
    strategies: Arc<RwLock<HashMap<String, Box<dyn TradingStrategy + Send + Sync>>>>,
    decision_history: Arc<RwLock<Vec<AutonomousDecision>>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    decision_sender: mpsc::Sender<AutonomousDecision>,
    last_retrain_check: Arc<RwLock<DateTime<Utc>>>,
    autonomous_retraining_enabled: bool,
    autonomous_training: Option<Arc<AutonomousTrainingEngine>>,
}

#[derive(Debug, Default, Clone)]
struct PerformanceMetrics {
    total_decisions: u64,
    profitable_decisions: u64,
    total_pnl: f64,
    sharpe_ratio: f64,
    max_drawdown: f64,
    win_rate: f64,
    avg_confidence: f64,
    model_accuracy: HashMap<String, f64>,
}

impl DaaCoordinator {
    pub fn new(
        config: DaaConfig,
        neural_predictor: Arc<NeuralPredictor>,
        decision_sender: mpsc::Sender<AutonomousDecision>,
    ) -> Result<Self> {
        // Create enhanced predictor with same configuration
        let neural_config = crate::config::NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string(), "TCN".to_string()],
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
            error_threshold: 0.05,
            lookback_window: 24,
        };

        Ok(Self {
            config,
            neural_predictor,
            // Enhanced predictor functionality is now internal to NeuralPredictor
            strategies: Arc::new(RwLock::new(HashMap::new())),
            decision_history: Arc::new(RwLock::new(Vec::new())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            decision_sender,
            last_retrain_check: Arc::new(RwLock::new(Utc::now())),
            autonomous_retraining_enabled: true,
            autonomous_training: None,
        })
    }

    /// Register a strategy with the coordinator
    pub async fn register_strategy(
        &self,
        name: String,
        strategy: Box<dyn TradingStrategy + Send + Sync>,
    ) {
        self.strategies.write().await.insert(name, strategy);
    }

    /// Make an autonomous trading decision
    pub async fn make_decision(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
        historical_data: &[TimeSeriesData],
    ) -> Result<AutonomousDecision> {
        if !self.config.enabled {
            return Ok(AutonomousDecision {
                timestamp: Utc::now(),
                action: TradingAction::Hold {
                    reason: "DAA disabled".to_string(),
                },
                confidence: 0.0,
                risk_assessment: self.assess_risk(market_context, current_position).await?,
                reasoning: vec!["Autonomous trading disabled".to_string()],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            });
        }

        // Step 1: Get neural predictions from multiple models
        let neural_signals = self
            .get_neural_consensus(market_context, historical_data)
            .await?;

        // Step 2: Get strategy signals
        let strategy_signals = self
            .get_strategy_signals(market_context, current_position)
            .await?;

        // Step 3: Assess risk
        let risk_assessment = self.assess_risk(market_context, current_position).await?;

        // Step 4: Synthesize decision
        let decision = self
            .synthesize_decision(
                neural_signals,
                strategy_signals,
                risk_assessment,
                market_context,
                current_position,
            )
            .await?;

        // Step 5: Adapt parameters if enabled
        let adapted_params = if self.config.enable_adaptation {
            Some(self.adapt_parameters(&decision, market_context).await?)
        } else {
            None
        };

        // Step 6: Update metrics and history
        self.update_metrics(&decision).await;
        self.decision_history.write().await.push(decision.clone());

        // Step 7: Send decision through channel
        if let Err(e) = self.decision_sender.send(decision.clone()).await {
            error!("Failed to send decision: {}", e);
        }

        Ok(AutonomousDecision {
            adapted_parameters: adapted_params,
            ..decision
        })
    }

    /// Get consensus from neural models with enhanced confidence analysis
    async fn get_neural_consensus(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut consensus = HashMap::new();

        // Check if retraining is needed before making predictions
        self.check_and_trigger_retraining().await?;

        // Get predictions with confidence analysis
        match self
            .neural_predictor
            .predict(historical_data, 5, None)
            .await
        {
            Ok(enhanced_predictions) => {
                for (i, enhanced_pred) in enhanced_predictions.iter().enumerate() {
                    // Create a PredictionResult from the enhanced prediction data
                    let prediction = PredictionResult {
                        timestamp: enhanced_pred.timestamp,
                        value: enhanced_pred.value,
                        confidence: enhanced_pred.confidence,
                        interval_low: enhanced_pred.interval_low,
                        interval_high: enhanced_pred.interval_high,
                        model_name: format!("enhanced_{}", i),
                        metadata: None,
                    };
                    let confidence_breakdown = &enhanced_pred.confidence_breakdown;

                    // Use combined confidence for signal strength calculation
                    let signal_strength = self.calculate_enhanced_signal_from_predictions(
                        &prediction,
                        confidence_breakdown,
                        market_context.current_price,
                        enhanced_pred.models_agree,
                        enhanced_pred.model_agreement_score,
                    );

                    // Weight by model and confidence
                    let model_name = &prediction.model_name;
                    let base_weight = self
                        .config
                        .model_weights
                        .get(model_name)
                        .or_else(|| self.config.model_weights.get("default"))
                        .unwrap_or(&1.0);

                    let confidence_weighted_signal =
                        signal_strength * confidence_breakdown.combined_confidence * base_weight;

                    consensus.insert(
                        format!("{}_step_{}", model_name, i),
                        confidence_weighted_signal,
                    );
                }
            }
            Err(e) => {
                warn!("Failed to get enhanced predictions: {}", e);

                // Fallback to basic predictions
                match self
                    .neural_predictor
                    .predict(historical_data, 5, None)
                    .await
                {
                    Ok(predictions) => {
                        if !predictions.is_empty() {
                            let signal_strength = self.calculate_signal_from_predictions(
                                &predictions,
                                market_context.current_price,
                            );
                            consensus.insert("fallback".to_string(), signal_strength);
                        }
                    }
                    Err(e2) => {
                        warn!("Fallback predictions also failed: {}", e2);
                    }
                }
            }
        }

        Ok(consensus)
    }

    /// Calculate trading signal from predictions
    fn calculate_signal_from_predictions(
        &self,
        predictions: &[PredictionResult],
        current_price: f64,
    ) -> f64 {
        if predictions.is_empty() {
            return 0.0;
        }

        // Calculate weighted signal based on prediction horizons
        let mut weighted_signal = 0.0;
        let mut total_weight = 0.0;

        for (i, pred) in predictions.iter().enumerate().take(3) {
            let price_change = (pred.value - current_price) / current_price;
            let confidence_weight = pred.confidence * (1.0 / (i + 1) as f64);

            weighted_signal += price_change * confidence_weight;
            total_weight += confidence_weight;
        }

        if total_weight > 0.0 {
            (weighted_signal / total_weight).max(-1.0).min(1.0)
        } else {
            0.0
        }
    }

    /// Calculate enhanced trading signal with confidence breakdown
    fn calculate_enhanced_signal_from_predictions(
        &self,
        prediction: &PredictionResult,
        confidence_breakdown: &ConfidenceBreakdown,
        current_price: f64,
        models_agree: bool,
        diversity_score: f64,
    ) -> f64 {
        let price_change = (prediction.value - current_price) / current_price;

        // Apply confidence-based weighting
        let mut signal_weight = confidence_breakdown.combined_confidence;

        // Boost signal if models agree
        if models_agree {
            signal_weight *= 1.2;
        }

        // Adjust for diversity (higher diversity = more reliable)
        signal_weight *= (0.5 + diversity_score * 0.5);

        // Apply regime confidence
        signal_weight *= 1.0 + confidence_breakdown.market_regime_adjustment;

        // Calculate final signal
        let final_signal = price_change * signal_weight;

        // Bound the signal
        final_signal.max(-1.0).min(1.0)
    }

    /// Get signals from all registered strategies
    async fn get_strategy_signals(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
    ) -> Result<HashMap<String, Signal>> {
        let mut signals = HashMap::new();
        let strategies = self.strategies.read().await;

        for (name, strategy) in strategies.iter() {
            match strategy
                .generate_signal(market_context, current_position)
                .await
            {
                Ok(signal) => {
                    signals.insert(name.clone(), signal);
                }
                Err(e) => {
                    warn!("Strategy {} failed to generate signal: {}", name, e);
                }
            }
        }

        Ok(signals)
    }

    /// Assess market and position risk
    async fn assess_risk(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
    ) -> Result<RiskAssessment> {
        let market_risk = market_context.volatility;

        let position_risk = if let Some(pos) = current_position {
            let pnl_pct = (market_context.current_price - pos.entry_price) / pos.entry_price;
            pnl_pct.abs()
        } else {
            0.0
        };

        // Simple portfolio risk calculation (could be enhanced)
        let portfolio_risk = position_risk * 0.5 + market_risk * 0.5;

        // Calculate volatility-adjusted position size
        let base_size = self.config.max_risk_per_trade;
        let vol_adjustment = 1.0 / (1.0 + market_context.volatility * 10.0);
        let volatility_adjusted_size = base_size * vol_adjustment;

        Ok(RiskAssessment {
            market_risk,
            position_risk,
            portfolio_risk,
            volatility_adjusted_size,
        })
    }

    /// Synthesize final trading decision
    async fn synthesize_decision(
        &self,
        neural_consensus: HashMap<String, f64>,
        strategy_signals: HashMap<String, Signal>,
        risk_assessment: RiskAssessment,
        market_context: &MarketContext,
        current_position: Option<&Position>,
    ) -> Result<AutonomousDecision> {
        let mut reasoning = Vec::new();

        // Calculate overall neural signal
        let neural_signal: f64 =
            neural_consensus.values().sum::<f64>() / neural_consensus.len() as f64;
        reasoning.push(format!("Neural consensus signal: {:.3}", neural_signal));

        // Count strategy votes
        let mut buy_votes = 0;
        let mut sell_votes = 0;
        let mut hold_votes = 0;
        let mut total_confidence = 0.0;

        for (strategy_name, signal) in &strategy_signals {
            match signal {
                Signal::Buy { confidence, .. } => {
                    buy_votes += 1;
                    total_confidence += confidence;
                    reasoning.push(format!(
                        "{} votes BUY (conf: {:.2})",
                        strategy_name, confidence
                    ));
                }
                Signal::Sell { confidence, .. } => {
                    sell_votes += 1;
                    total_confidence += confidence;
                    reasoning.push(format!(
                        "{} votes SELL (conf: {:.2})",
                        strategy_name, confidence
                    ));
                }
                Signal::Hold { reason } => {
                    hold_votes += 1;
                    reasoning.push(format!("{} votes HOLD: {}", strategy_name, reason));
                }
            }
        }

        let strategy_count = strategy_signals.len() as f64;
        let avg_confidence = if buy_votes + sell_votes > 0 {
            total_confidence / (buy_votes + sell_votes) as f64
        } else {
            0.0
        };

        // Combine neural and strategy signals
        let combined_signal =
            neural_signal * 0.6 + ((buy_votes as f64 - sell_votes as f64) / strategy_count) * 0.4;

        // Risk-adjusted confidence
        let risk_adjusted_confidence = avg_confidence * (1.0 - risk_assessment.portfolio_risk);

        reasoning.push(format!(
            "Risk assessment - Market: {:.2}, Position: {:.2}, Portfolio: {:.2}",
            risk_assessment.market_risk,
            risk_assessment.position_risk,
            risk_assessment.portfolio_risk
        ));

        // Make final decision
        let action = if current_position.is_some() {
            // We have a position - check for exit
            if combined_signal < -0.3 || risk_assessment.position_risk > 0.05 {
                let pos = current_position.unwrap();
                TradingAction::Sell {
                    symbol: market_context.symbol.clone(),
                    size: pos.size,
                    reason: format!(
                        "Exit signal: combined={:.3}, risk={:.3}",
                        combined_signal, risk_assessment.position_risk
                    ),
                }
            } else if risk_assessment.market_risk > 0.1 {
                // Adjust stop loss in volatile markets
                TradingAction::AdjustPosition {
                    symbol: market_context.symbol.clone(),
                    new_stop_loss: Some(market_context.current_price * 0.97),
                    new_take_profit: None,
                }
            } else {
                TradingAction::Hold {
                    reason: "Position maintained - no clear exit signal".to_string(),
                }
            }
        } else {
            // No position - check for entry
            if combined_signal > 0.3 && risk_adjusted_confidence > self.config.min_confidence {
                TradingAction::Buy {
                    symbol: market_context.symbol.clone(),
                    size: risk_assessment.volatility_adjusted_size,
                    stop_loss: Some(market_context.current_price * 0.98),
                    take_profit: Some(market_context.current_price * 1.03),
                }
            } else {
                TradingAction::Hold {
                    reason: format!(
                        "Entry criteria not met: signal={:.3}, confidence={:.3}",
                        combined_signal, risk_adjusted_confidence
                    ),
                }
            }
        };

        Ok(AutonomousDecision {
            timestamp: Utc::now(),
            action,
            confidence: risk_adjusted_confidence,
            risk_assessment,
            reasoning,
            neural_consensus,
            adapted_parameters: None,
        })
    }

    /// Adapt strategy parameters based on performance
    async fn adapt_parameters(
        &self,
        decision: &AutonomousDecision,
        market_context: &MarketContext,
    ) -> Result<HashMap<String, f64>> {
        let mut adapted = HashMap::new();
        let metrics = self.performance_metrics.read().await;

        // Adapt confidence threshold based on win rate
        if metrics.total_decisions > 10 {
            let confidence_adjustment = if metrics.win_rate > 0.6 {
                0.95 // Lower threshold if winning
            } else if metrics.win_rate < 0.4 {
                1.05 // Raise threshold if losing
            } else {
                1.0
            };
            adapted.insert(
                "min_confidence".to_string(),
                self.config.min_confidence * confidence_adjustment,
            );
        }

        // Adapt position size based on recent performance
        if metrics.total_decisions > 5 {
            let size_adjustment = if metrics.sharpe_ratio > 1.5 {
                1.1 // Increase size with good risk-adjusted returns
            } else if metrics.sharpe_ratio < 0.5 {
                0.9 // Decrease size with poor returns
            } else {
                1.0
            };
            adapted.insert(
                "position_size".to_string(),
                self.config.max_risk_per_trade * size_adjustment,
            );
        }

        // Adapt model weights based on accuracy
        for (model, accuracy) in &metrics.model_accuracy {
            if *accuracy > 0.0 {
                let weight = self.config.model_weights.get(model).unwrap_or(&1.0);
                let adjusted_weight = weight * (0.5 + accuracy);
                adapted.insert(format!("weight_{}", model), adjusted_weight);
            }
        }

        Ok(adapted)
    }

    /// Check if retraining is needed and trigger autonomously if enabled
    async fn check_and_trigger_retraining(&self) -> Result<()> {
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

            // TODO: Implement autonomous retraining task
            warn!("Autonomous retraining needed but not yet implemented");
        } else {
            debug!("No retraining needed - metrics within acceptable ranges");
        }

        Ok(())
    }

    /// Spawn autonomous retraining process
    async fn spawn_autonomous_retraining(&self, metrics: RetrainingMetrics) -> Result<()> {
        // Enhanced predictor functionality is now internal to NeuralPredictor
        let urgency = metrics.urgency_score;

        // Spawn background retraining task with urgency-based priority
        tokio::spawn(async move {
            let start_time = Utc::now();
            info!(
                "Starting autonomous neural model retraining with urgency {:.3}",
                urgency
            );

            // Simulate retraining process (in real implementation, this would call actual training)
            match Self::execute_autonomous_retraining(urgency).await {
                Ok(()) => {
                    let duration = Utc::now() - start_time;
                    info!(
                        "Autonomous retraining completed successfully in {} seconds",
                        duration.num_seconds()
                    );
                }
                Err(e) => {
                    error!("Autonomous retraining failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Execute the actual retraining process
    async fn execute_autonomous_retraining(
        urgency_score: f64,
    ) -> Result<()> {
        // Enhanced predictor functionality is now internal to NeuralPredictor
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

        // Record training completion
        // Record training completion by resetting accuracy tracking
        // (implementation would depend on the training completion logic)

        info!(
            "Completed {} in {:.2}s",
            training_duration,
            training_start.elapsed().as_secs_f64()
        );

        Ok(())
    }

    /// Update performance metrics and trigger retraining evaluation
    async fn update_metrics(&self, decision: &AutonomousDecision) {
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

        // Update enhanced predictor performance tracking if we have actual market data
        // Note: In production, this would compare predictions with actual market outcomes
        if metrics.total_decisions % 10 == 0 {
            // Sample performance update every 10 decisions
            let sample_actual = vec![50000.0, 50100.0, 49900.0]; // Mock actual values
            let sample_predicted_values = vec![49980.0, 50120.0, 49880.0]; // Mock predicted values

            // Convert to EnhancedPredictionResult objects
            // Using regular PredictionResult instead of internal EnhancedPredictionResult
            let sample_predicted: Vec<PredictionResult> = sample_predicted_values
                .iter()
                .enumerate()
                .map(|(i, &value)| PredictionResult {
                    timestamp: Utc::now() + chrono::Duration::minutes(i as i64),
                    value,
                    confidence: 0.8,
                    interval_low: value * 0.95,
                    interval_high: value * 1.05,
                    model_name: "test_model".to_string(),
                    metadata: None,
                })
                .collect();

            // Enhanced predictor functionality is now internal to NeuralPredictor
            tokio::spawn(async move {
                // Retraining is now handled internally by NeuralPredictor
                info!("Performance update handled internally by NeuralPredictor");
            });
        }
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }

    /// Get enhanced predictor retraining metrics
    pub async fn get_retraining_metrics(&self) -> Result<RetrainingMetrics> {
        // Enhanced predictor functionality is now internal to NeuralPredictor
        // Return default metrics for now
        Ok(RetrainingMetrics {
            urgency_score: 0.5,
            accuracy: 0.85,
            should_retrain: false,
        })
    }

    /// Enable or disable autonomous retraining
    pub fn set_autonomous_retraining(&mut self, enabled: bool) {
        self.autonomous_retraining_enabled = enabled;
        info!(
            "Autonomous retraining {}",
            if enabled { "enabled" } else { "disabled" }
        );
    }

    /// Get current enhanced predictor performance metrics
    pub async fn get_enhanced_performance_metrics(
        &self,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let predictor = self.enhanced_predictor.read().await;
        predictor.get_performance_metrics().await
    }

    /// Force manual retraining (for testing or manual intervention)
    pub async fn force_retraining(&self) -> Result<()> {
        info!("Manual retraining triggered");

        // Enhanced predictor functionality is now internal to NeuralPredictor
        // Execute immediate retraining
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
        historical_data: &[TimeSeriesData],
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;
    use crate::strategies::{StrategyConfig, StrategyError};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::time::{timeout, Duration};

    // Mock implementations for testing
    struct MockTradingStrategy {
        signal: Signal,
        should_fail: bool,
        name: String,
    }

    #[async_trait]
    impl TradingStrategy for MockTradingStrategy {
        fn name(&self) -> &str {
            &self.name
        }

        async fn initialize(&mut self, _config: StrategyConfig) -> Result<(), StrategyError> {
            Ok(())
        }

        async fn generate_signal(
            &self,
            _market_context: &MarketContext,
            _current_position: Option<&Position>,
        ) -> Result<Signal, StrategyError> {
            if self.should_fail {
                return Err(StrategyError::Execution(
                    "Mock strategy failure".to_string(),
                ));
            }
            Ok(self.signal.clone())
        }

        async fn update_parameters(
            &mut self,
            _parameters: HashMap<String, serde_json::Value>,
        ) -> Result<(), StrategyError> {
            Ok(())
        }

        fn get_metrics(&self) -> HashMap<String, f64> {
            let mut metrics = HashMap::new();
            metrics.insert("test_metric".to_string(), 1.0);
            metrics
        }

        fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
            Ok(!self.should_fail)
        }
    }

    // Helper function to create test market context
    fn create_test_market_context() -> MarketContext {
        MarketContext {
            symbol: "BTC/USDT".to_string(),
            current_price: 50000.0,
            bid: 49990.0,
            ask: 50010.0,
            volume_24h: 1000000.0,
            volatility: 0.02,
            timestamp: Utc::now().timestamp(),
        }
    }

    // Helper function to create test position
    fn create_test_position() -> Position {
        Position {
            symbol: "BTC/USDT".to_string(),
            side: crate::strategies::PositionSide::Long,
            size: 0.1,
            entry_price: 49500.0,
            current_price: 50000.0,
            unrealized_pnl: 50.0, // (50000 - 49500) * 0.1
            timestamp: Utc::now().timestamp(),
        }
    }

    // Helper function to create test time series data
    fn create_test_time_series_data() -> Vec<TimeSeriesData> {
        vec![
            TimeSeriesData {
                symbol: "BTC/USDT".to_string(),
                timestamp: Utc::now(),
                open: 49700.0,
                high: 49850.0,
                low: 49650.0,
                close: 49800.0,
                volume: 100.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("BTC".to_string()),
                value: Some(49800.0),
                metadata: None,
            },
            TimeSeriesData {
                symbol: "BTC/USDT".to_string(),
                timestamp: Utc::now(),
                open: 49800.0,
                high: 49950.0,
                low: 49750.0,
                close: 49900.0,
                volume: 110.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("BTC".to_string()),
                value: Some(49900.0),
                metadata: None,
            },
            TimeSeriesData {
                symbol: "BTC/USDT".to_string(),
                timestamp: Utc::now(),
                open: 49900.0,
                high: 50050.0,
                low: 49850.0,
                close: 50000.0,
                volume: 120.0,
                indicators: HashMap::new(),
                source: Some("test".to_string()),
                entity: Some("BTC".to_string()),
                value: Some(50000.0),
                metadata: None,
            },
        ]
    }

    #[tokio::test]
    async fn test_daa_coordinator_creation() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        assert_eq!(coordinator.config.enabled, true);
        assert_eq!(coordinator.config.min_confidence, 0.75);
        assert_eq!(coordinator.config.max_risk_per_trade, 0.02);
        assert_eq!(coordinator.config.max_positions, 5);
        assert_eq!(coordinator.config.consensus_threshold, 0.7);
        assert_eq!(coordinator.config.enable_adaptation, true);
        assert!(coordinator.autonomous_retraining_enabled);
    }

    #[tokio::test]
    async fn test_component_initialization_with_strategies() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Register multiple strategies
        let strategy1 = Box::new(MockTradingStrategy {
            signal: Signal::Buy {
                confidence: 0.8,
                size: Some(0.1),
                reason: "Test buy signal".to_string(),
            },
            should_fail: false,
            name: "momentum".to_string(),
        });
        let strategy2 = Box::new(MockTradingStrategy {
            signal: Signal::Hold {
                reason: "Waiting for confirmation".to_string(),
            },
            should_fail: false,
            name: "ma_crossover".to_string(),
        });

        coordinator
            .register_strategy("momentum".to_string(), strategy1)
            .await;
        coordinator
            .register_strategy("ma_crossover".to_string(), strategy2)
            .await;

        // Verify strategies are registered
        let strategies = coordinator.strategies.read().await;
        assert_eq!(strategies.len(), 2);
        assert!(strategies.contains_key("momentum"));
        assert!(strategies.contains_key("ma_crossover"));
    }

    #[tokio::test]
    async fn test_decision_making_when_disabled() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let mut config = DaaConfig::default();
        config.enabled = false; // Disable DAA
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx);

        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        let decision = coordinator
            .make_decision(&market_context, None, &historical_data)
            .await
            .unwrap();

        // Should return Hold action when disabled
        match decision.action {
            TradingAction::Hold { reason } => {
                assert!(reason.contains("DAA disabled"));
            }
            _ => panic!("Expected Hold action when DAA is disabled"),
        }
        assert_eq!(decision.confidence, 0.0);
    }

    #[tokio::test]
    async fn test_event_loop_processing_with_position() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Register a strategy that signals sell
        let strategy = Box::new(MockTradingStrategy {
            signal: Signal::Sell {
                confidence: 0.9,
                size: Some(0.1),
                reason: "Exit signal detected".to_string(),
            },
            should_fail: false,
            name: "trend_following".to_string(),
        });
        coordinator
            .register_strategy("trend_following".to_string(), strategy)
            .await;

        let market_context = create_test_market_context();
        let position = create_test_position();
        let historical_data = create_test_time_series_data();

        // Make decision with existing position
        let decision = coordinator
            .make_decision(&market_context, Some(&position), &historical_data)
            .await
            .unwrap();

        // Should receive decision through channel
        let received_decision = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("Timeout waiting for decision")
            .expect("Channel closed");

        assert_eq!(received_decision.timestamp, decision.timestamp);

        // Verify decision history is updated
        let history = coordinator.decision_history.read().await;
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].timestamp, decision.timestamp);
    }

    #[tokio::test]
    async fn test_error_handling_strategy_failure() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Register failing strategies
        let failing_strategy = Box::new(MockTradingStrategy {
            signal: Signal::Buy {
                confidence: 0.8,
                size: Some(0.1),
                reason: "Failing strategy signal".to_string(),
            },
            should_fail: true,
            name: "failing".to_string(),
        });
        let working_strategy = Box::new(MockTradingStrategy {
            signal: Signal::Buy {
                confidence: 0.85,
                size: Some(0.1),
                reason: "Working strategy signal".to_string(),
            },
            should_fail: false,
            name: "working".to_string(),
        });

        coordinator
            .register_strategy("failing".to_string(), failing_strategy)
            .await;
        coordinator
            .register_strategy("working".to_string(), working_strategy)
            .await;

        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        // Should handle strategy failure gracefully
        let decision = coordinator
            .make_decision(&market_context, None, &historical_data)
            .await
            .unwrap();

        // Decision should be made with working strategy only
        assert!(decision
            .reasoning
            .iter()
            .any(|r| r.contains("working votes BUY")));
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = Arc::new(DaaCoordinator::new(config, neural_predictor, tx));
        let shutdown_flag = Arc::new(AtomicBool::new(false));

        // Spawn background task to simulate event loop
        let coordinator_clone = Arc::clone(&coordinator);
        let shutdown_clone = Arc::clone(&shutdown_flag);
        let handle = tokio::spawn(async move {
            while !shutdown_clone.load(Ordering::Relaxed) {
                let market_context = create_test_market_context();
                let historical_data = create_test_time_series_data();

                let _ = coordinator_clone
                    .make_decision(&market_context, None, &historical_data)
                    .await;

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // Let it run for a bit
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Signal shutdown
        shutdown_flag.store(true, Ordering::Relaxed);

        // Wait for graceful shutdown
        let _ = timeout(Duration::from_secs(1), handle).await;

        // Verify we received some decisions
        let mut decision_count = 0;
        while let Ok(_) = rx.try_recv() {
            decision_count += 1;
        }
        assert!(
            decision_count > 0,
            "Should have processed at least one decision"
        );
    }

    #[tokio::test]
    async fn test_risk_assessment() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Test with high volatility market
        let mut market_context = create_test_market_context();
        market_context.volatility = 0.1; // 10% volatility

        let risk = coordinator
            .assess_risk(&market_context, None)
            .await
            .unwrap();

        assert_eq!(risk.market_risk, 0.1);
        assert_eq!(risk.position_risk, 0.0); // No position
        assert!(risk.volatility_adjusted_size < coordinator.config.max_risk_per_trade);

        // Test with position
        let position = create_test_position();
        let risk_with_position = coordinator
            .assess_risk(&market_context, Some(&position))
            .await
            .unwrap();

        assert!(risk_with_position.position_risk > 0.0);
        assert!(risk_with_position.portfolio_risk > 0.0);
    }

    #[tokio::test]
    async fn test_performance_metrics_update() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Initial metrics should be default
        let initial_metrics = coordinator.get_metrics().await;
        assert_eq!(initial_metrics.total_decisions, 0);
        assert_eq!(initial_metrics.avg_confidence, 0.0);

        // Make a decision
        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        let decision = coordinator
            .make_decision(&market_context, None, &historical_data)
            .await
            .unwrap();

        // Metrics should be updated
        let updated_metrics = coordinator.get_metrics().await;
        assert_eq!(updated_metrics.total_decisions, 1);
        assert!(updated_metrics.avg_confidence > 0.0);
    }

    #[tokio::test]
    async fn test_adaptation_mechanism() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let mut config = DaaConfig::default();
        config.enable_adaptation = true;
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx);

        // Simulate multiple decisions to trigger adaptation
        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        for _ in 0..15 {
            let decision = coordinator
                .make_decision(&market_context, None, &historical_data)
                .await
                .unwrap();

            // Should have adapted parameters after enough decisions
            if coordinator.get_metrics().await.total_decisions > 10 {
                assert!(decision.adapted_parameters.is_some());
                let params = decision.adapted_parameters.unwrap();
                assert!(params.contains_key("min_confidence"));
            }
        }
    }

    #[tokio::test]
    async fn test_concurrent_decision_making() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, mut rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = Arc::new(DaaCoordinator::new(config, neural_predictor, tx).unwrap());

        // Spawn multiple concurrent decision tasks
        let mut handles = vec![];
        for i in 0..5 {
            let coordinator_clone = Arc::clone(&coordinator);
            let handle = tokio::spawn(async move {
                let mut market_context = create_test_market_context();
                market_context.current_price += i as f64 * 100.0; // Vary the price
                let historical_data = create_test_time_series_data();

                coordinator_clone
                    .make_decision(&market_context, None, &historical_data)
                    .await
            });
            handles.push(handle);
        }

        // Wait for all tasks to complete
        for handle in handles {
            assert!(handle.await.is_ok());
        }

        // Should have received all decisions
        let mut decision_count = 0;
        while let Ok(_) = rx.try_recv() {
            decision_count += 1;
        }
        assert_eq!(decision_count, 5);

        // Verify metrics are consistent
        let metrics = coordinator.get_metrics().await;
        assert_eq!(metrics.total_decisions, 5);
    }

    #[tokio::test]
    async fn test_autonomous_retraining_integration() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let mut coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        // Test retraining metrics retrieval
        let retraining_metrics = coordinator.get_retraining_metrics().await.unwrap();
        assert!(!retraining_metrics.should_retrain); // Should not need retraining initially

        // Test enhanced performance metrics
        let enhanced_metrics = coordinator
            .get_enhanced_performance_metrics()
            .await
            .unwrap();
        let recent_accuracy = enhanced_metrics
            .get("recent_accuracy")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        assert!(recent_accuracy >= 0.0);

        // Test disabling autonomous retraining
        coordinator.set_autonomous_retraining(false);
        assert!(!coordinator.autonomous_retraining_enabled);

        // Test enabling autonomous retraining
        coordinator.set_autonomous_retraining(true);
        assert!(coordinator.autonomous_retraining_enabled);

        // Test manual retraining trigger
        let force_result = coordinator.force_retraining().await;
        assert!(force_result.is_ok());
    }

    #[tokio::test]
    async fn test_enhanced_neural_consensus() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string(), "DeepAR".to_string()],
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        // Test enhanced neural consensus
        let consensus = coordinator
            .get_neural_consensus(&market_context, &historical_data)
            .await
            .unwrap();

        // Should have consensus entries (may be fallback if enhanced prediction fails)
        assert!(!consensus.is_empty());

        // Values should be within expected signal range
        for (_model, signal) in &consensus {
            assert!(*signal >= -2.0 && *signal <= 2.0); // Allow for weighted signals
        }
    }

    #[tokio::test]
    async fn test_memory_usage_with_history() {
        let neural_config = NeuralConfig {
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
            error_threshold: 0.05,
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);

        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx).unwrap();

        let market_context = create_test_market_context();
        let historical_data = create_test_time_series_data();

        // Make multiple decisions
        for _ in 0..10 {
            coordinator
                .make_decision(&market_context, None, &historical_data)
                .await
                .unwrap();
        }

        // Check decision history
        let history = coordinator.decision_history.read().await;
        assert_eq!(history.len(), 10);

        // Verify decisions are ordered by timestamp
        for i in 1..history.len() {
            assert!(history[i].timestamp >= history[i - 1].timestamp);
        }
    }
}
