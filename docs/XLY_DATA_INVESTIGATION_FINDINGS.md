# XLY Data Investigation Findings

## Problem Summary
The swarm claimed to have refreshed the continuous aggregate `market_data_1h` with 53,302 hourly records total, but the application still only finds 36 XLY records and fails training due to insufficient data.

## Investigation Results

### 1. Database State Verification ✅
**Database contains abundant XLY data:**
- Total records in `market_data_1h`: **53,302**
- XLY records in continuous aggregate: **2,681**
- XLY records in last 90 days (app query range): **555**
- Time range for XLY: 2024-08-08 11:00:00+00 to 2025-08-13 15:00:00+00

### 2. Application Query Analysis ✅
**App is using the correct parameters:**
- Uses `TRAINING_HISTORY_DAYS=90` (90 days of historical data)
- Query range: 2025-05-15 15:00:00 UTC to 2025-08-13 15:00:00 UTC  
- Expected XLY records in this range: **555**
- App logs show: "📊 Loading 90 days of hourly data (TRAINING_HISTORY_DAYS=90) - requesting 2160 hour window"

### 3. Actual App Results 🚨
**Application only finds 36 XLY records:**
```
SELECT bucket as timestamp, symbol, open, high, low, close, volume::float8 as volume
FROM market_data_1h
WHERE symbol = 'XLY' AND bucket >= '2025-05-15 15:00:00' AND bucket <= '2025-08-13 15:00:00'
ORDER BY bucket ASC
```
- App log: "rows affected: 36, rows returned: 36"
- Time range in app data: "2025-08-06 13:00:00 to 2025-08-13 15:00:00 (36 data points)"

### 4. The Discrepancy 🔍
**Database vs Application Results:**
- Database query (same range): **555 records** ✅
- Application query (same range): **36 records** ❌
- Missing records: **519 records** (93.5% data loss)

### 5. Root Cause Analysis 🎯

The discrepancy suggests **a different table or query being used in the application code**. The logs show the application is using per-symbol isolation queries, which might be hitting a different code path.

**Evidence from logs:**
1. App uses training data service with symbol isolation
2. Logs show: "🔍 [DATA ISOLATION] Loading raw data for SYMBOL XLY ONLY"
3. App falls back to: "✅ [FALLBACK] Successfully loaded 36 recent market data samples for XLY"

### 6. The Real Issue 📍

The application's training data service is **NOT using the continuous aggregate `market_data_1h` for symbol-specific queries**. Instead, it's:

1. Attempting to load symbol-isolated data through a different path
2. Failing to get sufficient data (36 vs expected 555+ records)
3. Falling back to "recent market data samples"

### 7. Missing Code Path Investigation Required 🔧

Need to examine:
- `/src/integration/training_data_service.rs` lines ~430+ 
- Symbol isolation query implementation
- Why it's getting 36 instead of 555 records
- Whether it's querying `market_data` instead of `market_data_1h`

## Conclusion

**The continuous aggregate refresh DID work correctly** (53,302 total records, 2,681 XLY records). 

**The issue is in the application's training data service path** - there are two different data loading mechanisms:

1. **Main historical data loader** (working correctly):
   - Loads from `market_data_1h` with 90-day range
   - Gets 11,185 total records for all symbols in main.rs
   - Query: `2025-05-15 15:00:00 UTC to 2025-08-13 15:00:00 UTC`

2. **Training data service symbol isolation** (problematic):
   - Uses `data_access.get_market_data()` → `storage.query_range()`
   - Despite configuration for 90 days, only returns 36 XLY records
   - Falls back to "recent market data samples"

**Root Cause**: The `storage.query_range()` function has complex fallback logic that appears to be truncating results or using a different time calculation than the main loader.

**Next Steps:**
1. Debug why `storage.query_range()` returns only 36 records for XLY in 90-day range
2. Check if there's caching or different time calculations in `storage.rs`
3. Align training data service with main historical data loader logic
4. Consider using the same query path for both main loading and training data

## Data Verification Commands
```sql
-- Total records (working)
SELECT COUNT(*) FROM market_data_1h; -- 53,302

-- XLY records (working) 
SELECT COUNT(*) FROM market_data_1h WHERE symbol = 'XLY'; -- 2,681

-- XLY in app range (working)
SELECT COUNT(*) FROM market_data_1h WHERE symbol = 'XLY' 
AND bucket >= '2025-05-15 15:00:00' AND bucket <= '2025-08-13 15:00:00'; -- 555

-- App is getting only 36 - investigate training_data_service.rs
```