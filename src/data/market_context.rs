//! Market context for trading decisions

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketContext {
    pub symbol: String,
    pub current_price: f64,
    pub trend: String,
    pub volatility: f64,
    pub volume_24h: f64,
    pub indicators: serde_json::Value,
}