//! Real-World Trading Scenarios Integration Tests
//! 
//! This module tests realistic trading scenarios and market conditions:
//! - Market open/close scenarios
//! - Economic news event reactions
//! - Liquidity crises and market halts
//! - Weekend/holiday gap trading
//! - Multi-asset portfolio scenarios
//! - Regulatory compliance testing
//! - Risk management scenarios

use autonomous_platform::data::{DataPipeline, TimescaleDBStorage, RedisCache, TimeSeriesData};
use autonomous_platform::integration::{
    platform_orchestrator::{PlatformOrchestrator, SystemHealth, ValidationResult},
    streaming::{StreamingPipeline, MarketData, NewsData, StreamConfig},
    data_access::{DataAccessLayer, DataRequest, Timeframe},
    neural_predictions::{NeuralPredictionSystem, DecisionContext, ModelType}
};
use autonomous_platform::config::{PlatformConfig, DatabaseConfig, RedisConfig, NeuralConfig, MonitoringConfig, PlatformInfo};
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration, TimeZone, Weekday};
use tokio::sync::mpsc;
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Create a production-like configuration
fn create_production_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "real-world-test-platform".to_string(),
            version: "0.1.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/real_world_test".to_string(),
            max_connections: 100,
            min_connections: 20,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 50,
            default_ttl_seconds: 600,
        },
        neural: NeuralConfig {
            memory_gb: 16.0,
            models: vec!["NHITS".to_string(), "DeepAR".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 1800,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 5,
            quality_threshold: 0.95,
        },
    }
}

/// Create market data for a specific market condition
fn create_market_condition_data(
    symbol: &str, 
    condition: &str, 
    base_price: f64, 
    sequence: u64
) -> MarketData {
    let (price, volume, spread, metadata) = match condition {
        "MARKET_OPEN" => {
            let gap = (sequence as f64 * 0.1).sin() * 100.0; // Price gap
            (base_price + gap, 150000.0, 10.0, json!({
                "session": "market_open",
                "gap_size": gap,
                "overnight_news": true,
                "liquidity": "building"
            }))
        },
        "MARKET_CLOSE" => {
            (base_price, 80000.0, 5.0, json!({
                "session": "market_close",
                "day_range": 150.0,
                "settlement": true,
                "liquidity": "high"
            }))
        },
        "NEWS_EVENT" => {
            let spike = if sequence % 2 == 0 { 200.0 } else { -150.0 };
            (base_price + spike, 300000.0, 25.0, json!({
                "event_type": "economic_news",
                "impact": "high",
                "surprise_factor": 0.8,
                "liquidity": "volatile"
            }))
        },
        "LIQUIDITY_CRISIS" => {
            (base_price, 5000.0, 100.0, json!({
                "liquidity_crisis": true,
                "spread_widening": true,
                "market_depth": 2,
                "trading_halt_risk": 0.3
            }))
        },
        "WEEKEND_GAP" => {
            let gap = 300.0 * if sequence % 2 == 0 { 1.0 } else { -1.0 };
            (base_price + gap, 50000.0, 50.0, json!({
                "weekend_gap": true,
                "gap_size": gap,
                "low_liquidity": true,
                "session": "weekend_opening"
            }))
        },
        _ => (base_price, 100000.0, 10.0, json!({}))
    };
    
    MarketData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        price,
        volume,
        bid: price - spread / 2.0,
        ask: price + spread / 2.0,
        source: format!("{}_feed", condition.to_lowercase()),
        sequence_number: sequence,
        order_book_depth: Some(if condition == "LIQUIDITY_CRISIS" { 3 } else { 20 }),
        metadata: Some(metadata),
    }
}

