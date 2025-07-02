use autonomous_platform::data::{TimeSeriesData, PredictionResult};
use autonomous_platform::integration::neural_predictions::{
    NeuralPredictionSystem, DecisionContext, MarketConditions, PredictionRequest, ModelType
};
use autonomous_platform::integration::daa_fann::{
    DaaFannIntegration, Decision, Agent, ActionResult, EnhancedDecision
};
use chrono::{DateTime, Utc, TimeZone};
use serde_json::json;
use std::collections::HashMap;
use tokio;

#[tokio::test]
async fn test_neural_prediction_system_new() {
    // Test: Creating a new neural prediction system with memory allocation
    let memory_gb = 4.0;
    let result = NeuralPredictionSystem::new(memory_gb).await;
    
    assert!(result.is_ok(), "Neural prediction system should initialize successfully");
    
    let system = result.unwrap();
    assert_eq!(system.memory_allocation(), memory_gb);
}

#[tokio::test]
async fn test_get_prediction_for_decision_with_trading_context() {
    // Test: Getting neural predictions for trading decisions
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "trader_agent_001".to_string(),
        decision_type: "BUY_SIGNAL".to_string(),
        symbol: "BTCUSD".to_string(),
        market_data: create_test_market_data(),
        context_metadata: create_context_metadata(),
        required_confidence: 0.75,
        prediction_horizon: 60, // 1 hour
    };
    
    let result = system.get_prediction_for_decision(decision_context).await;
    
    assert!(result.is_ok(), "Should get prediction for valid decision context");
    
    let prediction_result = result.unwrap();
    assert_eq!(prediction_result.symbol, "BTCUSD");
    assert!(prediction_result.confidence >= 0.0 && prediction_result.confidence <= 1.0);
    assert!(prediction_result.model_used.is_some());
    assert!(!prediction_result.prediction_values.is_empty());
}

#[tokio::test]
async fn test_select_optimal_model_based_on_market_conditions() {
    // Test: Intelligent model selection based on market conditions
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    // High volatility conditions should prefer NHITS for short-term predictions
    let high_volatility_conditions = MarketConditions {
        volatility: 0.85,
        trend_strength: 0.6,
        liquidity: 0.9,
        session: "ACTIVE".to_string(),
        news_sentiment: 0.2, // Negative sentiment
        market_phase: "VOLATILE".to_string(),
    };
    
    let result = system.select_optimal_model(high_volatility_conditions).await;
    
    assert!(result.is_ok(), "Should select optimal model for market conditions");
    
    let selected_model = result.unwrap();
    // In high volatility, should prefer NHITS or TCN for pattern recognition
    assert!(matches!(selected_model, ModelType::NHITS | ModelType::TCN));
}

#[tokio::test]
async fn test_batch_predictions_for_multiple_agents() {
    // Test: Processing multiple prediction requests from different agents
    let system = NeuralPredictionSystem::new(8.0).await.unwrap();
    
    let requests = vec![
        PredictionRequest {
            agent_id: "risk_manager_001".to_string(),
            symbol: "ETHUSD".to_string(),
            prediction_type: "RISK_ASSESSMENT".to_string(),
            market_data: create_test_market_data(),
            required_models: vec![ModelType::DeepAR], // Probabilistic for risk
            context: json!({"risk_threshold": 0.02}),
        },
        PredictionRequest {
            agent_id: "momentum_trader_001".to_string(),
            symbol: "BTCUSD".to_string(),
            prediction_type: "MOMENTUM_SIGNAL".to_string(),
            market_data: create_test_market_data(),
            required_models: vec![ModelType::TCN], // Pattern recognition
            context: json!({"lookback_window": 24}),
        },
        PredictionRequest {
            agent_id: "portfolio_optimizer_001".to_string(),
            symbol: "ADAUSD".to_string(),
            prediction_type: "PORTFOLIO_ALLOCATION".to_string(),
            market_data: create_test_market_data(),
            required_models: vec![ModelType::MLP], // Non-linear relationships
            context: json!({"rebalance_threshold": 0.05}),
        },
    ];
    
    let results = system.batch_predictions(requests).await;
    
    assert!(results.is_ok(), "Batch predictions should succeed");
    
    let prediction_results = results.unwrap();
    assert_eq!(prediction_results.len(), 3);
    
    // Verify each prediction has appropriate characteristics
    for result in prediction_results {
        assert!(!result.symbol.is_empty());
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        assert!(result.model_used.is_some());
        assert!(!result.prediction_values.is_empty());
    }
}

