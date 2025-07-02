//! System Integration Tests
//! 
//! This module tests comprehensive cross-component integration scenarios:
//! - Data pipeline to DAA agent communication
//! - DAA agent to neural network coordination
//! - Multi-component failure recovery
//! - Real world trading scenarios
//! - Performance under load
//! - Memory usage patterns
//! - Inter-service communication

use autonomous_platform::data::{DataPipeline, TimescaleDBStorage, RedisCache, TimeSeriesData};
use autonomous_platform::integration::{
    platform_orchestrator::{PlatformOrchestrator, SystemHealth, ValidationResult},
    streaming::{StreamingPipeline, MarketData, NewsData, StreamConfig},
    data_access::{DataAccessLayer, DataRequest, Timeframe},
    neural_predictions::{NeuralPredictionSystem, DecisionContext, ModelType}
};
use autonomous_platform::config::{PlatformConfig, DatabaseConfig, RedisConfig, NeuralConfig, MonitoringConfig, PlatformInfo};
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use tokio::sync::mpsc;
use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Create a comprehensive test configuration
fn create_integration_config() -> PlatformConfig {
    PlatformConfig {
        platform: PlatformInfo {
            name: "integration-test-platform".to_string(),
            version: "0.1.0".to_string(),
        },
        database: DatabaseConfig {
            url: "postgres://test@localhost/integration_test".to_string(),
            max_connections: 50,
            min_connections: 10,
        },
        redis: RedisConfig {
            url: "redis://localhost:6379".to_string(),
            max_connections: 20,
            default_ttl_seconds: 300,
        },
        neural: NeuralConfig {
            memory_gb: 8.0,
            models: vec!["NHITS".to_string(), "DeepAR".to_string(), "TCN".to_string()],
            prediction_cache_ttl: 600,
        },
        monitoring: MonitoringConfig {
            metrics_interval_secs: 10,
            quality_threshold: 0.95,
        },
    }
}

/// Create realistic market data for integration testing
fn create_realistic_market_data(symbol: &str, base_price: f64, sequence: u64) -> MarketData {
    let price_variance = (sequence as f64 * 0.1).sin() * 50.0; // Simulate price movement
    let current_price = base_price + price_variance;
    
    MarketData {
        symbol: symbol.to_string(),
        timestamp: Utc::now(),
        price: current_price,
        volume: 1000.0 + (sequence as f64 * 10.0),
        bid: current_price - 5.0,
        ask: current_price + 5.0,
        source: "integration_feed".to_string(),
        sequence_number: sequence,
        order_book_depth: Some(20),
        metadata: Some(json!({
            "spread": 10.0,
            "last_trade_size": 0.5 + (sequence as f64 * 0.01),
            "volatility": 0.15 + (sequence as f64 * 0.001),
            "liquidity_score": 0.85 + (sequence as f64 * 0.001),
            "market_maker_count": 15 + (sequence % 10),
            "order_book_imbalance": (sequence as f64 * 0.05).sin()
        })),
    }
}

/// Create realistic news data
fn create_realistic_news_data(symbol: &str, sequence: u64) -> NewsData {
    let sentiment = (sequence as f64 * 0.2).sin() * 0.3 + 0.5; // Varying sentiment
    let relevance = 0.7 + (sequence as f64 * 0.01) % 0.3;
    
    NewsData {
        id: format!("news_{}_{}", symbol, sequence),
        timestamp: Utc::now(),
        title: format!("{} Market Update: {} Analysis", symbol, sequence),
        content: format!("Market analysis for {} shows {} trend with significant {}",
            symbol,
            if sentiment > 0.5 { "positive" } else { "negative" },
            if relevance > 0.8 { "impact" } else { "movement" }
        ),
        source: "integration_news_feed".to_string(),
        symbols: vec![symbol.to_string()],
        sentiment_score: sentiment,
        relevance_score: relevance,
        category: "market_analysis".to_string(),
        metadata: Some(json!({
            "author": "Market Analysis Bot",
            "tags": ["cryptocurrency", "analysis", "integration_test"],
            "word_count": 150 + (sequence % 50),
            "language": "en",
            "region": "global"
        })),
    }
}

