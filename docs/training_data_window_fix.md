# Training Data Window Configuration Fix

## Problem
The system is only loading 4 hours of historical data for training, which is completely inadequate for neural network pattern recognition.

## Root Cause
Hardcoded value in `src/main.rs:229`:
```rust
let start = end - chrono::Duration::hours(4);
```

## Solution

### 1. Environment-Based Configuration
Add to `.env`:
```bash
# Training data window (in days)
TRAINING_HISTORY_DAYS=90  # 3 months default
MIN_TRAINING_HISTORY_DAYS=30  # Minimum 1 month
MAX_TRAINING_HISTORY_DAYS=365  # Maximum 1 year
```

### 2. Dynamic Window Based on Model Type
Different models need different history:
- **LSTM/GRU**: 60-90 days (sequential patterns)
- **Transformer**: 90-180 days (attention mechanisms)
- **MLP**: 30-60 days (simpler patterns)
- **Ensemble**: 180-365 days (comprehensive analysis)

### 3. Market-Aware Windows
```rust
pub fn get_training_window(symbol: &str, model_type: ModelType) -> Duration {
    match classify_symbol(symbol) {
        SymbolType::Stock => Duration::days(90),    // Individual stocks: 3 months
        SymbolType::ETF => Duration::days(180),     // ETFs: 6 months (more stable)
        SymbolType::Crypto => Duration::days(365),  // Crypto: 1 year (24/7 trading)
    }
}
```

### 4. Progressive Loading Strategy
```rust
// Start with recent data for quick initialization
let quick_start = end - Duration::days(7);
// Then load full history asynchronously
let full_history = end - Duration::days(90);
```

### 5. Data Validation
Ensure sufficient data points:
```rust
const MIN_DATA_POINTS: usize = 1000;  // Minimum for training
const OPTIMAL_DATA_POINTS: usize = 10000;  // Optimal for convergence
```

## Implementation Priority
1. **Immediate**: Change hardcoded 4 hours to 30 days minimum
2. **Short-term**: Add environment variable configuration
3. **Medium-term**: Implement model-specific windows
4. **Long-term**: Progressive loading with validation

## Impact
- **Current**: Models cannot learn meaningful patterns
- **After Fix**: 
  - Better prediction accuracy
  - Proper trend detection
  - Seasonal pattern recognition
  - Market cycle awareness