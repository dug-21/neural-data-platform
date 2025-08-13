# Training Threshold Adjustment for Limited Data Scenarios

## Current Situation
- ETF symbols have minimal historical data (< 40 data points)
- System requires 1000 samples minimum for training
- This creates a deadlock: can't train without data, can't improve without training

## Recommended Adjustments

### 1. Progressive Training Thresholds
```rust
pub fn get_adaptive_threshold(symbol: &str, available_data: usize) -> usize {
    match classify_symbol(symbol) {
        SymbolType::ETF if available_data < 100 => {
            // Bootstrap mode for new ETFs
            max(available_data / 2, 20) // Minimum 20 samples
        },
        SymbolType::ETF if available_data < 500 => {
            // Early training mode
            max(available_data / 3, 100)
        },
        _ => {
            // Standard mode
            env::var("TRAINING_SAMPLE_THRESHOLD")
                .unwrap_or("1000".to_string())
                .parse()
                .unwrap_or(1000)
        }
    }
}
```

### 2. Tiered Training Strategy

#### Bootstrap Phase (< 100 samples)
- Use simple moving averages
- Basic trend detection
- Minimal neural network layers
- Focus on data collection

#### Early Phase (100-500 samples)
- Enable basic neural models (MLP only)
- Short prediction horizons (1-4 hours)
- Higher learning rates for quick adaptation
- Frequent retraining (every 10 new samples)

#### Standard Phase (500+ samples)
- Full model suite (MLP, LSTM, Transformer)
- Normal prediction horizons
- Standard learning rates
- Regular retraining schedule

### 3. Immediate Configuration Changes

Add to environment:
```bash
# Minimum samples for bootstrap training
BOOTSTRAP_SAMPLE_THRESHOLD=20

# Early phase threshold
EARLY_SAMPLE_THRESHOLD=100

# Standard threshold (existing)
TRAINING_SAMPLE_THRESHOLD=1000

# Enable progressive training
ENABLE_PROGRESSIVE_TRAINING=true
```

### 4. Data Augmentation for Limited Samples

When data < 100 samples:
- Use synthetic data generation
- Apply noise injection
- Time-series bootstrap resampling
- Cross-symbol transfer learning (use similar ETFs)

### 5. Monitoring Recommendations

Track these metrics:
- Data accumulation rate per symbol
- Training attempts vs successes
- Model performance at different data levels
- Time to reach each threshold

## Implementation Priority

1. **Immediate**: Lower threshold to 100 for ETFs to enable basic training
2. **Short-term**: Implement progressive thresholds
3. **Medium-term**: Add data augmentation
4. **Long-term**: Cross-symbol transfer learning

## Expected Impact

- ETFs can start training with 100 samples (~4 days of hourly data)
- Models improve progressively as data accumulates
- No deadlock between data requirements and model availability
- Faster time-to-value for new symbols