# Neural Trading Platform - ML/AI Architecture MVP

## Executive Summary

This document defines a minimal viable ML/AI architecture designed to prove the core neural network integration works while maintaining a path to the full V2 design. The MVP focuses on a single model pipeline that demonstrates real neural network prediction capabilities with ruv-FANN.

## MVP Design Principles

1. **Prove Neural Integration**: Demonstrate real ruv-FANN neural networks can predict market data
2. **Single Model Focus**: One well-tuned model rather than ensemble complexity
3. **Essential Features Only**: Core technical indicators with proven predictive value
4. **File-based Simplicity**: Basic model versioning without complex orchestration
5. **Clear Success Metrics**: Measurable validation of neural predictions

## 1. ML Ops Platform MVP

### Core Components

```
ML Pipeline MVP
├── Feature Pipeline (Single)
│   ├── Technical Indicators (SMA, RSI, MACD only)
│   ├── Price Returns (1-day, 5-day, 20-day)
│   └── Volume Ratios (basic volume analysis)
├── Neural Model (ruv-FANN)
│   ├── Single MLP Architecture (20→64→32→1)
│   └── Backpropagation Training
├── Training Pipeline (Simplified)
│   ├── Sliding Window Dataset Creation
│   ├── Sequential Training (no distributed)
│   └── Basic Checkpoint Saving
└── Model Versioning (File-based)
    ├── Semantic Versioning (v1.0.0)
    ├── Metadata JSON Files
    └── Performance Tracking
```

### Feature Engineering MVP

**Input Features (Total: 20)**
```rust
// Technical Indicators (12 features)
- SMA_5, SMA_10, SMA_20, SMA_50          // Simple Moving Averages
- RSI_14                                  // Relative Strength Index
- MACD, MACD_Signal, MACD_Histogram     // MACD components
- BB_Upper, BB_Middle, BB_Lower          // Bollinger Bands
- Volume_SMA_20                          // Volume moving average

// Price Features (5 features)  
- Price_Return_1d                        // 1-day price return
- Price_Return_5d                        // 5-day price return
- Price_Return_20d                       // 20-day price return
- High_Low_Ratio                         // (High-Low)/Close
- Close_SMA_Ratio                        // Close/SMA_20

// Volume Features (3 features)
- Volume_Ratio_5d                        // Volume/Volume_SMA_5
- Volume_Ratio_20d                       // Volume/Volume_SMA_20
- Price_Volume_Correlation               // 5-day correlation
```

### Neural Architecture (ruv-FANN)

**Single MLP Model Configuration**:
```rust
FannModelConfig {
    model_name: "market_predictor_mlp",
    input_size: 20,           // Feature vector size
    hidden_layers: [64, 32],  // Two hidden layers
    output_size: 1,           // Single prediction (next day return)
    
    // Activation Functions
    hidden_activation: "sigmoid",
    output_activation: "linear",
    
    // Training Parameters
    learning_rate: 0.001,
    momentum: 0.9,
    max_epochs: 1000,
    target_error: 0.001,
    
    // Adaptive Training (MVP keeps simple)
    adaptive_learning_rate: false,
    early_stopping_patience: 50,
}
```

**Rationale**: 
- 20 inputs → 64 neurons (3.2x expansion for pattern recognition)
- 64 → 32 neurons (2x compression for abstraction)  
- 32 → 1 output (full compression to prediction)
- Sigmoid activation for non-linearity, linear output for regression

### Training Pipeline MVP

**Data Requirements**:
```
Minimum Training Data: 1000 samples (4-5 trading weeks)
Validation Split: 20% (200 samples)
Input Window: 20 days of features
Prediction Horizon: 1 day (next trading day return)
```

**Training Process**:
1. **Data Preparation**
   - Load OHLCV data from TimescaleDB
   - Calculate technical indicators using existing feature pipeline
   - Create sliding windows (day N features → day N+1 return)
   - Split into train/validation sets

2. **Model Training**
   - Initialize ruv-FANN network with configuration
   - Train using incremental backpropagation
   - Monitor MSE every 10 epochs
   - Apply early stopping if no improvement for 50 epochs

3. **Validation & Checkpointing**
   - Evaluate on validation set every 50 epochs
   - Save checkpoint if validation improves
   - Calculate R² coefficient for model quality
   - Store final model with metadata

## 2. Model Execution MVP

### Inference Pipeline

```
Real-time Prediction Flow
├── Data Input (Latest 20 days)
├── Feature Calculation (20 features)
├── Model Prediction (ruv-FANN forward pass)
├── Confidence Scoring (based on training MSE)
└── Decision Output (buy/sell/hold)
```

