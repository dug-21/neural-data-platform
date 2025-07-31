use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::info;

use crate::error::Result;
use crate::integrations::agent::AgentClient;
use crate::models::{PositionSize, RiskAssessment, TradeDecision};

#[derive(Debug, Clone)]
pub struct TradingDecisionTool {
    agent_client: Arc<AgentClient>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum TradingRequest {
    GenerateDecision {
        symbol: String,
        prediction: f64,
        confidence: f64,
        risk_tolerance: Option<f64>,
    },
    AssessRisk {
        symbol: String,
        position_size: f64,
        entry_price: f64,
    },
    CalculatePosition {
        symbol: String,
        account_balance: f64,
        risk_percentage: f64,
        stop_loss_price: f64,
        entry_price: f64,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TradingResponse {
    TradeDecision(TradeDecision),
    RiskAssessment(RiskAssessment),
    PositionSize(PositionSize),
}

impl TradingDecisionTool {
    pub fn new(agent_client: Arc<AgentClient>) -> Self {
        Self { agent_client }
    }

    pub async fn execute(&self, request: TradingRequest) -> Result<TradingResponse> {
        match request {
            TradingRequest::GenerateDecision {
                symbol,
                prediction,
                confidence,
                risk_tolerance,
            } => {
                info!(
                    "Generating trade decision for {} (prediction: {}, confidence: {})",
                    symbol, prediction, confidence
                );
                // For now, use get_trading_signal as the basis for decision
                let signal = self.agent_client.get_trading_signal(&symbol).await?;
                let decision = TradeDecision {
                    symbol: symbol.clone(),
                    action: signal.action.clone(),
                    quantity: 0.0, // Will be calculated separately
                    price: signal.price,
                    stop_loss: None,   // TradingSignal doesn't have these fields
                    take_profit: None, // TradingSignal doesn't have these fields
                    reasoning: format!(
                        "Based on {} prediction with {}% confidence. {}",
                        prediction,
                        confidence * 100.0,
                        signal.reasoning
                    ),
                    timestamp: chrono::Utc::now(),
                    confidence,
                    reasons: vec![
                        format!("Prediction: {}", prediction),
                        format!("Confidence: {}%", confidence * 100.0),
                        signal.reasoning.clone(),
                    ],
                    entry_price: signal.price,
                    position_size: 0.0, // Will be calculated separately based on risk
                    risk_reward_ratio: 2.0, // Default 2:1 risk/reward ratio
                };
                Ok(TradingResponse::TradeDecision(decision))
            }
            TradingRequest::AssessRisk {
                symbol,
                position_size,
                entry_price,
            } => {
                info!("Assessing risk for {} position", symbol);
                // Simple risk assessment based on position size
                let portfolio = self.agent_client.get_portfolio().await?;
                let account_value = portfolio.total_value;
                let position_value = position_size * entry_price;
                let risk_percentage = (position_value / account_value) * 100.0;

                let assessment = RiskAssessment {
                    symbol: symbol.clone(),
                    risk_score: (risk_percentage / 10.0).min(1.0), // Normalize to 0-1
                    max_loss: position_value * 0.02,               // Assume 2% stop loss
                    probability_of_loss: 0.5,                      // Default 50% probability
                    risk_reward_ratio: 2.0,                        // Default 2:1 risk/reward
                    recommendation: if risk_percentage > 5.0 {
                        "Reduce position size".to_string()
                    } else if risk_percentage > 2.0 {
                        "Proceed with caution".to_string()
                    } else {
                        "Acceptable risk level".to_string()
                    },
                    risk_level: if risk_percentage > 5.0 {
                        "High".to_string()
                    } else if risk_percentage > 2.0 {
                        "Medium".to_string()
                    } else {
                        "Low".to_string()
                    },
                    exposure_percentage: risk_percentage,
                    recommendations: vec![
                        format!("Position represents {:.2}% of portfolio", risk_percentage),
                        if risk_percentage > 5.0 {
                            "Consider reducing position size".to_string()
                        } else {
                            "Risk level acceptable".to_string()
                        },
                    ],
                };
                Ok(TradingResponse::RiskAssessment(assessment))
            }
            TradingRequest::CalculatePosition {
                symbol,
                account_balance,
                risk_percentage,
                stop_loss_price,
                entry_price,
            } => {
                info!("Calculating position size for {}", symbol);
                // Calculate position size based on risk management rules
                let risk_amount = account_balance * (risk_percentage / 100.0);
                let price_risk = (entry_price - stop_loss_price).abs();
                let shares = if price_risk > 0.0 {
                    risk_amount / price_risk
                } else {
                    0.0
                };

                let position_size = PositionSize {
                    symbol: symbol.clone(),
                    recommended_size: shares,
                    max_size: shares * 1.5, // Allow up to 50% more than recommended
                    risk_per_trade: risk_amount,
                    account_risk_percent: risk_percentage,
                    recommended_shares: shares,
                    position_value: shares * entry_price,
                    risk_amount,
                    percentage_of_capital: (shares * entry_price / account_balance) * 100.0,
                    entry_price,
                    stop_loss: stop_loss_price,
                    risk_per_share: price_risk,
                };
                Ok(TradingResponse::PositionSize(position_size))
            }
        }
    }
}
