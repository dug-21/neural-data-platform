//! DAA Service Integration Adapter
//!
//! This module provides adapters for integrating with the JS/WASM DAA service,
//! enabling seamless communication between Rust trading strategies and the
//! distributed autonomous agent coordination layer.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;

use super::AdapterError;
use crate::data::TimeSeriesData;

/// DAA message format for agent communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAAMessage {
    pub agent_id: String,
    pub message_type: String,
    pub payload: Value,
    pub timestamp: DateTime<Utc>,
    pub correlation_id: Option<String>,
}

/// Trading decision from DAA agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAATradingDecision {
    pub action: TradingAction,
    pub symbol: String,
    pub quantity: f64,
    pub price: Option<f64>,
    pub confidence: f64,
    pub reasoning: Vec<String>,
    pub risk_assessment: RiskAssessment,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TradingAction {
    Buy,
    Sell,
    Hold,
    StopLoss,
    TakeProfit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_score: f64,
    pub max_drawdown: f64,
    pub position_size_recommendation: f64,
    pub stop_loss_price: Option<f64>,
    pub take_profit_price: Option<f64>,
}

/// Market analysis from DAA agents
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DAAMarketAnalysis {
    pub symbol: String,
    pub trend: MarketTrend,
    pub support_levels: Vec<f64>,
    pub resistance_levels: Vec<f64>,
    pub volatility: f64,
    pub sentiment: f64, // -1.0 to 1.0
    pub indicators: HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketTrend {
    Bullish,
    Bearish,
    Neutral,
    Volatile,
}

/// DAA Service Adapter for JS/WASM communication
pub struct DAAServiceAdapter;

impl DAAServiceAdapter {
    /// Convert TimeSeriesData to DAA format for analysis
    pub fn to_daa_format(data: &[TimeSeriesData]) -> Result<Value> {
        let formatted_data: Vec<Value> = data
            .iter()
            .map(|point| {
                json!({
                    "timestamp": point.timestamp.timestamp_millis(),
                    "symbol": point.symbol,
                    "ohlcv": {
                        "open": point.open,
                        "high": point.high,
                        "low": point.low,
                        "close": point.close,
                        "volume": point.volume
                    },
                    "indicators": point.indicators,
                    "metadata": point.metadata
                })
            })
            .collect();

        Ok(json!({
            "type": "market_data",
            "data": formatted_data,
            "source": "neural-trader",
            "version": "1.0"
        }))
    }

    /// Parse DAA trading decision from message
    pub fn parse_trading_decision(message: &DAAMessage) -> Result<DAATradingDecision> {
        serde_json::from_value(message.payload.clone())
            .context("Failed to parse DAA trading decision")
    }

    /// Parse DAA market analysis from message
    pub fn parse_market_analysis(message: &DAAMessage) -> Result<DAAMarketAnalysis> {
        serde_json::from_value(message.payload.clone())
            .context("Failed to parse DAA market analysis")
    }

    /// Create a DAA message for requesting analysis
    pub fn create_analysis_request(
        symbol: &str,
        data: &[TimeSeriesData],
        analysis_type: &str,
    ) -> Result<DAAMessage> {
        Ok(DAAMessage {
            agent_id: "neural-trader".to_string(),
            message_type: "analysis_request".to_string(),
            payload: json!({
                "symbol": symbol,
                "analysis_type": analysis_type,
                "data": Self::to_daa_format(data)?,
                "parameters": {
                    "lookback_window": data.len(),
                    "include_indicators": true,
                    "include_risk_assessment": true
                }
            }),
            timestamp: Utc::now(),
            correlation_id: Some(uuid::Uuid::new_v4().to_string()),
        })
    }

    /// Convert DAA trading decision to executable order format
    pub fn decision_to_order(decision: &DAATradingDecision) -> Value {
        json!({
            "type": "order",
            "symbol": decision.symbol,
            "side": match decision.action {
                TradingAction::Buy => "buy",
                TradingAction::Sell | TradingAction::StopLoss | TradingAction::TakeProfit => "sell",
                TradingAction::Hold => "none",
            },
            "quantity": decision.quantity,
            "price": decision.price,
            "order_type": if decision.price.is_some() { "limit" } else { "market" },
            "time_in_force": "GTC",
            "metadata": {
                "confidence": decision.confidence,
                "reasoning": decision.reasoning,
                "risk_score": decision.risk_assessment.risk_score,
                "stop_loss": decision.risk_assessment.stop_loss_price,
                "take_profit": decision.risk_assessment.take_profit_price,
            }
        })
    }

    /// Create performance metrics message for DAA learning
    pub fn create_performance_feedback(
        decision_id: &str,
        actual_pnl: f64,
        execution_price: f64,
        market_data: &TimeSeriesData,
    ) -> DAAMessage {
        DAAMessage {
            agent_id: "neural-trader".to_string(),
            message_type: "performance_feedback".to_string(),
            payload: json!({
                "decision_id": decision_id,
                "actual_pnl": actual_pnl,
                "execution_price": execution_price,
                "market_snapshot": {
                    "timestamp": market_data.timestamp,
                    "price": market_data.close,
                    "volume": market_data.volume,
                    "indicators": market_data.indicators
                },
                "feedback_type": "trade_outcome"
            }),
            timestamp: Utc::now(),
            correlation_id: Some(decision_id.to_string()),
        }
    }

    /// Convert market analysis to indicator updates
    pub fn analysis_to_indicators(analysis: &DAAMarketAnalysis) -> HashMap<String, f64> {
        let mut indicators = analysis.indicators.clone();

        // Add derived indicators
        indicators.insert("sentiment".to_string(), analysis.sentiment);
        indicators.insert("volatility".to_string(), analysis.volatility);
        indicators.insert(
            "trend_strength".to_string(),
            match analysis.trend {
                MarketTrend::Bullish => 1.0,
                MarketTrend::Bearish => -1.0,
                MarketTrend::Neutral => 0.0,
                MarketTrend::Volatile => 0.5,
            },
        );

        // Add support/resistance levels
        if let Some(support) = analysis.support_levels.first() {
            indicators.insert("primary_support".to_string(), *support);
        }
        if let Some(resistance) = analysis.resistance_levels.first() {
            indicators.insert("primary_resistance".to_string(), *resistance);
        }

        indicators
    }
}

/// FFI-safe wrapper for cross-boundary communication
#[repr(C)]
pub struct DAAServiceHandle {
    ptr: *mut std::ffi::c_void,
}

impl DAAServiceHandle {
    /// Create a new handle for FFI communication
    pub fn new() -> Self {
        Self {
            ptr: std::ptr::null_mut(),
        }
    }
}

// FFI exports for JS/WASM communication
#[no_mangle]
pub extern "C" fn daa_create_analysis_request(
    _symbol_ptr: *const u8,
    _symbol_len: usize,
    _data_ptr: *const u8,
    _data_len: usize,
) -> *mut u8 {
    // Implementation for FFI boundary crossing
    // This would deserialize the data and create the request
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn daa_parse_trading_decision(
    _message_ptr: *const u8,
    _message_len: usize,
) -> *mut u8 {
    // Implementation for FFI boundary crossing
    // This would parse the DAA message and return the decision
    std::ptr::null_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_daa_format_conversion() {
        let data = vec![TimeSeriesData {
            symbol: "ETH/USD".to_string(),
            timestamp: Utc::now(),
            open: 3000.0,
            high: 3100.0,
            low: 2950.0,
            close: 3050.0,
            volume: vec![5000.0],
            volume_value: 5000.0,
            indicators: HashMap::new(),
            source: None,
            entity: None,
            value: None,
            metadata: None,
            values: vec![3050.0],
            intervals: vec![0],
            timestamps: vec![Utc::now()],
            metadata_map: HashMap::new(),
        }];

        let daa_format = DAAServiceAdapter::to_daa_format(&data).unwrap();
        assert!(daa_format["type"] == "market_data");
        assert!(daa_format["data"].is_array());
    }

    #[test]
    fn test_trading_decision_conversion() {
        let decision = DAATradingDecision {
            action: TradingAction::Buy,
            symbol: "BTC/USD".to_string(),
            quantity: 0.1,
            price: Some(50000.0),
            confidence: 0.85,
            reasoning: vec!["Bullish trend".to_string()],
            risk_assessment: RiskAssessment {
                risk_score: 0.3,
                max_drawdown: 0.05,
                position_size_recommendation: 0.1,
                stop_loss_price: Some(48000.0),
                take_profit_price: Some(55000.0),
            },
            timestamp: Utc::now(),
        };

        let order = DAAServiceAdapter::decision_to_order(&decision);
        assert_eq!(order["type"], "order");
        assert_eq!(order["side"], "buy");
        assert_eq!(order["quantity"], 0.1);
    }
}
