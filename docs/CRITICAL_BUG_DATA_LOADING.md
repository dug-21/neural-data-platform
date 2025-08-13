# CRITICAL BUG: Training Data Loading Failure

## Problem Statement
The system has **208,168 XLV records** and **225,119 XLK records** in the database but only retrieves **36 records** for training, causing training to fail with "Insufficient data" errors.

## Root Cause
The `get_market_data()` function in `/workspaces/neural-trader/src/integration/data_access.rs` uses **hardcoded time ranges** instead of reading the configured environment variables.

### Current Broken Code (Lines 232-239)
```rust
let start_time = match timeframe {
    Timeframe::Minute => end_time - Duration::hours(1),
    Timeframe::FiveMinute => end_time - Duration::hours(4),
    Timeframe::FifteenMinute => end_time - Duration::hours(12),
    Timeframe::Hourly => end_time - Duration::days(1),      // ❌ Only 1 day!
    Timeframe::Daily => end_time - Duration::days(30),      // ❌ Only 30 days!
    Timeframe::Weekly => end_time - Duration::days(180),
};
```

### Environment Variables (Set but Ignored)
```bash
TRAINING_HISTORY_DAYS=90        # Should use 90 days of data
MIN_TRAINING_HISTORY_DAYS=30    # Minimum acceptable
MAX_TRAINING_HISTORY_DAYS=365   # Maximum allowed
```

## Impact
1. **Training Failure**: System requests 1070 samples but only gets 36 (1 day of hourly data)
2. **Wasted Data**: 2 years of loaded data (2.1M+ records) sits unused
3. **Poor Model Performance**: Models train on 7 fallback samples instead of thousands available

## Actual Data Available
- **XLK**: 225,119 records (2 years: 2023-07-24 to 2025-08-12)
- **XLV**: 208,168 records (2 years: 2023-07-24 to 2025-08-12)
- **Last 90 days**: 21,834 records for XLV alone
- **Last 1 day**: Only 233 records (what the code actually fetches)

## Why Only 36 Records?
The query fetches 1 day of data (233 records), but after aggregation to hourly timeframe and filtering, only 36 hourly candles remain.

## Required Fix
The `get_market_data()` function needs to:
1. Read `TRAINING_HISTORY_DAYS` environment variable
2. Use it to set the time range for queries
3. Fall back to defaults only if ENV var is not set

### Proposed Fix
```rust
// Read from environment
let training_days = std::env::var("TRAINING_HISTORY_DAYS")
    .unwrap_or_else(|_| "90".to_string())
    .parse::<i64>()
    .unwrap_or(90);

let start_time = match timeframe {
    Timeframe::Hourly => end_time - Duration::days(training_days),
    Timeframe::Daily => end_time - Duration::days(training_days * 4), // Or use MAX_TRAINING_HISTORY_DAYS
    // ... other timeframes
};
```

## Verification
After fix, the system should:
- Fetch ~21,000+ records for 90 days of hourly data
- Successfully train models with proper data volumes
- Stop falling back to 7-sample training

## Severity: CRITICAL
This bug renders the entire training system ineffective despite having years of data loaded.