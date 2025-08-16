//! Comprehensive tests for Enhanced DAA Coordinator
//!
//! These tests verify that the enhanced DAA coordinator preserves the critical
//! Byzantine consensus mechanisms while adding data context evaluation capabilities.

use super::super::daa_coordinator::*;
use crate::config::NeuralConfig;
use crate::neural::NeuralPredictor;
use crate::utils::market_hours::MarketHours;
use crate::strategies::{MarketContext, Position, Signal, TradingStrategy, StrategyConfig, StrategyError};
use crate::data::TimeSeriesData;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;

/// Mock trading strategy for testing consensus preservation
struct MockStrategy {
    name: String,
    signal: Signal,
    fail: bool,
}

#[async_trait]
impl TradingStrategy for MockStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn initialize(&mut self, _config: StrategyConfig) -> Result<(), StrategyError> {
        Ok(())
    }

    async fn generate_signal(
        &self,
        _market_context: &MarketContext,
        _current_position: Option<&Position>,
    ) -> Result<Signal, StrategyError> {
        if self.fail {
            return Err(StrategyError::Execution("Mock failure".to_string()));
        }
        Ok(self.signal.clone())
    }

    async fn update_parameters(
        &mut self,
        _parameters: HashMap<String, serde_json::Value>,
    ) -> Result<(), StrategyError> {
        Ok(())
    }

    fn get_metrics(&self) -> HashMap<String, f64> {
        HashMap::new()
    }

    fn can_execute(&self, _context: &MarketContext) -> Result<bool, StrategyError> {
        Ok(!self.fail)
    }
}

/// Create test DAA coordinator
async fn create_test_coordinator() -> Result<DaaCoordinator> {
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string(), "TCN".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.05,
    };
    
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let (tx, _rx) = mpsc::channel(100);
    let market_hours = Arc::new(MarketHours::default());
    
    let config = DaaConfig::default();
    DaaCoordinator::new(config, neural_predictor, tx, market_hours)
}

/// Create test market context
fn create_test_market_context() -> MarketContext {
    MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49995.0,
        ask: 50005.0,
        volume_24h: vec![1000000.0],
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    }
}

/// Create test historical data
fn create_test_historical_data() -> Vec<TimeSeriesData> {
    vec![
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now() - chrono::Duration::hours(2),
            open: 49800.0,
            high: 49950.0,
            low: 49750.0,
            close: 49900.0,
            volume: vec![800000.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(49900.0),
            metadata: None,
        },
        TimeSeriesData {
            symbol: "BTC/USDT".to_string(),
            timestamp: Utc::now() - chrono::Duration::hours(1),
            open: 49900.0,
            high: 50100.0,
            low: 49850.0,
            close: 50000.0,
            volume: vec![1200000.0],
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("BTC".to_string()),
            value: Some(50000.0),
            metadata: None,
        },
    ]
}

/// Create test data availability with different quality levels
fn create_test_data_availability(quality_level: &str) -> DataAvailability {
    match quality_level {
        "excellent" => DataAvailability {
            completeness: 1.0,
            freshness: 1.0,
            quality: 1.0,
            source_count: 5,
            market_coverage: 1.0,
            consistency: 1.0,
            latency_ms: 25.0,
            assessment_time: Utc::now(),
        },
        "good" => DataAvailability {
            completeness: 0.95,
            freshness: 0.9,
            quality: 0.9,
            source_count: 3,
            market_coverage: 0.95,
            consistency: 0.9,
            latency_ms: 50.0,
            assessment_time: Utc::now(),
        },
        "fair" => DataAvailability {
            completeness: 0.8,
            freshness: 0.7,
            quality: 0.75,
            source_count: 2,
            market_coverage: 0.8,
            consistency: 0.7,
            latency_ms: 100.0,
            assessment_time: Utc::now(),
        },
        "poor" => DataAvailability {
            completeness: 0.6,
            freshness: 0.5,
            quality: 0.6,
            source_count: 1,
            market_coverage: 0.6,
            consistency: 0.5,
            latency_ms: 200.0,
            assessment_time: Utc::now(),
        },
        "critical" => DataAvailability {
            completeness: 0.3,
            freshness: 0.2,
            quality: 0.4,
            source_count: 1,
            market_coverage: 0.3,
            consistency: 0.3,
            latency_ms: 500.0,
            assessment_time: Utc::now(),
        },
        _ => DataAvailability::default(),
    }
}

