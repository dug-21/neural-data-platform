use std::env;

/// Demonstration of the environment variable fix for data access layer
/// 
/// This script shows the logic that was implemented to fix the critical bug
/// where hardcoded time ranges were replaced with environment variables.

fn main() {
    println!("=== Neural Trader Data Access Environment Variables Fix Demo ===\n");

    // Simulate the fix implementation
    demo_environment_variable_reading();
    println!();
    demo_timeframe_duration_logic();
    println!();
    demo_backward_compatibility();
}

/// Demo the environment variable reading logic
fn demo_environment_variable_reading() {
    println!("1. Environment Variable Reading Logic:");
    
    // Set some test environment variables
    env::set_var("TRAINING_HISTORY_DAYS", "120");
    env::set_var("MIN_TRAINING_HISTORY_DAYS", "45");
    env::set_var("MAX_TRAINING_HISTORY_DAYS", "730");
    
    let (training_days, min_days, max_days) = get_training_history_days();
    
    println!("   TRAINING_HISTORY_DAYS: {} (from env)", training_days);
    println!("   MIN_TRAINING_HISTORY_DAYS: {} (from env)", min_days);
    println!("   MAX_TRAINING_HISTORY_DAYS: {} (from env)", max_days);
    
    // Clean up
    env::remove_var("TRAINING_HISTORY_DAYS");
    env::remove_var("MIN_TRAINING_HISTORY_DAYS");
    env::remove_var("MAX_TRAINING_HISTORY_DAYS");
}

/// Demo the timeframe duration logic that replaces hardcoded values
fn demo_timeframe_duration_logic() {
    println!("2. Timeframe Duration Logic (Before vs After Fix):");
    
    // Set environment variables
    env::set_var("TRAINING_HISTORY_DAYS", "90");
    env::set_var("MIN_TRAINING_HISTORY_DAYS", "30");
    env::set_var("MAX_TRAINING_HISTORY_DAYS", "365");
    
    println!("   BEFORE FIX (hardcoded):");
    println!("     Hourly -> 1 day (hardcoded)");
    println!("     Daily  -> 30 days (hardcoded)");
    println!("     Weekly -> 180 days (hardcoded)");
    
    println!("   AFTER FIX (environment-driven):");
    
    // Simulate the new logic
    let (training_days, min_days, max_days) = get_training_history_days();
    
    // Hourly: min(MIN_TRAINING_HISTORY_DAYS, 7) = min(30, 7) = 7
    let hourly_days = std::cmp::min(min_days, 7);
    println!("     Hourly -> {} days (min(MIN_TRAINING_HISTORY_DAYS, 7))", hourly_days);
    
    // Daily: constrained by min and max
    let daily_days = std::cmp::min(
        std::cmp::max(training_days, min_days),
        max_days
    );
    println!("     Daily  -> {} days (TRAINING_HISTORY_DAYS constrained)", daily_days);
    
    // Weekly: double training days but constrained by max
    let weekly_days = std::cmp::min(
        std::cmp::max(training_days * 2, min_days * 4),
        max_days
    );
    println!("     Weekly -> {} days (TRAINING_HISTORY_DAYS * 2 constrained)", weekly_days);
    
    // Clean up
    env::remove_var("TRAINING_HISTORY_DAYS");
    env::remove_var("MIN_TRAINING_HISTORY_DAYS");
    env::remove_var("MAX_TRAINING_HISTORY_DAYS");
}

/// Demo backward compatibility when env vars are not set
fn demo_backward_compatibility() {
    println!("3. Backward Compatibility (No Environment Variables Set):");
    
    // Ensure no env vars are set
    env::remove_var("TRAINING_HISTORY_DAYS");
    env::remove_var("MIN_TRAINING_HISTORY_DAYS");
    env::remove_var("MAX_TRAINING_HISTORY_DAYS");
    
    let (training_days, min_days, max_days) = get_training_history_days();
    
    println!("   Using defaults when environment variables are not set:");
    println!("     TRAINING_HISTORY_DAYS: {} (default)", training_days);
    println!("     MIN_TRAINING_HISTORY_DAYS: {} (default)", min_days);
    println!("     MAX_TRAINING_HISTORY_DAYS: {} (default)", max_days);
    
    println!("   This ensures the system works even if env vars are missing!");
}

/// Replicated the logic from DataAccessLayer::get_training_history_days()
fn get_training_history_days() -> (i64, i64, i64) {
    let training_history_days = env::var("TRAINING_HISTORY_DAYS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(90); // Default: 90 days

    let min_training_history_days = env::var("MIN_TRAINING_HISTORY_DAYS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30); // Default: 30 days

    let max_training_history_days = env::var("MAX_TRAINING_HISTORY_DAYS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(365); // Default: 365 days

    (training_history_days, min_training_history_days, max_training_history_days)
}