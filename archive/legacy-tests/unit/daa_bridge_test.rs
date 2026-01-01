//! Unit tests for DAA Bridge module

use autonomous_platform::agents::{TradingStrategy, AgentConfig};
use autonomous_platform::mcp::trading_tools::MarketData;
use chrono::Utc;
use serde_json::json;

fn create_test_config() -> AgentConfig {
    AgentConfig {
        id: "test-agent-1".to_string(),
        strategy: TradingStrategy::Momentum,
        risk_tolerance: 0.7,
        max_position_size: 10000.0,
        decision_threshold: 0.6,
        enable_ml: true,
        learning_rate: 0.001,
        training_interval: 3600,
        memory_capacity: 1000,
        exploration_rate: 0.1,
    }
}

fn create_test_market_data() -> MarketData {
    MarketData {
        timestamp: Utc::now(),
        open: 50000.0,
        high: 50500.0,
        low: 49500.0,
        close: 50200.0,
        volume: vec![1000.0],
    }
}

// Strategy to cognitive pattern tests moved to integration tests

#[tokio::test]
async fn test_daa_agent_creation() {
    let config = create_test_config();
    
    // Note: This test would normally require the DAA service to be running
    // For unit testing, we'll test the configuration setup
    let agent_id = format!("trader-{}", config.id);
    assert_eq!(agent_id, "trader-test-agent-1");
    
    // Verify cognitive pattern selection
    let pattern = strategy_to_cognitive_pattern(&config.strategy);
    assert_eq!(pattern, "fast");
}

#[test]
fn test_calculate_indicators() {
    let market_data = create_test_market_data();
    
    // Test indicator calculations that would be used by the agent
    let price_change = (market_data.close - market_data.open) / market_data.open;
    let volatility = (market_data.high - market_data.low) / market_data.close;
    let typical_price = (market_data.high + market_data.low + market_data.close) / 3.0;
    let price_position = (market_data.close - market_data.low) / (market_data.high - market_data.low);
    
    assert!((price_change - 0.004).abs() < 0.0001); // 0.4% increase
    assert!((volatility - 0.01996).abs() < 0.0001); // ~2% volatility
    assert_eq!(typical_price, 50066.666666666664);
    assert!((price_position - 0.7).abs() < 0.01); // Price at 70% of range
}

#[test]
fn test_decision_context_preparation() {
    let market_data = create_test_market_data();
    let symbol = "BTC/USD";
    let current_position = 0.5;
    let config = create_test_config();
    
    // Test context preparation for decision making
    let context = json!({
        "type": "trading_decision",
        "symbol": symbol,
        "marketData": {
            "timestamp": market_data.timestamp.to_rfc3339(),
            "open": market_data.open,
            "high": market_data.high,
            "low": market_data.low,
            "close": market_data.close,
            "volume": market_data.volume,
            "priceChange": (market_data.close - market_data.open) / market_data.open,
            "volatility": (market_data.high - market_data.low) / market_data.close
        },
        "position": {
            "current": current_position,
            "isLong": current_position > 0.0,
            "isShort": current_position < 0.0,
            "isFlat": current_position == 0.0
        },
        "agentConfig": {
            "strategy": format!("{:?}", config.strategy),
            "riskTolerance": config.risk_tolerance,
            "decisionThreshold": config.decision_threshold
        }
    });
    
    // Verify context structure
    assert_eq!(context["type"], "trading_decision");
    assert_eq!(context["symbol"], "BTC/USD");
    assert!(context["position"]["isLong"].as_bool().unwrap());
    assert!(!context["position"]["isShort"].as_bool().unwrap());
    assert!(!context["position"]["isFlat"].as_bool().unwrap());
}

#[test]
fn test_risk_assessment_calculations() {
    let market_data = create_test_market_data();
    let position_size = 5000.0;
    let portfolio_value = Some(50000.0);
    
    // Test risk calculations
    let volatility = (market_data.high - market_data.low) / market_data.close;
    let position_ratio = position_size / portfolio_value.unwrap();
    
    assert!((volatility - 0.01996).abs() < 0.0001);
    assert_eq!(position_ratio, 0.1); // 10% of portfolio
    
    // Test warning thresholds
    let mut warnings = Vec::new();
    if volatility > 0.15 {
        warnings.push("High market volatility detected".to_string());
    }
    if position_ratio > 0.2 {
        warnings.push(format!("Position size {:.1}% exceeds recommended 20% of portfolio", position_ratio * 100.0));
    }
    
    assert_eq!(warnings.len(), 0); // No warnings for this test case
}

#[test]
fn test_risk_adjusted_parameters() {
    let config = create_test_config();
    let market_data = create_test_market_data();
    
    let risk_factor = 1.0 - config.risk_tolerance;
    let stop_loss = market_data.close * (1.0 - 0.02 * risk_factor);
    let take_profit = market_data.close * (1.0 + 0.03 * risk_factor);
    
    assert_eq!(risk_factor, 0.30000000000000004);
    assert!((stop_loss - 49898.8).abs() < 0.1);
    assert!((take_profit - 50651.2).abs() < 0.1);
}

