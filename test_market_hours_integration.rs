/// Test to verify DAA market hours integration works correctly
/// This is a simple compilation test for our market hours fixes

use autonomous_platform::utils::market_hours::{MarketHours, Exchange};
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let market_hours = MarketHours::new();
    let now = Utc::now();
    
    let nyse_open = market_hours.is_market_open(Exchange::NYSE, now).await;
    let nasdaq_open = market_hours.is_market_open(Exchange::NASDAQ, now).await;
    let markets_open = nyse_open || nasdaq_open;
    
    if markets_open {
        println!("🔥 [MARKET HOURS] Markets are open - DAA should prioritize trading!");
        println!("   NYSE: {}, NASDAQ: {}", nyse_open, nasdaq_open);
    } else {
        println!("🌃 [AFTER-HOURS] Markets are closed - DAA can perform training");
    }
    
    // Test market status string generation
    let status = if nyse_open && nasdaq_open {
        "Both NYSE and NASDAQ open".to_string()
    } else if nyse_open {
        "NYSE open, NASDAQ closed".to_string()
    } else if nasdaq_open {
        "NASDAQ open, NYSE closed".to_string()
    } else {
        "All major markets closed".to_string()
    };
    
    println!("Market Status: {}", status);
    
    Ok(())
}