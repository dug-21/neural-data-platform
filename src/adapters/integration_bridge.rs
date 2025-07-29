//! Integration Bridge between DAA Service and Trading Strategies
//!
//! This module provides the integration layer that connects the JS/WASM DAA service
//! with Rust trading strategies, handling data flow and decision coordination.

use anyhow::{Context, Result};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use super::{
    daa_service::{DAAMessage, DAAServiceAdapter, DAATradingDecision},
    neuro_divergent::NeuroDivergentAdapter,
    AdapterError,
};
use crate::data::TimeSeriesData;
use crate::strategies::MarketContext;
use crate::strategies::{Signal, TradingStrategy};

/// Neural prediction result
#[derive(Debug, Clone)]
pub struct NeuralPrediction {
    pub predicted_price: f64,
    pub confidence: f64,
    pub trend_probability: f64,
}

/// Final action enum
#[derive(Debug, Clone)]
pub enum FinalAction {
    Buy,
    Sell,
    Hold,
}

/// Combined decision from all sources
#[derive(Debug, Clone)]
pub struct CombinedDecision {
    pub action: FinalAction,
    pub confidence: f64,
    pub strategy_signal: Signal,
    pub daa_decision: Option<DAATradingDecision>,
    pub neural_prediction: Option<NeuralPrediction>,
    pub reasoning: Vec<String>,
    pub risk_assessment: Option<Value>,
}

/// Bridge configuration
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    pub enable_daa_decisions: bool,
    pub enable_neural_predictions: bool,
    pub decision_weight_daa: f64,
    pub decision_weight_strategy: f64,
    pub min_confidence_threshold: f64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enable_daa_decisions: true,
            enable_neural_predictions: true,
            decision_weight_daa: 0.6,
            decision_weight_strategy: 0.4,
            min_confidence_threshold: 0.7,
        }
    }
}

/// Integration bridge for coordinating DAA and strategy decisions
pub struct IntegrationBridge {
    config: BridgeConfig,
    daa_tx: mpsc::Sender<DAAMessage>,
    daa_rx: Arc<RwLock<mpsc::Receiver<DAAMessage>>>,
    market_context: Arc<RwLock<MarketContext>>,
}

impl IntegrationBridge {
    /// Create a new integration bridge
    pub fn new(config: BridgeConfig) -> Self {
        let (daa_tx, daa_rx) = mpsc::channel(100);

        Self {
            config,
            daa_tx,
            daa_rx: Arc::new(RwLock::new(daa_rx)),
            market_context: Arc::new(RwLock::new(MarketContext::default())),
        }
    }

    /// Process market data through both DAA and neural predictions
    pub async fn process_market_data(
        &self,
        data: &[TimeSeriesData],
        strategy: &dyn TradingStrategy,
    ) -> Result<CombinedDecision> {
        // Update market context
        if let Some(latest) = data.last() {
            let mut context = self.market_context.write().await;
            // Update fields from latest data
            context.symbol = latest.symbol.clone();
            context.current_price = latest.close;
            context.volume_24h = latest.volume;
            context.timestamp = latest.timestamp.timestamp();
            // Update bid/ask with small spread
            context.bid = latest.close * 0.999;
            context.ask = latest.close * 1.001;
            // Simple volatility calculation
            if data.len() > 20 {
                let returns: Vec<f64> = data
                    .windows(2)
                    .map(|w| (w[1].close - w[0].close) / w[0].close)
                    .collect();
                let mean = returns.iter().sum::<f64>() / returns.len() as f64;
                let variance =
                    returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
                context.volatility = variance.sqrt();
            }
        }

        // Get strategy signal
        let context = self.market_context.read().await;
        let strategy_signal = strategy
            .generate_signal(&*context, None)
            .await
            .context("Failed to generate strategy signal")?;

        // Get DAA decision if enabled
        let daa_decision = if self.config.enable_daa_decisions {
            self.get_daa_decision(data).await.ok()
        } else {
            None
        };

        // Get neural prediction if enabled
        let neural_prediction = if self.config.enable_neural_predictions {
            self.get_neural_prediction(data).await.ok()
        } else {
            None
        };

        // Combine decisions
        self.combine_decisions(strategy_signal, daa_decision, neural_prediction)
    }

    /// Get decision from DAA service
    async fn get_daa_decision(&self, data: &[TimeSeriesData]) -> Result<DAATradingDecision> {
        let symbol = data
            .first()
            .ok_or_else(|| AdapterError::Serialization("Empty data".to_string()))?
            .symbol
            .clone();

        // Create analysis request
        let request = DAAServiceAdapter::create_analysis_request(&symbol, data, "comprehensive")?;

        // Send request to DAA service
        self.daa_tx
            .send(request.clone())
            .await
            .context("Failed to send DAA request")?;

        // Wait for response (with timeout)
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.wait_for_daa_response(&request.correlation_id.unwrap()),
        )
        .await
        .context("DAA response timeout")?;