**Implementation**:
```rust
pub struct MVPPredictor {
    model: FannModelAdapter,
    feature_pipeline: FeaturePipeline,
    confidence_threshold: f32,
    decision_logic: SimpleDecisionLogic,
}

impl MVPPredictor {
    pub async fn predict(&self, market_data: &[TimeSeriesData]) -> PredictionResult {
        // 1. Extract features from latest 20 days
        let features = self.feature_pipeline.extract_features(market_data);
        
        // 2. Run neural network prediction
        let prediction = self.model.predict(&features).await?;
        
        // 3. Calculate confidence based on training statistics
        let confidence = self.calculate_confidence(prediction.value);
        
        // 4. Make trading decision
        let decision = self.decision_logic.evaluate(prediction.value, confidence);
        
        PredictionResult {
            predicted_return: prediction.value,
            confidence,
            decision,
            timestamp: Utc::now(),
        }
    }
}
```

### Decision Making (Simplified)

**Simple Decision Logic**:
```rust
pub struct SimpleDecisionLogic {
    buy_threshold: f32,    // e.g., +0.02 (2% expected return)
    sell_threshold: f32,   // e.g., -0.02 (-2% expected return)
    min_confidence: f32,   // e.g., 0.6 (60% confidence minimum)
}

pub fn evaluate(&self, prediction: f32, confidence: f32) -> TradingDecision {
    if confidence < self.min_confidence {
        return TradingDecision::Hold;
    }
    
    match prediction {
        p if p > self.buy_threshold => TradingDecision::Buy,
        p if p < self.sell_threshold => TradingDecision::Sell,
        _ => TradingDecision::Hold,
    }
}
```

## 3. Core Capabilities

### Model Training Capability

**Training Data Service** (existing):
```rust
// Located at: src/integration/training_data_service.rs
pub struct TrainingDataService {
    pub fn prepare_training_data(&self, symbol: &str) -> TrainingData<f32>
    pub fn create_sliding_windows(&self, data: &[f32]) -> Vec<(Vec<f32>, f32)>
    pub fn validate_data_quality(&self, data: &TrainingData<f32>) -> ValidationResult
}
```

**Model Factory** (existing):
```rust
// Located at: src/neural/model_factory.rs
pub struct ModelFactory {
    pub fn create_mlp_model(&self, config: &FannModelConfig) -> FannModelAdapter
    pub fn train_model(&self, model: &mut FannModelAdapter, data: &TrainingData<f32>) -> TrainingResult
}
```

### Feature Engineering

**Existing Feature Pipeline** (leverage existing):
```rust
// Located at: src/features/mod.rs
pub struct FeaturePipeline {
    pub fn extract_technical_indicators(&self, data: &[f32]) -> Vec<f32>
    pub fn extract_price_features(&self, data: &[f32]) -> Vec<f32>
    pub fn extract_volume_features(&self, data: &[f32]) -> Vec<f32>
}
```

**MVP Feature Configuration**:
```rust
let mvp_config = FeatureConfig {
    window_size: 20,
    statistical_features: false,    // Skip for MVP
    fourier_features: false,        // Skip for MVP
    wavelet_features: false,        // Skip for MVP
    technical_features: true,       // Essential only
    normalize: true,
    standardize: true,
};
```

### Backtesting Capability

**Simple Backtesting Engine**:
```rust
pub struct MVPBacktester {
    initial_capital: f64,
    transaction_cost: f64,  // e.g., 0.001 (0.1% per trade)
}

pub struct BacktestResult {
    total_return: f64,
    sharpe_ratio: f64,
    max_drawdown: f64,
    win_rate: f64,
    total_trades: usize,
}

impl MVPBacktester {
    pub fn run_backtest(
        &self,
        predictions: &[PredictionResult],
        actual_returns: &[f64],
    ) -> BacktestResult {
        // Simple implementation:
        // 1. Execute trades based on predictions
        // 2. Calculate portfolio value over time
        // 3. Compute performance metrics
    }
}
```

## 4. Minimum Training Data Requirements

### Data Volume
- **Minimum Dataset**: 1000 trading days (~4 years)
- **Training Set**: 800 samples (80%)
- **Validation Set**: 200 samples (20%)
- **Feature Window**: 20 days per sample
- **Total Data Needed**: 1020 days of OHLCV data

