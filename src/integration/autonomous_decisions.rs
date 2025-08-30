//! Autonomous Decision Making Module
//!
//! Implements the decision-making logic for DAA agents with neural feedback

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::{OrderSide, OrderType, TradeOrder};
use crate::data::TimeSeriesData;
use crate::neural::{NeuralPredictor, NeuralPredictorTrait};
use crate::strategies::MarketContext;

/// Market trend classification
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Neutral,
    Volatile,
}

/// Decision confidence level
#[derive(Debug, Clone, Copy)]
pub enum ConfidenceLevel {
    VeryHigh(f64), // > 0.9
    High(f64),     // > 0.75
    Medium(f64),   // > 0.6
    Low(f64),      // > 0.4
    VeryLow(f64),  // <= 0.4
}

impl ConfidenceLevel {
    pub fn from_score(score: f64) -> Self {
        match score {
            s if s > 0.9 => ConfidenceLevel::VeryHigh(s),
            s if s > 0.75 => ConfidenceLevel::High(s),
            s if s > 0.6 => ConfidenceLevel::Medium(s),
            s if s > 0.4 => ConfidenceLevel::Low(s),
            s => ConfidenceLevel::VeryLow(s),
        }
    }

    pub fn value(&self) -> f64 {
        match self {
            ConfidenceLevel::VeryHigh(v)
            | ConfidenceLevel::High(v)
            | ConfidenceLevel::Medium(v)
            | ConfidenceLevel::Low(v)
            | ConfidenceLevel::VeryLow(v) => *v,
        }
    }
}

/// DAA Decision Maker
pub struct DaaDecisionMaker {
    neural_predictor: Arc<NeuralPredictor>,
    decision_threshold: f64,
    risk_tolerance: f64,
    adaptation_rate: f64,
    decision_history: Arc<RwLock<Vec<DecisionRecord>>>,
}

#[derive(Debug, Clone)]
struct DecisionRecord {
    timestamp: chrono::DateTime<chrono::Utc>,
    market_trend: MarketTrend,
    confidence: ConfidenceLevel,
    action_taken: String,
    outcome: Option<f64>,
}