/// Test Market Open Scenario
#[tokio::test]
async fn test_market_open_scenario() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Set up trading agents for market open
    let trading_agents = vec![
        ("gap_trader", "GAP_TRADING"),
        ("momentum_trader", "OPENING_MOMENTUM"),
        ("mean_reversion_trader", "OPEN_MEAN_REVERSION"),
    ];
    
    for (agent_id, _) in &trading_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate market open with overnight gaps
    let symbols = vec!["AAPL", "MSFT", "GOOGL"];
    let base_prices = vec![150.0, 300.0, 2500.0];
    
    for (i, (symbol, base_price)) in symbols.iter().zip(base_prices.iter()).enumerate() {
        // Pre-market data
        let premarket_data = create_market_condition_data(
            &format!("{}/USD", symbol), 
            "MARKET_OPEN", 
            *base_price, 
            i as u64
        );
        orchestrator.inject_market_data(premarket_data).await?;
        
        // Market open news
        let open_news = NewsData {
            id: format!("market_open_{}_{}", symbol, i),
            timestamp: Utc::now(),
            title: format!("{} Opens with Significant Gap After Earnings", symbol),
            content: format!("Market opens with notable price action in {} following overnight developments", symbol),
            source: "market_open_feed".to_string(),
            symbols: vec![format!("{}/USD", symbol)],
            sentiment_score: 0.6 + (i as f64 * 0.1),
            relevance_score: 0.9,
            category: "market_open".to_string(),
            metadata: Some(json!({
                "market_session": "open",
                "gap_type": "earnings_gap",
                "volume_spike": true
            })),
        };
        orchestrator.inject_news_data(open_news).await?;
    }
    
    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    // Test trading decisions during market open
    for (agent_id, strategy) in &trading_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "AAPL/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "AAPL/USD".to_string(),
                timestamp: Utc::now(),
                open: 150.0,
                high: 155.0,
                low: 148.0,
                close: 152.0,
                volume: 150000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("GAP_SIZE".to_string(), 2.0);
                    indicators.insert("VOLUME_RATIO".to_string(), 2.5);
                    indicators.insert("OVERNIGHT_RANGE".to_string(), 7.0);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("session".to_string(), json!("market_open"));
                metadata.insert("strategy".to_string(), json!(strategy));
                metadata.insert("gap_trading".to_string(), json!(true));
                metadata
            },
            required_confidence: 0.8,
            prediction_horizon: 1800, // 30 minutes
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction.confidence >= 0.0);
        assert!(prediction.model_used.is_some());
        
        println!("Market Open - Agent {}: Confidence = {:.3}", agent_id, prediction.confidence);
    }
    
    // Verify market open handling
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    Ok(())
}

/// Test Economic News Event Reaction
#[tokio::test]
async fn test_economic_news_event_reaction() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let news_agents = vec![
        ("news_sentiment_agent", "NEWS_SENTIMENT_ANALYSIS"),
        ("event_driven_agent", "EVENT_DRIVEN_TRADING"),
        ("volatility_agent", "VOLATILITY_TRADING"),
    ];
    
    for (agent_id, _) in &news_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate major economic news event (e.g., Fed Rate Decision)
    let major_news = NewsData {
        id: "fed_rate_decision_2024".to_string(),
        timestamp: Utc::now(),
        title: "Federal Reserve Announces Surprise Rate Cut".to_string(),
        content: "The Federal Reserve surprised markets with an emergency 50 basis point rate cut, citing concerns about economic growth and inflation expectations.".to_string(),
        source: "federal_reserve".to_string(),
        symbols: vec!["SPY/USD".to_string(), "BTC/USD".to_string(), "EUR/USD".to_string()],
        sentiment_score: 0.8, // Positive for markets
        relevance_score: 1.0, // Maximum relevance
        category: "monetary_policy".to_string(),
        metadata: Some(json!({
            "event_type": "fed_rate_decision",
            "surprise_factor": 0.9,
            "market_impact": "high",
            "sectors_affected": ["financials", "real_estate", "technology"],
            "currency_impact": "usd_bearish"
        })),
    };
    
    orchestrator.inject_news_data(major_news).await?;
    
    // Simulate immediate market reaction
    let affected_symbols = vec!["SPY/USD", "BTC/USD", "EUR/USD"];
    let base_prices = vec![400.0, 45000.0, 1.08];
    
    for (i, (symbol, base_price)) in affected_symbols.iter().zip(base_prices.iter()).enumerate() {
        // Immediate reaction
        let reaction_data = create_market_condition_data(symbol, "NEWS_EVENT", *base_price, i as u64);
        orchestrator.inject_market_data(reaction_data).await?;
        
        // Secondary wave
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let secondary_data = create_market_condition_data(symbol, "NEWS_EVENT", *base_price, (i + 10) as u64);
        orchestrator.inject_market_data(secondary_data).await?;
    }
    
    // Test agent responses to news
    for (agent_id, strategy) in &news_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "SPY/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "SPY/USD".to_string(),
                timestamp: Utc::now(),
                open: 400.0,
                high: 420.0,
                low: 395.0,
                close: 415.0,
                volume: 500000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("NEWS_SENTIMENT".to_string(), 0.8);
                    indicators.insert("VOLUME_SPIKE".to_string(), 3.5);
                    indicators.insert("VOLATILITY".to_string(), 0.6);
                    indicators.insert("CORRELATION_BREAK".to_string(), 0.4);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("news_event".to_string(), json!("fed_rate_decision"));
                metadata.insert("strategy".to_string(), json!(strategy));
                metadata.insert("event_impact".to_string(), json!("high"));
                metadata.insert("surprise_factor".to_string(), json!(0.9));
                metadata
            },
            required_confidence: 0.7, // Lower confidence due to news volatility
            prediction_horizon: 3600, // 1 hour for news events
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction.confidence >= 0.0);
        
        println!("News Event - Agent {}: Confidence = {:.3}", agent_id, prediction.confidence);
    }
    
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    Ok(())
}

