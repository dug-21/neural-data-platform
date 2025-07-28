# Neural Adapter Module

This module provides the infrastructure for integrating neuro-divergent neural models into the neural-trader system.

## Architecture

The module consists of three main components:

### 1. NeuroDivergentAdapter
- Implements the `DataAdapter` trait
- Manages model lifecycle (initialization, connection, disconnection)
- Handles prediction requests
- Configurable for different model types (TimeMixer, NeuralForecast, TimesFM, etc.)

### 2. DataConverter
- Handles format conversion between neural-trader and various model formats
- **Important**: Does NOT perform data normalization (handled upstream in event pipeline)
- Supports multiple output formats:
  - DataFrame (Polars)
  - NdArray (2D)
  - Tensor (3D)
  - DictArray

### 3. NeuralAdapterError
- Specialized error types for neural operations
- Converts to standard `AdapterError` for compatibility

## Usage

```rust
use neural_trader::adapters::neural::{NeuroDivergentAdapter, NeuralModelConfig};
use neural_trader::adapters::DataAdapter;

// Configure the model
let config = NeuralModelConfig {
    model_type: "TimeMixer".to_string(),
    lookback_window: 48,
    forecast_horizon: 12,
    batch_size: 64,
    use_gpu: false,
    model_params: serde_json::json!({}),
};

// Create and connect adapter
let mut adapter = NeuroDivergentAdapter::new(config);
adapter.connect().await?;

// Make predictions
let predictions = adapter.predict(&time_series_data).await?;

// Disconnect when done
adapter.disconnect().await?;
```

## Data Flow

1. **Input**: `Vec<TimeSeriesData>` (already normalized by event pipeline)
2. **Conversion**: DataConverter transforms to model-specific format
3. **Prediction**: Model processes data and returns predictions
4. **Output**: Predictions converted back to `TimeSeriesData` format

## Configuration

The `NeuralModelConfig` struct supports:
- Model type selection
- Lookback window size
- Forecast horizon
- Batch size
- GPU acceleration toggle
- Model-specific parameters via JSON

## Testing

The module includes comprehensive tests:
- Unit tests for data conversion
- Integration tests for adapter lifecycle
- Format conversion tests for all supported formats

Run tests with:
```bash
cargo test --package neural-trader --lib adapters::neural
```

## Future Enhancements

- [ ] Actual model integration (currently uses placeholders)
- [ ] Support for ensemble models
- [ ] Model versioning and checkpointing
- [ ] Performance metrics collection
- [ ] A/B testing infrastructure