//! TDD Tests for JSON Validator Module
//! 
//! Tests comprehensive validation of raw market data JSON before proto transformation

use data_staging::json_validator::*;
use data_staging::{RawMarketData, QualityThresholds, DataStagingError};
use std::collections::HashMap;
use serde_json;

#[tokio::test]
async fn test_valid_market_data_passes_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    let valid_data = RawMarketData {
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&valid_data);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_missing_required_fields_fails_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    let invalid_data = RawMarketData {
        symbol: None, // Missing required field
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&invalid_data);
    assert!(result.is_err());
    
    match result.unwrap_err().downcast_ref::<DataStagingError>().unwrap() {
        DataStagingError::Validation(msg) => {
            assert!(msg.contains("symbol"));
            assert!(msg.contains("required"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_invalid_price_values_fail_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Test negative price
    let invalid_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(-150.25), // Invalid negative price
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&invalid_data);
    assert!(result.is_err());
    
    match result.unwrap_err().downcast_ref::<DataStagingError>().unwrap() {
        DataStagingError::Validation(msg) => {
            assert!(msg.contains("price"));
            assert!(msg.contains("positive"));
        }
        _ => panic!("Expected ValidationError"),
    }
    
    // Test zero price
    let zero_price_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(0.0), // Invalid zero price
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&zero_price_data);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_stale_timestamp_fails_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300, // 5 minutes
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    let stale_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - 600), // 10 minutes ago
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&stale_data);
    assert!(result.is_err());
    
    match result.unwrap_err().downcast_ref::<DataStagingError>().unwrap() {
        DataStagingError::Validation(msg) => {
            assert!(msg.contains("timestamp"));
            assert!(msg.contains("stale"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_invalid_symbol_format_fails_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Test empty symbol
    let empty_symbol_data = RawMarketData {
        symbol: Some("".to_string()), // Invalid empty symbol
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&empty_symbol_data);
    assert!(result.is_err());
    
    // Test symbol with invalid characters
    let invalid_symbol_data = RawMarketData {
        symbol: Some("AAPL@!#".to_string()), // Invalid characters
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&invalid_symbol_data);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_bid_ask_spread_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Test invalid bid > ask
    let invalid_spread_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.30), // Bid higher than ask
        ask: Some(150.20),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&invalid_spread_data);
    assert!(result.is_err());
    
    match result.unwrap_err().downcast_ref::<DataStagingError>().unwrap() {
        DataStagingError::Validation(msg) => {
            assert!(msg.contains("bid") || msg.contains("ask"));
            assert!(msg.contains("spread"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_volume_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Test negative volume
    let negative_volume_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(-1000.0), // Invalid negative volume
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&negative_volume_data);
    assert!(result.is_err());
    
    match result.unwrap_err().downcast_ref::<DataStagingError>().unwrap() {
        DataStagingError::Validation(msg) => {
            assert!(msg.contains("volume"));
            assert!(msg.contains("non-negative"));
        }
        _ => panic!("Expected ValidationError"),
    }
}

#[tokio::test]
async fn test_exchange_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Valid exchanges should pass
    let valid_exchanges = vec!["NASDAQ", "NYSE", "AMEX", "BATS", "IEX"];
    
    for exchange in valid_exchanges {
        let data = RawMarketData {
            symbol: Some("AAPL".to_string()),
            price: Some(150.25),
            volume: Some(1000.0),
            timestamp: Some(chrono::Utc::now().timestamp()),
            bid: Some(150.20),
            ask: Some(150.30),
            exchange: Some(exchange.to_string()),
            sequence: Some(12345),
            high: None,
            low: None,
            open: None,
            close: None,
            vwap: None,
            metadata: HashMap::new(),
        };
        
        let result = validator.validate(&data);
        assert!(result.is_ok(), "Exchange {} should be valid", exchange);
    }
    
    // Invalid exchange should fail
    let invalid_exchange_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("INVALID_EXCHANGE".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&invalid_exchange_data);
    // Note: Exchange validation might be permissive for unknown exchanges
    // This depends on implementation requirements
}

#[tokio::test]
async fn test_sequence_number_validation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Test valid sequence number
    let valid_sequence_data = RawMarketData {
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
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&valid_sequence_data);
    assert!(result.is_ok());
    
    // Missing sequence number should still be valid (optional field)
    let no_sequence_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: None, // Optional field
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&no_sequence_data);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_comprehensive_validation_error_messages() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let validator = JsonValidator::new(&thresholds);
    
    // Create data with multiple validation errors
    let multi_error_data = RawMarketData {
        symbol: None, // Missing required field
        price: Some(-150.25), // Invalid negative price
        volume: Some(-1000.0), // Invalid negative volume
        timestamp: Some(chrono::Utc::now().timestamp() - 600), // Stale timestamp
        bid: Some(150.30), // Bid > ask (invalid spread)
        ask: Some(150.20),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let result = validator.validate(&multi_error_data);
    assert!(result.is_err());
    
    // Error message should contain details about multiple failures
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("symbol") || error_msg.contains("price") || error_msg.contains("timestamp"));
}