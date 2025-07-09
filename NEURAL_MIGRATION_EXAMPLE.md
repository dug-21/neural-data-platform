# Neural Migration Code Examples

## Complete Replacement for src/neural/mod.rs

```rust
//! Neural Network Integration Module using Neuro-Divergent
//! 
//! Provides neural network prediction capabilities via ruv-FANN's neuro-divergent library

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use async_trait::async_trait;

use crate::config::NeuralConfig;
use crate::data::TimeSeriesData;

// Import from vendored neuro-divergent library
use neuro_divergent_models::{
    NeuralForecast,
    foundation::{
        BaseModel, TimeSeriesInput, ForecastOutput, TimeSeriesDataset, 
        TimeSeriesSample, PredictionInterval
    },
};

// Import specific model implementations
use neuro_divergent_models::models::{LSTM, RNN, GRU};
use neuro_divergent_models::config::{LSTMConfig, RNNConfig, GRUConfig};
use neuro_divergent_models::advanced::nhits::{NHITS, NHITSConfig};
use neuro_divergent_models::specialized::{
    tcn::{TCN, TCNConfig},
    deepar::{DeepAR, DeepARConfig},
};
use neuro_divergent_models::basic::mlp::{MLP, MLPConfig};

// Backward compatibility wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionResult {
    pub timestamp: DateTime<Utc>,
    pub value: f64,
    pub confidence: f64,
    pub interval_low: f64,
    pub interval_high: f64,
    pub model_name: String,
}

impl PredictionResult {
    fn from_forecast_output(
        output: &ForecastOutput<f64>, 
        base_timestamp: DateTime<Utc>,
        model_name: &str
    ) -> Vec<Self> {
        let mut results = Vec::new();
        
        for (i, &value) in output.forecasts.iter().enumerate() {
            let timestamp = base_timestamp + chrono::Duration::minutes((i + 1) as i64);
            
            let (confidence, interval_low, interval_high) = if let Some(intervals) = &output.prediction_intervals {
                if let Some(interval) = intervals.get(i) {
                    (
                        interval.confidence_level as f64,
                        interval.lower_bound,
                        interval.upper_bound
                    )
                } else {
                    (0.8, value * 0.95, value * 1.05)
                }
            } else {
                (0.8, value * 0.95, value * 1.05)
            };
            
            results.push(PredictionResult {
                timestamp,
                value,
                confidence,
                interval_low,
                interval_high,
                model_name: model_name.to_string(),
            });
        }
        
        results
    }
}

pub struct NeuralPredictor {
    config: NeuralConfig,
    forecast_engine: Option<NeuralForecast<f64>>,
}

impl NeuralPredictor {
    pub fn new(config: NeuralConfig) -> Result<Self> {
        let mut forecast_engine = NeuralForecast::new();
        
        // Initialize models based on configuration
        for model_name in &config.models {
            let model: Box<dyn BaseModel<f64>> = match model_name.as_str() {
                "NHITS" => {
                    let config = NHITSConfig::default()
                        .with_horizon(24)
                        .with_input_size(48);
                    Box::new(NHITS::new(config)?)
                },
                "TCN" => {
                    let config = TCNConfig::default()
                        .with_horizon(24)
                        .with_input_size(48);
                    Box::new(TCN::new(config)?)
                },
                "DeepAR" => {
                    let config = DeepARConfig::default()
                        .with_horizon(24)
                        .with_input_size(48);
                    Box::new(DeepAR::new(config)?)
                },
                "MLP" => {
                    let config = MLPConfig::new(48, 24)
                        .with_hidden_layers(vec![64, 32])
                        .with_max_epochs(100);
                    Box::new(MLP::new(config)?)
                },
                "LSTM" => {
                    let config = LSTMConfig::default_with_horizon(24)
                        .with_architecture(128, 2, 0.1)
                        .with_training(1000, 0.001);
                    Box::new(LSTM::new(config)?)
                },
                _ => continue,
            };
            
            forecast_engine = forecast_engine.with_model(model);
        }
        
        let forecast_engine = forecast_engine.build()?;
        
        Ok(Self { 
            config, 
            forecast_engine: Some(forecast_engine) 
        })
    }
    
    pub async fn load_historical_data(&mut self, data: Vec<TimeSeriesData>) -> Result<()> {
        // Convert TimeSeriesData to TimeSeriesDataset
        let values: Vec<f64> = data.iter().map(|d| d.close).collect();
        
        // Create training dataset
        let mut samples = Vec::new();
        let window_size = 48; // Input window
        let horizon = 24; // Forecast horizon
        
        for i in 0..values.len().saturating_sub(window_size + horizon) {
            let input = TimeSeriesInput::new(
                values[i..i + window_size].to_vec()
            );
            let target = values[i + window_size..i + window_size + horizon].to_vec();
            
            samples.push(TimeSeriesSample {
                input,
                target,
                weight: None,
            });
        }
        
        let dataset = TimeSeriesDataset {
            samples,
            metadata: Default::default(),
        };
        
        // Train the models
        if let Some(ref mut engine) = self.forecast_engine {
            engine.fit(dataset)?;
        }
        
        Ok(())
    }
    
    pub async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        // Extract values from TimeSeriesData
        let values: Vec<f64> = data.iter().map(|d| d.close).collect();
        
        // Create input for prediction
        let input = TimeSeriesInput::new(values)
            .with_timestamp(data.last().map(|d| d.timestamp).unwrap_or_else(Utc::now));
        
        // Get forecast from the engine
        if let Some(ref engine) = self.forecast_engine {
            let forecast = engine.predict(&input)?;
            
            // Use the first model's name from config
            let model_name = self.config.models.first()
                .map(|s| s.as_str())
                .unwrap_or("ensemble");
            
            Ok(PredictionResult::from_forecast_output(
                &forecast,
                input.last_timestamp.unwrap_or_else(Utc::now),
                model_name
            ))
        } else {
            Err(anyhow::anyhow!("Forecast engine not initialized"))
        }
    }
    
    pub async fn predict_ensemble(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        models: &[String],
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        // For now, use the standard predict method
        // TODO: Implement proper ensemble predictions using multiple models
        self.predict(data, horizon, features).await
    }
    
    pub async fn get_feature_importance(&self) -> Result<HashMap<String, f64>> {
        // Return placeholder values for now
        // TODO: Implement actual feature importance extraction from models
        Ok(HashMap::from([
            ("price".to_string(), 0.4),
            ("volume".to_string(), 0.3),
            ("rsi".to_string(), 0.2),
            ("macd".to_string(), 0.1),
        ]))
    }
}

// Default implementation remains the same
impl Default for NeuralPredictor {
    fn default() -> Self {
        let config = NeuralConfig {
            memory_gb: 1.0,
            models: vec!["MLP".to_string()],
            prediction_cache_ttl: 300,
            model_load_timeout: 60,
            max_concurrent_predictions: 10,
            enable_model_monitoring: true,
            accuracy_threshold: 0.8,
        };
        Self::new(config).unwrap()
    }
}
```

## Required Cargo.toml Changes

```toml
[dependencies]
# ... existing dependencies ...

# Add neuro-divergent models from vendor
neuro-divergent-models = { path = "vendor/ruv-fann/neuro-divergent/neuro-divergent-models" }

# Remove any direct neural network dependencies that are replaced
# Remove: candle, torch-sys, etc.
```

## Update for src/strategies/neural_enhanced.rs

```rust
// Change imports at the top
use crate::neural::{NeuralPredictor, PredictionResult};

// No other changes needed - the API remains compatible!
```

## Key Differences to Note

1. **Model Configuration**: Each model now has its own strongly-typed config struct
2. **Training**: Models are trained through the NeuralForecast interface
3. **Predictions**: ForecastOutput provides richer information including intervals
4. **Flexibility**: Easy to add any of the 27+ available models
5. **Performance**: Leverages optimized ruv-FANN backend

## Migration Checklist

- [ ] Update Cargo.toml with neuro-divergent-models dependency
- [ ] Replace entire src/neural/mod.rs with new implementation
- [ ] Run `cargo check` to ensure compilation
- [ ] Run existing tests to verify compatibility
- [ ] Test each model type individually
- [ ] Benchmark performance vs old implementation
- [ ] Update documentation