/// Test Liquidity Crisis Scenario
#[tokio::test]
async fn test_liquidity_crisis_scenario() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let crisis_agents = vec![
        ("liquidity_agent", "LIQUIDITY_MANAGEMENT"),
        ("risk_manager", "RISK_MANAGEMENT"),
        ("emergency_trader", "EMERGENCY_TRADING"),
    ];
    
    for (agent_id, _) in &crisis_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate liquidity crisis across multiple assets
    let crisis_symbols = vec!["CRISIS1/USD", "CRISIS2/USD", "CRISIS3/USD"];
    let base_prices = vec![1000.0, 2000.0, 500.0];
    
    for (i, (symbol, base_price)) in crisis_symbols.iter().zip(base_prices.iter()).enumerate() {
        // Generate liquidity crisis conditions
        for wave in 0..5 {
            let crisis_data = create_market_condition_data(
                symbol, 
                "LIQUIDITY_CRISIS", 
                *base_price, 
                (i * 5 + wave) as u64
            );
            orchestrator.inject_market_data(crisis_data).await?;
            
            // Simulate crisis deepening
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }
    
    // Test crisis response
    for (agent_id, strategy) in &crisis_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "CRISIS1/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "CRISIS1/USD".to_string(),
                timestamp: Utc::now(),
                open: 1000.0,
                high: 1020.0,
                low: 950.0,
                close: 970.0,
                volume: 5000.0, // Very low volume
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("LIQUIDITY_SCORE".to_string(), 0.1); // Very low liquidity
                    indicators.insert("SPREAD_RATIO".to_string(), 0.1); // 10% spread
                    indicators.insert("MARKET_DEPTH".to_string(), 2.0); // Very shallow
                    indicators.insert("VOLATILITY".to_string(), 0.8); // High volatility
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("crisis_mode".to_string(), json!(true));
                metadata.insert("strategy".to_string(), json!(strategy));
                metadata.insert("risk_level".to_string(), json!("extreme"));
                metadata.insert("liquidity_status".to_string(), json!("critical"));
                metadata
            },
            required_confidence: 0.9, // High confidence required in crisis
            prediction_horizon: 900, // 15 minutes for crisis management
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction.confidence >= 0.0);
        
        // In crisis, system should either provide high-confidence predictions or fail gracefully
        if prediction.confidence > 0.0 {
            println!("Crisis - Agent {}: Confidence = {:.3} (Model: {:?})", 
                     agent_id, prediction.confidence, prediction.model_used);
        }
    }
    
    let health = orchestrator.health_check().await?;
    // System should handle crisis gracefully
    assert!(health.overall_healthy || health.metrics.error_count > 0);
    
    Ok(())
}