/// Test Streaming to DAA Integration
#[tokio::test]
async fn test_streaming_to_daa_integration() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Register multiple DAA agents with different strategies
    let agents = vec![
        ("momentum_agent", "MOMENTUM_STRATEGY"),
        ("arbitrage_agent", "ARBITRAGE_STRATEGY"),
        ("sentiment_agent", "SENTIMENT_STRATEGY"),
    ];
    
    for (agent_id, strategy) in &agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Stream market and news data
    let symbols = vec!["BTC/USD", "ETH/USD", "ADA/USD"];
    for (i, symbol) in symbols.iter().enumerate() {
        // Send market data
        let market_data = create_realistic_market_data(symbol, 1000.0 + (i as f64 * 1000.0), i as u64);
        orchestrator.inject_market_data(market_data).await?;
        
        // Send news data
        let news_data = create_realistic_news_data(symbol, i as u64);
        orchestrator.inject_news_data(news_data).await?;
    }
    
    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Verify all agents received appropriate events
    for (agent_id, _) in &agents {
        let events = orchestrator.get_agent_events(agent_id).await?;
        assert!(!events.is_empty(), "Agent {} should have received events", agent_id);
        
        // Verify event types
        let has_market_events = events.iter().any(|e| e.event_type == "market_data_update");
        let has_news_events = events.iter().any(|e| e.event_type == "news_update");
        
        assert!(has_market_events, "Agent {} should receive market events", agent_id);
        assert!(has_news_events, "Agent {} should receive news events", agent_id);
    }
    
    // Verify system health
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    assert!(health.streaming_pipeline_healthy);
    assert!(health.data_pipeline_healthy);
    
    Ok(())
}

/// Test DAA to Neural Coordination
#[tokio::test]
async fn test_daa_to_neural_coordination() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Set up scenario with multiple agents making decisions
    let agents_with_decisions = vec![
        ("trend_agent", "TREND_FOLLOWING", "BTC/USD"),
        ("mean_reversion_agent", "MEAN_REVERSION", "ETH/USD"),
        ("momentum_agent", "MOMENTUM_BREAKOUT", "ADA/USD"),
    ];
    
    let mut prediction_results = Vec::new();
    
    for (agent_id, decision_type, symbol) in &agents_with_decisions {
        // Register agent
        orchestrator.register_daa_agent(agent_id).await?;
        
        // Inject market data for the symbol
        let market_data = create_realistic_market_data(symbol, 2000.0, 1);
        orchestrator.inject_market_data(market_data).await?;
        
        // Create decision context for neural prediction
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: decision_type.to_string(),
            symbol: symbol.to_string(),
            market_data: TimeSeriesData {
                symbol: symbol.to_string(),
                timestamp: Utc::now(),
                open: 1980.0,
                high: 2020.0,
                low: 1960.0,
                close: 2000.0,
                volume: 50000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("RSI".to_string(), 65.5);
                    indicators.insert("MACD".to_string(), 12.5);
                    indicators.insert("SMA_20".to_string(), 1990.0);
                    indicators.insert("EMA_12".to_string(), 2005.0);
                    indicators.insert("BBANDS_UPPER".to_string(), 2050.0);
                    indicators.insert("BBANDS_LOWER".to_string(), 1950.0);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("strategy".to_string(), json!(decision_type));
                metadata.insert("risk_level".to_string(), json!(0.02));
                metadata.insert("position_size".to_string(), json!(0.1));
                metadata.insert("max_drawdown".to_string(), json!(0.05));
                metadata.insert("time_horizon".to_string(), json!("1h"));
                metadata
            },
            required_confidence: 0.8,
            prediction_horizon: 3600, // 1 hour
        };
        
        // Get neural prediction
        let prediction_result = orchestrator.get_neural_prediction(decision_context).await?;
        prediction_results.push((agent_id, prediction_result));
    }
    
    // Verify all predictions were generated
    assert_eq!(prediction_results.len(), agents_with_decisions.len());
    
    for (agent_id, prediction) in &prediction_results {
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.model_used.is_some());
        assert!(!prediction.prediction_values.is_empty());
        assert!(prediction.execution_recommendations.is_some());
        
        println!("Agent {}: Confidence = {:.3}, Model = {:?}", 
                 agent_id, prediction.confidence, prediction.model_used);
    }
    
    // Verify prediction metrics
    let prediction_metrics = orchestrator.get_prediction_metrics().await?;
    assert!(prediction_metrics.total_predictions >= agents_with_decisions.len() as u64);
    assert!(prediction_metrics.average_confidence > 0.0);
    assert!(!prediction_metrics.models_used.is_empty());
    
    Ok(())
}

