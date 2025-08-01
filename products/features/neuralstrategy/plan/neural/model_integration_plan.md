# Neural Architecture Integration Plan
## 27+ ruv-FANN Model Integration for Neural Trader

### Executive Summary

This document outlines the comprehensive integration plan for incorporating 27+ advanced neural forecasting models from the ruv-FANN `neuro-divergent` library into the Neural Trader system. The current factory implementation uses mocked MLP networks, but we now have access to real, production-ready implementations of state-of-the-art time series forecasting models.

**Key Findings:**
- **27+ Real Models Available**: Comprehensive collection of neural forecasting models in `vendor/ruv-fann/neuro-divergent`
- **BaseModel Trait**: Well-defined interface in `neuro_divergent_models::core::BaseModel`
- **Factory Refactoring Required**: Current implementation creates approximations instead of using real models
- **Multi-Modal Support**: Models support 6 data modalities for comprehensive market analysis

---

## 1. Available Neural Model Catalog

### 1.1 Basic Models (Foundation Layer)
| Model | Location | Characteristics | Use Case |
|-------|----------|-----------------|----------|
| **MLP** | `basic/mlp.rs` | Multi-layer perceptron, standard feedforward | Baseline predictions, feature extraction |
| **DLinear** | `basic/dlinear.rs` | Decomposition + Linear, trend/seasonal split | Simple trend analysis |
| **NLinear** | `basic/nlinear.rs` | Normalized linear model | Stationary time series |

### 1.2 Recurrent Models (Sequential Processing)
| Model | Location | Characteristics | Use Case |
|-------|----------|-----------------|----------|
| **RNN** | `recurrent/rnn.rs` | Basic recurrent network | Short-term dependencies |
| **LSTM** | `recurrent/` (alias to RNN currently) | Long Short-Term Memory | Long-term dependencies |
| **GRU** | `recurrent/` (alias to RNN currently) | Gated Recurrent Unit | Balanced memory/performance |

### 1.3 Specialized Models (Domain-Specific)
| Model | Location | Characteristics | Use Case |
|-------|----------|-----------------|----------|
| **DeepAR** | `specialized/deepar.rs` | Probabilistic forecasting, uncertainty | Risk quantification |
| **TCN** | `specialized/tcn.rs` | Temporal Convolutional Network | Pattern detection |
| **BiTCN** | `specialized/bitcn.rs` | Bidirectional TCN | Past-future correlation |
| **TimesNet** | `specialized/timesnet.rs` | Multi-scale temporal analysis | Complex patterns |
| **StemGNN** | `specialized/stemgnn.rs` | Graph neural for multivariate | Cross-asset relationships |
| **TSMixer** | `specialized/tsmixer.rs` | Time series mixing architecture | Feature fusion |
| **TSMixerx** | `specialized/tsmixerx.rs` | Extended TSMixer | Enhanced mixing |
| **TimeLLM** | `specialized/timellm.rs` | Language model for time series | Pattern recognition |

### 1.4 Transformer Models (Attention-Based)
| Model | Location | Characteristics | Use Case |
|-------|----------|-----------------|----------|
| **TFT** | `transformer/tft.rs` | Temporal Fusion Transformer | Multi-horizon, interpretable |
| **Informer** | `transformer/informer.rs` | Efficient long sequence attention | Long-term forecasting |
| **Autoformer** | `transformer/autoformer.rs` | Auto-correlation mechanism | Seasonal patterns |
| **FEDformer** | `transformer/fedformer.rs` | Frequency Enhanced Decomposition | Frequency domain analysis |
| **PatchTST** | `transformer/patchtst.rs` | Patch-based transformer | Efficient attention |
| **iTransformer** | `transformer/itransformer.rs` | Inverted transformer | Variable relationships |

### 1.5 Advanced Models (State-of-the-Art)
| Model | Location | Characteristics | Use Case |
|-------|----------|-----------------|----------|
| **NBEATS** | `advanced/nbeats.rs` | Neural basis expansion, interpretable | Decomposable forecasting |
| **NBEATSx** | `advanced/nbeatsx.rs` | Extended NBEATS | Enhanced interpretability |
| **NHITS** | `advanced/nhits.rs` | Hierarchical interpolation | Multi-resolution analysis |

---

## 2. Time Horizon Mapping

