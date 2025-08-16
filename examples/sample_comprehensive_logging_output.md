# Sample Comprehensive Logging Output

This document shows what the comprehensive symbol data loading logging will look like when training models.

## Example Output for ETF (SPY)

```
🚀 [SYMBOL_LOADING] ============================================
📈 [SYMBOL_LOADING] Processing training data for symbol: SPY
🏷️ [SYMBOL_TYPE] SPY classified as: ETF (Exchange-Traded Fund)
🏢 [SECTOR_MAPPING] SPY → Sector: Market (ID: market)
🏭 [CLUSTER_AVAILABILITY] ✅ Cluster pool ready for sector: market
📊 [DATA_LOADING] Loading OHLCV data for SPY
    📦 Sample count: 1000 data points
    📅 Time range: 2024-06-15 09:30:00 to 2024-08-12 16:00:00
    ⏱️ Duration: 1416 hours (59 days)
💰 [PRICE_ANALYSIS] SPY price statistics:
    📊 Range: $520.15 to $567.89 (spread: $47.74)
    📈 Average Open: $543.20, Average Close: $543.78
    📉 Price volatility: 8.77%
📊 [VOLUME_ANALYSIS] SPY volume statistics:
    📦 Range: 45000000 to 125000000
    📊 Average: 78500000
    📈 Volume ratio: 2.78x (max/min)
🔍 [DATA_SAMPLE] First 3 data points for SPY:
    #1: 2024-06-15 09:30 | O:$520.15 H:$522.80 L:$519.45 C:$521.90 V:82000000
    #2: 2024-06-15 10:30 | O:$521.90 H:$523.15 L:$520.88 C:$522.45 V:68000000
    #3: 2024-06-15 11:30 | O:$522.45 H:$524.20 L:$521.95 C:$523.10 V:75000000
🔍 [DATA_SAMPLE] Last 3 data points for SPY:
    #1000: 2024-08-12 14:00 | O:$566.20 H:$567.89 L:$565.15 C:$567.25 V:92000000
    #999: 2024-08-12 15:00 | O:$567.25 H:$567.80 L:$566.40 C:$567.55 V:88000000
    #998: 2024-08-12 16:00 | O:$567.55 H:$568.10 L:$566.95 C:$567.80 V:115000000
🚀 [SYMBOL_LOADING] ============================================
🔧 [NORMALIZATION] Enforcing MinMax normalization for 1000 data points
📈 [AGGREGATION] Data already in 1-hour format - no aggregation needed
📊 [NORMALIZATION] Original dataset statistics:
    💰 Price range: $520.15 to $567.89 (spread: $47.74)
    📦 Volume range: 45000000 to 125000000 (ratio: 2.78x)
✅ [NORMALIZATION] Successfully normalized 1000 data points for training
🔄 [TRAINING_PIPELINE] Routing SPY through sector-based architecture
    🏢 Sector: Market (market)
🏭 [CONTAINER] Using cluster pool 2-layer architecture for training: SPY
    🎯 Training mode: Sector-specific cluster pool
    🏭 Cluster pool ID: market
    📊 Processing 1000 normalized data points through 2-layer architecture
🎯 [TRAINING_MODE] ETF training mode activated for SPY
```

## Example Output for Individual Stock (AAPL)