#[tokio::test]
async fn test_byzantine_consensus_preservation() {
    let coordinator = create_test_coordinator().await.unwrap();
    
    // Register multiple strategies to test consensus voting
    let buy_strategy = Box::new(MockStrategy {
        name: "buy_strategy".to_string(),
        signal: Signal::Buy {
            confidence: 0.8,
            size: Some(0.1),
            reason: "Strong buy signal".to_string(),
        },
        fail: false,
    });
    
    let sell_strategy = Box::new(MockStrategy {
        name: "sell_strategy".to_string(),
        signal: Signal::Sell {
            confidence: 0.7,
            size: Some(0.05),
            reason: "Sell signal".to_string(),
        },
        fail: false,
    });
    
    let hold_strategy = Box::new(MockStrategy {
        name: "hold_strategy".to_string(),
        signal: Signal::Hold {
            reason: "Market uncertainty".to_string(),
        },
        fail: false,
    });
    
    coordinator.register_strategy("buy_strategy".to_string(), buy_strategy).await;
    coordinator.register_strategy("sell_strategy".to_string(), sell_strategy).await;
    coordinator.register_strategy("hold_strategy".to_string(), hold_strategy).await;
    
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Test base decision preserves consensus threshold
    let base_decision = coordinator
        .make_decision(&market_context, None, &historical_data)
        .await
        .unwrap();
    
    // CRITICAL: Verify consensus threshold is preserved (70%)
    assert_eq!(coordinator.config.consensus_threshold, 0.7);
    
    // Verify neural consensus is populated
    assert!(!base_decision.neural_consensus.is_empty());
    
    // Verify reasoning includes strategy votes (testing 60/40 weights preservation)
    let has_strategy_reasoning = base_decision.reasoning.iter()
        .any(|r| r.contains("votes"));
    assert!(has_strategy_reasoning, "Strategy voting should be included in reasoning");
    
    // Test enhanced decision preserves consensus
    let excellent_data = create_test_data_availability("excellent");
    let enhanced_decision = coordinator
        .evaluate_with_data_context(&market_context, None, &historical_data, excellent_data)
        .await
        .unwrap();
    
    // CRITICAL: Base decision should be identical (Byzantine consensus preserved)
    assert_eq!(enhanced_decision.base_decision.timestamp, base_decision.timestamp);
    assert_eq!(enhanced_decision.base_decision.confidence, base_decision.confidence);
    assert_eq!(enhanced_decision.base_decision.neural_consensus.len(), base_decision.neural_consensus.len());
    
    // Enhanced reasoning should extend, not replace base reasoning
    assert!(enhanced_decision.enhanced_reasoning.len() > enhanced_decision.base_decision.reasoning.len());
    for base_reason in &enhanced_decision.base_decision.reasoning {
        assert!(enhanced_decision.enhanced_reasoning.iter().any(|r| r == base_reason));
    }
}

