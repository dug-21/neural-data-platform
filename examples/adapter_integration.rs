//! Example demonstrating the integration adapters usage
//!
//! This example shows how to use the adapters to:
//! 1. Convert data between formats
//! 2. Integrate with DAA service
//! 3. Bridge trading strategies with AI decisions

use chrono::Utc;
use autonomous_platform::adapters::{
    daa_service::{DAAMessage, DAAServiceAdapter, TradingAction},
    integration_bridge::{BridgeBuilder, IntegrationBridge},
};
use autonomous_platform::data::TimeSeriesData;
use autonomous_platform::strategies::momentum::MomentumStrategy;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Neural Trader Integration Example ===\n");

    // 1. Create sample market data
    let mut data = vec![];
    let base_price = 50000.0;

    for i in 0..50 {
        let price_variation = (i as f64 * 0.1).sin() * 500.0;
        let price = base_price + price_variation;

        let mut indicators = HashMap::new();
        indicators.insert("rsi".to_string(), 50.0 + price_variation / 100.0);
        indicators.insert("macd".to_string(), price_variation / 100000.0);
        indicators.insert("volume_ma".to_string(), 1000.0 + (i as f64 * 10.0));

        data.push(TimeSeriesData {
            symbol: "BTC/USD".to_string(),
            timestamp: Utc::now() - chrono::Duration::minutes(50 - i),
            open: price - 50.0,
            high: price + 100.0,
            low: price - 100.0,
            close: price,
            volume: vec![1000.0 + (i as f64 * 5.0)],
            volume_value: 1000.0 + (i as f64 * 5.0),
            indicators,
            source: Some("example".to_string()),
            entity: Some("BTC/USD".to_string()),
            value: Some(price),
            metadata: None,
            values: vec![price],
            intervals: vec![i as u64],
            timestamps: vec![Utc::now() - chrono::Duration::minutes(50 - i)],
            metadata_map: HashMap::new(),
        });
    }

    // 2. Data format conversion (commented out - neuro_divergent adapter not available)
    println!("Data conversion step skipped - using raw time series data");
    println!("Working with {} data points", data.len());
    
    // 3. Basic data preparation
    println!("Basic data preparation...");
    let recent_data = &data[data.len().saturating_sub(20)..]; // Use last 20 points
    println!("Using {} recent data points for analysis", recent_data.len());

    // 4. Create DAA analysis request
    println!("Creating DAA analysis request...");
    let daa_request = DAAServiceAdapter::create_analysis_request(
        "BTC/USD",
        &data[30..], // Use recent data
        "technical_analysis",
    )?;
    println!("DAA Request ID: {:?}", daa_request.correlation_id);
    println!("Request type: {}\n", daa_request.message_type);

    // 5. Simulate DAA trading decision
    let mock_daa_decision = autonomous_platform::adapters::daa_service::DAATradingDecision {
        action: TradingAction::Buy,
        symbol: "BTC/USD".to_string(),
        quantity: 0.1,
        price: Some(data.last().unwrap().close * 0.99), // Limit buy below market
        confidence: 0.85,
        reasoning: vec![
            "Bullish momentum detected".to_string(),
            "RSI showing oversold conditions".to_string(),
            "Volume increasing".to_string(),
        ],
        risk_assessment: autonomous_platform::adapters::daa_service::RiskAssessment {
            risk_score: 0.3,
            max_drawdown: 0.05,
            position_size_recommendation: 0.1,
            stop_loss_price: Some(data.last().unwrap().close * 0.97),
            take_profit_price: Some(data.last().unwrap().close * 1.03),
        },
        timestamp: Utc::now(),
    };

    // 6. Convert decision to order
    println!("Converting DAA decision to order...");
    let order = DAAServiceAdapter::decision_to_order(&mock_daa_decision);
    println!("Order: {}", serde_json::to_string_pretty(&order)?);
    println!();

    // 7. Setup integration bridge
    println!("Setting up integration bridge...");
    let bridge = BridgeBuilder::new()
        .with_daa_weight(0.6)
        .with_strategy_weight(0.4)
        .with_confidence_threshold(0.7)
        .build();

    // 8. Create a momentum strategy
    let strategy = MomentumStrategy::new(10, 20);

    // 9. Process market data through bridge
    println!("Processing market data through integration bridge...");
    let combined_decision = bridge.process_market_data(&data[30..], &strategy).await?;

    println!("Combined Decision:");
    println!("  Action: {:?}", combined_decision.action);
    println!("  Confidence: {:.2}%", combined_decision.confidence * 100.0);
    println!("  Strategy Signal: {:?}", combined_decision.strategy_signal);
    println!("  Reasoning:");
    for reason in &combined_decision.reasoning {
        println!("    - {}", reason);
    }

    // 10. Simple prediction display (neuro_divergent conversion commented out)
    println!("\nDisplaying mock predictions...");
    let predictions = vec![51000.0, 51200.0, 51100.0, 51300.0, 51500.0];
    
    println!("Generated {} prediction points", predictions.len());
    for (i, pred) in predictions.iter().take(3).enumerate() {
        println!(
            "  T+{}: ${:.2}",
            i + 1,
            pred
        );
    }

    // 11. Send performance feedback
    println!("\nSending performance feedback...");
    let feedback = DAAServiceAdapter::create_performance_feedback(
        &daa_request.correlation_id.unwrap(),
        150.0, // $150 profit
        data.last().unwrap().close * 0.99,
        data.last().unwrap(),
    );
    println!(
        "Feedback sent for decision: {}",
        feedback.correlation_id.unwrap()
    );

    println!("\n=== Integration Example Complete ===");
    Ok(())
}