```
🚀 [SYMBOL_LOADING] ============================================
📈 [SYMBOL_LOADING] Processing training data for symbol: AAPL
🏷️ [SYMBOL_TYPE] AAPL classified as: Individual Stock
🏢 [SECTOR_MAPPING] AAPL → Sector: Technology (ID: technology)
🏭 [CLUSTER_AVAILABILITY] ✅ Cluster pool ready for sector: technology
📊 [DATA_LOADING] Loading OHLCV data for AAPL
    📦 Sample count: 1500 data points
    📅 Time range: 2024-05-01 09:30:00 to 2024-08-12 16:00:00
    ⏱️ Duration: 2520 hours (105 days)
💰 [PRICE_ANALYSIS] AAPL price statistics:
    📊 Range: $168.25 to $237.95 (spread: $69.70)
    📈 Average Open: $202.15, Average Close: $202.88
    📉 Price volatility: 34.35%
📊 [VOLUME_ANALYSIS] AAPL volume statistics:
    📦 Range: 25000000 to 180000000
    📊 Average: 68500000
    📈 Volume ratio: 7.20x (max/min)
🔍 [DATA_SAMPLE] First 3 data points for AAPL:
    #1: 2024-05-01 09:30 | O:$168.25 H:$170.45 L:$167.80 C:$169.88 V:95000000
    #2: 2024-05-01 10:30 | O:$169.88 H:$171.20 L:$169.15 C:$170.55 V:78000000
    #3: 2024-05-01 11:30 | O:$170.55 H:$172.10 L:$169.95 C:$171.25 V:82000000
🔍 [DATA_SAMPLE] Last 3 data points for AAPL:
    #1500: 2024-08-12 14:00 | O:$236.80 H:$237.95 L:$235.90 C:$237.25 V:112000000
    #1499: 2024-08-12 15:00 | O:$237.25 H:$237.80 L:$236.45 C:$237.40 V:95000000
    #1498: 2024-08-12 16:00 | O:$237.40 H:$237.75 L:$236.85 C:$237.50 V:125000000
🚀 [SYMBOL_LOADING] ============================================
🎯 [TRAINING_MODE] Individual stock training mode for AAPL
```

## Example Output for Cryptocurrency (BTCUSD)

```
🚀 [SYMBOL_LOADING] ============================================
📈 [SYMBOL_LOADING] Processing training data for symbol: BTCUSD
🏷️ [SYMBOL_TYPE] BTCUSD classified as: Cryptocurrency
❌ [SECTOR_MAPPING] Failed to map BTCUSD to any sector
🏭 [CLUSTER_AVAILABILITY] ❌ No cluster pool for sector: unknown
📊 [DATA_LOADING] Loading OHLCV data for BTCUSD
    📦 Sample count: 2000 data points
    📅 Time range: 2024-04-01 00:00:00 to 2024-08-12 23:00:00
    ⏱️ Duration: 3288 hours (137 days)
💰 [PRICE_ANALYSIS] BTCUSD price statistics:
    📊 Range: $25,234.50 to $73,856.90 (spread: $48,622.40)
    📈 Average Open: $49,545.70, Average Close: $49,652.30
    📉 Price volatility: 98.02%
📊 [VOLUME_ANALYSIS] BTCUSD volume statistics:
    📦 Range: 125000000 to 8500000000
    📊 Average: 2850000000
    📈 Volume ratio: 68.00x (max/min)
🎯 [TRAINING_MODE] Custom/sector training mode for BTCUSD
```

## Key Benefits of This Logging

1. **Complete Pipeline Visibility**: Every step of data loading is now logged with clear emoji indicators
2. **Symbol Type Classification**: Automatic detection of ETF vs stock vs crypto vs other asset types
3. **Data Quality Verification**: Price/volume ranges help identify potential data issues
4. **Training Mode Transparency**: Clear indication of which training path is being used
5. **Performance Monitoring**: Detailed statistics for optimization and analysis
6. **Debugging Support**: Sample data logging helps identify data pipeline issues

## Log Categories

- 🚀 Major operations and section headers
- 📈 Symbol processing and classification
- 🏷️ Symbol type identification
- 🏢 Sector mapping information
- 🏭 Cluster pool availability and usage
- 📊 Data loading and statistics
- 💰 Price analysis and ranges
- 📦 Volume analysis and ratios
- 🔍 Sample data verification
- 🔧 Data normalization processes
- 🔄 Training pipeline routing
- 🎯 Training mode selection
- ✅ Success indicators
- ❌ Error conditions
- ⚠️ Warning messages