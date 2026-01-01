//! Integration tests for DAA agent framework

use autonomous_platform::agents::{AgentConfig, AutonomousAgent, DAAAgent, TradingStrategy};
use autonomous_platform::data::MarketContext;
use chrono::Utc;
use std::collections::HashMap;

#[tokio::test]
async fn test_daa_agent_creation() {
    let config = AgentConfig {
        id: "test-agent-1".to_string(),
        strategy: TradingStrategy::Adaptive,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };

    let agent = DAAAgent::new(config.clone()).await;
    assert!(agent.is_ok());

    let agent = agent.unwrap();
    assert_eq!(agent.config.id, "test-agent-1");
}

#[tokio::test]
async fn test_daa_decision_making() {
    let config = AgentConfig {
        id: "decision-agent".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.3,
        max_position_size: 5000.0,
        decision_threshold: 0.6,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    let context = MarketContext {
        current_price: 100.0,
        volume: vec![10000.0],
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    };

    let decision = agent.make_decision(&context, 0.8).await;
    assert!(decision.is_ok());

    let decision = decision.unwrap();
    assert!(!decision.action.is_empty());
    assert!(decision.confidence >= 0.0 && decision.confidence <= 1.0);
    assert!(!decision.reasoning.is_empty());
}

#[tokio::test]
async fn test_risk_assessment() {
    let config = AgentConfig {
        id: "risk-agent".to_string(),
        strategy: TradingStrategy::MeanReversion,
        risk_tolerance: 0.2,
        max_position_size: 1000.0,
        decision_threshold: 0.8,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    let context = MarketContext {
        current_price: 150.0,
        volume: vec![5000.0],
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    };

    let risk = agent.assess_risk(&context).await;
    assert!(risk.is_ok());

    let risk_data = risk.unwrap();
    assert!(risk_data["riskScore"].as_f64().unwrap() >= 0.0);
    assert!(risk_data["riskScore"].as_f64().unwrap() <= 1.0);
}

#[tokio::test]
async fn test_strategy_signal() {
    let config = AgentConfig {
        id: "signal-agent".to_string(),
        strategy: TradingStrategy::Arbitrage,
        risk_tolerance: 0.4,
        max_position_size: 20000.0,
        decision_threshold: 0.65,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    let context = MarketContext {
        current_price: 99.5,
        volume: vec![15000.0],
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    };

    let signal = agent.get_strategy_signal(&context).await;
    assert!(signal.is_ok());

    let signal_data = signal.unwrap();
    assert!(signal_data.contains_key("signal"));
    assert!(signal_data.contains_key("strength"));
}

#[tokio::test]
async fn test_adaptive_performance() {
    let config = AgentConfig {
        id: "adaptive-agent".to_string(),
        strategy: TradingStrategy::Adaptive,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    // Simulate performance feedback
    let performance = serde_json::json!({
        "trades": 10,
        "winRate": 0.6,
        "profitFactor": 1.5,
        "sharpeRatio": 1.2
    });

    let result = agent.adapt_performance(performance).await;
    assert!(result.is_ok());

    let adaptation = result.unwrap();
    assert!(adaptation.contains_key("adapted"));
    assert!(adaptation["adapted"].as_bool().unwrap());
}

#[tokio::test]
async fn test_knowledge_sharing() {
    let config = AgentConfig {
        id: "knowledge-agent".to_string(),
        strategy: TradingStrategy::Hybrid(vec![
            TradingStrategy::Momentum,
            TradingStrategy::MeanReversion,
        ]),
        risk_tolerance: 0.35,
        max_position_size: 15000.0,
        decision_threshold: 0.7,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    let knowledge = serde_json::json!({
        "marketTrend": "bullish",
        "volatility": 0.25,
        "keyLevels": [98.0, 100.0, 102.0]
    });

    let result = agent.share_knowledge(knowledge).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_autonomous_agent_compatibility() {
    // Test that AutonomousAgent still works with DAA backend
    let config = AgentConfig {
        id: "compat-agent".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.4,
        max_position_size: 10000.0,
        decision_threshold: 0.6,
    };

    let agent = AutonomousAgent::new(config);

    let context = MarketContext {
        current_price: 101.0,
        volume: vec![12000.0],
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    };

    let decision = agent.analyze(&context, 0.75).await;
    assert!(decision.is_ok());

    let risk = agent.get_risk_assessment(&context).await;
    assert!(risk.is_ok());
}

#[tokio::test]
async fn test_multi_strategy_decision() {
    let config = AgentConfig {
        id: "multi-strategy".to_string(),
        strategy: TradingStrategy::Hybrid(vec![
            TradingStrategy::Momentum,
            TradingStrategy::MeanReversion,
            TradingStrategy::Arbitrage,
        ]),
        risk_tolerance: 0.3,
        max_position_size: 25000.0,
        decision_threshold: 0.75,
    };

    let agent = DAAAgent::new(config).await.unwrap();

    let context = MarketContext {
        current_price: 100.5,
        volume: vec![20000.0],
        timestamp: Utc::now(),
        metadata: HashMap::new(),
    };

    let decision = agent.make_decision(&context, 0.8).await.unwrap();

    // Hybrid strategy should provide breakdown
    assert!(decision.breakdown.is_some());
    let breakdown = decision.breakdown.unwrap();
    assert!(breakdown.get("momentum").is_some());
    assert!(breakdown.get("meanReversion").is_some());
    assert!(breakdown.get("arbitrage").is_some());
}