#[tokio::test]
async fn test_model_confidence_and_uncertainty_handling() {
    // Test: Handling model confidence and uncertainty bounds
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "uncertainty_analyst_001".to_string(),
        decision_type: "UNCERTAINTY_ANALYSIS".to_string(),
        symbol: "SOLUSD".to_string(),
        market_data: create_test_market_data(),
        context_metadata: create_context_metadata(),
        required_confidence: 0.9, // High confidence requirement
        prediction_horizon: 1440, // 24 hours
    };
    
    let result = system.get_prediction_for_decision(decision_context).await;
    
    assert!(result.is_ok(), "Should handle uncertainty analysis");
    
    let prediction_result = result.unwrap();
    assert!(prediction_result.uncertainty_bounds.is_some());
    assert!(prediction_result.confidence_interval.is_some());
    
    let uncertainty = prediction_result.uncertainty_bounds.unwrap();
    assert!(uncertainty.lower_bound < uncertainty.upper_bound);
}

#[tokio::test]
async fn test_caching_mechanism_for_predictions() {
    // Test: Prediction caching to avoid duplicate computations
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "cache_test_agent".to_string(),
        decision_type: "BUY_SIGNAL".to_string(),
        symbol: "BTCUSD".to_string(),
        market_data: create_test_market_data(),
        context_metadata: create_context_metadata(),
        required_confidence: 0.75,
        prediction_horizon: 60,
    };
    
    // First prediction - should compute and cache
    let start_time = std::time::Instant::now();
    let result1 = system.get_prediction_for_decision(decision_context.clone()).await;
    let first_duration = start_time.elapsed();
    
    assert!(result1.is_ok());
    
    // Second identical prediction - should use cache and be faster
    let start_time = std::time::Instant::now();
    let result2 = system.get_prediction_for_decision(decision_context).await;
    let second_duration = start_time.elapsed();
    
    assert!(result2.is_ok());
    
    // Cache hit should be significantly faster
    assert!(second_duration < first_duration);
    
    let prediction1 = result1.unwrap();
    let prediction2 = result2.unwrap();
    
    // Results should be identical (from cache)
    assert_eq!(prediction1.prediction_values, prediction2.prediction_values);
    assert_eq!(prediction1.confidence, prediction2.confidence);
}

#[tokio::test]
async fn test_model_fallback_mechanism() {
    // Test: Fallback to alternative models when primary model fails
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "fallback_test_agent".to_string(),
        decision_type: "COMPLEX_ANALYSIS".to_string(),
        symbol: "INVALID_SYMBOL".to_string(), // This should trigger fallback
        market_data: create_incomplete_market_data(), // Incomplete data
        context_metadata: create_context_metadata(),
        required_confidence: 0.5,
        prediction_horizon: 30,
    };
    
    let result = system.get_prediction_for_decision(decision_context).await;
    
    // Should still succeed with fallback model
    assert!(result.is_ok(), "Should fallback to alternative model");
    
    let prediction_result = result.unwrap();
    assert!(prediction_result.model_used.is_some());
    assert!(prediction_result.fallback_used);
}

// Helper functions for test data creation
fn create_test_market_data() -> TimeSeriesData {
    TimeSeriesData {
        symbol: "BTCUSD".to_string(),
        timestamp: Utc.timestamp_opt(1640995200, 0).unwrap(), // 2022-01-01
        open: 47000.0,
        high: 48500.0,
        low: 46500.0,
        close: 48000.0,
        volume: 1250000.0,
        indicators: {
            let mut indicators = HashMap::new();
            indicators.insert("RSI".to_string(), 65.5);
            indicators.insert("MACD".to_string(), 250.0);
            indicators.insert("BB_UPPER".to_string(), 49000.0);
            indicators.insert("BB_LOWER".to_string(), 46000.0);
            indicators.insert("SMA_20".to_string(), 47500.0);
            indicators.insert("EMA_12".to_string(), 47800.0);
            indicators
        },
    }
}

fn create_incomplete_market_data() -> TimeSeriesData {
    TimeSeriesData {
        symbol: "INCOMPLETE".to_string(),
        timestamp: Utc.timestamp_opt(1640995200, 0).unwrap(),
        open: 100.0,
        high: 105.0,
        low: 95.0,
        close: 102.0,
        volume: 0.0, // Missing volume data
        indicators: HashMap::new(), // No indicators
    }
}

fn create_context_metadata() -> HashMap<String, serde_json::Value> {
    let mut metadata = HashMap::new();
    metadata.insert("strategy".to_string(), json!("neural_momentum"));
    metadata.insert("risk_level".to_string(), json!(0.02));
    metadata.insert("position_size".to_string(), json!(0.1));
    metadata.insert("max_drawdown".to_string(), json!(0.05));
    metadata.insert("session".to_string(), json!("US_MARKET_HOURS"));
    metadata
}

