//! Multi-Modal Data Type Definitions
//! 
//! This module defines the various data types and modalities supported
//! by the multi-modal feature fusion system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Data modalities supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataModality {
    /// Price data (OHLCV, technical indicators)
    Price,
    /// Sentiment data (news, social media, analyst ratings)
    Sentiment,
    /// Economic data (macro indicators, interest rates, inflation)
    Economic,
    /// Fundamental data (quarterly reports, earnings, guidance)
    Fundamental,
    /// Order book and market microstructure data
    OrderBook,
    /// Alternative data (satellite imagery, credit card transactions)
    Alternative,
}

impl DataModality {
    /// Get the string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            DataModality::Price => "price",
            DataModality::Sentiment => "sentiment", 
            DataModality::Economic => "economic",
            DataModality::Fundamental => "fundamental",
            DataModality::OrderBook => "orderbook",
            DataModality::Alternative => "alternative",
        }
    }

    /// Get expected update frequency in seconds
    pub fn expected_frequency_seconds(&self) -> u64 {
        match self {
            DataModality::Price => 60,        // 1 minute
            DataModality::Sentiment => 1800,  // 30 minutes
            DataModality::Economic => 86400,  // 1 day
            DataModality::Fundamental => 7776000, // 90 days
            DataModality::OrderBook => 1,     // 1 second
            DataModality::Alternative => 3600, // 1 hour
        }
    }

    /// Get typical feature count for this modality
    pub fn typical_feature_count(&self) -> usize {
        match self {
            DataModality::Price => 150,
            DataModality::Sentiment => 50,
            DataModality::Economic => 80,
            DataModality::Fundamental => 40,
            DataModality::OrderBook => 100,
            DataModality::Alternative => 30,
        }
    }
}

/// Price data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub technical_indicators: HashMap<String, f64>,
    pub microstructure_features: Option<HashMap<String, f64>>,
}

/// Sentiment data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentimentData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub news_sentiment: f64,         // -1.0 to 1.0
    pub social_sentiment: f64,       // -1.0 to 1.0
    pub analyst_sentiment: f64,      // -1.0 to 1.0
    pub sentiment_momentum: f64,     // Rate of change
    pub sentiment_volatility: f64,   // Sentiment uncertainty
    pub news_volume: f64,            // Number of news articles
    pub social_volume: f64,          // Social media mentions
    pub sentiment_strength: f64,     // Confidence in sentiment
    pub entity_sentiment: HashMap<String, f64>, // Per-entity sentiment
    pub topic_sentiment: HashMap<String, f64>,  // Per-topic sentiment
}

/// Economic data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicData {
    pub timestamp: DateTime<Utc>,
    pub region: String,
    pub gdp_growth: Option<f64>,
    pub inflation_rate: Option<f64>,
    pub unemployment_rate: Option<f64>,
    pub interest_rate: Option<f64>,
    pub central_bank_rate: Option<f64>,
    pub money_supply_m2: Option<f64>,
    pub trade_balance: Option<f64>,
    pub consumer_confidence: Option<f64>,
    pub manufacturing_pmi: Option<f64>,
    pub services_pmi: Option<f64>,
    pub currency_strength: HashMap<String, f64>,
    pub commodity_prices: HashMap<String, f64>,
    pub yield_curve: HashMap<String, f64>, // "1Y", "2Y", "5Y", "10Y", etc.
}

/// Fundamental data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub market_cap: Option<f64>,
    pub pe_ratio: Option<f64>,
    pub pb_ratio: Option<f64>,
    pub debt_to_equity: Option<f64>,
    pub return_on_equity: Option<f64>,
    pub return_on_assets: Option<f64>,
    pub revenue_growth: Option<f64>,
    pub earnings_growth: Option<f64>,
    pub free_cash_flow: Option<f64>,
    pub dividend_yield: Option<f64>,
    pub book_value_per_share: Option<f64>,
    pub earnings_per_share: Option<f64>,
    pub revenue_per_share: Option<f64>,
    pub sector_metrics: HashMap<String, f64>,
    pub industry_metrics: HashMap<String, f64>,
    pub peer_comparison: HashMap<String, f64>,
}

/// Order book data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub spread: f64,
    pub mid_price: f64,
    pub imbalance: f64,              // (bid_size - ask_size) / (bid_size + ask_size)
    pub depth_imbalance: f64,        // Imbalance across multiple levels
    pub order_flow: f64,             // Net order flow
    pub trade_intensity: f64,        // Recent trading activity
    pub volatility_estimate: f64,    // Realized volatility from tick data
    pub liquidity_score: f64,        // Market liquidity assessment
    pub level_data: Vec<OrderBookLevel>, // Full order book levels
}

/// Order book level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: f64,
    pub size: f64,
    pub side: OrderSide,
    pub order_count: Option<u32>,
}

