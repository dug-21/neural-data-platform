//! TDD Tests for Quality Scorer Module
//! 
//! Tests the data quality scoring algorithms and metrics calculation

use data_staging::quality_scorer::*;
use data_staging::{RawMarketData, QualityThresholds, DataQualityMetrics};
use std::collections::HashMap;

#[tokio::test]
async fn test_high_quality_data_scores_well() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Perfect data with all fields
    let perfect_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - 10), // 10 seconds old
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&perfect_data);
    
    // Should score very highly
    assert!(metrics.overall_score > 0.9);
    assert!(metrics.freshness_score > 0.95);
    assert!(metrics.completeness_score > 0.9);
    assert_eq!(metrics.validity_score, 1.0);
    assert_eq!(metrics.missing_required_fields, 0);
    assert!(metrics.present_optional_fields >= 8);
    assert_eq!(metrics.data_age_seconds, 10);
    assert!(metrics.validation_errors.is_empty());
}

#[tokio::test]
async fn test_missing_required_fields_lowers_quality() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
            "volume".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Data missing required fields
    let incomplete_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: None, // Missing required field
        volume: None, // Missing required field
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
    
    let metrics = scorer.calculate_quality(&incomplete_data);
    
    // Should have lower scores due to missing required fields
    assert!(metrics.overall_score < 0.7);
    assert!(metrics.completeness_score < 0.6);
    assert_eq!(metrics.missing_required_fields, 2); // price and volume
    assert!(metrics.present_optional_fields > 0);
    
    // Should have validation errors
    assert!(!metrics.validation_errors.is_empty());
    assert!(metrics.validation_errors.iter().any(|e| e.contains("required")));
}

#[tokio::test]
async fn test_stale_data_lowers_freshness() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 60, // 1 minute threshold
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Data that's 5 minutes old
    let stale_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - 300), // 5 minutes old
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
    
    let metrics = scorer.calculate_quality(&stale_data);
    
    // Should have lower freshness score
    assert!(metrics.freshness_score < 0.5);
    assert!(metrics.overall_score < 0.8);
    assert_eq!(metrics.data_age_seconds, 300);
    
    // Should have validation error about staleness
    assert!(metrics.validation_errors.iter().any(|e| e.contains("stale") || e.contains("old")));
}

#[tokio::test]
async fn test_invalid_data_values_lower_validity() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Data with invalid values
    let invalid_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(-150.25), // Invalid negative price
        volume: Some(-1000.0), // Invalid negative volume
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.30), // Invalid: bid > ask
        ask: Some(150.20),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(149.0), // Invalid: high < low
        low: Some(151.0),
        open: Some(150.0),
        close: Some(150.25),
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&invalid_data);
    
    // Should have low validity score
    assert!(metrics.validity_score < 0.5);
    assert!(metrics.overall_score < 0.6);
    
    // Should have multiple validation errors
    assert!(metrics.validation_errors.len() >= 3);
    assert!(metrics.validation_errors.iter().any(|e| e.contains("price") && e.contains("positive")));
    assert!(metrics.validation_errors.iter().any(|e| e.contains("volume") && e.contains("negative")));
    assert!(metrics.validation_errors.iter().any(|e| e.contains("bid") || e.contains("ask")));
}

#[tokio::test]
async fn test_completeness_scoring_algorithm() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Test minimal data
    let minimal_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: None,
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: None,
        ask: None,
        exchange: None,
        sequence: None,
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let minimal_metrics = scorer.calculate_quality(&minimal_data);
    
    // Test complete data
    let complete_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: Some(151.0),
        low: Some(149.5),
        open: Some(150.0),
        close: Some(150.25),
        vwap: Some(150.1),
        metadata: HashMap::new(),
    };
    
    let complete_metrics = scorer.calculate_quality(&complete_data);
    
    // Complete data should score higher on completeness
    assert!(complete_metrics.completeness_score > minimal_metrics.completeness_score);
    assert!(complete_metrics.present_optional_fields > minimal_metrics.present_optional_fields);
    assert_eq!(minimal_metrics.present_optional_fields, 0); // Only required fields
    assert!(complete_metrics.present_optional_fields >= 8);
}

#[tokio::test]
async fn test_freshness_scoring_algorithm() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300, // 5 minutes
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    let base_data = |age_seconds: i64| RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - age_seconds),
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
    
    // Test different ages
    let fresh_metrics = scorer.calculate_quality(&base_data(10)); // 10 seconds
    let medium_metrics = scorer.calculate_quality(&base_data(150)); // 2.5 minutes
    let stale_metrics = scorer.calculate_quality(&base_data(600)); // 10 minutes
    
    // Freshness should decrease with age
    assert!(fresh_metrics.freshness_score > medium_metrics.freshness_score);
    assert!(medium_metrics.freshness_score > stale_metrics.freshness_score);
    
    assert_eq!(fresh_metrics.data_age_seconds, 10);
    assert_eq!(medium_metrics.data_age_seconds, 150);
    assert_eq!(stale_metrics.data_age_seconds, 600);
    
    // Very fresh data should score very high
    assert!(fresh_metrics.freshness_score > 0.9);
    
    // Data beyond threshold should score very low
    assert!(stale_metrics.freshness_score < 0.3);
}

