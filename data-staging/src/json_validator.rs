//! JSON Validation - Validates raw JSON market data structure and content
//!
//! Performs strict validation on incoming JSON to ensure it meets minimum
//! requirements before proto transformation.

use anyhow::{Result, bail};
use regex::Regex;
use crate::{RawMarketData, QualityThresholds, DataStagingError};

/// Validates raw JSON market data
pub struct JsonValidator {
    thresholds: QualityThresholds,
    symbol_regex: Regex,
}

impl JsonValidator {
    pub fn new(thresholds: &QualityThresholds) -> Self {
        let symbol_regex = Regex::new(r"^[A-Z]{1,10}$")
            .expect("Failed to compile symbol regex");
            
        Self {
            thresholds: thresholds.clone(),
            symbol_regex,
        }
    }
    
    pub fn validate(&self, data: &RawMarketData) -> Result<()> {
        // Check required fields
        for required_field in &self.thresholds.required_fields {
            match required_field.as_str() {
                "symbol" => {
                    let symbol = data.symbol.as_ref()
                        .ok_or_else(|| DataStagingError::Validation("Missing required field: symbol".to_string()))?;
                    
                    if symbol.is_empty() {
                        bail!("Symbol cannot be empty");
                    }
                    
                    if !self.symbol_regex.is_match(symbol) {
                        bail!("Invalid symbol format: {}. Must be 1-10 uppercase letters", symbol);
                    }
                }
                "price" => {
                    let price = data.price
                        .ok_or_else(|| DataStagingError::Validation("Missing required field: price".to_string()))?;
                    
                    if price <= 0.0 {
                        bail!("Price must be positive, got: {}", price);
                    }
                    
                    if price > 1_000_000.0 {
                        bail!("Price {} exceeds maximum allowed value", price);
                    }
                }
                "timestamp" => {
                    let timestamp = data.timestamp
                        .ok_or_else(|| DataStagingError::Validation("Missing required field: timestamp".to_string()))?;
                    
                    let now = chrono::Utc::now().timestamp();
                    let age_seconds = now - timestamp;
                    
                    if age_seconds > self.thresholds.max_age_seconds {
                        bail!("Data too old: {} seconds (max: {})", age_seconds, self.thresholds.max_age_seconds);
                    }
                    
                    if timestamp > now + 60 {
                        bail!("Timestamp {} is in the future", timestamp);
                    }
                }
                "volume" => {
                    if let Some(volume) = data.volume {
                        if volume < 0.0 {
                            bail!("Volume cannot be negative: {}", volume);
                        }
                    }
                }
                _ => {
                    // Unknown required field - skip validation but log warning
                    tracing::warn!("Unknown required field specified: {}", required_field);
                }
            }
        }
        
        // Validate optional fields if present
        if let Some(bid) = data.bid {
            if bid <= 0.0 {
                bail!("Bid price must be positive: {}", bid);
            }
        }
        
        if let Some(ask) = data.ask {
            if ask <= 0.0 {
                bail!("Ask price must be positive: {}", ask);
            }
        }
        
        // Validate bid/ask relationship if both present
        if let (Some(bid), Some(ask)) = (data.bid, data.ask) {
            if bid >= ask {
                bail!("Bid ({}) must be less than ask ({})", bid, ask);
            }
            
            let spread = ask - bid;
            let mid_price = (bid + ask) / 2.0;
            let spread_percentage = spread / mid_price * 100.0;
            
            if spread_percentage > 10.0 {
                bail!("Bid-ask spread too wide: {:.2}%", spread_percentage);
            }
        }
        
        // Validate OHLC data consistency if present
        if let (Some(open), Some(high), Some(low), Some(close)) = 
            (data.open, data.high, data.low, data.close) {
            
            if high < low {
                bail!("High ({}) cannot be less than low ({})", high, low);
            }
            
            if open < low || open > high {
                bail!("Open price ({}) not within high-low range ({}-{})", open, low, high);
            }
            
            if close < low || close > high {
                bail!("Close price ({}) not within high-low range ({}-{})", close, low, high);
            }
        }
        
        // Validate exchange if present
        if let Some(ref exchange) = data.exchange {
            if exchange.is_empty() {
                bail!("Exchange name cannot be empty");
            }
            
            if exchange.len() > 20 {
                bail!("Exchange name too long: {} (max 20 characters)", exchange);
            }
        }
        
        Ok(())
    }
    
    pub fn validate_json_structure(&self, json_str: &str) -> Result<()> {
        // Basic JSON validation
        let _: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| DataStagingError::JsonParsing(e))?;
        
        // Check for suspicious characters or patterns
        if json_str.contains('\0') {
            bail!("JSON contains null bytes");
        }
        
        if json_str.len() > 1_000_000 {
            bail!("JSON message too large: {} bytes", json_str.len());
        }
        
        // Check for potential security issues
        let suspicious_patterns = [
            "javascript:",
            "eval(",
            "function(",
            "<script",
            "document.",
            "window.",
        ];
        
        for pattern in &suspicious_patterns {
            if json_str.to_lowercase().contains(pattern) {
                bail!("JSON contains suspicious pattern: {}", pattern);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    fn create_test_thresholds() -> QualityThresholds {
        QualityThresholds {
            minimum_quality_score: 0.7,
            max_age_seconds: 300,
            required_fields: vec![
                "symbol".to_string(),
                "price".to_string(), 
                "timestamp".to_string(),
            ],
        }
    }
    
    fn create_valid_market_data() -> RawMarketData {
        RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp()),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some("NASDAQ".to_string()),
            sequence: Some(12345),
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    #[test]
    fn test_valid_data_passes() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let data = create_valid_market_data();
        
        assert!(validator.validate(&data).is_ok());
    }
    
    #[test]
    fn test_missing_required_field_fails() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let mut data = create_valid_market_data();
        data.symbol = None; // Remove required field
        
        assert!(validator.validate(&data).is_err());
    }
    
    #[test]
    fn test_negative_price_fails() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let mut data = create_valid_market_data();
        data.price = Some(-100.0);
        
        assert!(validator.validate(&data).is_err());
    }
    
    #[test]
    fn test_invalid_symbol_format_fails() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let mut data = create_valid_market_data();
        data.symbol = Some("invalid_symbol_123".to_string());
        
        assert!(validator.validate(&data).is_err());
    }
    
    #[test]
    fn test_old_timestamp_fails() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let mut data = create_valid_market_data();
        data.timestamp = Some(chrono::Utc::now().timestamp() - 1000); // 16+ minutes old
        
        assert!(validator.validate(&data).is_err());
    }
    
    #[test]
    fn test_bid_ask_validation() {
        let validator = JsonValidator::new(&create_test_thresholds());
        let mut data = create_valid_market_data();
        
        // Bid higher than ask should fail
        data.bid = Some(150.30);
        data.ask = Some(150.20);
        
        assert!(validator.validate(&data).is_err());
    }
    
    #[test]
    fn test_json_structure_validation() {
        let validator = JsonValidator::new(&create_test_thresholds());
        
        // Valid JSON should pass
        let valid_json = r#"{"symbol": "AAPL", "price": 150.25}"#;
        assert!(validator.validate_json_structure(valid_json).is_ok());
        
        // Invalid JSON should fail
        let invalid_json = r#"{"symbol": "AAPL", "price": 150.25"#; // Missing closing brace
        assert!(validator.validate_json_structure(invalid_json).is_err());
        
        // Suspicious content should fail
        let suspicious_json = r#"{"symbol": "AAPL", "script": "<script>alert('xss')</script>"}"#;
        assert!(validator.validate_json_structure(suspicious_json).is_err());
    }
}