/// Order side
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OrderSide {
    Bid,
    Ask,
}

/// Alternative data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeData {
    pub timestamp: DateTime<Utc>,
    pub symbol: Option<String>,
    pub data_type: AlternativeDataType,
    pub value: f64,
    pub confidence: f64,             // Confidence in the data point
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Alternative data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlternativeDataType {
    SatelliteImagery,
    CreditCardTransactions,
    WebTraffic,
    SocialMediaActivity,
    WeatherData,
    SupplyChainMetrics,
    EnergyConsumption,
    PatentFilings,
    ExecutiveMovements,
    ESGScores,
}

/// Multi-modal data container
#[derive(Debug, Clone)]
pub struct MultiModalData {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub price_data: Option<PriceData>,
    pub sentiment_data: Option<SentimentData>,
    pub economic_data: Option<EconomicData>,
    pub fundamental_data: Option<FundamentalData>,
    pub orderbook_data: Option<OrderBookData>,
    pub alternative_data: Vec<AlternativeData>,
}

impl MultiModalData {
    /// Create new multi-modal data container
    pub fn new(symbol: String, timestamp: DateTime<Utc>) -> Self {
        Self {
            timestamp,
            symbol,
            price_data: None,
            sentiment_data: None,
            economic_data: None,
            fundamental_data: None,
            orderbook_data: None,
            alternative_data: Vec::new(),
        }
    }

    /// Get available modalities
    pub fn available_modalities(&self) -> Vec<DataModality> {
        let mut modalities = Vec::new();
        
        if self.price_data.is_some() {
            modalities.push(DataModality::Price);
        }
        if self.sentiment_data.is_some() {
            modalities.push(DataModality::Sentiment);
        }
        if self.economic_data.is_some() {
            modalities.push(DataModality::Economic);
        }
        if self.fundamental_data.is_some() {
            modalities.push(DataModality::Fundamental);
        }
        if self.orderbook_data.is_some() {
            modalities.push(DataModality::OrderBook);
        }
        if !self.alternative_data.is_empty() {
            modalities.push(DataModality::Alternative);
        }
        
        modalities
    }

    /// Calculate data completeness score
    pub fn completeness_score(&self) -> f64 {
        let total_modalities = 6; // Total possible modalities
        let available_count = self.available_modalities().len();
        available_count as f64 / total_modalities as f64
    }

    /// Validate data consistency
    pub fn validate(&self) -> Result<(), String> {
        // Check timestamp consistency
        if let Some(price_data) = &self.price_data {
            if (price_data.timestamp - self.timestamp).num_seconds().abs() > 300 {
                return Err("Price data timestamp inconsistent".to_string());
            }
        }

        // Check symbol consistency
        if let Some(price_data) = &self.price_data {
            if price_data.symbol != self.symbol {
                return Err("Price data symbol mismatch".to_string());
            }
        }

        if let Some(sentiment_data) = &self.sentiment_data {
            if sentiment_data.symbol != self.symbol {
                return Err("Sentiment data symbol mismatch".to_string());
            }
        }

        // Validate sentiment ranges
        if let Some(sentiment_data) = &self.sentiment_data {
            if sentiment_data.news_sentiment < -1.0 || sentiment_data.news_sentiment > 1.0 {
                return Err("News sentiment out of range".to_string());
            }
            if sentiment_data.social_sentiment < -1.0 || sentiment_data.social_sentiment > 1.0 {
                return Err("Social sentiment out of range".to_string());
            }
        }

        // Validate price data
        if let Some(price_data) = &self.price_data {
            if price_data.high < price_data.low {
                return Err("Invalid price data: high < low".to_string());
            }
            if price_data.volume < 0.0 {
                return Err("Invalid volume: negative".to_string());
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_data_modality_properties() {
        assert_eq!(DataModality::Price.as_str(), "price");
        assert_eq!(DataModality::Price.expected_frequency_seconds(), 60);
        assert_eq!(DataModality::Price.typical_feature_count(), 150);
    }

    #[test]
    fn test_multimodal_data_creation() {
        let data = MultiModalData::new("AAPL".to_string(), Utc::now());
        assert_eq!(data.symbol, "AAPL");
        assert!(data.available_modalities().is_empty());
        assert_eq!(data.completeness_score(), 0.0);
    }

    #[test]
    fn test_data_validation() {
        let mut data = MultiModalData::new("AAPL".to_string(), Utc::now());
        
        // Valid case
        assert!(data.validate().is_ok());
        
        // Invalid price data
        data.price_data = Some(PriceData {
            timestamp: data.timestamp,
            symbol: "AAPL".to_string(),
            open: 150.0,
            high: 140.0, // Invalid: high < low
            low: 145.0,
            close: 148.0,
            volume: 1000000.0,
            technical_indicators: HashMap::new(),
            microstructure_features: None,
        });
        
        assert!(data.validate().is_err());
    }
}