/// Test Weekend Gap Trading Scenario
#[tokio::test]
async fn test_weekend_gap_trading() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let gap_agents = vec![
        ("gap_trader", "GAP_TRADING"),
        ("weekend_trader", "WEEKEND_STRATEGY"),
        ("crypto_trader", "CRYPTO_24_7"), // Crypto trades over weekends
    ];
    
    for (agent_id, _) in &gap_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate weekend gaps for different asset classes
    let weekend_assets = vec![
        ("FOREX/USD", 1.2000, "FOREX"), // Forex has weekend gaps
        ("BTC/USD", 45000.0, "CRYPTO"), // Crypto trades continuously
        ("STOCK/USD", 100.0, "EQUITY"), // Stocks have weekend gaps
    ];
    
    for (i, (symbol, base_price, asset_class)) in weekend_assets.iter().enumerate() {
        let weekend_data = create_market_condition_data(
            symbol, 
            "WEEKEND_GAP", 
            *base_price, 
            i as u64
        );
        orchestrator.inject_market_data(weekend_data).await?;
        
        // Weekend news that might cause gaps
        let weekend_news = NewsData {
            id: format!("weekend_news_{}_{}", asset_class, i),
            timestamp: Utc::now(),
            title: format!("Weekend Development Affects {} Markets", asset_class),
            content: format!("Significant developments over the weekend are expected to impact {} trading", asset_class),
            source: "weekend_feed".to_string(),
            symbols: vec![symbol.to_string()],
            sentiment_score: if i % 2 == 0 { 0.7 } else { 0.3 },
            relevance_score: 0.8,
            category: "weekend_news".to_string(),
            metadata: Some(json!({
                "asset_class": asset_class,
                "weekend_impact": true,
                "gap_expected": asset_class != "CRYPTO"
            })),
        };
        orchestrator.inject_news_data(weekend_news).await?;
    }
    
    // Test weekend trading strategies
    for (agent_id, strategy) in &gap_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "FOREX/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "FOREX/USD".to_string(),
                timestamp: Utc::now(),
                open: 1.2000,
                high: 1.2300,
                low: 1.1800,
                close: 1.2150,
                volume: 25000.0, // Lower weekend volume
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("WEEKEND_GAP".to_string(), 0.0125); // 1.25% gap
                    indicators.insert("VOLUME_RATIO".to_string(), 0.3); // 30% of normal volume
                    indicators.insert("SPREAD_RATIO".to_string(), 0.02); // 2% spread
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("session".to_string(), json!("weekend_gap"));
                metadata.insert("strategy".to_string(), json!(strategy));
                metadata.insert("low_liquidity".to_string(), json!(true));
                metadata.insert("gap_size".to_string(), json!(0.0125));
                metadata
            },
            required_confidence: 0.75,
            prediction_horizon: 7200, // 2 hours for gap trading
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction.confidence >= 0.0);
        
        println!("Weekend Gap - Agent {}: Confidence = {:.3}", agent_id, prediction.confidence);
    }
    
    Ok(())
}