#[tokio::test]
async fn test_validity_scoring_with_multiple_errors() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Data with multiple validity issues
    let problematic_data = RawMarketData {
        symbol: Some("INVALID_SYMBOL_TOO_LONG_123".to_string()), // Invalid symbol
        price: Some(f64::INFINITY), // Invalid infinite price
        volume: Some(f64::NAN), // Invalid NaN volume
        timestamp: Some(chrono::Utc::now().timestamp() + 3600), // Future timestamp
        bid: Some(150.30),
        ask: Some(150.20), // Inverted spread
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&problematic_data);
    
    // Should have very low validity score due to multiple issues
    assert!(metrics.validity_score < 0.3);
    assert!(metrics.overall_score < 0.4);
    
    // Should have many validation errors
    assert!(metrics.validation_errors.len() >= 3);
}

#[tokio::test]
async fn test_overall_score_calculation() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    // Test data with known characteristics
    let test_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0), // Optional but present
        timestamp: Some(chrono::Utc::now().timestamp() - 30), // 30 seconds old
        bid: Some(150.20),
        ask: Some(150.30),
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None, // Some optional fields missing
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&test_data);
    
    // Overall score should be weighted average of component scores
    let expected_overall = (
        metrics.freshness_score * 0.3 +
        metrics.completeness_score * 0.3 +
        metrics.validity_score * 0.4
    );
    
    // Allow small floating point differences
    assert!((metrics.overall_score - expected_overall).abs() < 0.01);
    
    // All scores should be between 0 and 1
    assert!(metrics.overall_score >= 0.0 && metrics.overall_score <= 1.0);
    assert!(metrics.freshness_score >= 0.0 && metrics.freshness_score <= 1.0);
    assert!(metrics.completeness_score >= 0.0 && metrics.completeness_score <= 1.0);
    assert!(metrics.validity_score >= 0.0 && metrics.validity_score <= 1.0);
}

#[tokio::test]
async fn test_edge_case_empty_symbol() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec!["symbol".to_string()],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    let empty_symbol_data = RawMarketData {
        symbol: Some("".to_string()), // Empty but present
        price: Some(150.25),
        volume: None,
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: None,
        ask: None,
        exchange: None,
        sequence: None,
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let metrics = scorer.calculate_quality(&empty_symbol_data);
    
    // Should count as missing required field even though Option::Some
    assert!(metrics.missing_required_fields > 0);
    assert!(!metrics.validation_errors.is_empty());
    assert!(metrics.validation_errors.iter().any(|e| e.contains("symbol") && e.contains("empty")));
}

#[tokio::test]
async fn test_metadata_impact_on_completeness() {
    let thresholds = QualityThresholds {
        minimum_quality_score: 0.7,
        max_age_seconds: 300,
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let scorer = QualityScorer::new(&thresholds);
    
    let mut rich_metadata = HashMap::new();
    rich_metadata.insert("source".to_string(), serde_json::Value::String("polygon".to_string()));
    rich_metadata.insert("feed_id".to_string(), serde_json::Value::Number(serde_json::Number::from(123)));
    rich_metadata.insert("conditions".to_string(), serde_json::Value::Array(vec![
        serde_json::Value::String("T".to_string()),
    ]));
    
    let data_with_metadata = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp()),
        bid: None,
        ask: None,
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: rich_metadata,
    };
    
    let data_without_metadata = RawMarketData {
        metadata: HashMap::new(),
        ..data_with_metadata.clone()
    };
    
    let metrics_with_metadata = scorer.calculate_quality(&data_with_metadata);
    let metrics_without_metadata = scorer.calculate_quality(&data_without_metadata);
    
    // Rich metadata should improve completeness score
    assert!(metrics_with_metadata.completeness_score >= metrics_without_metadata.completeness_score);
}

#[tokio::test]
async fn test_scoring_thresholds_configuration() {
    // Test with strict thresholds
    let strict_thresholds = QualityThresholds {
        minimum_quality_score: 0.95,
        max_age_seconds: 30, // 30 seconds
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "volume".to_string(),
            "timestamp".to_string(),
            "bid".to_string(),
            "ask".to_string(),
        ],
    };
    
    // Test with lenient thresholds
    let lenient_thresholds = QualityThresholds {
        minimum_quality_score: 0.5,
        max_age_seconds: 600, // 10 minutes
        required_fields: vec![
            "symbol".to_string(),
            "price".to_string(),
            "timestamp".to_string(),
        ],
    };
    
    let strict_scorer = QualityScorer::new(&strict_thresholds);
    let lenient_scorer = QualityScorer::new(&lenient_thresholds);
    
    // Same test data
    let test_data = RawMarketData {
        symbol: Some("AAPL".to_string()),
        price: Some(150.25),
        volume: Some(1000.0),
        timestamp: Some(chrono::Utc::now().timestamp() - 45), // 45 seconds old
        bid: None, // Missing for strict requirements
        ask: None, // Missing for strict requirements  
        exchange: Some("NASDAQ".to_string()),
        sequence: Some(12345),
        high: None,
        low: None,
        open: None,
        close: None,
        vwap: None,
        metadata: HashMap::new(),
    };
    
    let strict_metrics = strict_scorer.calculate_quality(&test_data);
    let lenient_metrics = lenient_scorer.calculate_quality(&test_data);
    
    // Strict scorer should be harsher
    assert!(lenient_metrics.overall_score > strict_metrics.overall_score);
    assert!(strict_metrics.missing_required_fields > lenient_metrics.missing_required_fields);
    
    // Different freshness due to different age thresholds
    assert!(lenient_metrics.freshness_score > strict_metrics.freshness_score);
}