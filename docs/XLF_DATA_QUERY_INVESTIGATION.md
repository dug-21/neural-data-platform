# XLF Data Query Investigation Report

## Issue Summary

**Problem**: Only 41 XLF records are being returned by queries despite having 2 years of data loaded (130,593+ records).

**Root Cause**: Data aggregation and table structure mismatch.

## Investigation Findings

### 1. Data Distribution Analysis

#### Raw Data (market_data table):
- **Total XLF records**: 130,593 records
- **Date range**: 2024-08-08 08:00:00+00 to 2025-08-13 15:41:00+00
- **Duration**: ~1 year of data (not 2 years as initially thought)
- **Resolution**: High frequency (likely minute or tick data)

#### Aggregated Data (market_data_1h table):
- **Total XLF records**: 41 records
- **Date range**: 2025-08-06 13:00:00+00 to 2025-08-12 20:00:00+00
- **Duration**: Only ~6 days of recent data
- **Resolution**: Hourly aggregated data

### 2. Query Logic Analysis

The application is designed to prefer hourly aggregated data (`market_data_1h`) over raw data (`market_data`) for performance reasons. However, the hourly aggregation table only contains very recent data (last 6 days), while the bulk of the historical data remains in the raw `market_data` table.

#### Code Analysis (src/data/storage.rs lines 256-301):
```rust
// First try market_data_1h (hourly aggregated data)
let hourly_results = sqlx::query(
    r#"
    SELECT bucket as timestamp, symbol, open, high, low, close, volume::float8 as volume
    FROM market_data_1h
    WHERE symbol = $1 
      AND bucket >= $2 
      AND bucket <= $3
    ORDER BY bucket ASC
"#,
)
```

The query correctly targets `market_data_1h` but this table only has 41 records for XLF, all from the last few days.

### 3. Time Range Calculation

The application uses a default 30-day lookback window (TRAINING_HISTORY_DAYS=30), which should return data from 2025-07-14 to 2025-08-13. However:

- `market_data_1h` only has data from 2025-08-06 onwards (6 days)
- `market_data` has data going back to 2024-08-08 (full year)

### 4. Fallback Logic Issue

The code has fallback logic to use `market_data_1m` if hourly data is insufficient, but:
- `market_data_1m` table doesn't exist in the database
- The fallback never triggers the raw `market_data` table

## Root Cause Analysis

The issue is **NOT** with:
- The SQL query syntax (✓ correct)
- The WHERE clause conditions (✓ correct) 
- The parameter binding (✓ correct)
- The LIMIT clause (✓ not the limiting factor)
- Time zone handling (✓ correct)

The issue **IS** with:
1. **Incomplete data aggregation**: The `market_data_1h` table only contains recent data
2. **Missing fallback path**: No fallback to the raw `market_data` table when hourly data is insufficient
3. **Data pipeline gap**: Historical data exists but hasn't been properly aggregated into hourly buckets

## Recommended Fixes

### Immediate Fix (Option A): Modify Fallback Logic
```rust
// After checking market_data_1h, if insufficient records, fall back to market_data
if results.is_empty() || results.len() < expected_minimum {
    let raw_results = sqlx::query(
        r#"
        SELECT time as timestamp, symbol, open, high, low, close, volume
        FROM market_data
        WHERE symbol = $1 
          AND time >= $2 
          AND time <= $3
        ORDER BY time ASC
        LIMIT $4
    "#,
    )
    .bind(symbol)
    .bind(start)
    .bind(end) 
    .bind(limit)
    .fetch_all(&self.pool)
    .await?;
    
    // Process raw_results...
}
```

### Long-term Fix (Option B): Complete Data Aggregation
Run a data aggregation job to populate `market_data_1h` with historical data:

```sql
INSERT INTO market_data_1h (bucket, symbol, open, high, low, close, volume)
SELECT 
    time_bucket('1 hour', time) as bucket,
    symbol,
    first(open, time) as open,
    max(high) as high,
    min(low) as low,
    last(close, time) as close,
    sum(volume) as volume
FROM market_data 
WHERE symbol = 'XLF'
  AND time >= '2024-08-08'
  AND time < '2025-08-06'  -- Don't duplicate existing data
GROUP BY bucket, symbol
ORDER BY bucket;
```

### Testing the Fix

To verify the fix works:

1. **Before fix**: Query returns 41 records
2. **After fix**: Query should return records from the full 30-day window
3. **Expected result**: ~720 hourly records (30 days × 24 hours) for market hours data

## Impact Assessment

- **Severity**: High - Training models with insufficient data
- **Affected components**: All neural training using XLF data
- **Data availability**: 130K+ records available but not accessible
- **Fix complexity**: Medium (requires either code change or data migration)

## Conclusion

The 41-record limit is due to incomplete data aggregation in the `market_data_1h` table, not a query or logic error. The vast majority of XLF data (130K+ records) exists in the raw `market_data` table but is not being accessed due to missing fallback logic.