/// Test Multi-Component Failure Recovery
#[tokio::test]
async fn test_multi_component_failure_recovery() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let agent_id = "recovery_test_agent";
    orchestrator.register_daa_agent(agent_id).await?;
    
    // Phase 1: Normal operation
    let market_data = create_realistic_market_data("RECOVERY/USD", 1500.0, 1);
    orchestrator.inject_market_data(market_data).await?;
    
    let decision_context = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "RECOVERY_TEST".to_string(),
        symbol: "RECOVERY/USD".to_string(),
        market_data: TimeSeriesData {
            symbol: "RECOVERY/USD".to_string(),
            timestamp: Utc::now(),
            open: 1490.0,
            high: 1510.0,
            low: 1480.0,
            close: 1500.0,
            volume: 10000.0,
            indicators: HashMap::new(),
        },
        context_metadata: HashMap::new(),
        required_confidence: 0.7,
        prediction_horizon: 1800,
    };
    
    let normal_prediction = orchestrator.get_neural_prediction(decision_context.clone()).await;
    assert!(normal_prediction.is_ok(), "Normal operation should succeed");
    
    // Phase 2: Simulate component failures with invalid data
    let invalid_market_data = MarketData {
        symbol: "RECOVERY/USD".to_string(),
        timestamp: Utc::now(),
        price: f64::NAN, // Invalid price
        volume: -1000.0, // Invalid volume
        bid: f64::INFINITY,
        ask: f64::NEG_INFINITY,
        source: "failure_simulation".to_string(),
        sequence_number: 2,
        order_book_depth: None,
        metadata: None,
    };
    
    let failure_result = orchestrator.inject_market_data(invalid_market_data).await;
    // Should handle failure gracefully (might succeed with error handling or fail appropriately)
    
    // Phase 3: System recovery - inject valid data again
    let recovery_market_data = create_realistic_market_data("RECOVERY/USD", 1520.0, 3);
    let recovery_inject = orchestrator.inject_market_data(recovery_market_data).await;
    
    // Phase 4: Test prediction recovery
    let recovery_prediction = orchestrator.get_neural_prediction(decision_context).await;
    
    // Verify recovery
    let health = orchestrator.health_check().await?;
    
    // System should either:
    // 1. Handle everything gracefully and remain healthy
    // 2. Track errors appropriately but continue functioning
    if failure_result.is_err() {
        assert!(health.metrics.error_count > 0, "Errors should be tracked");
    }
    
    assert!(recovery_inject.is_ok(), "Should recover from failure");
    assert!(recovery_prediction.is_ok() || health.metrics.error_count > 0);
    
    Ok(())
}