### 2.1 Short-Term (1-24 hours)
**Optimal Models:**
- **TCN/BiTCN**: Excellent for pattern detection in high-frequency data
- **TSMixer**: Real-time feature fusion capabilities
- **MLP**: Fast inference for immediate predictions
- **GRU**: Balanced memory usage for quick responses

**Configuration:**
```rust
TimeHorizonConfig::ShortTerm {
    primary_models: vec!["TCN", "TSMixer", "GRU"],
    ensemble_strategy: EnsembleStrategy::FastVoting,
    update_frequency: Duration::minutes(15),
    lookback_window: 24 * 4, // 4 days of hourly data
}
```

### 2.2 Medium-Term (1-7 days)
**Optimal Models:**
- **NBEATS**: Interpretable decomposition for daily patterns
- **TFT**: Multi-horizon forecasting with attention
- **DeepAR**: Probabilistic with uncertainty bounds
- **LSTM**: Long-term memory for weekly patterns

**Configuration:**
```rust
TimeHorizonConfig::MediumTerm {
    primary_models: vec!["NBEATS", "TFT", "DeepAR", "LSTM"],
    ensemble_strategy: EnsembleStrategy::WeightedAverage,
    update_frequency: Duration::hours(4),
    lookback_window: 30 * 24, // 30 days of hourly data
}
```

### 2.3 Long-Term (1-4 weeks)
**Optimal Models:**
- **Informer**: Efficient long-sequence processing
- **NHITS**: Multi-resolution hierarchical analysis
- **Autoformer**: Seasonal pattern recognition
- **TimesNet**: Multi-scale temporal relationships

**Configuration:**
```rust
TimeHorizonConfig::LongTerm {
    primary_models: vec!["Informer", "NHITS", "Autoformer", "TimesNet"],
    ensemble_strategy: EnsembleStrategy::StackedEnsemble,
    update_frequency: Duration::hours(12),
    lookback_window: 90 * 24, // 90 days of hourly data
}
```

---

## 3. Market Regime Mapping

### 3.1 Trending Markets (Strong Directional Movement)
**Specialized Models:**
- **DLinear**: Captures linear trends effectively
- **NBEATS (Trend Stack)**: Interpretable trend decomposition
- **LSTM**: Long-term directional patterns
- **Informer**: Long-sequence trend analysis

**Regime Detection:**
```rust
TrendingRegimeConfig {
    detection_threshold: 0.7, // ADX > 70
    models: vec!["DLinear", "NBEATS", "LSTM", "Informer"],
    weight_distribution: HashMap::from([
        ("DLinear", 0.3),
        ("NBEATS", 0.3),
        ("LSTM", 0.2),
        ("Informer", 0.2),
    ]),
}
```

### 3.2 Ranging Markets (Sideways Movement)
**Specialized Models:**
- **NBEATS (Seasonality Stack)**: Pattern recognition in cycles
- **TCN**: Pattern detection in constrained ranges
- **Autoformer**: Seasonal/cyclical pattern recognition
- **TSMixer**: Feature mixing for range-bound behavior

**Regime Detection:**
```rust
RangingRegimeConfig {
    detection_threshold: 0.3, // ADX < 30
    models: vec!["NBEATS", "TCN", "Autoformer", "TSMixer"],
    weight_distribution: HashMap::from([
        ("NBEATS", 0.35),
        ("TCN", 0.25),
        ("Autoformer", 0.25),
        ("TSMixer", 0.15),
    ]),
}
```

### 3.3 Volatile Markets (High Uncertainty)
**Specialized Models:**
- **DeepAR**: Probabilistic forecasting with uncertainty quantification
- **BiTCN**: Bidirectional pattern analysis
- **TimesNet**: Multi-scale volatility patterns
- **TFT**: Interpretable uncertainty estimates

**Regime Detection:**
```rust
VolatileRegimeConfig {
    detection_threshold: 0.02, // VIX > 25 or rolling volatility > 2%
    models: vec!["DeepAR", "BiTCN", "TimesNet", "TFT"],
    weight_distribution: HashMap::from([
        ("DeepAR", 0.4),  // Primary for uncertainty
        ("BiTCN", 0.2),
        ("TimesNet", 0.2),
        ("TFT", 0.2),
    ]),
}
```

---

## 4. Multi-Modal Data Fusion Pipeline

