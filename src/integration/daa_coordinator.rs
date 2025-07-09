//! DAA Coordinator for Autonomous Trading Decisions
//! 
//! This module integrates neural-enhanced strategies with Decentralized Autonomous Agents
//! for fully autonomous trading decisions based on neural feedback.

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn, debug, error};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::strategies::{TradingStrategy, Signal, MarketContext, Position};
use crate::neural::{NeuralPredictor, PredictionResult};
use crate::data::TimeSeriesData;

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
        reason: String 
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
    strategies: Arc<RwLock<HashMap<String, Box<dyn TradingStrategy + Send + Sync>>>>,
    decision_history: Arc<RwLock<Vec<AutonomousDecision>>>,
    performance_metrics: Arc<RwLock<PerformanceMetrics>>,
    decision_sender: mpsc::Sender<AutonomousDecision>,
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
    ) -> Self {
        Self {
            config,
            neural_predictor,
            strategies: Arc::new(RwLock::new(HashMap::new())),
            decision_history: Arc::new(RwLock::new(Vec::new())),
            performance_metrics: Arc::new(RwLock::new(PerformanceMetrics::default())),
            decision_sender,
        }
    }
    
    /// Register a strategy with the coordinator
    pub async fn register_strategy(&self, name: String, strategy: Box<dyn TradingStrategy + Send + Sync>) {
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
                action: TradingAction::Hold { reason: "DAA disabled".to_string() },
                confidence: 0.0,
                risk_assessment: self.assess_risk(market_context, current_position).await?,
                reasoning: vec!["Autonomous trading disabled".to_string()],
                neural_consensus: HashMap::new(),
                adapted_parameters: None,
            });
        }
        
        // Step 1: Get neural predictions from multiple models
        let neural_signals = self.get_neural_consensus(market_context, historical_data).await?;
        
        // Step 2: Get strategy signals
        let strategy_signals = self.get_strategy_signals(market_context, current_position).await?;
        
        // Step 3: Assess risk
        let risk_assessment = self.assess_risk(market_context, current_position).await?;
        
        // Step 4: Synthesize decision
        let decision = self.synthesize_decision(
            neural_signals,
            strategy_signals,
            risk_assessment,
            market_context,
            current_position,
        ).await?;
        
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
    
    /// Get consensus from neural models
    async fn get_neural_consensus(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
    ) -> Result<HashMap<String, f64>> {
        let mut consensus = HashMap::new();
        
        // Get predictions from each model
        let models = vec!["NHITS", "TCN", "DeepAR", "Transformer", "MLP"];
        for model in &models {
            match self.neural_predictor.predict(historical_data, 5, None).await {
                Ok(predictions) => {
                    if !predictions.is_empty() {
                        let signal_strength = self.calculate_signal_from_predictions(
                            &predictions,
                            market_context.current_price,
                        );
                        
                        let weight = self.config.model_weights.get(*model).unwrap_or(&1.0);
                        consensus.insert(model.to_string(), signal_strength * weight);
                    }
                }
                Err(e) => {
                    warn!("Failed to get predictions from {}: {}", model, e);
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
    
    /// Get signals from all registered strategies
    async fn get_strategy_signals(
        &self,
        market_context: &MarketContext,
        current_position: Option<&Position>,
    ) -> Result<HashMap<String, Signal>> {
        let mut signals = HashMap::new();
        let strategies = self.strategies.read().await;
        
        for (name, strategy) in strategies.iter() {
            match strategy.generate_signal(market_context, current_position).await {
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
        let neural_signal: f64 = neural_consensus.values().sum::<f64>() / neural_consensus.len() as f64;
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
                    reasoning.push(format!("{} votes BUY (conf: {:.2})", strategy_name, confidence));
                }
                Signal::Sell { confidence, .. } => {
                    sell_votes += 1;
                    total_confidence += confidence;
                    reasoning.push(format!("{} votes SELL (conf: {:.2})", strategy_name, confidence));
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
        let combined_signal = neural_signal * 0.6 + 
            ((buy_votes as f64 - sell_votes as f64) / strategy_count) * 0.4;
        
        // Risk-adjusted confidence
        let risk_adjusted_confidence = avg_confidence * (1.0 - risk_assessment.portfolio_risk);
        
        reasoning.push(format!("Risk assessment - Market: {:.2}, Position: {:.2}, Portfolio: {:.2}",
            risk_assessment.market_risk, risk_assessment.position_risk, risk_assessment.portfolio_risk));
        
        // Make final decision
        let action = if current_position.is_some() {
            // We have a position - check for exit
            if combined_signal < -0.3 || risk_assessment.position_risk > 0.05 {
                let pos = current_position.unwrap();
                TradingAction::Sell {
                    symbol: market_context.symbol.clone(),
                    size: pos.size,
                    reason: format!("Exit signal: combined={:.3}, risk={:.3}", 
                        combined_signal, risk_assessment.position_risk),
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
                    reason: format!("Entry criteria not met: signal={:.3}, confidence={:.3}",
                        combined_signal, risk_adjusted_confidence),
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
            adapted.insert("min_confidence".to_string(), 
                self.config.min_confidence * confidence_adjustment);
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
            adapted.insert("position_size".to_string(),
                self.config.max_risk_per_trade * size_adjustment);
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
    
    /// Update performance metrics
    async fn update_metrics(&self, decision: &AutonomousDecision) {
        let mut metrics = self.performance_metrics.write().await;
        
        metrics.total_decisions += 1;
        metrics.avg_confidence = (metrics.avg_confidence * (metrics.total_decisions - 1) as f64 
            + decision.confidence) / metrics.total_decisions as f64;
        
        // Update model accuracy tracking
        for (model, signal) in &decision.neural_consensus {
            let current_accuracy = metrics.model_accuracy.get(model).unwrap_or(&0.5);
            // Simple exponential moving average for accuracy
            let updated_accuracy = current_accuracy * 0.9 + signal.abs() * 0.1;
            metrics.model_accuracy.insert(model.clone(), updated_accuracy);
        }
    }
    
    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics {
        self.performance_metrics.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;
    
    #[tokio::test]
    async fn test_daa_coordinator_creation() {
        let neural_config = NeuralConfig::default();
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).unwrap());
        let (tx, _rx) = mpsc::channel(100);
        
        let config = DaaConfig::default();
        let coordinator = DaaCoordinator::new(config, neural_predictor, tx);
        
        assert_eq!(coordinator.config.enabled, true);
        assert_eq!(coordinator.config.min_confidence, 0.75);
    }
}