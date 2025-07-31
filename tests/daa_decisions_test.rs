//! Tests for the DAA Autonomous Decision System
//!
//! This module contains comprehensive tests for the autonomous decision-making
//! system using the daa-orchestrator and trading agents.

use autonomous_platform::{
    data::TimeSeriesData,
    integration::{autonomous_decisions::*, OrderSide, OrderType, TradeOrder},
    Result,
};
use chrono::{DateTime, Utc};
use serial_test::serial;
use std::collections::HashMap;
use tokio;

#[derive(Debug, Clone)]
pub struct MockMarketContext {
    pub symbol: String,
    pub current_price: f64,
    pub volume: f64,
    pub volatility: f64,
    pub trend: MarketTrend,
    pub support_level: f64,
    pub resistance_level: f64,
}

impl MockMarketContext {
    pub fn new_bullish(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            current_price: 100.0,
            volume: 1000000.0,
            volatility: 0.15,
            trend: MarketTrend::Bullish,
            support_level: 95.0,
            resistance_level: 110.0,
        }
    }

    pub fn new_bearish(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            current_price: 100.0,
            volume: 800000.0,
            volatility: 0.25,
            trend: MarketTrend::Bearish,
            support_level: 85.0,
            resistance_level: 105.0,
        }
    }

    pub fn new_sideways(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            current_price: 100.0,
            volume: 500000.0,
            volatility: 0.08,
            trend: MarketTrend::Sideways,
            support_level: 98.0,
            resistance_level: 102.0,
        }
    }
}

/// Test the creation and initialization of the AutonomousDecisionSystem
#[tokio::test]
#[serial]
async fn test_autonomous_decision_system_creation() {
    let result = AutonomousDecisionSystem::new().await;
    assert!(
        result.is_ok(),
        "Failed to create AutonomousDecisionSystem: {:?}",
        result.err()
    );

    let system = result.unwrap();
    assert_eq!(
        system.get_agent_count(),
        0,
        "New system should have no agents initially"
    );
}

/// Test spawning trading agents
#[tokio::test]
#[serial]
async fn test_spawn_trading_agents() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();

    let result = system.spawn_trading_agents().await;
    assert!(
        result.is_ok(),
        "Failed to spawn trading agents: {:?}",
        result.err()
    );

    // Should have all 5 agent types
    assert_eq!(
        system.get_agent_count(),
        5,
        "Should have spawned 5 trading agents"
    );

    // Verify agent types are present
    assert!(
        system.has_agent(&AgentType::MarketAnalysis),
        "Should have MarketAnalysis agent"
    );
    assert!(
        system.has_agent(&AgentType::RiskManagement),
        "Should have RiskManagement agent"
    );
    assert!(
        system.has_agent(&AgentType::SignalGeneration),
        "Should have SignalGeneration agent"
    );
    assert!(
        system.has_agent(&AgentType::Portfolio),
        "Should have Portfolio agent"
    );
    assert!(
        system.has_agent(&AgentType::Execution),
        "Should have Execution agent"
    );
}