### 4.1 Six Data Modalities

1. **Price Data**: OHLCV, spreads, depth
2. **Technical Indicators**: RSI, MACD, Bollinger Bands, etc.
3. **Volume Profile**: Volume distribution, order flow
4. **Market Microstructure**: Bid-ask spreads, tick data
5. **Sentiment Data**: News sentiment, social media, VIX
6. **Macroeconomic**: Interest rates, economic indicators

### 4.2 Fusion Architecture

```rust
#[derive(Debug, Clone)]
pub struct MultiModalFusionConfig {
    pub modalities: Vec<ModalityConfig>,
    pub fusion_strategy: FusionStrategy,
    pub preprocessing: PreprocessingConfig,
    pub feature_selection: FeatureSelectionConfig,
}

#[derive(Debug, Clone)]
pub enum FusionStrategy {
    EarlyFusion,    // Concatenate all features
    LateFusion,     // Separate models, combine outputs
    HybridFusion,   // Mix of early and late fusion
    AttentionFusion, // Attention-weighted fusion
}

impl MultiModalFusionConfig {
    pub fn new() -> Self {
        Self {
            modalities: vec![
                ModalityConfig {
                    name: "price".to_string(),
                    features: vec!["open", "high", "low", "close", "volume"],
                    preprocessing: vec![Normalization::ZScore, Scaling::MinMax],
                    weight: 0.3,
                },
                ModalityConfig {
                    name: "technical".to_string(),
                    features: vec!["rsi", "macd", "bb_upper", "bb_lower"],
                    preprocessing: vec![Normalization::RobustScaler],
                    weight: 0.2,
                },
                ModalityConfig {
                    name: "volume_profile".to_string(),
                    features: vec!["poc", "value_area_high", "value_area_low"],
                    preprocessing: vec![Normalization::ZScore],
                    weight: 0.15,
                },
                ModalityConfig {
                    name: "microstructure".to_string(),
                    features: vec!["bid_ask_spread", "order_imbalance"],
                    preprocessing: vec![Normalization::RobustScaler],
                    weight: 0.15,
                },
                ModalityConfig {
                    name: "sentiment".to_string(),
                    features: vec!["news_sentiment", "social_sentiment", "vix"],
                    preprocessing: vec![Normalization::ZScore],
                    weight: 0.1,
                },
                ModalityConfig {
                    name: "macro".to_string(),
                    features: vec!["interest_rates", "economic_indicators"],
                    preprocessing: vec![Normalization::StandardScaler],
                    weight: 0.1,
                },
            ],
            fusion_strategy: FusionStrategy::HybridFusion,
            preprocessing: PreprocessingConfig::default(),
            feature_selection: FeatureSelectionConfig::default(),
        }
    }
}
```

### 4.3 Model-Specific Fusion

Different models excel with different fusion strategies:

- **StemGNN**: Uses graph neural networks for cross-modal relationships
- **TFT**: Attention mechanism naturally handles multi-modal inputs
- **TSMixer**: Designed for feature mixing across modalities
- **TimeLLM**: Can process multi-modal data as sequences

---

## 5. Cluster Specialization Strategy

### 5.1 Performance Cluster
**Focus**: High-frequency trading, low-latency predictions
**Models**: TCN, TSMixer, MLP, GRU
**Optimization**: Fast inference, minimal memory footprint

### 5.2 Research Cluster  
**Focus**: Interpretable analysis, strategy development
**Models**: NBEATS, TFT, DeepAR, DLinear
**Optimization**: Interpretability, uncertainty quantification

### 5.3 Production Cluster
**Focus**: Robust real-world trading
**Models**: Ensemble of LSTM, Informer, NHITS, Autoformer
**Optimization**: Reliability, fault tolerance, consistent performance

### 5.4 Experimental Cluster
**Focus**: Testing new models and strategies
**Models**: TimeLLM, iTransformer, PatchTST, NBEATSx
**Optimization**: Flexibility, rapid prototyping

---

## 6. Factory Refactoring Plan

### 6.1 Current State Analysis
The existing factory in `src/neural/fann/networks/factory.rs` creates approximations:
- LSTM → Enhanced MLP layers (1.33x size)
- GRU → Enhanced MLP layers (1.25x size)  
- DeepAR → Double output for mean/variance
- TCN → Hierarchical layers simulating dilations
- NHITS → Multi-resolution layer structure
- Transformer → Parallel processing layers

