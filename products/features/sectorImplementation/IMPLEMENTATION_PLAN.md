# Sector-Based Two-Layer Architecture Implementation Plan

## Executive Summary
The neural-trader codebase **already contains** a complete two-layer architecture implementation. The components exist but are **not properly wired together**. This plan focuses on connecting existing functionality rather than creating new code.

## Current State Analysis

### ✅ EXISTING Components (No new code needed)

#### 1. **ClusterModelPool** (`src/neural/vendor_predictor.rs`)
- Lines 234-500: Complete two-layer pool management
- Manages sector ETF models + symbol specializations
- Memory efficient design already implemented
- **KEY METHOD**: `process_symbol()` - single entry point for both training and prediction

#### 2. **TrainingCoordinator** (`src/neural/training_coordinator.rs`)
- Lines 424-600: Full two-phase training orchestration
- `train_sector_model()` - Phase 1 ETF training (line 237)
- `train_specialization()` - Phase 2 symbol training (line 345)
- Already handles training sequence correctly

#### 3. **SectorMapper** (`src/data/sector_mapper.rs`)
- Lines 165-750: Production-ready sector hierarchy
- ETF representatives mapped (XLK→Technology, XLF→Financial, etc.)
- Symbol-to-sector classification working
- **SINGLE SOURCE OF TRUTH** for sector structure

#### 4. **SymbolSpecializationLayer** (`src/features/symbol_specialization.rs`)
- 900+ lines of specialization logic
- Memory-efficient (<2MB per symbol)
- Feature extraction and enhancement
- Graceful fallback to sector features

### ❌ The Problem: Broken Wiring

The issue is in `vendor_predictor.rs` line 2051-2065:

```rust
// CURRENT (WRONG):
fn get_training_symbols_for_model(&self, symbol: &str) -> Result<Vec<String>> {
    if symbol_loader::is_sector_etf(symbol) {
        // ✅ ETF correctly trains on ETF data only
        Ok(vec![symbol.to_string()])
    } else {
        // ❌ Individual stocks train in isolation (WRONG!)
        // Should reference sector model, not train independently
        Ok(vec![symbol.to_string()])
    }
}
```

## Implementation Plan

### Phase 1: Fix the Wiring (2-3 hours)

#### Step 1.1: Fix `get_training_symbols_for_model()` 
**File**: `src/neural/vendor_predictor.rs` (lines 2051-2065)

```rust
fn get_training_symbols_for_model(&self, symbol: &str) -> Result<Vec<String>> {
    // CRITICAL: Preserve two-layer architecture
    // Layer 1: ETF models train on ETF data only
    // Layer 2: Individual stocks use specialization layers
    
    if symbol_loader::is_sector_etf(symbol) {
        // ✅ CORRECT: ETF models train only on their own data
        info!("🎯 [SECTOR_MODEL] Training primary sector model for ETF: {}", symbol);
        Ok(vec![symbol.to_string()])
    } else {
        // ✅ FIXED: Individual stocks should NOT train full models
        // They should only train lightweight specializations
        info!("🔧 [SPECIALIZATION] {} will use sector model + specialization layer", symbol);
        
        // Return empty - specialization training handled separately
        Ok(vec![])  // Don't load data for full model training
    }
}
```

#### Step 1.2: Connect TrainingCoordinator to ClusterModelPool
**File**: `src/neural/vendor_predictor.rs` (line ~850)

```rust
// Add method to trigger two-phase training
pub async fn execute_two_phase_training(&self) -> Result<()> {
    // Use existing TrainingCoordinator
    let coordinator = TrainingCoordinator::new(
        self.sector_mapper.clone(),
        self.training_data_service.clone(),
        // ... existing config
    );
    
    // Phase 1: Train sector models on ETF data
    let phase1_results = coordinator.execute_training_phase().await?;
    
    // Phase 2: Train specializations (automatically triggered)
    // TrainingCoordinator already handles the transition
    
    Ok(())
}
```

#### Step 1.3: Route Predictions Through ClusterModelPool
**File**: `src/neural/vendor_predictor.rs` (line ~3000)

```rust
pub async fn predict(&self, symbol: &str, features: &[f64]) -> Result<f64> {
    // Determine sector for symbol
    let sector_info = self.sector_mapper.get_sector(symbol)?;
    
    // Get or create cluster pool for this sector
    let pool = self.cluster_pools.entry(sector_info.id)
        .or_insert_with(|| ClusterModelPool::new(sector_info.id));
    
    // Use existing process_symbol() method - it handles both layers!
    let prediction = pool.process_symbol(symbol, features).await?;
    
    Ok(prediction)
}
```

### Phase 2: Add Preservation Comments (1 hour)

#### Critical Comments to Add:

