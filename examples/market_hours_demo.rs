//! Market Hours Demo
//! 
//! Demonstrates the comprehensive market hours tracking functionality

use autonomous_platform::utils::market_hours::{Exchange, MarketHours, MarketSession};
use chrono::{Duration, Utc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Market Hours Tracking Demo\n");
    
    // Initialize market hours tracker
    let market_hours = MarketHours::new();
    let now = Utc::now();
    
    println!("Current UTC time: {}", now.format("%Y-%m-%d %H:%M:%S"));
    println!("\n=== Exchange Status ===");
    
    // Check status of all major exchanges
    for exchange in [
        Exchange::NYSE,
        Exchange::NASDAQ,
        Exchange::LSE,
        Exchange::TSE,
        Exchange::SSE,
        Exchange::BSE,
    ] {
        let status = market_hours.get_market_status(exchange, now).await;
        
        println!("\n{:?} ({}):", exchange, status.timezone);
        println!("  Status: {}", if status.is_open { "OPEN" } else { "CLOSED" });
        println!("  Session: {:?}", status.session);
        
        if status.is_holiday {
            println!("  🎄 Holiday");
        }
        
        if status.is_half_day {
            println!("  ⏰ Half-day (early close)");
        }
        
        if let Some(next_open) = status.next_open {
            println!("  Next open: {}", next_open.format("%Y-%m-%d %H:%M UTC"));
        }
        
        if let Some(next_close) = status.next_close {
            println!("  Next close: {}", next_close.format("%Y-%m-%d %H:%M UTC"));
        }
    }
    
    // Market intensity analysis
    println!("\n=== Market Intensity Analysis ===");
    let intensity = market_hours.get_market_intensity(now).await;
    println!("Global market intensity: {:.2}%", intensity.score * 100.0);
    println!("Active exchanges: {}", intensity.active_exchanges);
    println!("Dominant session: {:?}", intensity.dominant_session);
    println!("Volume estimate: {:.2}%", intensity.volume_estimate * 100.0);
    
    // Training window analysis
    println!("\n=== Training Window Analysis ===");
    let window = market_hours.get_training_window(now).await;
    println!("Current training window: {}", window);
    
    let resource_limit = market_hours.get_resource_limit(now).await;
    println!("Recommended resource limit: {:.0}%", resource_limit * 100.0);
    
    // Find next optimal training window
    if let Some((start, end)) = market_hours.find_next_training_window(
        now,
        Duration::hours(2),
        autonomous_platform::utils::market_hours::TrainingWindow::Good,
    ).await {
        println!("\nNext optimal training window:");
        println!("  Start: {}", start.format("%Y-%m-%d %H:%M UTC"));
        println!("  End: {}", end.format("%Y-%m-%d %H:%M UTC"));
        println!("  Duration: {} hours", (end - start).num_hours());
    }
    
    // Active exchanges
    println!("\n=== Currently Active Exchanges ===");
    let active = market_hours.get_active_exchanges(now).await;
    if active.is_empty() {
        println!("No exchanges currently active");
    } else {
        for (exchange, session) in active {
            println!("  {:?}: {:?}", exchange, session);
        }
    }
    
    // Simulate different times
    println!("\n=== Time Simulation ===");
    println!("Market activity over the next 24 hours:");
    
    let mut sim_time = now;
    for hour in 0..24 {
        if hour % 6 == 0 {
            let intensity = market_hours.get_market_intensity(sim_time).await;
            let active = market_hours.get_active_exchanges(sim_time).await;
            println!("\n  +{:2}h ({}):", hour, sim_time.format("%H:%M UTC"));
            println!("    Intensity: {:.0}%", intensity.score * 100.0);
            println!("    Active: {} exchanges", active.len());
            
            if !active.is_empty() {
                print!("    Exchanges: ");
                for (i, (ex, _)) in active.iter().enumerate() {
                    if i > 0 { print!(", "); }
                    print!("{:?}", ex);
                }
                println!();
            }
        }
        sim_time = sim_time + Duration::hours(1);
    }
    
    Ok(())
}