### 6.2 Refactoring Strategy

#### Phase 1: BaseModel Integration
```rust
use neuro_divergent_models::{
    BaseModel, ModelConfig, TimeSeriesData, ForecastResult,
    models::{LSTM, GRU, DeepAR, TCN, NHITS, NBEATS, TFT}
};

pub struct EnhancedNetworkFactory {
    /// Real model instances
    model_registry: HashMap<String, Box<dyn BaseModel<f32>>>,
    /// Model configurations  
    config_registry: HashMap<String, Box<dyn ModelConfig<f32>>>,
    /// Factory settings
    settings: FactorySettings,
}

impl EnhancedNetworkFactory {
    pub async fn create_real_model(&self, 
        model_name: &str, 
        config: &FannModelConfig
    ) -> Result<Box<dyn BaseModel<f32>>, NetworkError> {
        match model_name.to_uppercase().as_str() {
            "LSTM" => {
                let lstm_config = LSTMConfig::new()
                    .with_horizon(config.horizon)
                    .with_input_size(config.input_size)
                    .with_hidden_layers(config.hidden_layers.clone())
                    .with_learning_rate(config.learning_rate);
                
                let lstm = LSTM::new(lstm_config)?;
                Ok(Box::new(lstm))
            },
            "NBEATS" => {
                let nbeats_config = NBEATSConfig::interpretable(
                    config.horizon, 
                    config.input_size
                );
                let nbeats = NBEATS::new(nbeats_config)?;
                Ok(Box::new(nbeats))
            },
            "DEEPAR" => {
                let deepar_config = DeepARConfig::new()
                    .with_horizon(config.horizon)
                    .with_probabilistic_output(true);
                let deepar = DeepAR::new(deepar_config)?;
                Ok(Box::new(deepar))
            },
            // ... other models
            _ => Err(NetworkError::UnsupportedModel(model_name.to_string()))
        }
    }
}
```

#### Phase 2: Model Adapter Layer
```rust
pub struct ModelAdapter<T: Float> {
    inner_model: Box<dyn BaseModel<T>>,
    preprocessing: PreprocessingPipeline<T>,
    postprocessing: PostprocessingPipeline<T>,
}

impl<T: Float> ModelAdapter<T> {
    pub fn new(model: Box<dyn BaseModel<T>>) -> Self {
        Self {
            inner_model: model,
            preprocessing: PreprocessingPipeline::default(),
            postprocessing: PostprocessingPipeline::default(),
        }
    }
    
    pub fn fit(&mut self, data: &[T]) -> Result<(), ModelError> {
        let processed_data = self.preprocessing.transform(data)?;
        let ts_data = TimeSeriesData::new(processed_data);
        self.inner_model.fit(&ts_data)
    }
    
    pub fn predict(&self, data: &[T]) -> Result<Vec<T>, ModelError> {
        let processed_data = self.preprocessing.transform(data)?;
        let ts_data = TimeSeriesData::new(processed_data);
        let result = self.inner_model.predict(&ts_data)?;
        self.postprocessing.transform(&result.forecasts)
    }
}
```

#### Phase 3: Legacy Compatibility
```rust
pub struct NetworkCompatibilityLayer<T: Float> {
    model_adapter: ModelAdapter<T>,
}

impl<T: Float> NetworkCompatibilityLayer<T> {
    // Maintain existing Network<T> interface
    pub fn run(&mut self, input: &[T]) -> Vec<T> {
        self.model_adapter.predict(input).unwrap_or_default()
    }
    
    // Maintain existing training interface
    pub fn train(&mut self, data: &TrainingData<T>) -> Result<(), NetworkError> {
        let flat_data: Vec<T> = data.inputs.into_iter().flatten().collect();
        self.model_adapter.fit(&flat_data)
            .map_err(|e| NetworkError::TrainingError(e.to_string()))
    }
}
```

---

## 7. Performance Analysis

### 7.1 Model Complexity Comparison

| Model | Parameters | Training Time | Inference Speed | Memory Usage |
|-------|------------|---------------|-----------------|--------------|
| MLP | Low | Fast | Very Fast | Low |
| LSTM | Medium | Medium | Medium | Medium |
| NBEATS | High | Slow | Fast | Medium |
| TFT | Very High | Very Slow | Medium | High |
| Informer | High | Slow | Fast | High |
| DeepAR | Medium | Medium | Medium | Medium |