/// Test Real-World High-Frequency Trading Scenario
#[tokio::test]
async fn test_high_frequency_trading_scenario() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Set up HFT agents
    let hft_agents = vec![
        ("hft_arbitrage", "ARBITRAGE"),
        ("hft_market_making", "MARKET_MAKING"),
        ("hft_momentum", "MOMENTUM_SCALPING"),
    ];
    
    for (agent_id, _) in &hft_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Simulate high-frequency market data stream
    let symbols = vec!["BTC/USD", "ETH/USD"];
    let start_time = Instant::now();
    let mut total_operations = 0;
    
    for round in 0..10 {
        let mut round_handles = Vec::new();
        
        for (i, symbol) in symbols.iter().enumerate() {
            let orchestrator_clone = orchestrator.clone();
            let symbol_clone = symbol.to_string();
            
            let handle = tokio::spawn(async move {
                // Rapid market data updates
                for tick in 0..20 {
                    let sequence = (round * 100) + (i * 20) + tick;
                    let market_data = create_realistic_market_data(
                        &symbol_clone, 
                        3000.0 + (i as f64 * 1000.0), 
                        sequence as u64
                    );
                    
                    let _ = orchestrator_clone.inject_market_data(market_data).await;
                }
                20 // Return number of operations
            });
            
            round_handles.push(handle);
        }
        
        for handle in round_handles {
            if let Ok(ops) = handle.await {
                total_operations += ops;
            }
        }
    }
    
    let processing_time = start_time.elapsed();
    let throughput = total_operations as f64 / processing_time.as_secs_f64();
    
    println!("HFT Test: {} operations in {:.2}s, throughput: {:.1} ops/sec", 
             total_operations, processing_time.as_secs_f64(), throughput);
    
    // Allow processing to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Verify system performance under HFT load
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    assert!(health.metrics.total_requests >= total_operations as u64);
    assert!(throughput > 50.0, "Should handle at least 50 operations per second");
    
    // Test predictions under HFT conditions
    for (agent_id, strategy) in &hft_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: "BTC/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "BTC/USD".to_string(),
                timestamp: Utc::now(),
                open: 2990.0,
                high: 3010.0,
                low: 2980.0,
                close: 3000.0,
                volume: 100000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("RSI".to_string(), 55.0);
                    indicators.insert("MACD".to_string(), 5.0);
                    indicators.insert("VWAP".to_string(), 2995.0);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("strategy".to_string(), json!(strategy));
                metadata.insert("latency_requirement".to_string(), json!("ultra_low"));
                metadata.insert("position_size".to_string(), json!(0.01));
                metadata
            },
            required_confidence: 0.6, // Lower confidence for HFT
            prediction_horizon: 300, // 5 minutes
        };
        
        let prediction_result = orchestrator.get_neural_prediction(decision_context).await?;
        assert!(prediction_result.confidence >= 0.0);
        assert!(prediction_result.model_used.is_some());
    }
    
    Ok(())
}