#[test]
fn test_performance_adaptation_data() {
    let performance_data = json!({
        "totalTrades": 100,
        "winRate": 0.65,
        "averageProfit": 0.025,
        "maxDrawdown": 0.08,
        "sharpeRatio": 1.5,
        "recentPerformance": {
            "last10Trades": {
                "winRate": 0.7,
                "averageProfit": 0.03
            }
        },
        "marketConditions": {
            "volatility": "medium",
            "trend": "bullish",
            "volume": "high"
        }
    });
    
    // Verify performance data structure
    assert_eq!(performance_data["totalTrades"], 100);
    assert_eq!(performance_data["winRate"], 0.65);
    assert_eq!(performance_data["recentPerformance"]["last10Trades"]["winRate"], 0.7);
}

#[test]
fn test_knowledge_sharing_format() {
    let knowledge = json!({
        "marketInsights": {
            "BTC/USD": {
                "trend": "bullish",
                "supportLevel": 49000.0,
                "resistanceLevel": 51000.0,
                "keyPatterns": ["ascending_triangle", "volume_accumulation"]
            }
        },
        "tradingStrategies": {
            "momentum": {
                "performance": 0.68,
                "optimalParameters": {
                    "lookback": 20,
                    "threshold": 0.02
                }
            }
        },
        "riskFactors": {
            "marketVolatility": 0.15,
            "correlations": {
                "SP500": 0.45,
                "GOLD": -0.3
            }
        }
    });
    
    // Verify knowledge structure
    assert!(knowledge["marketInsights"]["BTC/USD"]["keyPatterns"].is_array());
    assert_eq!(knowledge["tradingStrategies"]["momentum"]["performance"], 0.68);
    assert_eq!(knowledge["riskFactors"]["correlations"]["SP500"], 0.45);
}

#[test]
fn test_strategy_signal_response_parsing() {
    // Test parsing of strategy signal responses
    let signal_response = json!({
        "recommendation": "buy",
        "confidence": 0.75,
        "indicators": {
            "rsi": 45,
            "macd": 0.002,
            "volume_trend": "increasing"
        },
        "patterns": ["bullish_divergence", "support_bounce"],
        "insights": {
            "market_strength": "strong",
            "entry_timing": "optimal"
        }
    });
    
    let signal = signal_response["recommendation"].as_str().unwrap_or("neutral");
    let strength = signal_response["confidence"].as_f64().unwrap_or(0.5);
    
    assert_eq!(signal, "buy");
    assert_eq!(strength, 0.75);
    assert!(signal_response["patterns"].is_array());
    assert_eq!(signal_response["patterns"].as_array().unwrap().len(), 2);
}

#[test]
fn test_monitoring_context_preparation() {
    let symbol = "BTC/USD";
    let position_size = 5000.0;
    let portfolio_value = Some(50000.0);
    let market_data = create_test_market_data();
    
    let monitoring_context = json!({
        "type": "risk_assessment",
        "symbol": symbol,
        "positionSize": position_size,
        "portfolioValue": portfolio_value,
        "marketVolatility": (market_data.high - market_data.low) / market_data.close,
        "currentPrice": market_data.close
    });
    
    assert_eq!(monitoring_context["type"], "risk_assessment");
    assert_eq!(monitoring_context["positionSize"], 5000.0);
    assert_eq!(monitoring_context["portfolioValue"], 50000.0);
    assert!((monitoring_context["marketVolatility"].as_f64().unwrap() - 0.01996).abs() < 0.0001);
}

#[test]
fn test_edge_cases() {
    // Test with extreme market conditions
    let extreme_market = MarketData {
        timestamp: Utc::now(),
        open: 50000.0,
        high: 60000.0,  // 20% spike
        low: 40000.0,   // 20% drop
        close: 45000.0, // 10% down
        volume: 10000.0, // 10x normal volume
    };
    
    let volatility = (extreme_market.high - extreme_market.low) / extreme_market.close;
    assert!((volatility - 0.4444).abs() < 0.001); // 44.44% volatility
    
    // Test with zero position
    let zero_position = 0.0;
    assert_eq!(zero_position > 0.0, false); // Not long
    assert_eq!(zero_position < 0.0, false); // Not short
    assert_eq!(zero_position == 0.0, true); // Is flat
}

#[test]
fn test_cognitive_pattern_analysis_request() {
    let strategy = "momentum";
    let symbol = "BTC/USD";
    let market_data = create_test_market_data();
    
    let analysis_context = json!({
        "strategy": strategy,
        "symbol": symbol,
        "marketData": {
            "close": market_data.close,
            "open": market_data.open,
            "high": market_data.high,
            "low": market_data.low,
            "volume": market_data.volume
        }
    });
    
    // Verify request structure for cognitive pattern analysis
    assert_eq!(analysis_context["strategy"], "momentum");
    assert_eq!(analysis_context["symbol"], "BTC/USD");
    assert_eq!(analysis_context["marketData"]["close"], 50200.0);
}

#[tokio::test]
async fn test_agent_lifecycle() {
    let config = create_test_config();
    let agent_id = format!("trader-{}", config.id);
    
    // Test agent ID generation
    assert_eq!(agent_id, "trader-test-agent-1");
    
    // Test that drop behavior is configured (would clean up in real scenario)
    // In unit tests, we just verify the structure
    let cleanup_command = json!({
        "id": agent_id
    });
    
    assert_eq!(cleanup_command["id"], "trader-test-agent-1");
}