        DAAServiceAdapter::parse_trading_decision(&response)
    }

    /// Get neural network prediction
    async fn get_neural_prediction(&self, data: &[TimeSeriesData]) -> Result<NeuralPrediction> {
        // Convert to neuro-divergent format
        let _df = NeuroDivergentAdapter::to_neuro_divergent_df(data)?;

        // Prepare model input
        let (_features, _) = NeuroDivergentAdapter::prepare_model_input(
            data, 20, // lookback window
            5,  // forecast horizon
        )?;

        // Here we would call the actual neural model
        // For now, return a placeholder
        Ok(NeuralPrediction {
            predicted_price: data.last().unwrap().close * 1.01,
            confidence: 0.75,
            trend_probability: 0.6,
        })
    }

    /// Wait for DAA response with correlation ID
    async fn wait_for_daa_response(&self, correlation_id: &str) -> DAAMessage {
        let mut rx = self.daa_rx.write().await;

        while let Some(message) = rx.recv().await {
            if message.correlation_id.as_deref() == Some(correlation_id) {
                return message;
            }
        }

        panic!("DAA channel closed unexpectedly");
    }

    /// Combine decisions from multiple sources
    fn combine_decisions(
        &self,
        strategy_signal: Signal,
        daa_decision: Option<DAATradingDecision>,
        neural_prediction: Option<NeuralPrediction>,
    ) -> Result<CombinedDecision> {
        let mut total_confidence = 0.0;
        let mut weighted_action = 0.0;
        let mut reasoning = Vec::new();

        // Weight strategy signal
        let strategy_weight = self.config.decision_weight_strategy;
        let (strategy_action_value, strategy_confidence) = match &strategy_signal {
            Signal::Buy { confidence, .. } => (1.0, *confidence),
            Signal::Sell { confidence, .. } => (-1.0, *confidence),
            Signal::Hold { .. } => (0.0, 0.5),
        };
        weighted_action += strategy_action_value * strategy_weight * strategy_confidence;
        total_confidence += strategy_weight * strategy_confidence;
        reasoning.push(format!("Strategy signal: {:?}", strategy_signal));

        // Weight DAA decision
        if let Some(daa) = &daa_decision {
            let daa_weight = self.config.decision_weight_daa * daa.confidence;
            let daa_action_value = match &daa.action {
                super::daa_service::TradingAction::Buy => 1.0,
                super::daa_service::TradingAction::Sell
                | super::daa_service::TradingAction::StopLoss
                | super::daa_service::TradingAction::TakeProfit => -1.0,
                super::daa_service::TradingAction::Hold => 0.0,
            };
            weighted_action += daa_action_value * daa_weight;
            total_confidence += daa_weight;
            reasoning.extend(daa.reasoning.clone());
        }

        // Weight neural prediction
        if let Some(neural) = &neural_prediction {
            let neural_weight = 0.2 * neural.confidence;
            let neural_action_value = if neural.trend_probability > 0.6 {
                1.0
            } else if neural.trend_probability < 0.4 {
                -1.0
            } else {
                0.0
            };
            weighted_action += neural_action_value * neural_weight;
            total_confidence += neural_weight;
            reasoning.push(format!(
                "Neural prediction: {:.2}% trend probability",
                neural.trend_probability * 100.0
            ));
        }

        // Normalize
        let final_confidence =
            total_confidence / (strategy_weight + self.config.decision_weight_daa + 0.2);
        let normalized_action = weighted_action / total_confidence;

        // Determine final action
        let final_action = if final_confidence < self.config.min_confidence_threshold {
            FinalAction::Hold
        } else if normalized_action > 0.3 {
            FinalAction::Buy
        } else if normalized_action < -0.3 {
            FinalAction::Sell
        } else {
            FinalAction::Hold
        };

        let risk_assessment = daa_decision
            .as_ref()
            .and_then(|d| serde_json::to_value(&d.risk_assessment).ok());

        Ok(CombinedDecision {
            action: final_action,
            confidence: final_confidence,
            strategy_signal,
            daa_decision,
            neural_prediction,
            reasoning,
            risk_assessment,
        })
    }

    /// Send performance feedback to DAA for learning
    pub async fn send_performance_feedback(
        &self,
        decision_id: &str,
        actual_pnl: f64,
        execution_price: f64,
        market_data: &TimeSeriesData,
    ) -> Result<()> {
        let feedback = DAAServiceAdapter::create_performance_feedback(
            decision_id,
            actual_pnl,
            execution_price,
            market_data,
        );

        self.daa_tx
            .send(feedback)
            .await
            .context("Failed to send performance feedback")?;

        Ok(())
    }
}

/// Bridge builder for easier configuration
pub struct BridgeBuilder {
    config: BridgeConfig,
}

impl BridgeBuilder {
    pub fn new() -> Self {
        Self {
            config: BridgeConfig::default(),
        }
    }

    pub fn with_daa_weight(mut self, weight: f64) -> Self {
        self.config.decision_weight_daa = weight;
        self
    }

    pub fn with_strategy_weight(mut self, weight: f64) -> Self {
        self.config.decision_weight_strategy = weight;
        self
    }

    pub fn with_confidence_threshold(mut self, threshold: f64) -> Self {
        self.config.min_confidence_threshold = threshold;
        self
    }

    pub fn build(self) -> IntegrationBridge {
        IntegrationBridge::new(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_builder() {
        let bridge = BridgeBuilder::new()
            .with_daa_weight(0.7)
            .with_strategy_weight(0.3)
            .with_confidence_threshold(0.8)
            .build();

        assert_eq!(bridge.config.decision_weight_daa, 0.7);
        assert_eq!(bridge.config.decision_weight_strategy, 0.3);
        assert_eq!(bridge.config.min_confidence_threshold, 0.8);
    }
}
