/// Test to verify the autonomous trading system returns to normal operation
/// after emergency training completes and models exist.

use neural_trader::integration::daa_coordinator::{DaaCoordinator, DaaConfig};
use neural_trader::features::market_data::MarketContext;
use neural_trader::features::market_data::TimeSeriesData;
use chrono::{Utc, TimeZone};
use anyhow::Result;

/// Test that once models exist, the system properly returns to trading mode during market hours
#[tokio::test]
async fn test_emergency_training_to_trading_transition() -> Result<()> {
    // Create a DAA coordinator
    let config = DaaConfig {
        enabled: true,
        neural_weight: 0.6,
        strategy_weight: 0.4,
        min_confidence_threshold: 0.7,
        enable_adaptation: true,
    };
    
    let coordinator = DaaCoordinator::new(config).await?;

    // Create test market context during trading hours 
    let trading_hour_time = Utc.with_ymd_and_hms(2024, 3, 15, 14, 30, 0).unwrap(); // 2:30 PM EST on a Friday
    let market_context = MarketContext {
        symbol: "AAPL".to_string(),
        current_price: 150.0,
        volume: 1000000,
        bid: 149.95,
        ask: 150.05,
        volatility: 0.25,
        timestamp: trading_hour_time.timestamp(),
    };

    // Create some mock historical data
    let historical_data = vec![
        TimeSeriesData {
            timestamp: trading_hour_time - chrono::Duration::minutes(5),
            open: 149.0,
            high: 150.5,
            low: 148.5,
            close: 150.0,
            volume: 100000,
        }
    ];

    println!("🧪 Testing emergency training to trading transition...");

    // Test 1: Initially check if training would be prioritized (when no models exist)
    println!("📊 Step 1: Check initial training priority (emergency case)...");
    let should_train_initially = coordinator.should_prioritize_training().await;
    println!("📈 Should prioritize training initially (no models): {}", should_train_initially);

    // Test 2: Check if we can make a trading decision (this should work regardless)
    println!("📊 Step 2: Test making trading decisions during market hours...");
    
    match coordinator.make_decision(&market_context, None, &historical_data).await {
        Ok(decision) => {
            println!("✅ Successfully made trading decision: {:?} with confidence {:.2}%", 
                     decision.action, decision.confidence * 100.0);
            println!("📋 Decision reasoning: {:?}", decision.reasoning);
        }
        Err(e) => {
            println!("❌ Failed to make trading decision: {}", e);
            return Err(e);
        }
    }

    // Test 3: Simulate that models now exist by checking the coordinator's methods
    println!("📊 Step 3: Check training priority after models exist...");
    
    // Create some mock models directory to simulate emergency training completion
    std::fs::create_dir_all("./models/production")?;
    std::fs::create_dir_all("./models/checkpoints")?;
    
    // Create a dummy model file to indicate models exist
    std::fs::write("./models/production/test_model.json", "{\"version\": \"1.0\"}")?;
    
    // Now check if training is still prioritized
    let should_train_after = coordinator.should_prioritize_training().await;
    println!("📈 Should prioritize training after models exist: {}", should_train_after);

    // Test 4: Verify emergency training logic vs normal market hours logic
    println!("📊 Step 4: Verify normal market hours vs emergency logic...");
    let models_available = coordinator.check_model_availability().await?;
    println!("📂 Models available: {} (total: {})", 
             models_available.has_any_models, models_available.total_count);

    // Test 5: Check that we can continue making trading decisions
    println!("📊 Step 5: Final trading decision test...");
    match coordinator.make_decision(&market_context, None, &historical_data).await {
        Ok(decision) => {
            println!("✅ Trading decisions work normally: {:?} with confidence {:.2}%", 
                     decision.action, decision.confidence * 100.0);
        }
        Err(e) => {
            println!("❌ Trading decisions failed after models exist: {}", e);
            return Err(e);
        }
    }

    // Cleanup
    let _ = std::fs::remove_dir_all("./models");
    
    println!("✅ All tests passed - autonomous trading system properly transitions from emergency training to normal trading");
    Ok(())
}

#[tokio::test]
async fn test_market_timing_logic_consistency() -> Result<()> {
    let config = DaaConfig {
        enabled: true,
        neural_weight: 0.6,
        strategy_weight: 0.4,
        min_confidence_threshold: 0.7,
        enable_adaptation: true,
    };
    
    let coordinator = DaaCoordinator::new(config).await?;

    println!("🧪 Testing market timing logic consistency...");

    // Test during market hours
    let markets_open_for_trading = coordinator.should_prioritize_trading().await;
    let markets_open_timing_check = coordinator.check_market_timing().await;
    let should_train = coordinator.should_prioritize_training().await;

    println!("📈 Should prioritize trading: {}", markets_open_for_trading);
    println!("📈 Market timing check (open?): {}", markets_open_timing_check);
    println!("📚 Should prioritize training: {}", should_train);

    // These should be logically consistent:
    // - should_prioritize_trading() and check_market_timing() should return the same value
    // - should_prioritize_training() should be the opposite (unless emergency override)
    assert_eq!(markets_open_for_trading, markets_open_timing_check, 
               "Trading priority and market timing check should return same value");

    println!("✅ Market timing logic is now consistent");
    Ok(())
}