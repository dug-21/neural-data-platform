use chrono::{DateTime, Utc};
use autonomous_platform::utils::market_hours::{MarketHours, Exchange};

#[tokio::main]
async fn main() {
    println!("=== Market Hours Debug Analysis ===");
    
    let market_hours = MarketHours::new();
    let now = Utc::now();
    
    println!("Current UTC time: {}", now.format("%Y-%m-%d %H:%M:%S UTC"));
    println!("Current ET time should be: {}", 
             (now + chrono::Duration::hours(-4)).format("%Y-%m-%d %H:%M:%S EDT"));
    
    // Test NYSE market status
    let nyse_open = market_hours.is_market_open(Exchange::NYSE, now).await;
    let nasdaq_open = market_hours.is_market_open(Exchange::NASDAQ, now).await;
    
    println!("\n=== Market Status ===");
    println!("NYSE is open: {}", nyse_open);
    println!("NASDAQ is open: {}", nasdaq_open);
    
    // Test session detection
    let nyse_session = market_hours.get_session(Exchange::NYSE, now).await;
    println!("NYSE session: {:?}", nyse_session);
    
    // Test market intensity
    let intensity = market_hours.get_market_intensity(now).await;
    println!("Market intensity: {:?}", intensity);
    
    // Test specific trading hours
    let test_times = vec![
        // 9:25 AM EDT (13:25 UTC) - should be pre-market
        Utc::now().date_naive().and_hms_opt(13, 25, 0).unwrap().and_utc(),
        // 9:35 AM EDT (13:35 UTC) - should be open
        Utc::now().date_naive().and_hms_opt(13, 35, 0).unwrap().and_utc(),
        // 3:45 PM EDT (19:45 UTC) - should be open (current time)
        now,
        // 4:05 PM EDT (20:05 UTC) - should be closed
        Utc::now().date_naive().and_hms_opt(20, 5, 0).unwrap().and_utc(),
    ];
    
    println!("\n=== Time Tests ===");
    for test_time in test_times {
        let is_open = market_hours.is_market_open(Exchange::NYSE, test_time).await;
        let session = market_hours.get_session(Exchange::NYSE, test_time).await;
        println!("{} UTC ({}EDT) -> Open: {}, Session: {:?}", 
                 test_time.format("%H:%M"),
                 (test_time + chrono::Duration::hours(-4)).format("%H:%M"),
                 is_open,
                 session);
    }
}