#[tokio::test]
async fn test_data_quality_impact_levels() {
    let coordinator = create_test_coordinator().await.unwrap();
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Test different data quality levels
    let quality_levels = ["excellent", "good", "fair", "poor", "critical"];
    let mut results = Vec::new();
    
    for level in &quality_levels {
        let data_availability = create_test_data_availability(level);
        let enhanced_decision = coordinator
            .evaluate_with_data_context(&market_context, None, &historical_data, data_availability)
            .await
            .unwrap();
        
        results.push((level, enhanced_decision.data_adjusted_confidence, enhanced_decision.base_decision.confidence));
    }
    
    // Verify data quality adjustments are progressive
    for i in 1..results.len() {
        let (prev_level, prev_adjusted, prev_base) = results[i-1];
        let (curr_level, curr_adjusted, curr_base) = results[i];
        
        // Base confidence should be the same (consensus preserved)
        assert_eq!(prev_base, curr_base, 
                   "Base confidence should be identical regardless of data quality");
        
        // Adjusted confidence should decrease with worse data quality
        assert!(prev_adjusted >= curr_adjusted,
                "Data-adjusted confidence should decrease from {} to {}", prev_level, curr_level);
    }
    
    // Verify specific thresholds
    let excellent_result = results.iter().find(|(level, _, _)| *level == "excellent").unwrap();
    let critical_result = results.iter().find(|(level, _, _)| *level == "critical").unwrap();
    
    // Excellent quality should have minimal adjustment
    assert!((excellent_result.1 - excellent_result.2).abs() < 0.1);
    
    // Critical quality should have significant adjustment
    assert!(critical_result.1 < critical_result.2 * 0.8);
}

#[tokio::test]
async fn test_enhanced_market_timing() {
    let coordinator = create_test_coordinator().await.unwrap();
    
    // Test with different market contexts
    let contexts = vec![
        (create_test_market_context(), "normal"),
        ({
            let mut ctx = create_test_market_context();
            ctx.volume_24h = vec![50000.0]; // Low volume
            ctx
        }, "low_volume"),
        ({
            let mut ctx = create_test_market_context();
            ctx.ask = ctx.bid + 100.0; // Wide spread
            ctx
        }, "wide_spread"),
    ];
    
    for (context, label) in contexts {
        let data_availability = create_test_data_availability("good");
        let timing_result = coordinator
            .check_enhanced_market_timing(&context, &data_availability)
            .await
            .unwrap();
        
        // Verify timing score is valid
        assert!(timing_result.timing_score >= 0.0 && timing_result.timing_score <= 1.0,
                "Timing score should be between 0 and 1 for {}", label);
        
        // Verify components are calculated
        assert!(timing_result.volume_pattern_score >= 0.0);
        assert!(timing_result.liquidity_score >= 0.0);
        
        // Verify recommendation matches score
        match timing_result.timing_score {
            score if score >= 0.8 => assert!(matches!(timing_result.recommendation, TimingRecommendation::Optimal)),
            score if score >= 0.7 => assert!(matches!(timing_result.recommendation, TimingRecommendation::Good)),
            score if score >= 0.6 => assert!(matches!(timing_result.recommendation, TimingRecommendation::Acceptable)),
            score if score >= 0.4 => assert!(matches!(timing_result.recommendation, TimingRecommendation::Poor)),
            _ => assert!(matches!(timing_result.recommendation, TimingRecommendation::Avoid)),
        }
    }
}

#[tokio::test]
async fn test_data_availability_assessment() {
    // Test utility function for data availability assessment
    let market_context = create_test_market_context();
    
    // Test with empty data
    let empty_data = vec![];
    let availability = super::super::daa_coordinator_enhanced::assess_data_availability(&empty_data, &market_context);
    assert_eq!(availability.completeness, 0.0);
    assert!(availability.overall_score() < 0.5);
    
    // Test with good data
    let good_data = create_test_historical_data();
    let availability = super::super::daa_coordinator_enhanced::assess_data_availability(&good_data, &market_context);
    assert!(availability.completeness > 0.0);
    assert!(availability.freshness > 0.5); // Recent data
    assert!(availability.quality > 0.5);   // Valid data points
    assert!(availability.overall_score() > 0.5);
    
    // Test threshold checking
    assert!(availability.meets_threshold(0.5));
    assert!(!availability.meets_threshold(0.95));
}

