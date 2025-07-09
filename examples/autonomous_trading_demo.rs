//! Autonomous Trading Demo
//! 
//! This example demonstrates how neural-trader uses vendored ruv-fann and DAA libraries
//! for autonomous trading decisions without any custom/placeholder implementations.

use std::sync::Arc;
use tokio::time::{sleep, Duration};
use chrono::Utc;
use anyhow::Result;

use autonomous_platform::{
    // Core components
    config::{Config, NeuralConfig},
    data::{TimeSeriesData, MarketContext},
    
    // Neural components (using vendored ruv-fann)
    neural::{NeuralPredictor, NeuralPredictorTrait, fann_predictor::FannPredictor},
    
    // Strategy components
    strategies::{
        TradingStrategy, StrategyConfig, Position,
        neural_enhanced::NeuralEnhancedStrategy,
    },
    
    // DAA integration (using vendored DAA service)
    agents::daa_bridge::DAAAgent,
    agents::{AgentConfig, TradingStrategy as AgentStrategy},
    
    // Integration bridge
    adapters::{
        integration_bridge::{IntegrationBridge, BridgeBuilder},
        neuro_divergent::NeuroDivergentAdapter,
        daa_service::DAAServiceAdapter,
    },
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();
    
    println!("🚀 Autonomous Trading Demo - Using Vendored Libraries Only!");
    println!("=" * 60);
    
    // Step 1: Initialize Neural Predictor with real FANN networks
    println!("\n📊 Step 1: Initializing FANN Neural Networks...");
    let neural_config = NeuralConfig {
        memory_gb: 2.0,
        models: vec![
            "NHITS".to_string(),     // Hierarchical interpolation
            "TCN".to_string(),       // Temporal convolutional
            "DeepAR".to_string(),    // Probabilistic forecasting
            "Transformer".to_string(), // Attention-based
        ],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.85,
    };
    
    let fann_predictor = Arc::new(FannPredictor::new(neural_config.clone())?);
    println!("✅ FANN neural networks initialized with {} models", neural_config.models.len());
    
    // Step 2: Initialize DAA Agent for autonomous decision-making
    println!("\n🤖 Step 2: Creating DAA Autonomous Agent...");
    let agent_config = AgentConfig {
        id: "demo-agent-001".to_string(),
        name: "Autonomous Trader".to_string(),
        strategy: AgentStrategy::Adaptive,
        risk_tolerance: 0.7,
        max_position_size: 10000.0,
        decision_threshold: 0.65,
        learning_rate: 0.001,
        enable_meta_learning: true,
    };
    
    let daa_agent = DAAAgent::new(agent_config.clone()).await?;
    println!("✅ DAA agent created with adaptive cognitive pattern");
    
    // Step 3: Create Neural-Enhanced Strategy
    println!("\n📈 Step 3: Setting up Neural-Enhanced Strategy...");
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let mut strategy = NeuralEnhancedStrategy::new(neural_predictor.clone());
    
    let strategy_config = StrategyConfig {
        name: "neural_enhanced".to_string(),
        enabled: true,
        risk_limit: 0.02,
        position_size: 0.01,
        parameters: serde_json::json!({
            "min_confidence": 0.7,
            "neural_weight": 0.6,
            "momentum_weight": 0.3,
            "mean_reversion_weight": 0.1,
        }),
    };
    strategy.initialize(strategy_config).await?;
    println!("✅ Neural-enhanced strategy initialized");
    
    // Step 4: Setup Integration Bridge
    println!("\n🔗 Step 4: Creating Integration Bridge...");
    let bridge = BridgeBuilder::new()
        .with_daa_weight(0.6)
        .with_strategy_weight(0.4)
        .with_confidence_threshold(0.7)
        .build();
    println!("✅ Integration bridge configured (DAA: 60%, Strategy: 40%)");
    
    // Step 5: Generate sample market data
    println!("\n📊 Step 5: Generating market data...");
    let mut market_data = generate_sample_market_data("BTC/USD", 100);
    
    // Use neuro-divergent adapter to enhance data
    let df = NeuroDivergentAdapter::to_neuro_divergent_df(&market_data)?;
    println!("✅ Market data converted to neuro-divergent format with {} rows", df.height());
    
    // Step 6: Run autonomous trading simulation
    println!("\n🎮 Step 6: Running Autonomous Trading Simulation...");
    println!("-" * 60);
    
    let mut position: Option<Position> = None;
    let mut total_pnl = 0.0;
    let mut trade_count = 0;
    
    // Simulate 10 trading periods
    for i in 0..10 {
        println!("\n📍 Period {}/10", i + 1);
        
        // Get current market slice
        let current_data = &market_data[..50 + i];
        let current_price = current_data.last().unwrap().close;
        
        // 1. Get neural predictions using FANN
        println!("  🧠 Getting FANN neural predictions...");
        let predictions = fann_predictor.predict_ensemble(
            current_data,
            5,
            &neural_config.models,
            None,
        ).await?;
        
        if let Some(pred) = predictions.first() {
            println!("    - Predicted price: ${:.2} (confidence: {:.2}%)", 
                pred.value, pred.confidence * 100.0);
            println!("    - Model: {}", pred.model_name);
        }
        
        // 2. Get DAA autonomous decision
        println!("  🤖 Getting DAA autonomous decision...");
        let market_context = current_data.last().unwrap();
        let daa_market_data = autonomous_platform::mcp::trading_tools::MarketData {
            timestamp: market_context.timestamp,
            open: market_context.open,
            high: market_context.high,
            low: market_context.low,
            close: market_context.close,
            volume: market_context.volume,
        };
        
        let daa_decision = daa_agent.make_decision(
            "BTC/USD",
            &daa_market_data,
            position.as_ref().map(|p| p.size).unwrap_or(0.0),
            1000.0,
        ).await?;
        
        println!("    - Action: {}", daa_decision.action);
        println!("    - Confidence: {:.2}%", daa_decision.confidence * 100.0);
        println!("    - Reasoning: {}", daa_decision.reasoning);
        
        // 3. Get strategy signal
        println!("  📊 Getting strategy signal...");
        let market_ctx = MarketContext {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now().timestamp(),
            current_price,
            bid: current_price * 0.999,
            ask: current_price * 1.001,
            volume_24h: market_context.volume * 24.0,
            volatility: 0.02,
        };
        
        let signal = strategy.generate_signal(&market_ctx, position.as_ref()).await?;
        
        // 4. Combine decisions through bridge
        println!("  🔗 Combining decisions...");
        let combined = bridge.process_market_data(current_data, &strategy).await?;
        
        println!("    - Final action: {:?}", combined.action);
        println!("    - Combined confidence: {:.2}%", combined.confidence * 100.0);
        
        // 5. Execute trade based on combined decision
        match combined.action {
            autonomous_platform::adapters::integration_bridge::FinalAction::Buy => {
                if position.is_none() {
                    position = Some(Position {
                        symbol: "BTC/USD".to_string(),
                        size: 1000.0,
                        entry_price: current_price,
                        entry_time: Utc::now(),
                        position_type: autonomous_platform::strategies::PositionType::Long,
                    });
                    trade_count += 1;
                    println!("  💰 BOUGHT at ${:.2}", current_price);
                }
            }
            autonomous_platform::adapters::integration_bridge::FinalAction::Sell => {
                if let Some(pos) = &position {
                    let pnl = (current_price - pos.entry_price) * pos.size / pos.entry_price;
                    total_pnl += pnl;
                    println!("  💸 SOLD at ${:.2} (P&L: ${:.2})", current_price, pnl);
                    position = None;
                }
            }
            autonomous_platform::adapters::integration_bridge::FinalAction::Hold => {
                println!("  ⏸️  HOLDING position");
            }
        }
        
        // Add some market movement
        if i < market_data.len() - 50 {
            market_data[50 + i].close *= 1.0 + (rand::random::<f64>() - 0.5) * 0.02;
        }
        
        // Small delay to simulate real-time
        sleep(Duration::from_millis(500)).await;
    }
    
    // Final summary
    println!("\n" + "=" * 60);
    println!("📊 TRADING SUMMARY");
    println!("=" * 60);
    println!("Total trades: {}", trade_count);
    println!("Total P&L: ${:.2}", total_pnl);
    println!("Final position: {}", if position.is_some() { "Open" } else { "Closed" });
    
    println!("\n✅ Demo completed successfully!");
    println!("\n🎯 Key Components Used:");
    println!("  - ruv-fann: Real FANN neural networks for predictions");
    println!("  - neuro-divergent: Advanced time series models");
    println!("  - DAA Service: Autonomous agent decision-making");
    println!("  - Integration Bridge: Combining multiple decision sources");
    println!("\n💡 No placeholder code - all using vendored libraries!");
    
    Ok(())
}