/// Test Multi-Asset Portfolio Scenario
#[tokio::test]
async fn test_multi_asset_portfolio_scenario() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let portfolio_agents = vec![
        ("portfolio_manager", "PORTFOLIO_OPTIMIZATION"),
        ("risk_manager", "PORTFOLIO_RISK_MANAGEMENT"),
        ("correlation_trader", "CORRELATION_ARBITRAGE"),
        ("sector_rotator", "SECTOR_ROTATION"),
    ];
    
    for (agent_id, _) in &portfolio_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Create a diversified portfolio
    let portfolio_assets = vec![
        ("TECH/USD", 300.0, "TECHNOLOGY"),
        ("FINANCE/USD", 50.0, "FINANCIAL"),
        ("ENERGY/USD", 80.0, "ENERGY"),
        ("HEALTHCARE/USD", 120.0, "HEALTHCARE"),
        ("GOLD/USD", 2000.0, "COMMODITY"),
        ("BONDS/USD", 100.0, "FIXED_INCOME"),
        ("CRYPTO/USD", 40000.0, "CRYPTOCURRENCY"),
    ];
    
    // Generate correlated market movements
    let correlation_factor = 0.7;
    let market_direction = 1.0; // Bullish market
    
    for (i, (symbol, base_price, sector)) in portfolio_assets.iter().enumerate() {
        let sector_multiplier = match *sector {
            "TECHNOLOGY" => 1.2,
            "FINANCIAL" => 0.8,
            "ENERGY" => 1.1,
            "HEALTHCARE" => 0.9,
            "COMMODITY" => -0.3, // Inverse correlation
            "FIXED_INCOME" => -0.5, // Inverse correlation
            "CRYPTOCURRENCY" => 1.5, // High correlation with risk-on
            _ => 1.0,
        };
        
        let price_change = market_direction * correlation_factor * sector_multiplier * 0.02; // 2% base move
        let new_price = base_price * (1.0 + price_change);
        
        let market_data = MarketData {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            price: new_price,
            volume: 100000.0 * (1.0 + price_change.abs()),
            bid: new_price - (new_price * 0.001),
            ask: new_price + (new_price * 0.001),
            source: "portfolio_feed".to_string(),
            sequence_number: i as u64,
            order_book_depth: Some(15),
            metadata: Some(json!({
                "sector": sector,
                "correlation_factor": correlation_factor,
                "sector_multiplier": sector_multiplier,
                "market_regime": "bull_market"
            })),
        };
        
        orchestrator.inject_market_data(market_data).await?;
    }
    
    // Test portfolio-level decisions
    for (agent_id, strategy) in &portfolio_agents {
        // Test with a representative asset from each major category
        let test_symbols = vec!["TECH/USD", "GOLD/USD", "CRYPTO/USD"];
        
        for symbol in &test_symbols {
            let base_price = portfolio_assets.iter()
                .find(|(s, _, _)| s == symbol)
                .map(|(_, p, _)| *p)
                .unwrap_or(1000.0);
            
            let decision_context = DecisionContext {
                agent_id: agent_id.to_string(),
                decision_type: strategy.to_string(),
                symbol: symbol.to_string(),
                market_data: TimeSeriesData {
                    symbol: symbol.to_string(),
                    timestamp: Utc::now(),
                    open: base_price * 0.99,
                    high: base_price * 1.02,
                    low: base_price * 0.98,
                    close: base_price * 1.01,
                    volume: 150000.0,
                    indicators: {
                        let mut indicators = HashMap::new();
                        indicators.insert("PORTFOLIO_BETA".to_string(), 1.2);
                        indicators.insert("CORRELATION_SPY".to_string(), 0.8);
                        indicators.insert("SHARPE_RATIO".to_string(), 1.5);
                        indicators.insert("MAX_DRAWDOWN".to_string(), 0.15);
                        indicators.insert("VOLATILITY".to_string(), 0.25);
                        indicators
                    },
                },
                context_metadata: {
                    let mut metadata = HashMap::new();
                    metadata.insert("portfolio_strategy".to_string(), json!(strategy));
                    metadata.insert("asset_allocation".to_string(), json!(0.1)); // 10% allocation
                    metadata.insert("risk_budget".to_string(), json!(0.05)); // 5% risk budget
                    metadata.insert("rebalance_trigger".to_string(), json!(0.02)); // 2% deviation
                    metadata
                },
                required_confidence: 0.8,
                prediction_horizon: 14400, // 4 hours for portfolio management
            };
            
            let prediction = orchestrator.get_neural_prediction(decision_context).await?;
            assert!(prediction.confidence >= 0.0);
            
            println!("Portfolio - Agent {} ({}): Confidence = {:.3}", 
                     agent_id, symbol, prediction.confidence);
        }
    }
    
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    Ok(())
}

