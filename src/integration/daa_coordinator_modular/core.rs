//! DAA Coordinator Core Implementation
//!
//! Main coordinator struct and initialization logic.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::daa::autonomous_training::{AutonomousTrainingEngine, PerformanceSnapshot};
use crate::data::TimeSeriesData;
use crate::neural::{NeuralPredictor, PredictionResult};
use crate::strategies::{MarketContext, Position, Signal, TradingStrategy};

use super::config::{DaaConfig, PerformanceMetrics};
use super::decisions::{AutonomousDecision, TradingAction, RiskAssessment};

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

impl DaaCoordinator {
    pub fn new(
        config: DaaConfig,
        neural_predictor: Arc<NeuralPredictor>,
        decision_sender: mpsc::Sender<AutonomousDecision>,
    ) -> Result<Self> {
        // Create enhanced predictor with same configuration
        let _neural_config = crate::config::NeuralConfig::default(); // Simplified to avoid missing fields

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

    // Helper methods that need to be implemented
    async fn get_neural_consensus(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        // Implementation moved from original file
        let mut consensus = HashMap::new();

        match self
            .neural_predictor
            .predict(historical_data, 5, None)
            .await
        {
            Ok(predictions) => {
                for (i, prediction) in predictions.iter().enumerate() {
                    let signal_strength = (prediction.value - market_context.current_price) 
                        / market_context.current_price;
                    let model_name = &prediction.model_name;
                    let base_weight = self
                        .config
                        .model_weights
                        .get(model_name)
                        .unwrap_or(&1.0);

                    let weighted_signal = signal_strength * prediction.confidence * base_weight;
                    consensus.insert(
                        format!("{}_step_{}", model_name, i),
                        weighted_signal,
                    );
                }
            }
            Err(e) => {
                warn!("Failed to get neural predictions: {}", e);
                consensus.insert("fallback".to_string(), 0.0);
            }
        }

        Ok(consensus)
    }

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

        let portfolio_risk = position_risk * 0.5 + market_risk * 0.5;
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

    async fn synthesize_decision(
        &self,
        neural_consensus: HashMap<String, f64>,
        strategy_signals: HashMap<String, Signal>,
        risk_assessment: RiskAssessment,
        market_context: &MarketContext,
        current_position: Option<&Position>,
    ) -> Result<AutonomousDecision> {
        // Basic decision synthesis logic
        let neural_signal: f64 = if !neural_consensus.is_empty() {
            neural_consensus.values().sum::<f64>() / neural_consensus.len() as f64
        } else {
            0.0
        };

        let action = if neural_signal > 0.3 && current_position.is_none() {
            TradingAction::Buy {
                symbol: market_context.symbol.clone(),
                size: risk_assessment.volatility_adjusted_size,
                stop_loss: Some(market_context.current_price * 0.98),
                take_profit: Some(market_context.current_price * 1.03),
            }
        } else if neural_signal < -0.3 && current_position.is_some() {
            TradingAction::Sell {
                symbol: market_context.symbol.clone(),
                size: current_position.unwrap().size,
                reason: "Exit signal".to_string(),
            }
        } else {
            TradingAction::Hold {
                reason: "No clear signal".to_string(),
            }
        };

        Ok(AutonomousDecision {
            timestamp: Utc::now(),
            action,
            confidence: neural_signal.abs(),
            risk_assessment,
            reasoning: vec![format!("Neural signal: {:.3}", neural_signal)],
            neural_consensus,
            adapted_parameters: None,
        })
    }

    async fn adapt_parameters(
        &self,
        _decision: &AutonomousDecision,
        _market_context: &MarketContext,
    ) -> Result<HashMap<String, f64>> {
        // Placeholder for parameter adaptation
        Ok(HashMap::new())
    }

    async fn update_metrics(&self, decision: &AutonomousDecision) {
        let mut metrics = self.performance_metrics.write().await;
        metrics.total_decisions += 1;
        metrics.avg_confidence = (metrics.avg_confidence * (metrics.total_decisions - 1) as f64
            + decision.confidence)
            / metrics.total_decisions as f64;
    }
}