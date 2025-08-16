# Data Evolution Strategy for Neural Models

## Problem Statement

The neural-trader system needs to support 6 data modalities for optimal performance:
1. **Price Data** (Available now)
2. **Volume Data** (Available now) 
3. **Economic Data** (Not available)
4. **Sentiment Data** (Not available)
5. **Order Book Data** (Not available)
6. **Fundamental Data** (Not available)

Currently, we only have 1-minute price aggregates for a few symbols. The system must be built to handle this gracefully and evolve as new data becomes available.

## Solution: Lazy Model Activation

### Core Principle
Models are configured but only activated when their data requirements are satisfied. This allows us to build the complete system now without waiting for all data sources.

### Model Activation Lifecycle

```
┌─────────────────┐     Data Available?     ┌─────────────────┐
│   Configured    │ ──────────Yes──────────▶│     Active      │
│   (Lazy Pool)   │                          │   (Predicting)  │
└─────────────────┘                          └─────────────────┘
         │                                            │
         │                                            │
         └──────────No──────────┐                    │
                               ▼                      │
                        ┌─────────────────┐           │
                        │    Waiting      │           │
                        │  (Monitoring)   │◀──────────┘
                        └─────────────────┘
```

## Implementation Details

### 1. Configuration-Based Model Requirements

**⚠️ IMPORTANT**: The previous hardcoded model categories were **wrong assumptions**. Instead, we use **completely configurable** model definitions:

```toml
# config/models.toml - No hardcoded assumptions!

[models.lstm_flexible]
architecture = "LSTM"
input_size = 24
hidden_size = 64

[models.lstm_flexible.data_requirements]
required = ["price"]              # Minimum: just price
optional = ["volume", "sentiment"] # Will use if available  
preferred = ["price", "volume"]   # Best performance
adaptive_complexity = true       # Adjust size based on data

[models.tft_minimal]
architecture = "TFT"
d_model = 128
num_heads = 4

[models.tft_minimal.data_requirements]
required = ["price"]              # TFT CAN work with just price!
optional = ["volume", "economic", "sentiment"]
preferred = ["price", "volume", "economic"]
fallback_mode = "basic_attention" # Reduce complexity if needed

[models.deepar_adaptive] 
architecture = "DeepAR"
hidden_size = 100

[models.deepar_adaptive.data_requirements]
required = ["price"]              # Start with price-only
optional = ["sentiment", "volume"] 
preferred = ["price", "sentiment"]
adaptive_complexity = true       # Scale with available data
```

**Key Benefits:**
- **No assumptions** about which models need which data
- **Every model configurable** - TFT can work with price-only or full multi-modal
- **User-controlled** - Easy to experiment with different combinations
- **Adaptive complexity** - Models automatically adjust to available data

### 2. Data Availability Tracker

```rust
pub struct DataAvailabilityTracker {
    /// Available data types per symbol
    availability: DashMap<String, HashSet<DataType>>,
    /// Data quality metrics
    quality: DashMap<(String, DataType), DataQuality>,
    /// Minimum history available
    history_depth: DashMap<(String, DataType), usize>,
}

impl DataAvailabilityTracker {
    pub async fn register_data_arrival(
        &self,
        symbol: &str,
        data_type: DataType,
        quality: DataQuality,
        history_depth: usize
    ) -> Result<()> {
        self.availability
            .entry(symbol.to_string())
            .or_default()
            .insert(data_type.clone());
        
        self.quality
            .insert((symbol.to_string(), data_type.clone()), quality);
        
        self.history_depth
            .insert((symbol.to_string(), data_type), history_depth);
        
        // Notify predictor of new data availability
        self.notify_predictor().await?;
        
        Ok(())
    }
}
```

### 3. Dynamic Model Activation

```rust
impl VendorPredictor {
    /// Called when new data type becomes available
    pub async fn on_data_availability_change(
        &mut self,
        symbol: &str,
        new_data: DataType
    ) -> Result<Vec<String>> {
        let mut activated_models = Vec::new();
        
        // Check each lazy model
        for (model_id, config) in self.lazy_models.iter() {
            if self.can_activate_now(model_id, symbol, &new_data)? {
                // Create and activate the model
                let model = ModelFactory::create_model(
                    &config.architecture,
                    config.clone()
                )?;
                
                // Load any existing checkpoints
                if let Ok(checkpoint) = self.load_checkpoint(model_id).await {
                    model.load_state(checkpoint)?;
                }
                
                // Move from lazy to active
                self.active_models.insert(model_id.clone(), model);
                activated_models.push(model_id.clone());
                
                info!("✅ Activated {} for {} with new {} data", 
                    model_id, symbol, new_data);
            }
        }
        
        // Remove activated models from lazy pool
        for id in &activated_models {
            self.lazy_models.remove(id);
        }
        
        Ok(activated_models)
    }
}
```

### 4. Graceful Prediction Degradation

```rust
impl VendorPredictor {
    pub async fn predict_with_available_data(
        &self,
        symbol: &str,
        request: &PredictionRequest
    ) -> Result<PredictionResult> {
        // Get available data for this symbol
        let available_data = self.data_tracker
            .get_available_data(symbol)
            .await?;
        
        // Get models that can work with this data
        let usable_models = self.get_usable_models(symbol, &available_data)?;
        
        if usable_models.is_empty() {
            warn!("No models available for {} with current data", symbol);
            return Err(anyhow!("Insufficient data for prediction"));
        }
        
        // Build TimeSeriesData with available features
        let ts_data = self.build_time_series_data(symbol, &available_data)?;
        
        // Run ensemble prediction with available models
        let predictions = self.ensemble_predict(&usable_models, &ts_data).await?;
        
        // Adjust confidence based on data completeness
        let data_completeness = available_data.len() as f64 / 6.0; // 6 total modalities
        let adjusted_confidence = predictions.confidence * (0.5 + 0.5 * data_completeness);
        
        Ok(PredictionResult {
            value: predictions.value,
            confidence: adjusted_confidence,
            models_used: usable_models.len(),
            data_completeness,
            ..predictions
        })
    }
}
```

## Benefits of This Approach

### 1. **Immediate Deployment**
- System works immediately with just price data
- No need to wait for all data sources

### 2. **Automatic Evolution**
- Models activate automatically as data arrives
- No code changes needed for new data

### 3. **Transparent Degradation**
- Confidence scores reflect data availability
- Users know when predictions are limited

### 4. **Future Proof**
- Easy to add new models for new data types
- Vendor models handle diverse inputs naturally

### 5. **No Wasted Resources**
- Only active models consume memory
- Lazy models wait efficiently

## Example Evolution Timeline

### Day 1: Launch with Price Data
```
Active Models: MLP_Basic, DLinear_Price, TCN_Price (3 models)
Lazy Models: 24 models waiting for data
Confidence: ~50% (limited data)
```

### Month 1: Volume Data Arrives
```
Active Models: Previous 3 + MLP_Volume, BiTCN_PV, GRU_PV (6 models)
Lazy Models: 21 models
Confidence: ~65% (better volume patterns)
```

### Month 3: Sentiment Integration
```
Active Models: Previous 6 + DeepAR_Sentiment, LSTM_Sentiment (8 models)
Lazy Models: 19 models
Confidence: ~75% (market sentiment included)
```

### Month 6: Full Multi-Modal
```
Active Models: All 27 models
Lazy Models: None
Confidence: ~95% (all data modalities)
```

## Conclusion

This approach allows us to build the complete neural architecture immediately while gracefully handling the reality that most data sources aren't available yet. The system automatically evolves and improves as new data arrives, without requiring any code changes or manual intervention.