#[tokio::test]
async fn test_consensus_weights_preservation() {
    let coordinator = create_test_coordinator().await.unwrap();
    
    // Add strategies with different signal strengths
    let strong_buy = Box::new(MockStrategy {
        name: "strong_buy".to_string(),
        signal: Signal::Buy {
            confidence: 0.9,
            size: Some(0.2),
            reason: "Very strong buy".to_string(),
        },
        fail: false,
    });
    
    let weak_sell = Box::new(MockStrategy {
        name: "weak_sell".to_string(),
        signal: Signal::Sell {
            confidence: 0.6,
            size: Some(0.05),
            reason: "Weak sell".to_string(),
        },
        fail: false,
    });
    
    coordinator.register_strategy("strong_buy".to_string(), strong_buy).await;
    coordinator.register_strategy("weak_sell".to_string(), weak_sell).await;
    
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Get base decision
    let base_decision = coordinator
        .make_decision(&market_context, None, &historical_data)
        .await
        .unwrap();
    
    // Verify 60/40 neural/strategy weighting is implicit in the decision
    // The neural signal should have higher influence than individual strategy votes
    assert!(!base_decision.neural_consensus.is_empty());
    assert!(base_decision.reasoning.iter().any(|r| r.contains("Neural consensus")));
    assert!(base_decision.reasoning.iter().any(|r| r.contains("votes")));
    
    // Test that enhanced decision doesn't change this balance
    let data_availability = create_test_data_availability("good");
    let enhanced_decision = coordinator
        .evaluate_with_data_context(&market_context, None, &historical_data, data_availability)
        .await
        .unwrap();
    
    // The base decision neural consensus should be unchanged
    assert_eq!(enhanced_decision.base_decision.neural_consensus.len(), 
               base_decision.neural_consensus.len());
    
    // Enhanced reasoning should add to, not replace, the consensus reasoning
    let neural_reasoning_count = enhanced_decision.enhanced_reasoning.iter()
        .filter(|r| r.contains("Neural consensus"))
        .count();
    assert_eq!(neural_reasoning_count, 1); // Should have exactly one neural consensus line
}

#[tokio::test]
async fn test_error_handling_in_enhanced_methods() {
    let coordinator = create_test_coordinator().await.unwrap();
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Test with invalid data availability
    let invalid_data = DataAvailability {
        completeness: -1.0, // Invalid value
        freshness: 2.0,     // Invalid value
        quality: 0.5,
        source_count: 0,
        market_coverage: 0.5,
        consistency: 0.5,
        latency_ms: 50.0,
        assessment_time: Utc::now(),
    };
    
    // Should handle gracefully (clamp values)
    let result = coordinator
        .evaluate_with_data_context(&market_context, None, &historical_data, invalid_data)
        .await;
    
    assert!(result.is_ok(), "Should handle invalid data gracefully");
    
    // Test timing with edge case market context
    let edge_context = MarketContext {
        symbol: "TEST/USDT".to_string(),
        current_price: 0.0, // Edge case
        bid: 0.0,
        ask: 0.0,
        volume_24h: vec![],
        volatility: 0.0,
        timestamp: Utc::now().timestamp(),
    };
    
    let timing_result = coordinator
        .check_enhanced_market_timing(&edge_context, &DataAvailability::default())
        .await;
    
    assert!(timing_result.is_ok(), "Should handle edge case market context");
    let timing = timing_result.unwrap();
    assert!(timing.timing_score >= 0.0 && timing.timing_score <= 1.0);
}

#[tokio::test]
async fn test_confidence_adjustment_limits() {
    let coordinator = create_test_coordinator().await.unwrap();
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Test with critical data quality
    let critical_data = create_test_data_availability("critical");
    let enhanced_decision = coordinator
        .evaluate_with_data_context(&market_context, None, &historical_data, critical_data)
        .await
        .unwrap();
    
    // Even with critical data quality, confidence shouldn't be reduced by more than 30%
    let max_reduction = 0.3;
    let min_allowed = enhanced_decision.base_decision.confidence * (1.0 - max_reduction);
    
    assert!(enhanced_decision.data_adjusted_confidence >= min_allowed,
            "Confidence reduction should not exceed 30% limit: {} >= {}", 
            enhanced_decision.data_adjusted_confidence, min_allowed);
    
    // Confidence should still be within valid range
    assert!(enhanced_decision.data_adjusted_confidence >= 0.0);
    assert!(enhanced_decision.data_adjusted_confidence <= 1.0);
}