#[tokio::test]
async fn test_integration_with_daa_orchestrator() {
    // Test: Integration with DAA orchestrator for autonomous decision making
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    // Simulate DAA agent requesting prediction for autonomous decision
    let daa_decision_context = DecisionContext {
        agent_id: "daa_autonomous_trader".to_string(),
        decision_type: "AUTONOMOUS_TRADE_EXECUTION".to_string(),
        symbol: "ETHUSD".to_string(),
        market_data: create_test_market_data(),
        context_metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("autonomous_mode".to_string(), json!(true));
            metadata.insert("decision_authority".to_string(), json!("HIGH"));
            metadata.insert("risk_budget".to_string(), json!(10000.0));
            metadata.insert("execution_window".to_string(), json!(300)); // 5 minutes
            metadata
        },
        required_confidence: 0.85, // High confidence for autonomous execution
        prediction_horizon: 15, // 15 minutes for execution window
    };
    
    let result = system.get_prediction_for_decision(daa_decision_context).await;
    
    assert!(result.is_ok(), "Should integrate with DAA orchestrator");
    
    let prediction_result = result.unwrap();
    
    // Autonomous decisions should have high confidence and clear recommendations
    assert!(prediction_result.confidence >= 0.8);
    assert!(prediction_result.execution_recommendations.is_some());
    assert!(prediction_result.risk_assessment.is_some());
    
    let execution_recs = prediction_result.execution_recommendations.unwrap();
    assert!(!execution_recs.is_empty());
}

#[tokio::test]
async fn test_memory_storage_integration() {
    // Test: Integration with Memory storage for result persistence
    let system = NeuralPredictionSystem::new(4.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "memory_test_agent".to_string(),
        decision_type: "LONG_TERM_FORECAST".to_string(),
        symbol: "BTCUSD".to_string(),
        market_data: create_test_market_data(),
        context_metadata: create_context_metadata(),
        required_confidence: 0.7,
        prediction_horizon: 4320, // 3 days
    };
    
    let result = system.get_prediction_for_decision(decision_context).await;
    
    assert!(result.is_ok(), "Should get prediction and store in memory");
    
    let prediction_result = result.unwrap();
    
    // Verify that the result is stored in memory with correct key structure
    let memory_key = format!(
        "swarm-auto-centralized-1751484080479/daa-fann-integration/predictions/{}_{}",
        prediction_result.symbol,
        prediction_result.timestamp
    );
    
    // The actual memory storage will be tested in integration
    assert!(!memory_key.is_empty());
    assert!(prediction_result.stored_in_memory);
}

// =================== DAA-FANN INTEGRATION TESTS ===================

#[tokio::test]
async fn test_daa_fann_integration_initialization() {
    // Test: Initialize DAA-FANN integration system
    let memory_gb = 8.0;
    let result = DaaFannIntegration::new(memory_gb).await;
    
    assert!(result.is_ok(), "DAA-FANN integration should initialize successfully");
    
    let integration = result.unwrap();
    assert_eq!(integration.memory_allocation(), memory_gb);
    assert!(integration.is_connected());
}

#[tokio::test]
async fn test_daa_agent_requests_fann_forecast() {
    // Test: DAA agent requests FANN forecast for decision making
    let integration = DaaFannIntegration::new(6.0).await.unwrap();
    
    let daa_agent = Agent {
        id: "momentum_trader_daa".to_string(),
        agent_type: "TradingAgent".to_string(),
        capabilities: vec!["trading".to_string(), "risk_management".to_string()],
        decision_authority: "HIGH".to_string(),
        active: true,
    };
    
    let decision = Decision {
        agent_id: daa_agent.id.clone(),
        decision_type: "EXECUTE_TRADE".to_string(),
        symbol: "BTCUSD".to_string(),
        market_data: create_test_market_data(),
        confidence_required: 0.8,
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(5),
        context: json!({
            "position_size": 0.1,
            "risk_budget": 5000.0,
            "strategy": "momentum_breakout"
        }),
    };
    
    let result = integration.handle_prediction_request(&daa_agent, &decision).await;
    
    assert!(result.is_ok(), "Should handle DAA prediction request successfully");
    
    let forecast_result = result.unwrap();
    assert_eq!(forecast_result.symbol, "BTCUSD");
    assert!(forecast_result.confidence >= 0.8); // Meets DAA requirements
    assert!(forecast_result.execution_window_valid);
    assert!(forecast_result.daa_compatible);
}

