# Phase 1 Architecture: Vendor Model Foundation

## System Overview

Phase 1 architecture establishes the vendor model foundation by replacing FANN with direct BaseModel<f32> integration while preserving all DAA autonomous capabilities. The design prioritizes clean separation of concerns and maintainable interfaces.

## 1. High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Market Data Ingestion                     │
│                    (Redis Streams - Unchanged)                   │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Enhanced Neural Adapter                        │
│                    (Interface Preserved)                         │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                     VendorPredictor                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │   ModelFactory  │  │   SectorMapper  │  │ DataConverter   │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│              Vendor BaseModel<f32> Layer                        │
│  [LSTM] [GRU] [TCN] [TFT] [DeepAR] [NBEATS] [MLP] [DLinear]    │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│                Performance Tracking System                       │
│  ┌─────────────────┐              ┌─────────────────┐           │
│  │PerformanceTracker│─────────────▶│  DAA Integration│           │
│  └─────────────────┘              └─────────────────┘           │
└─────────────────┬───────────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────────┐
│              DAA Autonomous Training System                      │
│                    (Fully Preserved)                            │
└─────────────────────────────────────────────────────────────────┘
```

## 2. Core Components

### 2.1 VendorPredictor (New Primary Component)

**Responsibility**: Central orchestrator for vendor model predictions and sector-based processing.

```rust
pub struct VendorPredictor {
    /// Active vendor models keyed by ModelKey(sector, model_type, variant)
    models: Arc<DashMap<ModelKey, Box<dyn BaseModel<f32>>>>,
    
    /// Lazy models waiting for data availability
    lazy_models: Arc<DashMap<String, ModelConfig>>,
    
    /// Sector mapping and aggregation
    sector_mapper: Arc<SectorMapper>,
    
    /// Data conversion utilities
    data_converter: Arc<DataConverter>,
    
    /// Performance tracking integration
    performance_tracker: Arc<ModelPerformanceTracker>,
    
    /// DAA integration for autonomous training decisions
    daa_integration: Arc<DAAPerformanceIntegration>,
    
    /// Configuration and runtime settings
    config: VendorPredictorConfig,
}

impl VendorPredictor {
    /// Main prediction entry point
    pub async fn predict(
        &self,
        symbol: &str,
        market_data: &MarketData
    ) -> Result<PredictionResult> {
        // 1. Get sector for symbol
        let sector_info = self.sector_mapper.get_sector(symbol)?;
        
        // 2. Convert to vendor TimeSeriesData format
        let ts_data = self.data_converter.to_vendor_format(market_data)?;
        
        // 3. Get active models for this sector
        let models = self.get_active_models_for_sector(&sector_info.sector_id)?;
        
        // 4. Run parallel predictions
        let predictions = self.run_ensemble_predictions(&models, &ts_data).await?;
        
        // 5. Track performance and notify DAA
        self.track_prediction_performance(symbol, &predictions).await?;
        
        Ok(predictions)
    }
}
```

### 2.2 ModelFactory (Enhanced)

**Responsibility**: Create and configure vendor models from configuration.

```rust
pub struct ModelFactory;

impl ModelFactory {
    /// Create vendor model from configuration
    pub fn create_model(
        architecture: &str,
        config: ModelConfig
    ) -> Result<Box<dyn BaseModel<f32>>> {
        match architecture {
            "LSTM" => Ok(Box::new(LSTM::new(LSTMConfig {
                input_size: config.input_size,
                hidden_size: config.hidden_size,
                num_layers: config.num_layers.unwrap_or(1),
                dropout: config.dropout.unwrap_or(0.0),
                bidirectional: config.bidirectional.unwrap_or(false),
            })?)),
            
            "TFT" => Ok(Box::new(TFT::new(TFTConfig {
                d_model: config.d_model.unwrap_or(128),
                num_heads: config.num_heads.unwrap_or(4),
                num_encoder_layers: config.num_encoder_layers.unwrap_or(6),
                dropout: config.dropout.unwrap_or(0.1),
            })?)),
            
            "DeepAR" => Ok(Box::new(DeepAR::new(DeepARConfig {
                input_size: config.input_size,
                hidden_size: config.hidden_size,
                num_layers: config.num_layers.unwrap_or(2),
                dropout: config.dropout.unwrap_or(0.1),
            })?)),
            
            // ... additional models
            _ => Err(anyhow!("Unsupported model architecture: {}", architecture))
        }
    }
    
    /// Get model capabilities for data requirements
    pub fn get_model_capabilities(architecture: &str) -> ModelCapabilities {
        match architecture {
            "LSTM" | "GRU" | "RNN" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: false,
                min_sequence_length: 10,
                optimal_sequence_length: 100,
            },
            "TFT" => ModelCapabilities {
                requires_sequential_data: true,
                supports_exogenous: true,
                supports_static: true,
                min_sequence_length: 24,
                optimal_sequence_length: 168,
            },
            // ... other models
            _ => ModelCapabilities::default()
        }
    }
}
```

### 2.3 SectorMapper (New Component)

**Responsibility**: Manage symbol-to-sector mapping and sector-level aggregations.

```rust
pub struct SectorMapper {
    /// Static symbol-to-sector mappings
    symbol_sectors: Arc<DashMap<String, SectorInfo>>,
    
