# Architecture Variance Analysis: Sector-Based Training

## Intended Architecture (Correct)
1. **Primary Layer**: 10 sector models trained on ETF representatives (XLK, XLF, XLV, etc.)
   - Each sector model trains ONLY on its ETF data (e.g., XLK for technology sector)
   - This captures sector-wide patterns and trends
   
2. **Secondary Layer**: Lightweight specialization models per individual symbol
   - Each symbol has a small specialization layer
   - Builds on top of the sector model
   - Much lighter training (8MB vs 512MB for sector model)

## Current Implementation (INCORRECT)

### Problem Location: `/src/neural/vendor_predictor.rs`

The `get_training_symbols_for_model()` function (lines 2051-2065) is doing SYMBOL ISOLATION incorrectly:

```rust
// Current WRONG implementation:
if symbol_loader::is_sector_etf(symbol) {
    // ETF ISOLATION: Load ONLY the ETF symbol's data
    info!("🎯 [SYMBOL_ISOLATION] ETF model for {}: Loading ONLY ETF data (not sector aggregation)", symbol);
    Ok(vec![symbol.to_string()])  // ❌ This is correct for ETF
} else {
    // INDIVIDUAL STOCK: Load only that stock's data
    info!("🎯 [SYMBOL_ISOLATION] Individual stock model for {}: Loading ONLY stock data", symbol);
    Ok(vec![symbol.to_string()])  // ❌ This should reference sector model, not train independently
}
```

### What's Wrong:
1. **ETF Training**: ✅ Correctly trains on ETF data only (good!)
2. **Individual Stock Training**: ❌ Trains each stock independently instead of as specialization layer
3. **Missing**: No connection between sector models and individual stock models

## Required Changes

### 1. Fix Training Architecture
The system needs to:
- **First**: Train the 10 primary sector models (one per ETF)
- **Then**: Train lightweight specialization layers for each symbol within that sector
- **Connection**: Individual stock models should reference their sector's primary model

### 2. Memory Allocation (from sector_models.toml)
```toml
[sectors.technology]
etf_representative = "XLK"
shared_memory_mb = 512      # Primary sector model
specialization_memory_mb = 8 # Per-symbol specialization
```

### 3. Missing Implementation
- No code for training primary sector models first
- No code for specialization layers that build on sector models
- `cluster_pools` exist but aren't being used properly

## Where the Variance Occurs

1. **Line 2057**: Correctly identifies ETF and trains on ETF data ✅
2. **Line 2061**: Incorrectly trains individual stocks in isolation ❌
3. **Missing**: No two-layer training orchestration
4. **Missing**: No sector model → stock specialization flow

## Impact
- System trains 111+ independent models instead of 10 sector + specializations
- Each stock model has no sector context
- Memory usage is inefficient
- Training time is excessive
- Models lack sector-wide pattern recognition

## Solution Required
1. Implement proper two-layer training:
   - Train sector models on ETF data first
   - Train specialization layers that reference sector models
2. Fix the `get_training_symbols_for_model()` to understand hierarchy
3. Implement proper model referencing between layers