#[tokio::test]
async fn test_fann_forecast_influences_daa_decision() {
    // Test: FANN prediction results influence DAA agent decisions
    let integration = DaaFannIntegration::new(6.0).await.unwrap();
    
    let decision = Decision {
        agent_id: "risk_manager_daa".to_string(),
        decision_type: "RISK_ASSESSMENT".to_string(),
        symbol: "ETHUSD".to_string(),
        market_data: create_high_volatility_market_data(),
        confidence_required: 0.9,
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(10),
        context: json!({
            "max_risk": 0.02,
            "portfolio_exposure": 0.3,
            "volatility_threshold": 0.4
        }),
    };
    
    let result = integration.process_daa_decision(&decision).await;
    
    assert!(result.is_ok(), "Should process DAA decision with FANN influence");
    
    let action_result = result.unwrap();
    assert_eq!(action_result.decision_id, decision.agent_id);
    assert!(action_result.fann_influenced);
    assert!(action_result.risk_adjusted);
    
    // High volatility should trigger conservative recommendations
    if let Some(recommendations) = action_result.recommendations {
        assert!(recommendations.iter().any(|r| r.contains("REDUCE") || r.contains("CONSERVATIVE")));
    }
}

#[tokio::test]
async fn test_bidirectional_daa_fann_coordination() {
    // Test: Continuous feedback loop between DAA decisions and FANN forecasts
    let integration = DaaFannIntegration::new(8.0).await.unwrap();
    
    let decision_context = DecisionContext {
        agent_id: "portfolio_optimizer_daa".to_string(),
        decision_type: "PORTFOLIO_REBALANCE".to_string(),
        symbol: "PORTFOLIO".to_string(),
        market_data: create_test_market_data(),
        context_metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("current_allocation".to_string(), json!({
                "BTC": 0.4,
                "ETH": 0.3,
                "ADA": 0.2,
                "CASH": 0.1
            }));
            metadata.insert("target_volatility".to_string(), json!(0.15));
            metadata.insert("rebalance_threshold".to_string(), json!(0.05));
            metadata
        },
        required_confidence: 0.85,
        prediction_horizon: 1440, // 24 hours
    };
    
    let result = integration.coordinate_decision_with_forecast(decision_context).await;
    
    assert!(result.is_ok(), "Should coordinate DAA decision with FANN forecast");
    
    let enhanced_decision = result.unwrap();
    assert!(enhanced_decision.fann_enhanced);
    assert!(enhanced_decision.confidence >= 0.85);
    assert!(enhanced_decision.portfolio_optimization.is_some());
    
    let optimization = enhanced_decision.portfolio_optimization.unwrap();
    assert!(!optimization.new_allocation.is_empty());
    assert!(optimization.expected_return > 0.0);
    assert!(optimization.risk_level <= 0.15); // Within target volatility
}

