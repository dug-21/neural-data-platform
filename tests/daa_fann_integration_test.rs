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
        market_data: create_test_high_volatility_market_data(),
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

fn create_test_high_volatility_market_data() -> TimeSeriesData {
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

// =================== ENHANCED MULTI-AGENT INTEGRATION TESTS ===================

mod common;
use common::{
    create_realistic_market_data, create_high_volatility_market_data,
    MarketScenario, create_decision_metadata, memory, assertions
};

#[tokio::test]
async fn test_multi_agent_consensus_mechanism() {
    // Test: Multiple agents reaching consensus on market decisions
    let integration = DaaFannIntegration::new(12.0).await.unwrap();
    
    // Create diverse agent types with different specializations
    let agents = vec![
        Agent {
            id: "risk_specialist_001".to_string(),
            agent_type: "RiskSpecialist".to_string(),
            capabilities: vec!["risk_assessment".to_string(), "volatility_analysis".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
        Agent {
            id: "momentum_trader_001".to_string(),
            agent_type: "MomentumTrader".to_string(),
            capabilities: vec!["trend_analysis".to_string(), "momentum_signals".to_string()],
            decision_authority: "MEDIUM".to_string(),
            active: true,
        },
        Agent {
            id: "mean_reversion_001".to_string(),
            agent_type: "MeanReversionTrader".to_string(),
            capabilities: vec!["statistical_arbitrage".to_string(), "mean_reversion".to_string()],
            decision_authority: "MEDIUM".to_string(),
            active: true,
        },
        Agent {
            id: "market_maker_001".to_string(),
            agent_type: "MarketMaker".to_string(),
            capabilities: vec!["liquidity_provision".to_string(), "spread_capture".to_string()],
            decision_authority: "LOW".to_string(),
            active: true,
        },
        Agent {
            id: "sentiment_analyst_001".to_string(),
            agent_type: "SentimentAnalyst".to_string(),
            capabilities: vec!["news_analysis".to_string(), "social_sentiment".to_string()],
            decision_authority: "LOW".to_string(),
            active: true,
        },
    ];
    
    // Create decision scenario requiring consensus
    let market_scenario = MarketScenario::HighVolatility.generate_data("BTC/USD", 45000.0);
    
    let consensus_decisions = agents.iter().map(|agent| {
        Decision {
            agent_id: agent.id.clone(),
            decision_type: "CONSENSUS_TRADE".to_string(),
            symbol: "BTC/USD".to_string(),
            market_data: market_scenario.clone(),
            confidence_required: 0.8,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(10),
            context: json!({
                "consensus_required": true,
                "agent_specialization": agent.agent_type,
                "decision_weight": match agent.decision_authority.as_str() {
                    "HIGH" => 3.0,
                    "MEDIUM" => 2.0,
                    "LOW" => 1.0,
                    _ => 1.0
                }
            }),
        }
    }).collect::<Vec<_>>();
    
    // Process consensus decision
    let consensus_result = integration.process_consensus_decisions(&agents, &consensus_decisions).await;
    
    assert!(consensus_result.is_ok(), "Consensus processing should succeed");
    
    let consensus = consensus_result.unwrap();
    assert!(consensus.consensus_reached, "Agents should reach consensus");
    assert!(consensus.confidence_score > 0.8, "High confidence consensus required");
    assert_eq!(consensus.participating_agents, 5);
    assert!(!consensus.final_decision.is_empty());
    
    // Risk specialist should have highest influence due to HIGH authority
    assert!(consensus.agent_weights.get("risk_specialist_001").unwrap() > &2.5);
    
    // Verify decision is weighted properly
    assert!(consensus.weighted_confidence > consensus.simple_average_confidence);
    
    // Store consensus results in memory
    let memory_data = memory::store_test_results(
        "multi_agent_consensus",
        true,
        HashMap::from([
            ("consensus_confidence".to_string(), json!(consensus.confidence_score)),
            ("participating_agents".to_string(), json!(consensus.participating_agents)),
            ("consensus_time_ms".to_string(), json!(consensus.processing_time_ms))
        ])
    );
}

#[tokio::test]
async fn test_agent_disagreement_resolution() {
    // Test: How system handles conflicting agent recommendations
    let integration = DaaFannIntegration::new(8.0).await.unwrap();
    
    // Create agents with conflicting views
    let bullish_agent = Agent {
        id: "bullish_momentum_001".to_string(),
        agent_type: "MomentumBull".to_string(),
        capabilities: vec!["uptrend_detection".to_string()],
        decision_authority: "MEDIUM".to_string(),
        active: true,
    };
    
    let bearish_agent = Agent {
        id: "bearish_reversal_001".to_string(),
        agent_type: "ReversionBear".to_string(),
        capabilities: vec!["reversal_signals".to_string()],
        decision_authority: "MEDIUM".to_string(),
        active: true,
    };
    
    let neutral_arbiter = Agent {
        id: "neutral_arbiter_001".to_string(),
        agent_type: "NeutralArbiter".to_string(),
        capabilities: vec!["conflict_resolution".to_string(), "risk_management".to_string()],
        decision_authority: "HIGH".to_string(),
        active: true,
    };
    
    // Create conflicting decisions
    let conflicting_decisions = vec![
        Decision {
            agent_id: bullish_agent.id.clone(),
            decision_type: "BUY_STRONG".to_string(),
            symbol: "ETH/USD".to_string(),
            market_data: MarketScenario::TrendingUp.generate_data("ETH/USD", 3000.0),
            confidence_required: 0.85,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(5),
            context: json!({
                "direction": "BULLISH",
                "strength": "STRONG",
                "position_size": 0.2
            }),
        },
        Decision {
            agent_id: bearish_agent.id.clone(),
            decision_type: "SELL_STRONG".to_string(),
            symbol: "ETH/USD".to_string(),
            market_data: MarketScenario::TrendingDown.generate_data("ETH/USD", 3000.0),
            confidence_required: 0.85,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(5),
            context: json!({
                "direction": "BEARISH",
                "strength": "STRONG",
                "position_size": 0.2
            }),
        },
        Decision {
            agent_id: neutral_arbiter.id.clone(),
            decision_type: "ARBITRATE_CONFLICT".to_string(),
            symbol: "ETH/USD".to_string(),
            market_data: create_realistic_market_data("ETH/USD", 3000.0, 0.05),
            confidence_required: 0.9,
            execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(3),
            context: json!({
                "arbitration_mode": true,
                "conflict_resolution": true,
                "risk_limit": 0.1
            }),
        },
    ];
    
    let agents_list = vec![bullish_agent, bearish_agent, neutral_arbiter];
    
    // Process conflicting decisions
    let resolution_result = integration.resolve_agent_conflicts(&agents_list, &conflicting_decisions).await;
    
    assert!(resolution_result.is_ok(), "Conflict resolution should succeed");
    
    let resolution = resolution_result.unwrap();
    assert!(resolution.conflict_detected, "Should detect agent conflict");
    assert_eq!(resolution.conflicting_agents.len(), 2); // Bull and bear agents
    assert_eq!(resolution.arbiter_agent, "neutral_arbiter_001");
    
    // Final decision should be influenced by high-authority arbiter
    assert!(resolution.final_decision.decision_authority >= "HIGH");
    assert!(resolution.risk_adjusted, "Decision should be risk-adjusted due to conflict");
    
    // Position size should be reduced due to uncertainty
    assert!(resolution.recommended_position_size < 0.15); // Less than either original
    
    // Confidence should reflect uncertainty
    assert!(resolution.confidence_with_uncertainty < 0.8);
}

#[tokio::test]
async fn test_neural_model_fallback_scenarios() {
    // Test: Model fallback when primary models fail or have low confidence
    let integration = DaaFannIntegration::new(6.0).await.unwrap();
    
    // Test scenario 1: Primary model unavailable
    let primary_unavailable_context = DecisionContext {
        agent_id: "model_fallback_test_001".to_string(),
        decision_type: "FORECAST_WITH_FALLBACK".to_string(),
        symbol: "ADA/USD".to_string(),
        market_data: create_realistic_market_data("ADA/USD", 1.2, 0.03),
        context_metadata: {
            let mut metadata = create_decision_metadata("fallback_test", 0.02);
            metadata.insert("primary_model".to_string(), json!("NHITS"));
            metadata.insert("primary_model_status".to_string(), json!("UNAVAILABLE"));
            metadata.insert("require_prediction".to_string(), json!(true));
            metadata
        },
        required_confidence: 0.7,
        prediction_horizon: 120,
    };
    
    let fallback_result = integration.handle_model_fallback(primary_unavailable_context).await;
    
    assert!(fallback_result.is_ok(), "Should handle model fallback gracefully");
    
    let fallback_prediction = fallback_result.unwrap();
    assert!(fallback_prediction.fallback_used, "Should indicate fallback was used");
    assert!(fallback_prediction.model_used.is_some());
    assert_ne!(fallback_prediction.model_used.unwrap(), "NHITS"); // Should use different model
    assert!(fallback_prediction.confidence >= 0.7, "Should meet confidence requirements");
    assert!(fallback_prediction.fallback_chain.len() > 0, "Should record fallback chain");
    
    // Test scenario 2: Model cascade (multiple fallbacks)
    let cascade_context = DecisionContext {
        agent_id: "model_cascade_test_001".to_string(),
        decision_type: "FORECAST_CASCADE".to_string(),
        symbol: "SOL/USD".to_string(),
        market_data: create_high_volatility_market_data("SOL/USD", 100.0),
        context_metadata: {
            let mut metadata = create_decision_metadata("cascade_test", 0.03);
            metadata.insert("primary_models".to_string(), json!(["NHITS", "DeepAR"]));
            metadata.insert("models_status".to_string(), json!({
                "NHITS": "UNAVAILABLE",
                "DeepAR": "LOW_CONFIDENCE"
            }));
            metadata.insert("min_confidence".to_string(), json!(0.8));
            metadata
        },
        required_confidence: 0.8,
        prediction_horizon: 60,
    };
    
    let cascade_result = integration.handle_model_cascade(cascade_context).await;
    
    assert!(cascade_result.is_ok(), "Should handle model cascade");
    
    let cascade_prediction = cascade_result.unwrap();
    assert!(cascade_prediction.cascade_used, "Should indicate cascade was used");
    assert!(cascade_prediction.fallback_chain.len() >= 2, "Should have tried multiple models");
    assert!(cascade_prediction.final_model_confidence >= 0.8, "Should meet confidence threshold");
    
    // Test scenario 3: All models fail - emergency mode
    let emergency_context = DecisionContext {
        agent_id: "emergency_mode_test_001".to_string(),
        decision_type: "EMERGENCY_FORECAST".to_string(),
        symbol: "DOGE/USD".to_string(),
        market_data: create_realistic_market_data("DOGE/USD", 0.08, 0.2), // High volatility
        context_metadata: {
            let mut metadata = create_decision_metadata("emergency_test", 0.05);
            metadata.insert("all_models_failed".to_string(), json!(true));
            metadata.insert("emergency_mode".to_string(), json!(true));
            metadata.insert("use_historical_average".to_string(), json!(true));
            metadata
        },
        required_confidence: 0.5, // Lower threshold for emergency
        prediction_horizon: 30,
    };
    
    let emergency_result = integration.handle_emergency_mode(emergency_context).await;
    
    assert!(emergency_result.is_ok(), "Should handle emergency mode");
    
    let emergency_prediction = emergency_result.unwrap();
    assert!(emergency_prediction.emergency_mode_used, "Should indicate emergency mode");
    assert!(emergency_prediction.model_used.is_none() || 
            emergency_prediction.model_used.unwrap() == "HistoricalAverage", 
            "Should use basic fallback method");
    assert!(emergency_prediction.confidence >= 0.5, "Should meet lowered confidence threshold");
    assert!(emergency_prediction.risk_warning_issued, "Should issue risk warning in emergency mode");
}

#[tokio::test]
async fn test_real_world_market_stress_scenarios() {
    // Test: System behavior during market stress events
    let integration = DaaFannIntegration::new(10.0).await.unwrap();
    
    // Scenario 1: Flash crash (2010-style)
    let flash_crash_data = MarketScenario::FlashCrashRecovery.generate_data("SPY", 400.0);
    let flash_crash_decision = Decision {
        agent_id: "crisis_manager_001".to_string(),
        decision_type: "CRISIS_RESPONSE".to_string(),
        symbol: "SPY".to_string(),
        market_data: flash_crash_data,
        confidence_required: 0.95, // High confidence needed during crisis
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(1),
        context: json!({
            "crisis_type": "FLASH_CRASH",
            "severity": "HIGH",
            "risk_limit": 0.005, // 0.5% max risk
            "emergency_protocols": true
        }),
    };
    
    let crisis_result = integration.handle_crisis_scenario(&flash_crash_decision).await;
    
    assert!(crisis_result.is_ok(), "Should handle flash crash scenario");
    
    let crisis_response = crisis_result.unwrap();
    assert!(crisis_response.crisis_detected, "Should detect crisis");
    assert!(crisis_response.emergency_protocols_activated, "Should activate emergency protocols");
    assert!(crisis_response.position_sizes_reduced, "Should reduce position sizes");
    assert!(crisis_response.trading_halted_recommendations.len() > 0, "Should recommend trading halts");
    assert!(crisis_response.risk_limits_tightened, "Should tighten risk limits");
    
    // Scenario 2: Market regime change (Bull to Bear)
    let regime_change_agents = vec![
        Agent {
            id: "regime_detector_001".to_string(),
            agent_type: "RegimeDetector".to_string(),
            capabilities: vec!["regime_analysis".to_string(), "structural_breaks".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
        Agent {
            id: "portfolio_rebalancer_001".to_string(),
            agent_type: "PortfolioRebalancer".to_string(),
            capabilities: vec!["allocation_optimization".to_string(), "risk_parity".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
    ];
    
    let regime_change_decisions = vec![
        Decision {
            agent_id: "regime_detector_001".to_string(),
            decision_type: "REGIME_CHANGE_DETECTION".to_string(),
            symbol: "MARKET_REGIME".to_string(),
            market_data: create_high_volatility_market_data("VIX", 35.0),
            confidence_required: 0.9,
            execution_deadline: chrono::Utc::now() + chrono::Duration::hours(1),
            context: json!({
                "previous_regime": "BULL_MARKET",
                "detected_regime": "BEAR_MARKET",
                "regime_probability": 0.87,
                "volatility_spike": true,
                "correlation_breakdown": true
            }),
        },
        Decision {
            agent_id: "portfolio_rebalancer_001".to_string(),
            decision_type: "REGIME_ADAPTATION".to_string(),
            symbol: "PORTFOLIO".to_string(),
            market_data: create_realistic_market_data("PORTFOLIO", 100.0, 0.15),
            confidence_required: 0.85,
            execution_deadline: chrono::Utc::now() + chrono::Duration::hours(2),
            context: json!({
                "current_allocation": {
                    "stocks": 0.6,
                    "bonds": 0.3,
                    "alternatives": 0.1
                },
                "target_allocation": {
                    "stocks": 0.3,
                    "bonds": 0.5,
                    "alternatives": 0.2
                },
                "rebalance_urgency": "HIGH"
            }),
        },
    ];
    
    let regime_result = integration.coordinate_regime_change(&regime_change_agents, &regime_change_decisions).await;
    
    assert!(regime_result.is_ok(), "Should handle regime change");
    
    let regime_response = regime_result.unwrap();
    assert!(regime_response.regime_change_detected, "Should detect regime change");
    assert!(regime_response.rebalancing_triggered, "Should trigger rebalancing");
    assert!(regime_response.new_risk_parameters.is_some(), "Should set new risk parameters");
    assert!(regime_response.coordination_successful, "Agents should coordinate successfully");
    
    // Verify risk parameters were adjusted for bear market
    let risk_params = regime_response.new_risk_parameters.unwrap();
    assert!(risk_params.max_position_size < 0.05, "Should reduce max position size");
    assert!(risk_params.correlation_threshold > 0.8, "Should increase correlation threshold");
    assert!(risk_params.volatility_limit < 0.2, "Should decrease volatility limit");
}

#[tokio::test]
async fn test_swarm_intelligence_emergence() {
    // Test: Emergent behavior from agent interactions
    let integration = DaaFannIntegration::new(16.0).await.unwrap();
    
    // Create a diverse swarm of agents
    let swarm_agents = vec![
        Agent {
            id: "scout_001".to_string(),
            agent_type: "ScoutAgent".to_string(),
            capabilities: vec!["opportunity_detection".to_string(), "market_scanning".to_string()],
            decision_authority: "LOW".to_string(),
            active: true,
        },
        Agent {
            id: "scout_002".to_string(),
            agent_type: "ScoutAgent".to_string(),
            capabilities: vec!["opportunity_detection".to_string(), "market_scanning".to_string()],
            decision_authority: "LOW".to_string(),
            active: true,
        },
        Agent {
            id: "worker_001".to_string(),
            agent_type: "WorkerAgent".to_string(),
            capabilities: vec!["trade_execution".to_string(), "order_management".to_string()],
            decision_authority: "MEDIUM".to_string(),
            active: true,
        },
        Agent {
            id: "worker_002".to_string(),
            agent_type: "WorkerAgent".to_string(),
            capabilities: vec!["trade_execution".to_string(), "order_management".to_string()],
            decision_authority: "MEDIUM".to_string(),
            active: true,
        },
        Agent {
            id: "queen_001".to_string(),
            agent_type: "QueenAgent".to_string(),
            capabilities: vec!["strategic_oversight".to_string(), "resource_allocation".to_string()],
            decision_authority: "HIGH".to_string(),
            active: true,
        },
    ];
    
    // Simulate swarm discovering and exploiting arbitrage opportunity
    let arbitrage_opportunities = vec![
        ("BTC/USD", 45000.0, "Exchange_A"),
        ("BTC/USD", 45100.0, "Exchange_B"),
        ("BTC/USD", 45050.0, "Exchange_C"),
    ];
    
    let mut swarm_decisions = Vec::new();
    
    // Scouts detect opportunities
    for (i, (symbol, price, exchange)) in arbitrage_opportunities.iter().enumerate() {
        swarm_decisions.push(Decision {
            agent_id: format!("scout_{:03}", (i % 2) + 1),
            decision_type: "OPPORTUNITY_DETECTION".to_string(),
            symbol: symbol.to_string(),
            market_data: {
                let mut data = create_realistic_market_data(symbol, *price, 0.001);
                data.source = exchange.to_string();
                data
            },
            confidence_required: 0.6,
            execution_deadline: chrono::Utc::now() + chrono::Duration::seconds(30),
            context: json!({
                "opportunity_type": "ARBITRAGE",
                "exchange": exchange,
                "signal_strength": 0.8,
                "scout_id": format!("scout_{:03}", (i % 2) + 1)
            }),
        });
    }
    
    // Workers plan execution
    swarm_decisions.push(Decision {
        agent_id: "worker_001".to_string(),
        decision_type: "EXECUTION_PLANNING".to_string(),
        symbol: "BTC/USD".to_string(),
        market_data: create_realistic_market_data("BTC/USD", 45050.0, 0.001),
        confidence_required: 0.8,
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(1),
        context: json!({
            "buy_exchange": "Exchange_A",
            "sell_exchange": "Exchange_B",
            "profit_target": 100.0,
            "max_position": 1.0
        }),
    });
    
    // Queen provides strategic oversight
    swarm_decisions.push(Decision {
        agent_id: "queen_001".to_string(),
        decision_type: "STRATEGIC_APPROVAL".to_string(),
        symbol: "BTC/USD".to_string(),
        market_data: create_realistic_market_data("BTC/USD", 45050.0, 0.001),
        confidence_required: 0.9,
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(2),
        context: json!({
            "swarm_coordination": true,
            "risk_budget": 10000.0,
            "strategic_priority": "PROFIT_OPTIMIZATION",
            "coordination_bonus": 1.2
        }),
    });
    
    // Process swarm intelligence
    let swarm_result = integration.process_swarm_intelligence(&swarm_agents, &swarm_decisions).await;
    
    assert!(swarm_result.is_ok(), "Swarm intelligence should process successfully");
    
    let swarm_response = swarm_result.unwrap();
    assert!(swarm_response.swarm_coordination_successful, "Swarm should coordinate successfully");
    assert!(swarm_response.emergent_behavior_detected, "Should detect emergent behavior");
    assert_eq!(swarm_response.participating_agents, 5);
    
    // Verify swarm amplification effects
    assert!(swarm_response.collective_confidence > 0.9, "Collective confidence should be high");
    assert!(swarm_response.efficiency_multiplier > 1.0, "Should have efficiency gains from coordination");
    assert!(swarm_response.risk_distribution_optimized, "Risk should be distributed optimally");
    
    // Check emergent properties
    let emergent_properties = &swarm_response.emergent_properties;
    assert!(emergent_properties.contains_key("collective_intelligence"));
    assert!(emergent_properties.contains_key("adaptive_specialization"));
    assert!(emergent_properties.contains_key("resource_optimization"));
    
    // Verify profit optimization from swarm behavior
    assert!(swarm_response.expected_profit > 
            swarm_response.individual_agent_profit * swarm_response.efficiency_multiplier);
}

#[tokio::test]
async fn test_memory_driven_agent_learning() {
    // Test: Agents learning and adapting from historical memory
    let integration = DaaFannIntegration::new(8.0).await.unwrap();
    
    // Create learning agent
    let learning_agent = Agent {
        id: "adaptive_learner_001".to_string(),
        agent_type: "AdaptiveLearner".to_string(),
        capabilities: vec![
            "pattern_recognition".to_string(), 
            "strategy_adaptation".to_string(),
            "memory_integration".to_string()
        ],
        decision_authority: "MEDIUM".to_string(),
        active: true,
    };
    
    // Simulate historical decision outcomes stored in memory
    let historical_memory = vec![
        json!({
            "decision_id": "hist_001",
            "symbol": "ETH/USD",
            "action": "BUY",
            "confidence": 0.8,
            "outcome": "SUCCESS",
            "profit_loss": 150.0,
            "market_conditions": "TRENDING_UP",
            "timestamp": (Utc::now() - chrono::Duration::days(1)).timestamp()
        }),
        json!({
            "decision_id": "hist_002",
            "symbol": "ETH/USD",
            "action": "BUY",
            "confidence": 0.9,
            "outcome": "FAILURE",
            "profit_loss": -80.0,
            "market_conditions": "HIGH_VOLATILITY",
            "timestamp": (Utc::now() - chrono::Duration::hours(12)).timestamp()
        }),
        json!({
            "decision_id": "hist_003",
            "symbol": "ETH/USD",
            "action": "SELL",
            "confidence": 0.7,
            "outcome": "SUCCESS",
            "profit_loss": 200.0,
            "market_conditions": "TRENDING_DOWN",
            "timestamp": (Utc::now() - chrono::Duration::hours(6)).timestamp()
        }),
    ];
    
    // Store historical data in memory
    for (i, historical_record) in historical_memory.iter().enumerate() {
        let memory_key = format!("swarm-auto-centralized-1751484080479/agent-learning/history_{}", i);
        integration.store_historical_decision(&memory_key, historical_record).await
            .expect("Should store historical data");
    }
    
    // Create new decision that should learn from history
    let learning_decision = Decision {
        agent_id: learning_agent.id.clone(),
        decision_type: "ADAPTIVE_TRADE".to_string(),
        symbol: "ETH/USD".to_string(),
        market_data: create_high_volatility_market_data("ETH/USD", 3000.0),
        confidence_required: 0.8,
        execution_deadline: chrono::Utc::now() + chrono::Duration::minutes(5),
        context: json!({
            "use_historical_learning": true,
            "learning_window_hours": 48,
            "adaptation_strength": 0.7,
            "pattern_matching_enabled": true
        }),
    };
    
    // Process decision with memory-driven learning
    let learning_result = integration.process_memory_driven_learning(&learning_agent, &learning_decision).await;
    
    assert!(learning_result.is_ok(), "Memory-driven learning should succeed");
    
    let learning_response = learning_result.unwrap();
    assert!(learning_response.historical_patterns_found, "Should find historical patterns");
    assert!(learning_response.strategy_adapted, "Should adapt strategy based on history");
    assert!(learning_response.confidence_adjusted, "Should adjust confidence based on past outcomes");
    
    // Agent should learn from high volatility failure
    assert!(learning_response.risk_adjustment > 0.1, "Should increase risk adjustment");
    assert!(learning_response.adapted_confidence < learning_decision.confidence_required, 
            "Should lower confidence in high volatility");
    
    // Verify learning outcomes
    let learning_insights = &learning_response.learning_insights;
    assert!(learning_insights.contains_key("pattern_success_rate"));
    assert!(learning_insights.contains_key("volatility_impact"));
    assert!(learning_insights.contains_key("confidence_calibration"));
    
    let success_rate = learning_insights.get("pattern_success_rate").unwrap().as_f64().unwrap();
    assert!(success_rate >= 0.0 && success_rate <= 1.0, "Success rate should be between 0 and 1");
    
    // Store learning outcomes back to memory for future reference
    let learning_memory_key = memory::integration_test_results_key();
    let learning_metrics = memory::store_test_results(
        "memory_driven_learning",
        true,
        HashMap::from([
            ("patterns_found".to_string(), json!(learning_response.historical_patterns_found)),
            ("strategy_adapted".to_string(), json!(learning_response.strategy_adapted)),
            ("success_rate".to_string(), json!(success_rate)),
            ("risk_adjustment".to_string(), json!(learning_response.risk_adjustment))
        ])
    );
}