    /// Sector ETF representatives
    sector_etfs: Arc<DashMap<SectorId, String>>,
    
    /// Dynamic sector updates
    sector_updates: Arc<RwLock<Vec<SectorUpdate>>>,
    
    /// Configuration
    config: SectorConfig,
}

impl SectorMapper {
    /// Get sector information for symbol
    pub fn get_sector(&self, symbol: &str) -> Result<SectorInfo> {
        self.symbol_sectors
            .get(symbol)
            .map(|entry| entry.clone())
            .ok_or_else(|| anyhow!("Unknown sector for symbol: {}", symbol))
    }
    
    /// Get all symbols in a sector
    pub fn get_symbols_in_sector(&self, sector: &SectorId) -> Vec<String> {
        self.symbol_sectors
            .iter()
            .filter(|entry| &entry.value().sector_id == sector)
            .map(|entry| entry.key().clone())
            .collect()
    }
    
    /// Calculate sector-level aggregated features
    pub async fn get_sector_features(
        &self,
        sector: &SectorId,
        market_data: &HashMap<String, MarketData>
    ) -> Result<SectorFeatures> {
        let symbols = self.get_symbols_in_sector(sector);
        let mut sector_data = Vec::new();
        
        for symbol in symbols {
            if let Some(data) = market_data.get(&symbol) {
                if let Some(info) = self.symbol_sectors.get(&symbol) {
                    sector_data.push((data, info.weight_in_sector));
                }
            }
        }
        
        // Calculate weighted sector metrics
        Ok(SectorFeatures {
            weighted_price_change: self.calculate_weighted_price_change(&sector_data),
            weighted_volume: self.calculate_weighted_volume(&sector_data),
            breadth_ratio: self.calculate_breadth_ratio(&sector_data),
            momentum_score: self.calculate_momentum_score(&sector_data),
        })
    }
}
```

### 2.4 DataConverter (New Component)

**Responsibility**: Convert between internal data formats and vendor TimeSeriesData.

```rust
pub struct DataConverter {
    config: DataConverterConfig,
}

impl DataConverter {
    /// Convert MarketData to vendor TimeSeriesData<f32>
    pub fn to_vendor_format(
        &self,
        market_data: &MarketData,
        sector_features: Option<&SectorFeatures>
    ) -> Result<TimeSeriesData<f32>> {
        // Primary time series (price data)
        let mut ts_data = TimeSeriesData::new(
            market_data.prices
                .iter()
                .map(|&p| p as f32)
                .collect()
        );
        
        // Add exogenous variables if available
        let mut exogenous = Vec::new();
        
        if !market_data.volume.is_empty() {
            exogenous.push(
                market_data.volume
                    .iter()
                    .map(|&v| v as f32)
                    .collect()
            );
        }
        
        if !market_data.volatility.is_empty() {
            exogenous.push(
                market_data.volatility
                    .iter()
                    .map(|&vol| vol as f32)
                    .collect()
            );
        }
        
        // Add sector features if available
        if let Some(sector_feat) = sector_features {
            exogenous.push(vec![sector_feat.weighted_price_change as f32; market_data.prices.len()]);
            exogenous.push(vec![sector_feat.momentum_score as f32; market_data.prices.len()]);
        }
        
        if !exogenous.is_empty() {
            ts_data = ts_data.with_exogenous(exogenous)?;
        }
        
        // Add static features (symbol metadata)
        let static_features = vec![
            market_data.market_cap as f32,
            market_data.sector_id as f32,
            market_data.volatility_regime as f32,
        ];
        
        ts_data = ts_data.with_static_features(static_features);
        
        Ok(ts_data)
    }
    
    /// Convert vendor ForecastResult back to internal format
    pub fn from_vendor_format(
        &self,
        forecast: ForecastResult<f32>,
        model_id: &str
    ) -> Result<PredictionResult> {
        Ok(PredictionResult {
            value: forecast.forecasts[0] as f64,
            confidence: forecast.metadata
                .get("confidence")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0.5),
            model_id: model_id.to_string(),
            timestamp: Utc::now(),
            metadata: forecast.metadata,
        })
    }
}
```

### 2.5 Enhanced Neural Adapter (Modified)

**Responsibility**: Maintain existing interface while routing to vendor models.

```rust
impl EnhancedNeuralAdapter {
    /// Main neural signal entry point (interface preserved)
    pub async fn get_neural_signals(
        &self,
        request: NeuralRequest
    ) -> Result<NeuralResponse> {
        // Route to VendorPredictor instead of FANN
        let prediction = self.vendor_predictor
            .predict(&request.symbol, &request.market_data)
            .await?;
        
        // Convert to existing response format
        Ok(NeuralResponse {
            predictions: vec![prediction.value],
            confidence: prediction.confidence,
            model_agreement: self.calculate_ensemble_agreement(&prediction),
            metadata: prediction.metadata,
        })
    }
}
```

## 3. Data Flow Architecture

### 3.1 Prediction Flow

```
Market Data → Enhanced Neural Adapter → VendorPredictor
                                           │
                                           ├─ SectorMapper (get sector)
                                           ├─ DataConverter (format conversion)
                                           ├─ ModelFactory (get active models)
                                           │
                                           ▼
                                    Vendor BaseModel<f32>
                                           │
                                           ▼
                                    Performance Tracker → DAA System