#[tokio::test]
async fn test_batch_daa_agent_coordination() {
    // Test: Multiple DAA agents coordinating through FANN predictions
    let integration = DaaFannIntegration::new(12.0).await.unwrap();
    
    let agents = vec![
        Agent {
            id: "trader_agent_001".to_string(),
            agent_type: "TradingAgent".to_string(),
            capabilities: vec!["trading".to_string()],
            decision_authority: "MEDIUM".to_string(),
            active: true,
        },
        Agent {
            id: "risk_agent_001".to_string(),
            agent_type: "RiskAgent".to_string(),  
            capabilities: vec!["risk_management".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
        Agent {
            id: "portfolio_agent_001".to_string(),
            agent_type: "PortfolioAgent".to_string(),
            capabilities: vec!["portfolio_optimization".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
    ];
    
    let decisions = vec![
        Decision {
            agent_id: "trader_agent_001".to_string(),
            decision_type: "BUY_SIGNAL".to_string(),
            symbol: "BTCUSD".to_string(),
            market_data: create_test_market_data(),
            confidence_required: 0.75,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(5),
            context: json!({"position_size": 0.05}),
        },
        Decision {
            agent_id: "risk_agent_001".to_string(),
            decision_type: "RISK_CHECK".to_string(),
            symbol: "BTCUSD".to_string(),
            market_data: create_test_market_data(),
            confidence_required: 0.9,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(2),
            context: json!({"max_risk": 0.02}),
        },
        Decision {
            agent_id: "portfolio_agent_001".to_string(),
            decision_type: "ALLOCATION_CHECK".to_string(),
            symbol: "BTCUSD".to_string(),
            market_data: create_test_market_data(),
            confidence_required: 0.8,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(3),
            context: json!({"current_allocation": 0.3}),
        },
    ];
    
    let result = integration.coordinate_multi_agent_decisions(&agents, &decisions).await;
    
    assert!(result.is_ok(), "Should coordinate multiple DAA agent decisions");
    
    let coordination_result = result.unwrap();
    assert_eq!(coordination_result.processed_decisions, 3);
    assert!(coordination_result.coordination_successful);
    assert!(coordination_result.consensus_reached);
    
    // Risk agent should have veto power over trading decisions
    assert!(coordination_result.risk_validated);
}

#[tokio::test]
async fn test_daa_fann_memory_integration() {
    // Test: DAA-FANN integration stores results in Memory for swarm coordination
    let integration = DaaFannIntegration::new(6.0).await.unwrap();
    
    let decision = Decision {
        agent_id: "memory_test_daa".to_string(),
        decision_type: "STRATEGIC_ANALYSIS".to_string(),
        symbol: "ADAUSD".to_string(),
        market_data: create_test_market_data(),
        confidence_required: 0.8,
        execution_deadline: chrono::Utc::now() + chrono::Duration::hours(1),
        context: json!({
            "analysis_type": "long_term_trend",
            "store_in_memory": true,
            "memory_key": "strategic_analysis_ada"
        }),
    };
    
    let result = integration.process_daa_decision(&decision).await;
    
    assert!(result.is_ok(), "Should process decision and store in memory");
    
    let action_result = result.unwrap();
    assert!(action_result.stored_in_memory);
    
    let expected_memory_key = "swarm-auto-centralized-1751484080479/daa-fann-links/results/strategic_analysis_ada";
    assert_eq!(action_result.memory_key.unwrap(), expected_memory_key);
    
    // Verify results can be retrieved for other agents
    let memory_result = integration.get_memory_result("strategic_analysis_ada").await;
    assert!(memory_result.is_ok());
    assert!(memory_result.unwrap().is_some());
}

#[tokio::test]
async fn test_daa_fann_real_time_streaming() {
    // Test: Real-time streaming integration between DAA and FANN
    let integration = DaaFannIntegration::new(8.0).await.unwrap();
    
    // Simulate streaming market data updates
    let streaming_decisions = vec![
        create_streaming_decision("BTCUSD", 0.1),
        create_streaming_decision("ETHUSD", 0.15),
        create_streaming_decision("ADAUSD", 0.08),
    ];
    
    let result = integration.process_streaming_decisions(streaming_decisions).await;
    
    assert!(result.is_ok(), "Should handle streaming DAA decisions");
    
    let streaming_result = result.unwrap();
    assert_eq!(streaming_result.processed_count, 3);
    assert!(streaming_result.average_processing_time_ms < 100.0); // Sub-100ms processing
    assert!(streaming_result.all_forecasts_generated);
    
    // Verify each decision received appropriate FANN forecasts
    for decision_result in streaming_result.decision_results {
        assert!(decision_result.forecast_confidence >= 0.7);
        assert!(decision_result.processing_time_ms < 50.0); // Individual processing time
        assert!(decision_result.model_selected.is_some());
    }
}

// Helper functions for DAA-FANN integration tests

fn create_high_volatility_market_data() -> TimeSeriesData {
    TimeSeriesData {
        symbol: "ETHUSD".to_string(),
        timestamp: Utc.timestamp_opt(1640995200, 0).unwrap(),
        open: 3800.0,
        high: 4200.0, // High volatility range
        low: 3400.0,
        close: 3900.0,
        volume: 2500000.0,
        indicators: {
            let mut indicators = HashMap::new();
            indicators.insert("RSI".to_string(), 75.0); // Overbought
            indicators.insert("VOLATILITY".to_string(), 0.8); // High volatility
            indicators.insert("MACD".to_string(), -150.0); // Bearish divergence
            indicators.insert("BB_WIDTH".to_string(), 800.0); // Wide Bollinger bands = high volatility
            indicators.insert("VIX".to_string(), 35.0); // High fear index
            indicators
        },
    }
}

fn create_streaming_decision(symbol: &str, position_size: f64) -> Decision {
    Decision {
        agent_id: format!("streaming_agent_{}", symbol.to_lowercase()),
        decision_type: "STREAMING_TRADE".to_string(),
        symbol: symbol.to_string(),
        market_data: {
            let mut data = create_test_market_data();
            data.symbol = symbol.to_string();
            data
        },
        confidence_required: 0.7,
        execution_deadline: chrono::Utc::now() + chrono::Duration::seconds(30),
        context: json!({
            "position_size": position_size,
            "streaming": true,
            "priority": "HIGH"
        }),
    }
}