/// Generate sample market data for demonstration
fn generate_sample_market_data(symbol: &str, count: usize) -> Vec<TimeSeriesData> {
    use std::collections::HashMap;
    
    let mut data = Vec::new();
    let mut price = 50000.0; // Starting BTC price
    let base_time = Utc::now() - chrono::Duration::hours(count as i64);
    
    for i in 0..count {
        // Simulate price movement
        let change = (rand::random::<f64>() - 0.5) * 0.02; // ±2% movement
        price *= 1.0 + change;
        
        // Calculate OHLCV
        let high = price * (1.0 + rand::random::<f64>() * 0.005);
        let low = price * (1.0 - rand::random::<f64>() * 0.005);
        let volume = 1000.0 + rand::random::<f64>() * 500.0;
        
        // Calculate indicators
        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 30.0 + rand::random::<f64>() * 40.0);
        indicators.insert("macd".to_string(), change * 100.0);
        indicators.insert("volume_ratio".to_string(), volume / 1000.0);
        
        data.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: base_time + chrono::Duration::hours(i as i64),
            open: price * 0.999,
            high,
            low,
            close: price,
            volume,
            indicators,
            source: Some("demo".to_string()),
            entity: Some(symbol.to_string()),
            value: Some(price),
            metadata: None,
        });
    }
    
    data
}

// Add rand as a dev dependency for the demo
use rand;