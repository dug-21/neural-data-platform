fn main() {
    // Simulate the fallback logic calculation from storage.rs
    
    // Example: User requests 7 days of data
    let days_requested = 7;
    let records_found_hourly = 41;  // From the logs - this is what we're getting
    
    println!("=== Fallback Logic Debug ===");
    println!("Days requested: {}", days_requested);
    println!("Records found in market_data_1h: {}", records_found_hourly);
    
    // This is the calculation from storage.rs lines 302-304
    let duration_days = days_requested;
    let expected_hourly_records = duration_days * 8; // 8 trading hours per day
    let sufficient_data_threshold = (expected_hourly_records as f64 * 0.5) as usize; // At least 50% coverage
    
    println!("\nCalculated values:");
    println!("  duration_days: {}", duration_days);
    println!("  expected_hourly_records: {} (8 hours × {} days)", expected_hourly_records, duration_days);
    println!("  sufficient_data_threshold: {} (50% of expected)", sufficient_data_threshold);
    
    // This is the condition check from storage.rs line 308
    let should_fallback = records_found_hourly == 0 || records_found_hourly < sufficient_data_threshold;
    
    println!("\nFallback condition check:");
    println!("  results.is_empty(): {}", records_found_hourly == 0);
    println!("  results.len() < sufficient_data_threshold: {} < {} = {}", 
             records_found_hourly, sufficient_data_threshold, records_found_hourly < sufficient_data_threshold);
    println!("  Should fallback: {}", should_fallback);
    
    if should_fallback {
        println!("\n✅ FALLBACK SHOULD TRIGGER");
        println!("   - Should query 'market_data' table");
        println!("   - Should access 130,593+ records");
        println!("   - Should log: 'Hourly data insufficient ({} < {} expected)'", records_found_hourly, sufficient_data_threshold);
    } else {
        println!("\n❌ FALLBACK WILL NOT TRIGGER");
        println!("   - Will use {} records from market_data_1h", records_found_hourly);
        println!("   - Will NOT access the raw market_data table");
    }
    
    // Test with different scenarios
    println!("\n=== Testing Different Scenarios ===");
    
    let test_cases = vec![
        ("1 day", 1, 20),
        ("3 days", 3, 41),
        ("7 days", 7, 41),
        ("30 days", 30, 41),
    ];
    
    for (label, days, found_records) in test_cases {
        let expected = days * 8;
        let threshold = (expected as f64 * 0.5) as usize;
        let will_fallback = found_records < threshold;
        
        println!("{}: {} records found, {} expected, {} threshold → {}", 
                 label, found_records, expected, threshold,
                 if will_fallback { "FALLBACK" } else { "NO FALLBACK" });
    }
}