/// Test Market Volatility Response
#[tokio::test]
async fn test_market_volatility_response() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let agent_id = "volatility_agent";
    orchestrator.register_daa_agent(agent_id).await?;
    
    // Phase 1: Normal market conditions
    let base_price = 4000.0;
    let normal_data = create_realistic_market_data("VOL/USD", base_price, 1);
    orchestrator.inject_market_data(normal_data).await?;
    
    // Phase 2: High volatility scenario - simulate flash crash
    let volatile_prices = vec![
        4000.0, 3950.0, 3800.0, 3600.0, 3200.0, // Crash
        3400.0, 3600.0, 3750.0, 3900.0, 3950.0, // Recovery
    ];
    
    for (i, price) in volatile_prices.iter().enumerate() {
        let volatile_data = MarketData {
            symbol: "VOL/USD".to_string(),
            timestamp: Utc::now(),
            price: *price,
            volume: 50000.0 + (i as f64 * 10000.0), // Increasing volume during volatility
            bid: price - 50.0,
            ask: price + 50.0, // Wide spread during volatility
            source: "volatility_test".to_string(),
            sequence_number: (i + 2) as u64,
            order_book_depth: Some(5), // Thin order book
            metadata: Some(json!({
                "volatility_event": true,
                "price_change_pct": if i > 0 { 
                    ((price - volatile_prices[i-1]) / volatile_prices[i-1]) * 100.0 
                } else { 0.0 },
                "volume_spike": true
            })),
        };
        
        orchestrator.inject_market_data(volatile_data).await?;
        
        // Small delay to simulate real-time feed
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    
    // Test prediction during high volatility
    let volatile_decision = DecisionContext {
        agent_id: agent_id.to_string(),
        decision_type: "VOLATILITY_RESPONSE".to_string(),
        symbol: "VOL/USD".to_string(),
        market_data: TimeSeriesData {
            symbol: "VOL/USD".to_string(),
            timestamp: Utc::now(),
            open: 4000.0,
            high: 4000.0,
            low: 3200.0,  // Large range indicating high volatility
            close: 3950.0,
            volume: 500000.0, // High volume
            indicators: {
                let mut indicators = HashMap::new();
                indicators.insert("RSI".to_string(), 25.0); // Oversold
                indicators.insert("MACD".to_string(), -50.0); // Strong sell signal
                indicators.insert("VOLATILITY".to_string(), 0.85); // High volatility
                indicators.insert("VIX".to_string(), 75.0); // Fear index
                indicators
            },
        },
        context_metadata: {
            let mut metadata = HashMap::new();
            metadata.insert("market_condition".to_string(), json!("high_volatility"));
            metadata.insert("risk_level".to_string(), json!(0.01)); // Lower risk during volatility
            metadata.insert("flash_crash_detected".to_string(), json!(true));
            metadata
        },
        required_confidence: 0.9, // Higher confidence required during volatility
        prediction_horizon: 900, // 15 minutes
    };
    
    let volatility_prediction = orchestrator.get_neural_prediction(volatile_decision).await?;
    
    // Verify system handles volatility appropriately
    assert!(volatility_prediction.confidence >= 0.0);
    assert!(volatility_prediction.model_used.is_some());
    
    // Check if system detected volatility in recommendations
    if let Some(recommendations) = &volatility_prediction.execution_recommendations {
        println!("Volatility recommendations: {}", recommendations);
    }
    
    // Verify system health during volatility
    let health = orchestrator.health_check().await?;
    assert!(health.overall_healthy);
    
    // Should have processed all volatile data points
    assert!(health.metrics.total_requests >= volatile_prices.len() as u64);
    
    Ok(())
}

/// Test Multi-Agent Consensus Scenarios
#[tokio::test]
async fn test_multi_agent_consensus() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Set up consensus agents with different strategies
    let consensus_agents = vec![
        ("technical_agent", "TECHNICAL_ANALYSIS", 0.8),
        ("fundamental_agent", "FUNDAMENTAL_ANALYSIS", 0.7),
        ("sentiment_agent", "SENTIMENT_ANALYSIS", 0.6),
        ("momentum_agent", "MOMENTUM_ANALYSIS", 0.75),
        ("arbitrage_agent", "ARBITRAGE_ANALYSIS", 0.85),
    ];
    
    for (agent_id, _, _) in &consensus_agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Inject market data that all agents will analyze
    let consensus_symbol = "CONSENSUS/USD";
    let market_data = create_realistic_market_data(consensus_symbol, 5000.0, 1);
    orchestrator.inject_market_data(market_data).await?;
    
    // Get predictions from all agents
    let mut agent_predictions = Vec::new();
    
    for (agent_id, strategy, required_confidence) in &consensus_agents {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: strategy.to_string(),
            symbol: consensus_symbol.to_string(),
            market_data: TimeSeriesData {
                symbol: consensus_symbol.to_string(),
                timestamp: Utc::now(),
                open: 4950.0,
                high: 5050.0,
                low: 4900.0,
                close: 5000.0,
                volume: 75000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("RSI".to_string(), 62.0);
                    indicators.insert("MACD".to_string(), 15.0);
                    indicators.insert("SMA_50".to_string(), 4980.0);
                    indicators.insert("EMA_20".to_string(), 5010.0);
                    indicators.insert("BOLLINGER_UPPER".to_string(), 5100.0);
                    indicators.insert("BOLLINGER_LOWER".to_string(), 4900.0);
                    indicators.insert("VOLUME_MA".to_string(), 65000.0);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("consensus_round".to_string(), json!(1));
                metadata.insert("agent_weight".to_string(), json!(1.0 / consensus_agents.len() as f64));
                metadata.insert("strategy_type".to_string(), json!(strategy));
                metadata
            },
            required_confidence: *required_confidence,
            prediction_horizon: 1800, // 30 minutes
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        agent_predictions.push((agent_id, prediction));
    }
    
    // Analyze consensus
    let mut total_confidence = 0.0;
    let mut bullish_signals = 0;
    let mut bearish_signals = 0;
    let mut neutral_signals = 0;
    
    for (agent_id, prediction) in &agent_predictions {
        total_confidence += prediction.confidence;
        
        // Analyze prediction direction (simplified)
        if prediction.confidence > 0.7 {
            if prediction.prediction_values.get("price_direction").unwrap_or(&0.0) > &0.0 {
                bullish_signals += 1;
            } else {
                bearish_signals += 1;
            }
        } else {
            neutral_signals += 1;
        }
        
        println!("Agent {}: Confidence = {:.3}, Model = {:?}", 
                 agent_id, prediction.confidence, prediction.model_used);
    }
    
    let average_confidence = total_confidence / consensus_agents.len() as f64;
    let consensus_strength = if bullish_signals > bearish_signals + neutral_signals {
        "BULLISH"
    } else if bearish_signals > bullish_signals + neutral_signals {
        "BEARISH"
    } else {
        "NEUTRAL"
    };
    
    println!("Consensus Result: {} (Avg Confidence: {:.3})", consensus_strength, average_confidence);
    println!("Signals - Bullish: {}, Bearish: {}, Neutral: {}", 
             bullish_signals, bearish_signals, neutral_signals);
    
    // Verify consensus process
    assert_eq!(agent_predictions.len(), consensus_agents.len());
    assert!(average_confidence >= 0.0 && average_confidence <= 1.0);
    
    // All agents should have made predictions
    for (_, prediction) in &agent_predictions {
        assert!(prediction.model_used.is_some());
        assert!(!prediction.prediction_values.is_empty());
    }
    
    Ok(())
}

