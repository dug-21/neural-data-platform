# Phase 1 Specification: Vendor Model Foundation

## Executive Summary

Phase 1 transforms the neural-trader system from a single-symbol architecture with fake FANN models to a scalable vendor-model-based foundation. This phase implements direct BaseModel<f32> integration while preserving all DAA autonomous portfolio management capabilities.

## 1. Functional Requirements

### 1.1 Vendor Model Integration
- **REQ-VM-001**: Replace all FANN models with vendor BaseModel<f32> implementations
- **REQ-VM-002**: Support minimum 10 vendor model architectures (MLP, LSTM, GRU, TCN, TFT, DeepAR, NBEATS, NHITS, DLinear, NLinear)
- **REQ-VM-003**: Enable lazy model loading based on configurable data requirements
- **REQ-VM-004**: Provide TimeSeriesData<f32> conversion from internal market data format

### 1.2 DAA Autonomous System Preservation
- **REQ-DAA-001**: Maintain 60% neural / 40% strategy voting weights in autonomous decisions
- **REQ-DAA-002**: Preserve Byzantine fault tolerance in consensus mechanisms
- **REQ-DAA-003**: Continue autonomous training schedule with performance-driven decisions
- **REQ-DAA-004**: Integrate performance data feed into DAA training decisions

### 1.3 Sector-Based Architecture Foundation
- **REQ-SECTOR-001**: Implement sector mapping for individual symbols (10 sectors minimum)
- **REQ-SECTOR-002**: Enable sector-level model sharing with symbol-specific enhancements
- **REQ-SECTOR-003**: Support sector ETF representatives for validation
- **REQ-SECTOR-004**: Provide configurable sector classification system

### 1.4 Performance Tracking Integration
- **REQ-PERF-001**: Track individual model performance metrics (accuracy, Sharpe ratio, drawdown)
- **REQ-PERF-002**: Provide real-time performance data to DAA training system
- **REQ-PERF-003**: Calculate model value scores for optimization decisions
- **REQ-PERF-004**: Support performance-based model activation/deactivation

## 2. Technical Requirements

### 2.1 Architecture Components

#### 2.1.1 VendorPredictor
```rust
pub struct VendorPredictor {
    models: Arc<DashMap<ModelKey, Box<dyn BaseModel<f32>>>>,
    sector_mapper: Arc<SectorMapper>,
    performance_tracker: Arc<ModelPerformanceTracker>,
    config: VendorPredictorConfig,
}
```

#### 2.1.2 ModelFactory
- **REQ-FACTORY-001**: Create vendor models from configuration
- **REQ-FACTORY-002**: Support all BaseModel<f32> implementations
- **REQ-FACTORY-003**: Handle model initialization parameters
- **REQ-FACTORY-004**: Provide model capability discovery

#### 2.1.3 SectorMapper
- **REQ-MAPPER-001**: Map symbols to sectors via configuration
- **REQ-MAPPER-002**: Support dynamic sector updates
- **REQ-MAPPER-003**: Provide sector ETF mappings
- **REQ-MAPPER-004**: Calculate sector-level aggregations

### 2.2 Data Requirements

#### 2.2.1 Input Data Support
- **REQ-DATA-001**: Accept 1-minute price aggregates (current capability)
- **REQ-DATA-002**: Support volume data when available
- **REQ-DATA-003**: Gracefully handle missing data modalities
- **REQ-DATA-004**: Enable future data type integration without code changes

#### 2.2.2 TimeSeriesData Conversion
- **REQ-CONV-001**: Convert MarketData to TimeSeriesData<f32>
- **REQ-CONV-002**: Support exogenous variables (volume, volatility)
- **REQ-CONV-003**: Handle static features (sector, market cap)
- **REQ-CONV-004**: Manage variable-length time series

### 2.3 Integration Requirements

#### 2.3.1 DAA System Integration
- **REQ-INT-001**: Maintain existing DAACoordinator interface
- **REQ-INT-002**: Provide performance metrics to AutonomousTrainingEngine
- **REQ-INT-003**: Support real-time training decision feedback
- **REQ-INT-004**: Preserve autonomous portfolio optimization