```

### 3.2 Model Activation Flow

```
Configuration Load → ModelFactory → Model Creation
                                           │
                                           ▼
                                    Data Requirements Check
                                           │
                                           ├─ Requirements Met → Active Models
                                           └─ Requirements Missing → Lazy Models
                                                      │
                                                      ▼
                                              Data Availability Monitor
                                                      │
                                                      ▼
                                              Automatic Activation
```

## 4. Performance Tracking Integration

### 4.1 Real-Time Performance Flow

```rust
pub struct ModelPerformanceTracker {
    /// Track predictions and outcomes
    pub async fn record_prediction(
        &self,
        symbol: &str,
        model_id: &str,
        prediction: &PredictionResult,
        actual_outcome: Option<f64>,
        market_context: &MarketContext,
    ) -> Result<()> {
        // Update model metrics
        let metrics = self.update_model_metrics(symbol, model_id, prediction, actual_outcome).await?;
        
        // Feed to DAA system for training decisions
        self.daa_integration.notify_performance_update(symbol, model_id, &metrics).await?;
        
        Ok(())
    }
}

pub struct DAAPerformanceIntegration {
    training_engine: Arc<AutonomousTrainingEngine>,
}

impl DAAPerformanceIntegration {
    /// Send performance data to DAA for autonomous decisions
    pub async fn notify_performance_update(
        &self,
        symbol: &str,
        model_id: &str,
        metrics: &ModelMetrics
    ) -> Result<()> {
        let daa_input = DAAPerformanceInput {
            prediction_accuracy: metrics.prediction_accuracy,
            consecutive_failures: metrics.consecutive_failures,
            sharpe_ratio: metrics.sharpe_ratio,
            // ... other metrics
        };
        
        // DAA makes autonomous training decision
        let decision = self.training_engine
            .make_autonomous_training_decision(model_id, symbol, daa_input)
            .await?;
        
        if decision.should_train {
            info!("🤖 DAA Autonomous Training Triggered: {} for {} ({})", 
                model_id, symbol, decision.reasoning);
        }
        
        Ok(())
    }
}
```

## 5. Configuration Architecture

### 5.1 Model Configuration Structure

```toml
[models.lstm_technology]
architecture = "LSTM"
sector = "technology"
input_size = 24
hidden_size = 64
num_layers = 2
dropout = 0.1

[models.lstm_technology.data_requirements]
required = ["price"]
optional = ["volume", "volatility"]
min_history = 100

[models.tft_financial]
architecture = "TFT"
sector = "financial"
d_model = 128
num_heads = 4

[models.tft_financial.data_requirements]
required = ["price", "volume"]
optional = ["economic", "sentiment"]
min_history = 200
```

### 5.2 Sector Configuration Structure

```toml
[sectors.technology]
etf_representative = "XLK"

[[sectors.technology.symbols]]
symbol = "AAPL"
weight = 0.22
sub_sector = "Consumer Electronics"
market_cap_tier = "LargeCap"

[[sectors.technology.symbols]]
symbol = "MSFT"
weight = 0.21
sub_sector = "Software"
market_cap_tier = "LargeCap"
```

## 6. Architectural Decisions

### 6.1 Decision: Direct BaseModel Integration (vs Adapter Pattern)
- **Rationale**: Eliminates adapter complexity and performance overhead
- **Impact**: Cleaner code, better performance, easier maintenance
- **Trade-offs**: Vendor dependency, but provides superior models

### 6.2 Decision: Sector-Based Model Organization
- **Rationale**: Enables scalability without resource explosion
- **Impact**: 10 sector models vs 100+ symbol models
- **Trade-offs**: Slight accuracy loss vs massive efficiency gain

### 6.3 Decision: Lazy Model Loading
- **Rationale**: Handle data evolution gracefully
- **Impact**: System works with current data, expands automatically
- **Trade-offs**: Added complexity vs future-proof design

### 6.4 Decision: Performance-Driven DAA Integration
- **Rationale**: Enable truly autonomous training decisions
- **Impact**: Models improve automatically based on real performance
- **Trade-offs**: Additional monitoring overhead vs autonomous optimization

## 7. Migration Strategy

### 7.1 Component Replacement Order
1. **VendorPredictor**: Replace FannPredictor core
2. **ModelFactory**: Replace fake model creation
3. **DataConverter**: Add vendor format conversion
4. **SectorMapper**: Add sector-based routing
5. **Performance Integration**: Connect to DAA system

### 7.2 Validation Strategy
- Parallel prediction validation during migration
- Performance benchmarking against FANN baseline
- DAA integration testing with mock performance data
- Memory usage validation with multiple models

This architecture provides a solid foundation for vendor model integration while preserving all DAA autonomous capabilities and preparing for sector-based scalability in subsequent phases.