/// Test Model Fallback and Selection
#[tokio::test]
async fn test_model_fallback_and_selection() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    let agent_id = "model_selection_agent";
    orchestrator.register_daa_agent(agent_id).await?;
    
    // Test different model requirements
    let model_test_scenarios = vec![
        ("SHORT_TERM", 300, 0.9),   // 5 minutes, high confidence
        ("MEDIUM_TERM", 1800, 0.8), // 30 minutes, medium confidence
        ("LONG_TERM", 7200, 0.7),   // 2 hours, lower confidence
    ];
    
    for (scenario_name, horizon, confidence) in model_test_scenarios {
        let decision_context = DecisionContext {
            agent_id: agent_id.to_string(),
            decision_type: format!("MODEL_SELECTION_{}", scenario_name),
            symbol: "MODEL/USD".to_string(),
            market_data: TimeSeriesData {
                symbol: "MODEL/USD".to_string(),
                timestamp: Utc::now(),
                open: 6000.0,
                high: 6100.0,
                low: 5900.0,
                close: 6050.0,
                volume: 80000.0,
                indicators: {
                    let mut indicators = HashMap::new();
                    indicators.insert("RSI".to_string(), 58.0);
                    indicators.insert("MACD".to_string(), 8.0);
                    indicators.insert("STOCH_K".to_string(), 65.0);
                    indicators.insert("ADX".to_string(), 25.0);
                    indicators
                },
            },
            context_metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("scenario".to_string(), json!(scenario_name));
                metadata.insert("time_horizon".to_string(), json!(horizon));
                metadata.insert("preferred_models".to_string(), json!(["NHITS", "DeepAR"]));
                metadata
            },
            required_confidence: confidence,
            prediction_horizon: horizon,
        };
        
        let prediction = orchestrator.get_neural_prediction(decision_context).await?;
        
        // Verify model selection worked
        assert!(prediction.confidence >= 0.0);
        assert!(prediction.model_used.is_some());
        
        let model_used = prediction.model_used.as_ref().unwrap();
        println!("Scenario {}: Used model {}, Confidence: {:.3}", 
                 scenario_name, model_used, prediction.confidence);
        
        // Verify appropriate model was selected for time horizon
        match scenario_name {
            "SHORT_TERM" => {
                // Should prefer fast models for short-term predictions
                assert!(model_used.contains("NHITS") || model_used.contains("TCN"));
            },
            "LONG_TERM" => {
                // Should allow any model for long-term predictions
                assert!(model_used.contains("DeepAR") || model_used.contains("NHITS") || model_used.contains("TCN"));
            },
            _ => {
                // Medium term can use any model
                assert!(!model_used.is_empty());
            }
        }
    }
    
    Ok(())
}