#### 2.3.2 Enhanced Neural Adapter
- **REQ-ADAPTER-001**: Replace FANN calls with vendor model predictions
- **REQ-ADAPTER-002**: Maintain NeuralRequest/NeuralResponse interface
- **REQ-ADAPTER-003**: Provide confidence scoring from vendor models
- **REQ-ADAPTER-004**: Support ensemble prediction aggregation

## 3. Performance Requirements

### 3.1 Scalability
- **REQ-SCALE-001**: Support 10+ symbols initially (vs current 1)
- **REQ-SCALE-002**: Enable 100+ symbol scaling in future phases
- **REQ-SCALE-003**: Reduce memory usage by 50% through sector-based models
- **REQ-SCALE-004**: Maintain prediction latency under 100ms per symbol

### 3.2 Reliability
- **REQ-REL-001**: Achieve 99.5% uptime during market hours
- **REQ-REL-002**: Handle model failures gracefully with fallback predictions
- **REQ-REL-003**: Support hot model reloading without downtime
- **REQ-REL-004**: Maintain prediction quality during data outages

## 4. Configuration Requirements

### 4.1 Model Configuration
```toml
[models.lstm_price]
architecture = "LSTM"
input_size = 24
hidden_size = 64
data_requirements.required = ["price"]
data_requirements.optional = ["volume"]
```

### 4.2 Sector Configuration
```toml
[sectors.technology]
etf_representative = "XLK"
symbols = [
    { symbol = "AAPL", weight = 0.22, sub_sector = "Consumer Electronics" },
    { symbol = "MSFT", weight = 0.21, sub_sector = "Software" },
]
```

### 4.3 DAA Integration Configuration
```toml
[daa.performance_integration]
performance_check_interval = "5m"
emergency_accuracy_threshold = 0.5
critical_accuracy_threshold = 0.7
```

## 5. Compliance Requirements

### 5.1 Integration-First Mandate
- **REQ-MANDATE-001**: Complete replacement of neural engine (approved exception)
- **REQ-MANDATE-002**: Preserve all existing DAA interfaces and capabilities
- **REQ-MANDATE-003**: Maintain autonomous trading functionality
- **REQ-MANDATE-004**: Ensure performance tracking integration

### 5.2 Quality Requirements
- **REQ-QUAL-001**: Achieve 90%+ unit test coverage
- **REQ-QUAL-002**: Pass all integration tests with DAA system
- **REQ-QUAL-003**: Demonstrate performance improvement over FANN baseline
- **REQ-QUAL-004**: Validate memory usage reduction targets

## 6. Success Metrics

### 6.1 Functional Success
- All 10+ vendor models successfully integrated and operational
- DAA autonomous trading maintains 60/40 neural/strategy weighting
- Sector-based architecture reduces model count by 90% (10 vs 100+)
- Performance tracking feeds real-time data to DAA training decisions

### 6.2 Performance Success
- Memory usage reduced by 50% compared to symbol-per-model approach
- Prediction latency maintained under 100ms per symbol
- Model accuracy equals or exceeds current FANN baseline
- System supports 10+ concurrent symbols with room for scaling

### 6.3 Integration Success
- Zero downtime migration from FANN to vendor models
- All DAA autonomous capabilities preserved and operational
- Performance data successfully drives autonomous training decisions
- Sector classification accurately maps all configured symbols

## 7. Risk Mitigation

### 7.1 Technical Risks
- **RISK-001**: Vendor model API incompatibilities
  - *Mitigation*: Extensive testing with vendor BaseModel trait
- **RISK-002**: Performance degradation during migration
  - *Mitigation*: Parallel testing and gradual rollout capabilities
- **RISK-003**: DAA integration breaking changes
  - *Mitigation*: Interface preservation and comprehensive integration tests

### 7.2 Operational Risks
- **RISK-004**: Market hours downtime during deployment
  - *Mitigation*: After-market deployment window and rollback procedures
- **RISK-005**: Data format incompatibilities
  - *Mitigation*: Robust data conversion layer with validation

This specification ensures Phase 1 delivers a solid vendor model foundation while maintaining all critical DAA autonomous capabilities and preparing for the sector-based scalability of subsequent phases.