### Data Quality Requirements
- **Completeness**: No missing trading days
- **Consistency**: Same data provider (Alpaca preferred)
- **Frequency**: Daily OHLCV bars
- **Symbols**: Start with single liquid symbol (e.g., SPY, AAPL)

### Data Preparation Pipeline
```rust
pub struct DataPreparationService {
    pub fn validate_data_completeness(&self, data: &[TimeSeriesData]) -> bool
    pub fn fill_missing_values(&self, data: &mut [TimeSeriesData])
    pub fn create_feature_target_pairs(&self, data: &[TimeSeriesData]) -> Vec<(Vec<f32>, f32)>
    pub fn split_train_validation(&self, data: Vec<(Vec<f32>, f32)>) -> (TrainingData<f32>, TrainingData<f32>)
}
```

## 5. Success Metrics for MVP Validation

### Primary Success Metrics

1. **Neural Network Training Success**
   - Final training MSE < 0.001
   - Validation R² > 0.05 (better than random)
   - Training converges within 1000 epochs
   - No overfitting (validation loss doesn't diverge)

2. **Prediction Quality**
   - Direction accuracy > 52% (better than random)
   - Mean Absolute Error < 0.02 (2% daily return error)
   - Predictions are statistically significant (p < 0.05)

3. **System Integration**
   - Model loads and predicts in < 500ms
   - Feature pipeline processes 20 days in < 100ms
   - No memory leaks during continuous operation
   - Model persists and reloads correctly

### Backtest Performance Targets

**Minimum Viable Performance**:
```
Sharpe Ratio: > 0.3 (basic risk-adjusted return)
Max Drawdown: < 15% (reasonable risk management)
Win Rate: > 50% (better than random)
Annual Return: > 5% (beats risk-free rate)
```

**Comparison Baseline**:
- Buy-and-hold strategy on same symbol
- Simple moving average crossover strategy
- Random trading simulation

### Technical Performance Metrics

```rust
pub struct PerformanceMetrics {
    // Model Quality
    pub training_mse: f64,
    pub validation_r_squared: f64,
    pub direction_accuracy: f64,
    
    // System Performance
    pub prediction_latency_ms: u64,
    pub feature_extraction_time_ms: u64,
    pub model_size_mb: f64,
    
    // Trading Performance
    pub backtest_sharpe_ratio: f64,
    pub backtest_max_drawdown: f64,
    pub backtest_win_rate: f64,
}
```

## Implementation Priority

### Phase 1: Core Infrastructure (Week 1)
1. Set up FannModelAdapter with MVP configuration
2. Implement simplified feature pipeline (20 features)
3. Create training data preparation service
4. Build basic model persistence (file-based)

### Phase 2: Training Pipeline (Week 2) 
1. Implement sliding window dataset creation
2. Add neural network training with early stopping
3. Build validation and performance tracking
4. Create model checkpointing system

### Phase 3: Prediction Pipeline (Week 3)
1. Implement real-time feature extraction
2. Add model inference and confidence scoring
3. Build simple decision logic
4. Integrate with existing market data feed

### Phase 4: Validation (Week 4)
1. Run comprehensive backtests
2. Validate against performance targets
3. Document results and lessons learned
4. Plan next phase improvements

## File Organization

```
src/
├── neural/
│   ├── mvp_predictor.rs          # Main MVP predictor
│   └── fann_model_adapter.rs     # Already exists
├── features/
│   ├── mvp_features.rs           # Simplified feature set
│   └── mod.rs                    # Already exists  
├── integration/
│   ├── mvp_training_service.rs   # MVP training pipeline
│   └── training_data_service.rs  # Already exists
├── backtesting/
│   └── mvp_backtester.rs         # Simple backtesting
└── bin/
    └── mvp_trainer.rs            # Training executable
```

## Risk Mitigation

### Technical Risks
- **Model Convergence**: Use proven MLP architecture with conservative learning rate
- **Overfitting**: Mandatory validation split and early stopping
- **Data Quality**: Validate data completeness before training
- **Integration**: Leverage existing adapters and infrastructure

### Performance Risks
- **Prediction Quality**: Set realistic performance targets (> random)
- **System Latency**: Profile and optimize critical path
- **Memory Usage**: Monitor model size and feature extraction cost
- **Training Time**: Limit to 1000 epochs maximum

### Success Definition
The MVP is successful if:
1. Neural network trains successfully and makes predictions
2. Predictions are statistically better than random
3. System integrates cleanly with existing infrastructure
4. Performance metrics meet minimum viable thresholds

This MVP provides a solid foundation for validating the neural network integration while maintaining a clear path to scale toward the full V2 architecture.