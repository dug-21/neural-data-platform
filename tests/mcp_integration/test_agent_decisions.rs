//! TDD Tests for MCP Agent Decision Tool

use anyhow::Result;
use serde_json::json;
use std::sync::Arc;

use autonomous_platform::mcp::trading_tools::TradingMcpTools;
use autonomous_platform::agents::{AutonomousAgent, AgentConfig, TradingStrategy};
use autonomous_platform::data::MarketContext;
use autonomous_platform::config::load_default_config;

#[tokio::test]
async fn test_agent_decision_basic_trade() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let agent_config = AgentConfig {
        id: "test-agent-001".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.5,
        max_position_size: 10000.0,
        decision_threshold: 0.7,
    };
    
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), Default::default(), agent.clone());
    
    // Set up market context
    let market_context = MarketContext {
        symbol: "BTC/USD".to_string(),
        current_price: 45000.0,
        trend: "bullish".to_string(),
        volatility: 0.02,
        volume_24h: 1_000_000.0,
        indicators: json!({
            "rsi": 65.0,
            "macd": {"signal": "buy", "strength": 0.8},
            "moving_avg_50": 44500.0,
            "moving_avg_200": 43000.0
        }),
    };
    
    agent.update_market_context(market_context).await?;
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "position_size": 5000.0,
        "current_position": 0.0
    });
    
    let result = tools.agent_decision(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "BTC/USD");
    assert!(result["decision"].is_string());
    
    let decision = result["decision"].as_str().unwrap();
    assert!(["buy", "sell", "hold"].contains(&decision));
    
    assert!(result["confidence"].as_f64().unwrap() >= 0.0);
    assert!(result["confidence"].as_f64().unwrap() <= 1.0);
    
    assert!(result["reasoning"].is_string());
    assert!(result["risk_assessment"].is_object());
    
    Ok(())
}

#[tokio::test]
async fn test_agent_decision_with_existing_position() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let agent_config = AgentConfig {
        id: "test-agent-002".to_string(),
        strategy: TradingStrategy::MeanReversion,
        risk_tolerance: 0.3,
        max_position_size: 20000.0,
        decision_threshold: 0.8,
    };
    
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), Default::default(), agent.clone());
    
    // Act
    let params = json!({
        "symbol": "ETH/USD",
        "position_size": 10000.0,
        "current_position": 5000.0,
        "entry_price": 2900.0,
        "current_price": 3100.0
    });
    
    let result = tools.agent_decision(params).await?;
    
    // Assert
    assert_eq!(result["symbol"], "ETH/USD");
    assert!(result["current_pnl"].is_number());
    assert!(result["position_recommendation"].is_object());
    
    let position_rec = &result["position_recommendation"];
    assert!(position_rec["action"].is_string());
    assert!(position_rec["size"].is_number());
    assert!(position_rec["stop_loss"].is_number());
    assert!(position_rec["take_profit"].is_number());
    
    Ok(())
}

#[tokio::test]
async fn test_agent_decision_risk_management() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let agent_config = AgentConfig {
        id: "test-agent-003".to_string(),
        strategy: TradingStrategy::Arbitrage,
        risk_tolerance: 0.1, // Very conservative
        max_position_size: 50000.0,
        decision_threshold: 0.9,
    };
    
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), Default::default(), agent.clone());
    
    // Set high volatility market
    let market_context = MarketContext {
        symbol: "SOL/USD".to_string(),
        current_price: 100.0,
        trend: "volatile".to_string(),
        volatility: 0.15, // High volatility
        volume_24h: 500_000.0,
        indicators: json!({
            "rsi": 85.0, // Overbought
            "atr": 5.0 // High average true range
        }),
    };
    
    agent.update_market_context(market_context).await?;
    
    // Act
    let params = json!({
        "symbol": "SOL/USD",
        "position_size": 30000.0,
        "portfolio_value": 100000.0
    });
    
    let result = tools.agent_decision(params).await?;
    
    // Assert
    assert!(result["risk_warnings"].is_array());
    let warnings = result["risk_warnings"].as_array().unwrap();
    assert!(!warnings.is_empty()); // Should have warnings due to high volatility
    
    assert!(result["adjusted_position_size"].is_number());
    let adjusted_size = result["adjusted_position_size"].as_f64().unwrap();
    assert!(adjusted_size < 30000.0); // Should reduce position due to risk
    
    Ok(())
}

#[tokio::test]
async fn test_agent_decision_multi_strategy() -> Result<()> {
    // Arrange
    let config = load_default_config()?;
    let agent_config = AgentConfig {
        id: "test-agent-004".to_string(),
        strategy: TradingStrategy::Hybrid(vec![
            TradingStrategy::Momentum,
            TradingStrategy::MeanReversion,
        ]),
        risk_tolerance: 0.5,
        max_position_size: 15000.0,
        decision_threshold: 0.75,
    };
    
    let agent = Arc::new(AutonomousAgent::new(agent_config)?);
    let tools = TradingMcpTools::new(Default::default(), Default::default(), Default::default(), agent.clone());
    
    // Act
    let params = json!({
        "symbol": "BTC/USD",
        "strategy_weights": {
            "momentum": 0.6,
            "mean_reversion": 0.4
        }
    });
    
    let result = tools.agent_decision(params).await?;
    
    // Assert
    assert!(result["strategy_signals"].is_object());
    let signals = result["strategy_signals"].as_object().unwrap();
    assert!(signals.contains_key("momentum"));
    assert!(signals.contains_key("mean_reversion"));
    
    assert!(result["combined_signal"].is_object());
    assert!(result["decision_breakdown"].is_object());
    
    Ok(())
}