**File**: `src/neural/vendor_predictor.rs`
```rust
// ============================================================
// CRITICAL ARCHITECTURE: Two-Layer Sector-Based Model System
// ============================================================
// 
// This system implements a hierarchical two-layer architecture:
// 
// LAYER 1 - Sector Models (Primary):
//   - 10 ETF-based models (XLK, XLF, XLV, etc.)
//   - Each trained ONLY on its ETF representative data
//   - Captures sector-wide patterns and trends
//   - Memory: 320-512MB per sector model
//
// LAYER 2 - Symbol Specializations (Secondary):
//   - Lightweight adaptation layers per symbol
//   - Builds on top of sector model predictions
//   - Memory: 6-8MB per specialization
//   - Quick adaptation to symbol-specific patterns
//
// TRAINING SEQUENCE (CRITICAL - DO NOT CHANGE):
//   1. Phase 1: Train all sector models on ETF data
//   2. Phase 2: Train specializations using sector models as base
//
// PREDICTION FLOW:
//   1. Sector model provides baseline prediction
//   2. Specialization layer adjusts for symbol-specific patterns
//   3. Combined prediction returned to user
//
// WARNING: Do NOT allow individual stocks to train full models!
// They must ONLY train specialization layers on top of sector models.
// ============================================================
```

**File**: `src/data/sector_mapper.rs`
```rust
// ============================================================
// SINGLE SOURCE OF TRUTH: Sector Hierarchy Definition
// ============================================================
// 
// This module defines the sector structure used by BOTH:
//   - Training pipeline (TrainingCoordinator)
//   - Trading decisions (DAACoordinator)
//
// ETF Representatives (DO NOT CHANGE):
//   Technology → XLK
//   Financial → XLF
//   Healthcare → XLV
//   Energy → XLE
//   Consumer Discretionary → XLY
//   Consumer Staples → XLP
//   Industrials → XLI
//   Materials → XLB
//   Utilities → XLU
//   Real Estate → XLRE
//
// All components MUST reference this module for sector structure.
// ============================================================
```

### Phase 3: Integration Testing (2 hours)

#### Test 3.1: Verify ETF-Only Training
```rust
#[test]
async fn test_etf_trains_on_etf_data_only() {
    let predictor = VendorPredictor::new(...);
    let data = predictor.get_training_symbols_for_model("XLK").unwrap();
    assert_eq!(data, vec!["XLK"]); // Should only return XLK
}
```

#### Test 3.2: Verify Stock Specialization
```rust
#[test]
async fn test_stock_uses_specialization_not_full_model() {
    let predictor = VendorPredictor::new(...);
    let data = predictor.get_training_symbols_for_model("AAPL").unwrap();
    assert_eq!(data, vec![]); // Should return empty - no full model training
}
```

#### Test 3.3: Verify Two-Phase Training
```rust
#[test]
async fn test_two_phase_training_sequence() {
    let coordinator = TrainingCoordinator::new(...);
    let results = coordinator.execute_training_phase().await.unwrap();
    
    // Should complete Phase 1 (sectors) before Phase 2 (specializations)
    assert!(results.phase1_completed);
    assert!(results.phase2_completed);
}
```

### Phase 4: Documentation (1 hour)

Create documentation files:
1. `products/features/sectorImplementation/ARCHITECTURE.md` - System design
2. `products/features/sectorImplementation/TRAINING_FLOW.md` - Training sequence
3. `products/features/sectorImplementation/MAINTENANCE.md` - Future developer guide

## Resource Requirements

### Memory Allocation (from sector_models.toml)
- **Per Sector Model**: 320-512MB
- **Per Specialization**: 6-8MB
- **Total for 10 sectors + 100 symbols**: ~4GB

### Training Time Estimates
- **Phase 1**: 10 sector models × 5 minutes = 50 minutes
- **Phase 2**: 100 specializations × 30 seconds = 50 minutes
- **Total**: ~100 minutes for full training

## Success Criteria

1. ✅ ETF models train ONLY on ETF data
2. ✅ Individual stocks use specialization layers, not full models
3. ✅ Training follows Phase 1 → Phase 2 sequence
4. ✅ Predictions use both sector model + specialization
5. ✅ Memory usage stays within configured limits
6. ✅ Same hierarchy used for training and trading

## Risk Mitigation

1. **Risk**: Accidentally creating duplicate functionality
   - **Mitigation**: Use ONLY existing components
   
2. **Risk**: Breaking existing trading decisions
   - **Mitigation**: ClusterModelPool.process_symbol() already handles both

3. **Risk**: Future developers breaking hierarchy
   - **Mitigation**: Comprehensive comments and documentation

## Timeline

- **Day 1** (4 hours):
  - Fix wiring in vendor_predictor.rs
  - Add preservation comments
  
- **Day 2** (3 hours):
  - Integration testing
  - Documentation
  
- **Total**: 7 hours of work

## Conclusion

The two-layer architecture is **85% complete**. We only need to:
1. Fix the wiring in `get_training_symbols_for_model()`
2. Connect TrainingCoordinator to ClusterModelPool
3. Add clear comments to preserve the architecture

No new functionality needed - just proper connection of existing components.