#[tokio::test]
async fn test_market_session_detection() {
    let coordinator = create_test_coordinator().await.unwrap();
    
    // Test session detection (will use current time)
    let session = coordinator.determine_market_session().await;
    
    // Should return a valid session
    assert!(matches!(session, 
        MarketSession::PreMarket | 
        MarketSession::Opening | 
        MarketSession::Regular | 
        MarketSession::Lunch | 
        MarketSession::Closing | 
        MarketSession::AfterHours | 
        MarketSession::Weekend
    ));
    
    // Test timing scoring for different sessions
    let data_availability = create_test_data_availability("good");
    let market_context = create_test_market_context();
    
    let timing_result = coordinator
        .check_enhanced_market_timing(&market_context, &data_availability)
        .await
        .unwrap();
    
    // Verify session is properly incorporated into timing score
    match timing_result.market_session {
        MarketSession::Regular => assert!(timing_result.timing_score >= 0.6), // Should be high
        MarketSession::Weekend => assert!(timing_result.timing_score <= 0.8), // Should be lower
        _ => {} // Other sessions vary
    }
}

#[tokio::test]
async fn test_volume_pattern_analysis() {
    let coordinator = create_test_coordinator().await.unwrap();
    
    // Test with high volume
    let high_volume_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49995.0,
        ask: 50005.0,
        volume_24h: vec![2000000.0], // High volume
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let high_volume_score = coordinator
        .analyze_volume_patterns(&high_volume_context)
        .await;
    
    // Test with low volume
    let low_volume_context = MarketContext {
        symbol: "BTC/USDT".to_string(),
        current_price: 50000.0,
        bid: 49995.0,
        ask: 50005.0,
        volume_24h: vec![5000.0], // Low volume
        volatility: 0.02,
        timestamp: Utc::now().timestamp(),
    };
    
    let low_volume_score = coordinator
        .analyze_volume_patterns(&low_volume_context)
        .await;
    
    // High volume should score better than low volume
    assert!(high_volume_score > low_volume_score);
    assert!(high_volume_score >= 0.7); // High volume threshold
    assert!(low_volume_score <= 0.5);  // Low volume threshold
}

#[tokio::test]
async fn test_enhanced_reasoning_generation() {
    let coordinator = create_test_coordinator().await.unwrap();
    let market_context = create_test_market_context();
    let historical_data = create_test_historical_data();
    
    // Test with comprehensive data availability
    let comprehensive_data = DataAvailability {
        completeness: 0.8,
        freshness: 0.9,
        quality: 0.85,
        source_count: 1, // Low source count to trigger warning
        market_coverage: 0.9,
        consistency: 0.8,
        latency_ms: 150.0, // High latency to trigger warning
        assessment_time: Utc::now(),
    };
    
    let enhanced_decision = coordinator
        .evaluate_with_data_context(&market_context, None, &historical_data, comprehensive_data)
        .await
        .unwrap();
    
    // Verify enhanced reasoning includes data quality assessment
    let has_data_quality = enhanced_decision.enhanced_reasoning.iter()
        .any(|r| r.contains("Data Quality Assessment"));
    assert!(has_data_quality);
    
    // Verify enhanced reasoning includes market timing
    let has_timing = enhanced_decision.enhanced_reasoning.iter()
        .any(|r| r.contains("Market Timing"));
    assert!(has_timing);
    
    // Verify warnings are included for problematic conditions
    let has_latency_warning = enhanced_decision.enhanced_reasoning.iter()
        .any(|r| r.contains("High data latency"));
    assert!(has_latency_warning);
    
    let has_source_warning = enhanced_decision.enhanced_reasoning.iter()
        .any(|r| r.contains("Limited data sources"));
    assert!(has_source_warning);
    
    // Verify enhanced analysis summary is included
    let has_enhanced_analysis = enhanced_decision.enhanced_reasoning.iter()
        .any(|r| r.contains("Enhanced Analysis"));
    assert!(has_enhanced_analysis);
}