//! TDD Tests for core types - Written BEFORE implementation
//! Following London School: Test behavior and interactions

use neural_core::types::*;
use chrono::Utc;
use approx::assert_relative_eq;

#[test]
fn test_market_data_creation() {
    // Given
    let symbol = "AAPL".to_string();
    let price = 150.50;
    let volume = 1000000;
    let timestamp = Utc::now();
    
    // When
    let market_data = MarketData::new(symbol.clone(), price, volume, timestamp);
    
    // Then
    assert_eq!(market_data.symbol(), &symbol);
    assert_relative_eq!(market_data.price(), price);
    assert_eq!(market_data.volume(), volume);
    assert_eq!(market_data.timestamp(), timestamp);
}

#[test]
fn test_market_data_validation() {
    // Given invalid price
    let result = MarketData::new("AAPL".to_string(), -10.0, 1000, Utc::now());
    
    // Then should fail validation
    assert!(result.validate().is_err());
}

#[test]
fn test_prediction_confidence_bounds() {
    // Given
    let value = 155.0;
    let confidence = 0.85;
    
    // When
    let prediction = Prediction::new(value, confidence);
    
    // Then
    assert!(prediction.confidence() >= 0.0 && prediction.confidence() <= 1.0);
}

#[test]
fn test_signal_strength_normalization() {
    // Given
    let signal = Signal::buy("AAPL", 1.5); // Strength > 1.0
    
    // Then should normalize to [0, 1]
    assert!(signal.strength() <= 1.0);
    assert!(signal.strength() >= 0.0);
}

#[test]
fn test_trading_decision_from_prediction() {
    // Given
    let prediction = Prediction::new(160.0, 0.9);
    let current_price = 150.0;
    
    // When
    let decision = TradingDecision::from_prediction(&prediction, current_price);
    
    // Then
    assert_eq!(decision.action(), TradingAction::Buy);
    assert_relative_eq!(decision.confidence(), 0.9);
}