/// Test Cross-Component Memory Usage
#[tokio::test]
async fn test_cross_component_memory_usage() -> Result<()> {
    let config = create_integration_config();
    let orchestrator = PlatformOrchestrator::new(config).await?;
    orchestrator.start_platform().await?;
    
    // Set up multiple agents
    let agents = vec!["memory_agent_1", "memory_agent_2", "memory_agent_3"];
    for agent_id in &agents {
        orchestrator.register_daa_agent(agent_id).await?;
    }
    
    // Generate data across multiple components
    let symbols = vec!["MEM1/USD", "MEM2/USD", "MEM3/USD"];
    
    // Phase 1: Data ingestion and storage
    for (i, symbol) in symbols.iter().enumerate() {
        for sequence in 0..50 {
            let market_data = create_realistic_market_data(symbol, 7000.0 + (i as f64 * 500.0), sequence);
            orchestrator.inject_market_data(market_data).await?;
        }
    }
    
    // Phase 2: Generate predictions (memory intensive)
    for (agent_idx, agent_id) in agents.iter().enumerate() {
        for (symbol_idx, symbol) in symbols.iter().enumerate() {
            let decision_context = DecisionContext {
                agent_id: agent_id.to_string(),
                decision_type: "MEMORY_USAGE_TEST".to_string(),
                symbol: symbol.to_string(),
                market_data: TimeSeriesData {
                    symbol: symbol.to_string(),
                    timestamp: Utc::now(),
                    open: 7000.0 + (symbol_idx as f64 * 500.0),
                    high: 7100.0 + (symbol_idx as f64 * 500.0),
                    low: 6900.0 + (symbol_idx as f64 * 500.0),
                    close: 7050.0 + (symbol_idx as f64 * 500.0),
                    volume: 100000.0,
                    indicators: {
                        let mut indicators = HashMap::new();
                        indicators.insert("RSI".to_string(), 50.0 + (agent_idx as f64 * 5.0));
                        indicators.insert("MACD".to_string(), agent_idx as f64 * 2.0);
                        indicators
                    },
                },
                context_metadata: {
                    let mut metadata = HashMap::new();
                    metadata.insert("memory_test".to_string(), json!(true));
                    metadata.insert("agent_index".to_string(), json!(agent_idx));
                    metadata.insert("symbol_index".to_string(), json!(symbol_idx));
                    metadata
                },
                required_confidence: 0.7,
                prediction_horizon: 1800,
            };
            
            let _ = orchestrator.get_neural_prediction(decision_context).await;
        }
    }
    
    // Phase 3: Check memory usage and system health
    let health = orchestrator.health_check().await?;
    
    // Verify system handled memory usage appropriately
    assert!(health.overall_healthy);
    assert!(health.metrics.total_requests > 0);
    
    // Store results in memory as requested
    let memory_key = "swarm-auto-centralized-1751484080479/integration-testing-final/results";
    orchestrator.store_results_in_memory(memory_key).await?;
    
    // Verify memory storage
    let memory_data = orchestrator.get_memory_data(memory_key).await?;
    assert!(memory_data.contains_key("system_health"));
    assert!(memory_data.contains_key("performance_metrics"));
    
    println!("Cross-component memory test completed successfully");
    println!("Total requests processed: {}", health.metrics.total_requests);
    println!("Average processing time: {:.2}ms", health.metrics.processing_latency_ms);
    
    Ok(())
}