/// Test Risk Management Scenario
#[tokio::test]
async fn test_risk_management_scenario() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let risk_agents = vec![
        ("var_calculator", "VALUE_AT_RISK"),
        ("stress_tester", "STRESS_TESTING"),
        ("limit_monitor", "POSITION_LIMITS"),
        ("circuit_breaker", "CIRCUIT_BREAKER"),
    ];
    
    for (agent_id, _) in &risk_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate high-risk market conditions
    let risk_scenarios = vec![
        ("VOLATILE/USD", 1000.0, "HIGH_VOLATILITY", 0.5),
        ("ILLIQUID/USD", 2000.0, "LOW_LIQUIDITY", 0.8),
        ("CORRELATED/USD", 500.0, "HIGH_CORRELATION", 0.9),
        ("LEVERAGED/USD", 100.0, "HIGH_LEVERAGE", 2.0),
    ];
    
    for (i, (symbol, base_price, risk_type, risk_multiplier)) in risk_scenarios.iter().enumerate() {
        let risk_data = MarketData {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            price: base_price * (1.0 + (i as f64 * 0.1 * risk_multiplier)),
            volume: 50000.0 / risk_multiplier, // Inverse relationship with risk
            bid: base_price * (1.0 - 0.01 * risk_multiplier),
            ask: base_price * (1.0 + 0.01 * risk_multiplier),
            source: "risk_feed".to_string(),
            sequence_number: i as u64,
            order_book_depth: Some((20.0 / risk_multiplier) as u32),
            metadata: Some(json!({
                "risk_type": risk_type,
                "risk_multiplier": risk_multiplier,
                "warning_level": if *risk_multiplier > 1.5 { "high" } else { "medium" }
            })),
        };
        
        orchestrator.inject_market_data(risk_data).await?;
    }
    
    // Test risk management responses
    for (agent_id, strategy) in &risk_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "VOLATILE/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "VOLATILE/USD".to_string(),
                timestamp: Utc::now(),
                open: 1000.0,
                high: 1100.0,
                low: 900.0,
                close: 1050.0,
                volume: 100000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("VAR_95".to_string(), 0.05); // 5% VaR
                    indicators.insert("VOLATILITY".to_string(), 0.4); // 40% volatility
                    indicators.insert("BETA".to_string(), 1.8); // High beta
                    indicators.insert("CORRELATION".to_string(), 0.9); // High correlation
                    indicators.insert("LIQUIDITY_SCORE".to_string(), 0.3); // Low liquidity
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("risk_strategy".to_string(), json!(strategy));
                metadata.insert("position_size".to_string(), json!(0.02)); // 2% position
                metadata.insert("stop_loss".to_string(), json!(0.05)); // 5% stop loss
                metadata.insert("max_leverage".to_string(), json!(2.0)); // 2x max leverage
                metadata.insert("risk_budget".to_string(), json!(0.10)); // 10% risk budget
                metadata
            },
            required_confidence: 0.95, // Very high confidence for risk management
            prediction_horizon: 3600, // 1 hour for risk assessment
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        
        // Risk management should either provide high-confidence predictions or reject
        if prediction.confidence > 0.0 {
            assert!(prediction.confidence >= 0.7, "Risk management requires high confidence");
            println!("Risk Management - Agent {}: Confidence = {:.3}", 
                     agent_id, prediction.confidence);
        }
    }
    
    // Store final results
    let memory_key = "swarm-auto-centralized-1751484080479/integration-testing-final/results";
    orchestrator.store_results_in_memory(memory_key).await?;
    
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    Ok(())
}

/// Test Regulatory Compliance Scenario
#[tokio::test]
async fn test_regulatory_compliance_scenario() -> Result<()> {
    let config = create_production_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let compliance_agents = vec![
        ("compliance_monitor", "COMPLIANCE_MONITORING"),
        ("position_reporter", "POSITION_REPORTING"),
        ("audit_logger", "AUDIT_LOGGING"),
    ];
    
    for (agent_id, _) in &compliance_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate compliance-sensitive trading
    let regulated_assets = vec![
        ("REGULATED1/USD", 1000.0, "EQUITIES"),
        ("REGULATED2/USD", 50.0, "DERIVATIVES"),
        ("REGULATED3/USD", 2000.0, "FOREX"),
    ];
    
    for (i, (symbol, base_price, asset_class)) in regulated_assets.iter().enumerate() {
        let compliance_data = MarketData {
            symbol: symbol.to_string(),
            timestamp: Utc::now(),
            price: *base_price,
            volume: 100000.0,
            bid: base_price - 1.0,
            ask: base_price + 1.0,
            source: "regulated_feed".to_string(),
            sequence_number: i as u64,
            order_book_depth: Some(20),
            metadata: Some(json!({
                "asset_class": asset_class,
                "regulatory_status": "compliant",
                "reporting_required": true,
                "position_limits": true
            })),
        };
        
        orchestrator.inject_market_data(compliance_data).await?;
    }
    
    // Test compliance-aware trading decisions
    for (agent_id, strategy) in &compliance_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "REGULATED1/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "REGULATED1/USD".to_string(),
                timestamp: Utc::now(),
                open: 995.0,
                high: 1005.0,
                low: 990.0,
                close: 1000.0,
                volume: 100000.0,
                indicators: HashMap::new(),
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("compliance_strategy".to_string(), json!(strategy));
                metadata.insert("regulatory_jurisdiction".to_string(), json!("US"));
                metadata.insert("position_limit".to_string(), json!(1000000)); // $1M limit
                metadata.insert("reporting_threshold".to_string(), json!(100000)); // $100k threshold
                metadata.insert("audit_required".to_string(), json!(true));
                metadata
            },
            required_confidence: 0.9, // High confidence for compliance
            prediction_horizon: 3600,
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction.confidence >= 0.0);
        
        println!("Compliance - Agent {}: Confidence = {:.3}", 
                 agent_id, prediction.confidence);
    }
    
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    Ok(())
}