/// Test autonomous decision making in bullish market conditions
#[tokio::test]
#[serial]
async fn test_autonomous_decision_bullish_market() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    let market_context = MarketContext {
        symbol: "BTC-USD".to_string(),
        current_price: 50000.0,
        volume: 1500000.0,
        volatility: 0.12,
        trend: MarketTrend::Bullish,
        support_level: 48000.0,
        resistance_level: 55000.0,
        rsi: 65.0,
        macd_signal: 0.8,
        bollinger_position: 0.7,
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(market_context).await;
    assert!(
        decision.is_ok(),
        "Failed to make autonomous decision: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();
    assert_eq!(trading_decision.symbol, "BTC-USD");
    assert!(trading_decision.confidence > 0.0 && trading_decision.confidence <= 1.0);

    // In bullish conditions, should lean towards buy signals
    match trading_decision.action {
        TradingAction::Buy | TradingAction::Hold => {
            // Expected behavior in bullish market
        }
        TradingAction::Sell => {
            // Should have strong risk management reason
            assert!(
                trading_decision.risk_score > 0.7,
                "Sell in bullish market should have high risk justification"
            );
        }
    }
}

/// Test autonomous decision making in bearish market conditions
#[tokio::test]
#[serial]
async fn test_autonomous_decision_bearish_market() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    let market_context = MarketContext {
        symbol: "ETH-USD".to_string(),
        current_price: 3000.0,
        volume: 800000.0,
        volatility: 0.35,
        trend: MarketTrend::Bearish,
        support_level: 2500.0,
        resistance_level: 3200.0,
        rsi: 25.0,
        macd_signal: -0.6,
        bollinger_position: 0.2,
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(market_context).await;
    assert!(
        decision.is_ok(),
        "Failed to make autonomous decision: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();
    assert_eq!(trading_decision.symbol, "ETH-USD");

    // In bearish conditions, should be more conservative
    match trading_decision.action {
        TradingAction::Sell | TradingAction::Hold => {
            // Expected behavior in bearish market
        }
        TradingAction::Buy => {
            // Should be based on strong oversold signals
            assert!(
                trading_decision.confidence > 0.8,
                "Buy in bearish market should have very high confidence"
            );
        }
    }
}

/// Test multi-agent consensus decision making
#[tokio::test]
#[serial]
async fn test_multi_agent_consensus_decision() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    let scenario = TradingScenario {
        symbol: "AAPL".to_string(),
        scenario_type: ScenarioType::EarningsAnnouncement,
        market_conditions: HashMap::from([
            ("volatility".to_string(), 0.45),
            ("volume_spike".to_string(), 2.5),
            ("news_sentiment".to_string(), 0.7),
        ]),
        time_horizon: TimeHorizon::ShortTerm,
        timestamp: Utc::now(),
    };

    let consensus = system.coordinate_multi_agent_decision(scenario).await;
    assert!(
        consensus.is_ok(),
        "Failed to reach consensus decision: {:?}",
        consensus.err()
    );

    let consensus_decision = consensus.unwrap();
    assert_eq!(consensus_decision.symbol, "AAPL");
    assert!(
        consensus_decision.agent_votes.len() >= 3,
        "Should have votes from multiple agents"
    );
    assert!(
        consensus_decision.consensus_strength >= 0.0
            && consensus_decision.consensus_strength <= 1.0
    );

    // Verify voting process
    let total_votes: f64 = consensus_decision.agent_votes.values().sum();
    assert!(
        (total_votes - 1.0).abs() < 0.01,
        "Agent votes should sum to approximately 1.0"
    );
}

/// Test decision engine with conflicting agent opinions
#[tokio::test]
#[serial]
async fn test_decision_engine_conflict_resolution() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    // Create a scenario where agents might disagree
    let conflicting_context = MarketContext {
        symbol: "TSLA".to_string(),
        current_price: 800.0,
        volume: 2000000.0,
        volatility: 0.60,             // Very high volatility
        trend: MarketTrend::Sideways, // Mixed signals
        support_level: 750.0,
        resistance_level: 850.0,
        rsi: 50.0,               // Neutral RSI
        macd_signal: 0.1,        // Weak signal
        bollinger_position: 0.5, // Middle of bands
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(conflicting_context).await;
    assert!(
        decision.is_ok(),
        "Failed to resolve conflicting opinions: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();

    // In conflicting scenarios, confidence should be moderate and action conservative
    assert!(
        trading_decision.confidence < 0.8,
        "High conflict should result in lower confidence"
    );

    // Should likely hold or have very small position sizes
    match trading_decision.action {
        TradingAction::Hold => {
            // Most expected outcome
        }
        TradingAction::Buy | TradingAction::Sell => {
            assert!(
                trading_decision.position_size < 0.5,
                "Conflicting signals should result in smaller position sizes"
            );
        }
    }
}

/// Test risk management agent behavior
#[tokio::test]
#[serial]
async fn test_risk_management_agent() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    // High risk scenario
    let high_risk_context = MarketContext {
        symbol: "MEME-COIN".to_string(),
        current_price: 1.0,
        volume: 10000.0, // Low volume
        volatility: 2.5, // Extremely high volatility
        trend: MarketTrend::Bullish,
        support_level: 0.5,
        resistance_level: 2.0,
        rsi: 85.0, // Overbought
        macd_signal: 1.5,
        bollinger_position: 0.95, // Near upper band
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(high_risk_context).await;
    assert!(
        decision.is_ok(),
        "Failed to handle high risk scenario: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();

    // Risk management should override bullish signals in extreme risk scenarios
    assert!(
        trading_decision.risk_score > 0.7,
        "Should identify high risk"
    );
    assert!(
        trading_decision.position_size < 0.3,
        "Should limit position size in high risk"
    );

    // Should not recommend large buy positions despite bullish trend
    if let TradingAction::Buy = trading_decision.action {
        assert!(
            trading_decision.position_size < 0.2,
            "High risk should severely limit buy position size"
        );
    }
}

/// Test portfolio agent coordination
#[tokio::test]
#[serial]
async fn test_portfolio_agent_coordination() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    // Set up portfolio state
    let mut portfolio_state = PortfolioState::new();
    portfolio_state.add_position("BTC-USD", 0.4); // 40% allocation
    portfolio_state.add_position("ETH-USD", 0.3); // 30% allocation
    portfolio_state.set_cash_allocation(0.3); // 30% cash

    system.set_portfolio_state(portfolio_state);

    // Test decision for new position when portfolio is well-balanced
    let market_context = MarketContext {
        symbol: "ADA-USD".to_string(),
        current_price: 2.0,
        volume: 500000.0,
        volatility: 0.25,
        trend: MarketTrend::Bullish,
        support_level: 1.8,
        resistance_level: 2.5,
        rsi: 55.0,
        macd_signal: 0.3,
        bollinger_position: 0.6,
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(market_context).await;
    assert!(
        decision.is_ok(),
        "Failed to make portfolio-aware decision: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();

    // Portfolio agent should limit new positions when already well-allocated
    if let TradingAction::Buy = trading_decision.action {
        assert!(
            trading_decision.position_size <= 0.3,
            "Should not over-allocate when portfolio is balanced"
        );
    }
}

/// Test execution agent order generation
#[tokio::test]
#[serial]
async fn test_execution_agent_order_generation() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    let market_context = MarketContext {
        symbol: "SOL-USD".to_string(),
        current_price: 150.0,
        volume: 1000000.0,
        volatility: 0.20,
        trend: MarketTrend::Bullish,
        support_level: 140.0,
        resistance_level: 165.0,
        rsi: 60.0,
        macd_signal: 0.5,
        bollinger_position: 0.65,
        timestamp: Utc::now(),
    };

    let decision = system.make_autonomous_decision(market_context).await;
    assert!(
        decision.is_ok(),
        "Failed to generate execution decision: {:?}",
        decision.err()
    );

    let trading_decision = decision.unwrap();

    // Test order generation
    if let TradingAction::Buy = trading_decision.action {
        let order = system.generate_trade_order(&trading_decision).await;
        assert!(
            order.is_ok(),
            "Failed to generate trade order: {:?}",
            order.err()
        );

        let trade_order = order.unwrap();
        assert_eq!(trade_order.symbol, "SOL-USD");
        assert!(matches!(trade_order.side, OrderSide::Buy));
        assert!(trade_order.quantity > 0.0);

        // Should use appropriate order type based on market conditions
        match trade_order.order_type {
            OrderType::Market => {
                // Acceptable for liquid markets
            }
            OrderType::Limit => {
                // Should have reasonable limit price
                assert!(trade_order.price.is_some());
                let limit_price = trade_order.price.unwrap();
                assert!(
                    limit_price > 140.0 && limit_price < 165.0,
                    "Limit price should be within reasonable range"
                );
            }
            _ => {
                // Other order types should be justified
            }
        }
    }
}

/// Test autonomous adaptation to market changes
#[tokio::test]
#[serial]
async fn test_autonomous_adaptation() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    // Initial bullish conditions
    let initial_context = MarketContext {
        symbol: "BNB-USD".to_string(),
        current_price: 400.0,
        volume: 800000.0,
        volatility: 0.15,
        trend: MarketTrend::Bullish,
        support_level: 380.0,
        resistance_level: 430.0,
        rsi: 70.0,
        macd_signal: 0.6,
        bollinger_position: 0.8,
        timestamp: Utc::now(),
    };

    let initial_decision = system
        .make_autonomous_decision(initial_context)
        .await
        .unwrap();

    // Simulate rapid market change to bearish
    let changed_context = MarketContext {
        symbol: "BNB-USD".to_string(),
        current_price: 350.0, // Price dropped
        volume: 1500000.0,    // Volume spike
        volatility: 0.45,     // High volatility
        trend: MarketTrend::Bearish,
        support_level: 320.0,
        resistance_level: 380.0,
        rsi: 30.0, // Oversold
        macd_signal: -0.8,
        bollinger_position: 0.1,
        timestamp: Utc::now(),
    };

    let adapted_decision = system
        .make_autonomous_decision(changed_context)
        .await
        .unwrap();

    // System should adapt its decision based on changed conditions
    assert_ne!(
        initial_decision.action, adapted_decision.action,
        "Decision should change with market conditions"
    );

    // Risk assessment should increase
    assert!(
        adapted_decision.risk_score > initial_decision.risk_score,
        "Risk score should increase in volatile conditions"
    );
}

/// Test system performance under stress
#[tokio::test]
#[serial]
async fn test_system_performance_stress() {
    let mut system = AutonomousDecisionSystem::new().await.unwrap();
    system.spawn_trading_agents().await.unwrap();

    let symbols = vec!["BTC-USD", "ETH-USD", "ADA-USD", "SOL-USD", "AVAX-USD"];
    let mut decisions = Vec::new();

    let start_time = std::time::Instant::now();

    // Process multiple decisions rapidly
    for symbol in symbols {
        let market_context = MarketContext {
            symbol: symbol.to_string(),
            current_price: 100.0,
            volume: 1000000.0,
            volatility: 0.20,
            trend: MarketTrend::Bullish,
            support_level: 95.0,
            resistance_level: 110.0,
            rsi: 60.0,
            macd_signal: 0.4,
            bollinger_position: 0.7,
            timestamp: Utc::now(),
        };

        let decision = system.make_autonomous_decision(market_context).await;
        assert!(
            decision.is_ok(),
            "Failed to make decision for {}: {:?}",
            symbol,
            decision.err()
        );
        decisions.push(decision.unwrap());
    }

    let elapsed = start_time.elapsed();

    // Should complete all decisions within reasonable time
    assert!(
        elapsed.as_millis() < 5000,
        "Decision making took too long: {:?}",
        elapsed
    );
    assert_eq!(
        decisions.len(),
        5,
        "Should have made decisions for all symbols"
    );

    // All decisions should be valid
    for decision in decisions {
        assert!(decision.confidence > 0.0);
        assert!(decision.position_size >= 0.0 && decision.position_size <= 1.0);
    }
}
