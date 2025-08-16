# Neural Trader Two-Layer Sector Architecture

## Overview

The Neural Trader implements a sophisticated two-layer neural architecture designed for efficient sector-based trading with minimal memory footprint.

```
┌─────────────────────────────────────────────────────────────┐
│                     LAYER 1: SECTOR MODELS                   │
│                                                               │
│  ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐                  │
│  │ XLK │ │ XLF │ │ XLV │ │ XLE │ │ XLY │  (10 ETF Models)  │
│  └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘                  │
│     │       │       │       │       │                        │
│  320-512MB each, trained on ETF data only                    │
└─────┼───────┼───────┼───────┼───────┼───────────────────────┘
      │       │       │       │       │
      ▼       ▼       ▼       ▼       ▼
┌─────────────────────────────────────────────────────────────┐
│                LAYER 2: SYMBOL SPECIALIZATIONS               │
│                                                               │
│  AAPL    JPM     JNJ     XOM    AMZN   (100+ symbols)       │
│  MSFT    BAC     PFE     CVX    HD                          │
│  GOOGL   WFC     UNH     COP    NKE                         │
│                                                               │
│  6-8MB each, lightweight adaptation layers                   │
└─────────────────────────────────────────────────────────────┘
```

## Layer 1: Sector Models (Primary)

### Purpose
Capture sector-wide patterns, trends, and correlations using ETF representative data.

### Implementation
- **Location**: `ClusterModelPool` in `src/neural/vendor_predictor.rs`
- **Training Data**: ETF-only (e.g., XLK for Technology sector)
- **Memory**: 320-512MB per sector
- **Models**: 10 total (one per major sector)

### ETF Representatives
| Sector | ETF | Description |
|--------|-----|-------------|
| Technology | XLK | Technology Select Sector SPDR Fund |
| Financial | XLF | Financial Select Sector SPDR Fund |
| Healthcare | XLV | Health Care Select Sector SPDR Fund |
| Energy | XLE | Energy Select Sector SPDR Fund |
| Consumer Disc. | XLY | Consumer Discretionary Select SPDR |
| Consumer Staples | XLP | Consumer Staples Select Sector SPDR |
| Industrials | XLI | Industrial Select Sector SPDR Fund |
| Materials | XLB | Materials Select Sector SPDR Fund |
| Utilities | XLU | Utilities Select Sector SPDR Fund |
| Real Estate | XLRE | Real Estate Select Sector SPDR Fund |

## Layer 2: Symbol Specializations (Secondary)

### Purpose
Adapt sector predictions to individual symbol characteristics and behaviors.

### Implementation
- **Location**: `SymbolSpecializationLayer` in `src/features/symbol_specialization.rs`
- **Training Data**: Individual symbol data with sector model as base
- **Memory**: 6-8MB per symbol
- **Models**: One per trading symbol (100+)

### Key Features
- Deviation patterns from sector baseline
- Attention weights for symbol-specific features
- Fine-tuning parameters
- Graceful fallback to sector predictions

## Training Sequence

### Phase 1: Sector Model Training
```
1. Load ETF data for each sector (90 days, hourly)
2. Train base neural model on ETF patterns
3. Validate on recent data (last 30 days)
4. Save sector model to ClusterModelPool
5. Repeat for all 10 sectors
```

### Phase 2: Specialization Training
```
1. Load sector model for symbol's sector
2. Generate sector baseline predictions
3. Calculate symbol-specific deviations
4. Train lightweight specialization layer
5. Validate specialization accuracy
6. Save to specialization pool
```

## Prediction Flow

```
Input: Symbol + Market Features
         │
         ▼
┌──────────────────┐
│  Sector Mapper   │ ← Identifies sector (e.g., AAPL → Technology)
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Sector Model    │ ← XLK model provides baseline
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Specialization   │ ← AAPL-specific adjustments
└────────┬─────────┘
         │
         ▼
    Final Prediction
```

## Memory Optimization

### Total Memory Budget: 4GB

#### Breakdown:
- **Sector Models**: 10 × 400MB = 4,000MB (worst case)
- **Actual Usage**: 10 × 320MB = 3,200MB (typical)
- **Specializations**: 100 × 8MB = 800MB
- **Total**: ~4GB

### Optimization Strategies:
1. **Lazy Loading**: Models loaded only when needed
2. **Shared Features**: Sector feature extractors shared across symbols
3. **Memory Pooling**: Reuse allocated memory buffers
4. **Idle Timeout**: Unload unused models after 15 minutes

## Integration Points

### 1. Training Pipeline
- **Module**: `TrainingCoordinator`
- **File**: `src/neural/training_coordinator.rs`
- **Integration**: Orchestrates two-phase training

### 2. Trading Decisions
- **Module**: `DAACoordinator`
- **File**: `src/integration/daa_coordinator.rs`
- **Integration**: Uses predictions for autonomous decisions

### 3. Sector Mapping
- **Module**: `SectorMapper`
- **File**: `src/data/sector_mapper.rs`
- **Integration**: Single source of truth for hierarchy

## Performance Metrics

### Training Performance
- **Phase 1**: 50 minutes (10 sectors × 5 min)
- **Phase 2**: 50 minutes (100 symbols × 30 sec)
- **Total**: 100 minutes

### Inference Performance
- **Sector Model**: ~10ms per prediction
- **Specialization**: ~2ms additional
- **Total Latency**: ~12ms per symbol

### Accuracy Targets
- **Sector Models**: 65-75% directional accuracy
- **With Specialization**: 70-80% directional accuracy
- **Improvement**: 5-10% over sector baseline

## Critical Design Decisions

### Why ETF-Only Training for Sectors?
- ETFs represent pure sector performance
- Avoid noise from individual stock volatility
- Consistent price ranges across time
- Better generalization to sector patterns

### Why Two Layers Instead of One?
- Memory efficiency (4GB vs 40GB)
- Faster training (100 min vs 10 hours)
- Better generalization
- Easier updates and maintenance

### Why Not Aggregate Stock Data for Sectors?
- Price range normalization issues
- Individual stock noise overwhelms patterns
- Market cap weighting complexity
- ETFs already provide clean sector signal

## Maintenance Guidelines

### DO:
- ✅ Train sector models on ETF data only
- ✅ Use specializations for individual stocks
- ✅ Follow Phase 1 → Phase 2 sequence
- ✅ Reference SectorMapper for hierarchy

### DON'T:
- ❌ Train individual stocks as standalone models
- ❌ Aggregate multiple stock prices for sector training
- ❌ Skip Phase 1 sector training
- ❌ Modify ETF representative mappings

## Future Enhancements

1. **Dynamic Sector Rebalancing**: Adjust sector membership quarterly
2. **Cross-Sector Correlation**: Model inter-sector relationships
3. **Adaptive Specialization**: Auto-tune specialization depth
4. **Multi-Timeframe Models**: Separate models for different horizons