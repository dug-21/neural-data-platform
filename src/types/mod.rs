//! Shared type definitions for the Neural Trader system
//!
//! This module contains common types used across different components
//! to ensure consistency and avoid duplication.

use serde::{Deserialize, Serialize};

/// Market data types supported by the neural trader system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataType {
    /// Open, High, Low, Close, Volume price data
    OHLCV,
    /// News sentiment analysis data
    News,
    /// Social media sentiment data
    Social,
    /// Technical indicators data
    TechnicalIndicators,
    /// Order book depth data
    OrderBook,
    /// Trade execution data
    Trades,
    /// Economic indicators and fundamentals
    Economic,
    /// Options and derivatives data
    Options,
    /// Cross-asset correlation data
    Correlation,
    /// Alternative data sources
    Alternative,
}

impl std::fmt::Display for DataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataType::OHLCV => write!(f, "OHLCV"),
            DataType::News => write!(f, "News"),
            DataType::Social => write!(f, "Social"),
            DataType::TechnicalIndicators => write!(f, "TechnicalIndicators"),
            DataType::OrderBook => write!(f, "OrderBook"),
            DataType::Trades => write!(f, "Trades"),
            DataType::Economic => write!(f, "Economic"),
            DataType::Options => write!(f, "Options"),
            DataType::Correlation => write!(f, "Correlation"),
            DataType::Alternative => write!(f, "Alternative"),
        }
    }
}

impl std::str::FromStr for DataType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ohlcv" => Ok(DataType::OHLCV),
            "news" => Ok(DataType::News),
            "social" => Ok(DataType::Social),
            "technicalindicators" | "technical_indicators" => Ok(DataType::TechnicalIndicators),
            "orderbook" | "order_book" => Ok(DataType::OrderBook),
            "trades" => Ok(DataType::Trades),
            "economic" => Ok(DataType::Economic),
            "options" => Ok(DataType::Options),
            "correlation" => Ok(DataType::Correlation),
            "alternative" => Ok(DataType::Alternative),
            _ => Err(format!("Unknown data type: {}", s)),
        }
    }
}

/// Data type pattern for enhanced performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataTypePattern {
    /// Single data type
    Single(DataType),
    /// Multiple data types combined
    Combined(Vec<DataType>),
    /// Time-series pattern with specific lookback
    TimeSeries { 
        data_type: DataType,
        lookback_periods: usize,
    },
    /// Cross-correlation pattern between data types
    Correlation {
        primary: DataType,
        secondary: DataType,
        lag_periods: i32,
    },
}