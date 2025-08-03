# TimeSeriesData Fix Strategy

## Key Finding

**There's already a `TimeSeriesData::new()` method that properly initializes all 19 fields!**

```rust
// Located in src/data/mod.rs:78
pub fn new(symbol: String, timestamp: DateTime<Utc>) -> Self
```

## The Problem

Tests and code are trying to create TimeSeriesData using struct literals instead of the `new()` method:

```rust
// ❌ WRONG - Missing required fields
TimeSeriesData {
    symbol: "TEST".to_string(),
    timestamp: Utc::now(),
    open: 100.0,
    // ... missing 14 other fields
}

// ✅ CORRECT - Use the constructor
let mut data = TimeSeriesData::new("TEST".to_string(), Utc::now());
data.open = 100.0;
data.high = 101.0;
data.low = 99.0;
data.close = 100.0;
data.add_volume(1000.0);
```

## Fix Strategy

### 1. **Update Test Helpers** (tests/helpers/test_utils.rs)

Change from:
```rust
data.push(TimeSeriesData {
    symbol: "TEST".to_string(),
    timestamp: base_time + chrono::Duration::seconds(i as i64 * 60),
    open: price - 0.5,
    // ...
});
```

To:
```rust
let mut ts_data = TimeSeriesData::new(
    "TEST".to_string(), 
    base_time + chrono::Duration::seconds(i as i64 * 60)
);
ts_data.open = price - 0.5;
ts_data.high = price + 1.0;
ts_data.low = price - 1.0;
ts_data.close = price;
ts_data.add_volume(volume);
data.push(ts_data);
```

### 2. **Update Storage Tests** (tests/data_storage_test.rs)

The storage tests are using a different TimeSeriesData (from storage module). Need to check if they should use the main TimeSeriesData or keep using storage::TimeSeriesData.

### 3. **Fix Source Code Usage**

Files that need updating:
- `src/data/sector_aggregator.rs:555` - Missing fields
- `src/data/data_converter.rs:646` - Missing fields
- `src/features/technical_indicators/*.rs` - Multiple files with missing fields

### 4. **Phase 3 Alignment**

According to Phase 3 design:
- TimeSeriesData should support **dynamic data types** via `metadata_map`
- Should be **channel-agnostic** - data can come from any source
- Should support **multi-scope** - symbol, market, sector, geographic

The current structure already supports this through:
- `metadata_map: HashMap<String, serde_json::Value>` - For dynamic data types
- `source: Option<String>` - For channel-agnostic sources
- `entity: Option<String>` - For different scopes (can be symbol/sector/market)

## Recommended Actions

### Immediate (Fix Compilation):
1. Replace all struct literal creation with `TimeSeriesData::new()`
2. Use the existing helper methods like `add_value()`, `add_volume()`
3. For OHLC data, add a convenience method:

```rust
impl TimeSeriesData {
    pub fn with_ohlc(mut self, open: f64, high: f64, low: f64, close: f64) -> Self {
        self.open = open;
        self.high = high;
        self.low = low;
        self.close = close;
        self
    }
}
```

### Long-term (Phase 3 Alignment):
1. Use `metadata_map` for dynamic data types instead of adding new fields
2. Leverage `source` field for channel information
3. Use `entity` field for scope (symbol/sector/market/geographic)

## Integration-First Compliance

This approach:
- ✅ Extends existing TimeSeriesData (no replacement)
- ✅ Uses existing constructor and methods
- ✅ Maintains backward compatibility
- ✅ Aligns with Phase 3 dynamic data vision