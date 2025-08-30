//! DAA Strategy Integration and Management
//!
//! Strategy signal collection and synthesis for autonomous decisions.

use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::warn;

use crate::strategies::{MarketContext, Position, Signal, TradingStrategy};
use super::decisions::{AutonomousDecision, TradingAction, RiskAssessment};
use super::config::DaaConfig;

/// Strategy management functionality for DAA Coordinator
pub struct StrategyManager {
    strategies: Arc<RwLock<HashMap<String, Box<dyn TradingStrategy + Send + Sync>>>>,
    config: DaaConfig,
}

impl StrategyManager {
    pub fn new(config: DaaConfig) -> Self {
        Self {
            strategies: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a strategy with the coordinator
    pub async fn register_strategy(
        &self,
        name: String,
        strategy: Box<dyn TradingStrategy + Send + Sync>,
    ) {
        self.strategies.write().await.insert(name, strategy);
    }

    /// Get signals from all registered strategies
    pub async fn get_strategy_signals(
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
    pub async fn assess_risk(
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
    pub async fn synthesize_decision(
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
                        "Entry conditions not met: signal={:.3}, confidence={:.3}",
                        combined_signal, risk_adjusted_confidence
                    ),
                }
            }
        };

        let final_confidence = if matches!(action, TradingAction::Hold { .. }) {
            0.0
        } else {
            risk_adjusted_confidence
        };

        Ok(AutonomousDecision {
            timestamp: chrono::Utc::now(),
            action,
            confidence: final_confidence,
            risk_assessment,
            reasoning,
            neural_consensus,
            adapted_parameters: None,
        })
    }
}