impl DaaDecisionMaker {
    pub fn new(neural_predictor: Arc<NeuralPredictor>) -> Self {
        Self {
            neural_predictor,
            decision_threshold: 0.7,
            risk_tolerance: 0.02,
            adaptation_rate: 0.1,
            decision_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Analyze market trend using neural predictions
    pub async fn analyze_market_trend(
        &self,
        data: &[TimeSeriesData],
    ) -> Result<(MarketTrend, ConfidenceLevel)> {
        // Get neural predictions
        let predictions = self
            .neural_predictor
            .predict(data, 10, None)
            .await
            .context("Failed to get neural predictions")?;

        if predictions.is_empty() || data.is_empty() {
            return Ok((MarketTrend::Neutral, ConfidenceLevel::VeryLow(0.0)));
        }

        let current_price = data.last().unwrap().close;

        // Analyze trend from predictions
        let mut bullish_signals = 0;
        let mut bearish_signals = 0;
        let mut total_confidence = 0.0;

        for (i, pred) in predictions.iter().enumerate().take(5) {
            let price_change = (pred.value - current_price) / current_price;
            let weighted_confidence = pred.confidence * (1.0 / (i + 1) as f64);

            if price_change > 0.01 {
                bullish_signals += 1;
            } else if price_change < -0.01 {
                bearish_signals += 1;
            }

            total_confidence += weighted_confidence;
        }

        // Calculate volatility
        let volatility = self.calculate_volatility(data);

        // Determine trend
        let trend = if volatility > 0.05 {
            MarketTrend::Volatile
        } else if bullish_signals > bearish_signals + 1 {
            MarketTrend::Bullish
        } else if bearish_signals > bullish_signals + 1 {
            MarketTrend::Bearish
        } else {
            MarketTrend::Neutral
        };

        let avg_confidence = total_confidence / predictions.len().min(5) as f64;
        let confidence = ConfidenceLevel::from_score(avg_confidence);

        debug!(
            "Market trend analysis: {:?} with confidence {:?}",
            trend, confidence
        );

        Ok((trend, confidence))
    }

    /// Make an autonomous trading decision
    pub async fn make_trading_decision(
        &self,
        market_context: &MarketContext,
        historical_data: &[TimeSeriesData],
        current_balance: f64,
    ) -> Result<Option<TradeOrder>> {
        let (trend, confidence) = self.analyze_market_trend(historical_data).await?;

        // Record decision process
        let record = DecisionRecord {
            timestamp: chrono::Utc::now(),
            market_trend: trend,
            confidence: confidence.clone(),
            action_taken: "Analyzing".to_string(),
            outcome: None,
        };
        self.decision_history.write().await.push(record);

        // Check if confidence meets threshold
        if confidence.value() < self.decision_threshold {
            info!("Confidence too low for trading: {:.2}", confidence.value());
            return Ok(None);
        }

        // Calculate position size based on risk tolerance
        let position_size = self.calculate_position_size(
            current_balance,
            market_context.volatility,
            confidence.value(),
        );

        // Generate trade order based on trend
        let order = match trend {
            MarketTrend::Bullish => Some(TradeOrder {
                symbol: market_context.symbol.clone(),
                side: OrderSide::Buy,
                quantity: position_size,
                order_type: OrderType::Market,
                price: None,
            }),
            MarketTrend::Bearish => Some(TradeOrder {
                symbol: market_context.symbol.clone(),
                side: OrderSide::Sell,
                quantity: position_size,
                order_type: OrderType::Market,
                price: None,
            }),
            MarketTrend::Neutral | MarketTrend::Volatile => None,
        };

        if let Some(ref o) = order {
            info!(
                "DAA decision: {:?} {} {} at market price (confidence: {:.2})",
                o.side,
                o.quantity,
                o.symbol,
                confidence.value()
            );
        }

        Ok(order)
    }

    /// Calculate appropriate position size
    fn calculate_position_size(&self, balance: f64, volatility: f64, confidence: f64) -> f64 {
        // Base position size as percentage of balance
        let base_size = balance * self.risk_tolerance;

        // Adjust for volatility (inverse relationship)
        let volatility_adjustment = 1.0 / (1.0 + volatility * 10.0);

        // Adjust for confidence
        let confidence_adjustment = confidence;

        base_size * volatility_adjustment * confidence_adjustment
    }

    /// Calculate market volatility
    fn calculate_volatility(&self, data: &[TimeSeriesData]) -> f64 {
        if data.len() < 2 {
            return 0.02; // Default volatility
        }

        let returns: Vec<f64> = data
            .windows(2)
            .map(|w| (w[1].close - w[0].close) / w[0].close)
            .collect();

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;

        variance.sqrt()
    }

    /// Adapt decision parameters based on outcomes
    pub async fn adapt_from_outcome(
        &mut self,
        outcome_pnl: f64,
        _market_conditions: &MarketContext,
    ) -> Result<()> {
        let mut history = self.decision_history.write().await;

        if let Some(last_decision) = history.last_mut() {
            last_decision.outcome = Some(outcome_pnl);
        }

        // Calculate success rate from recent decisions
        let recent_decisions: Vec<_> = history
            .iter()
            .rev()
            .take(20)
            .filter_map(|d| d.outcome)
            .collect();

        if recent_decisions.len() >= 5 {
            let success_rate = recent_decisions.iter().filter(|&&pnl| pnl > 0.0).count() as f64
                / recent_decisions.len() as f64;

            // Adapt decision threshold based on success rate
            if success_rate > 0.7 {
                // Lower threshold if very successful
                self.decision_threshold *= (1.0 - self.adaptation_rate);
            } else if success_rate < 0.3 {
                // Raise threshold if unsuccessful
                self.decision_threshold *= (1.0 + self.adaptation_rate);
            }

            // Keep threshold in reasonable bounds
            self.decision_threshold = self.decision_threshold.max(0.5).min(0.9);

            debug!(
                "Adapted decision threshold to {:.2} based on {:.1}% success rate",
                self.decision_threshold,
                success_rate * 100.0
            );
        }

        Ok(())
    }

    /// Get decision history summary
    pub async fn get_decision_summary(&self) -> HashMap<String, f64> {
        let history = self.decision_history.read().await;

        let total_decisions = history.len() as f64;
        let profitable_decisions = history
            .iter()
            .filter(|d| d.outcome.unwrap_or(0.0) > 0.0)
            .count() as f64;

        let avg_confidence =
            history.iter().map(|d| d.confidence.value()).sum::<f64>() / total_decisions.max(1.0);

        let total_pnl = history.iter().filter_map(|d| d.outcome).sum::<f64>();

        HashMap::from([
            ("total_decisions".to_string(), total_decisions),
            ("profitable_decisions".to_string(), profitable_decisions),
            (
                "win_rate".to_string(),
                profitable_decisions / total_decisions.max(1.0),
            ),
            ("avg_confidence".to_string(), avg_confidence),
            ("total_pnl".to_string(), total_pnl),
            ("current_threshold".to_string(), self.decision_threshold),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NeuralConfig;

    #[tokio::test]
    async fn test_confidence_level() {
        assert!(matches!(
            ConfidenceLevel::from_score(0.95),
            ConfidenceLevel::VeryHigh(_)
        ));
        assert!(matches!(
            ConfidenceLevel::from_score(0.8),
            ConfidenceLevel::High(_)
        ));
        assert!(matches!(
            ConfidenceLevel::from_score(0.65),
            ConfidenceLevel::Medium(_)
        ));
        assert!(matches!(
            ConfidenceLevel::from_score(0.45),
            ConfidenceLevel::Low(_)
        ));
        assert!(matches!(
            ConfidenceLevel::from_score(0.3),
            ConfidenceLevel::VeryLow(_)
        ));
    }

    #[tokio::test]
    async fn test_daa_decision_maker_creation() {
        let neural_config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            accuracy_threshold: 0.8,
            ..Default::default()
        };
        let neural_predictor = Arc::new(NeuralPredictor::new(neural_config).await.unwrap());

        let decision_maker = DaaDecisionMaker::new(neural_predictor);
        assert_eq!(decision_maker.decision_threshold, 0.7);
        assert_eq!(decision_maker.risk_tolerance, 0.02);
    }
}