### 7.2 Accuracy vs Performance Trade-offs

**High Accuracy, High Cost:**
- TFT, Informer, NHITS, TimesNet

**Balanced Accuracy/Performance:**
- NBEATS, LSTM, TCN, DeepAR

**Fast Inference, Lower Accuracy:**
- MLP, DLinear, NLinear, GRU

### 7.3 Memory Requirements

```rust
#[derive(Debug, Clone)]
pub struct ModelResourceRequirements {
    pub model_name: String,
    pub estimated_parameters: usize,
    pub memory_mb: f64,
    pub training_time_minutes: f64,
    pub inference_ms: f64,
}

impl ModelResourceRequirements {
    pub fn calculate_ensemble_requirements(models: &[String]) -> EnsembleRequirements {
        let total_memory: f64 = models.iter()
            .map(|m| Self::get_model_memory(m))
            .sum();
        
        let max_inference_time: f64 = models.iter()
            .map(|m| Self::get_model_inference_time(m))
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);
            
        EnsembleRequirements {
            total_memory_mb: total_memory,
            parallel_inference_ms: max_inference_time,
            sequential_inference_ms: models.iter()
                .map(|m| Self::get_model_inference_time(m))
                .sum(),
        }
    }
}
```

---

## 8. Implementation Roadmap

### Phase 1: Foundation (Weeks 1-2)
- [ ] Integrate `neuro_divergent_models` dependency
- [ ] Create BaseModel adapter layer
- [ ] Implement basic model factory refactoring
- [ ] Add configuration management for real models

### Phase 2: Core Models (Weeks 3-4)
- [ ] Integrate LSTM, GRU, MLP models
- [ ] Add NBEATS and DeepAR for interpretability
- [ ] Implement TCN for pattern detection
- [ ] Create model validation and testing framework

### Phase 3: Advanced Models (Weeks 5-6)  
- [ ] Integrate Transformer models (TFT, Informer)
- [ ] Add NHITS for multi-resolution analysis
- [ ] Implement specialized models (TimesNet, TSMixer)
- [ ] Create ensemble coordination system

### Phase 4: Multi-Modal Fusion (Weeks 7-8)
- [ ] Design multi-modal data pipeline
- [ ] Implement fusion strategies
- [ ] Add preprocessing and feature selection
- [ ] Create modality-specific optimizations

### Phase 5: Production Integration (Weeks 9-10)
- [ ] Implement time horizon routing
- [ ] Add market regime detection
- [ ] Create cluster specialization
- [ ] Performance optimization and monitoring

### Phase 6: Testing & Validation (Weeks 11-12)
- [ ] Comprehensive backtesting framework
- [ ] Model performance comparison
- [ ] Production deployment testing
- [ ] Documentation and training materials

---

## 9. Risk Mitigation

### 9.1 Technical Risks
- **Dependency Management**: Pin specific versions of ruv-fann
- **API Compatibility**: Maintain backward compatibility layers
- **Performance Regression**: Comprehensive benchmarking before deployment
- **Memory Usage**: Implement model loading/unloading strategies

### 9.2 Operational Risks
- **Training Time**: Implement incremental training strategies
- **Model Degradation**: Automated retraining pipelines
- **Data Quality**: Robust preprocessing and validation
- **Monitoring**: Real-time model performance tracking

---

## 10. Success Metrics

### 10.1 Technical Metrics
- **Prediction Accuracy**: MAPE, RMSE, directional accuracy
- **Inference Speed**: Sub-100ms for real-time models
- **Memory Efficiency**: <8GB RAM for full ensemble
- **Model Interpretability**: Decomposition quality for NBEATS/TFT

### 10.2 Business Metrics
- **Sharpe Ratio Improvement**: Target +0.5 improvement
- **Maximum Drawdown Reduction**: Target -20% improvement
- **Win Rate**: Target 55%+ directional accuracy
- **Risk-Adjusted Returns**: Consistent outperformance of baseline

---

This integration plan provides a comprehensive roadmap for transitioning from mock MLP approximations to real, state-of-the-art neural forecasting models. The phased approach ensures stability while enabling the full power of modern time series forecasting architectures.