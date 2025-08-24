# Neural Trader V2 Interface Specifications (CORRECTED)

## SPARC Phase: Architecture - Interface Design

### Document Information
- **Version**: 2.0
- **Date**: 2025-08-23
- **Status**: Single Rust Binary Interface Design
- **Scope**: Embedded component interfaces with ruv-FANN and DAA Coordinator
- **Architecture**: Clean, testable, embedded interfaces (no microservices)

---

## Executive Summary

This document defines all interface contracts for Neural Trader V2 services, designed from the ground up for maximum testability and maintainability. Every interface includes comprehensive mocking support, error handling, and validation to ensure robust system integration.

### Design Principles

1. **Contract-First Development**: All interfaces defined before implementation
2. **Mock-Friendly Design**: Every interface designed for easy mocking and testing
3. **Comprehensive Error Handling**: Typed errors with recovery strategies
4. **Validation at Boundaries**: All data validated at service boundaries
5. **Versioned Evolution**: Forward and backward compatibility built-in

---

## Interface Architecture Overview

### Component Communication Patterns (Single Binary)

```
┌────────────────────────────────────────────────────────────────┐
│              Neural Trader Rust Binary Interfaces             │
├────────────────────────────────────────────────────────────────┤
│                                                                │
│  ┌─────────────┐   ┌─────────────┐   ┌─────────────────────┐    │
│  │   Market    │   │   Feature   │   │   Trading Module    │    │
│  │    Data     │   │ Engineering │   │ + ruv-FANN Models  │    │
│  │   Module    │   │   Module    │   │ + DAA Coordinator  │    │
│  └─────────────┘   └─────────────┘   └─────────────────────┘    │
│         │                  │                  │                 │
│         │   Rust Channels   │   Rust Channels  │                 │
│         ↓                  ↓                  ↓                 │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │           Internal Event Bus (Rust Channels)            │    │
│  │           + External EventBus (Redis Streams)           │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                │
│  │   Trading   │               │               │   Shared    │  │
│  │   Service   │               │               │  Services   │  │
│  └─────────────┘               │               └─────────────┘  │
│         │                      │                      │        │
│         │                      ↓                      │        │
│         └─────────── Storage Interfaces ──────────────┘        │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

---

## gRPC Service Interfaces

### Market Data Service Interface

#### Service Definition

```protobuf
syntax = "proto3";
package neural_trader.market_data.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/empty.proto";

// Market Data Service - Real-time data ingestion and quality monitoring
service MarketDataService {
  // Real-time data streaming
  rpc StreamMarketData(StreamRequest) returns (stream MarketDataEvent);
  
  // Historical data retrieval
  rpc GetHistoricalData(HistoricalDataRequest) returns (HistoricalDataResponse);
  
  // Data quality metrics
  rpc GetDataQualityMetrics(QualityMetricsRequest) returns (DataQualityMetrics);
  
  // Provider management
  rpc ListDataProviders(google.protobuf.Empty) returns (DataProviderList);
  rpc GetProviderStatus(ProviderStatusRequest) returns (ProviderStatus);
  
  // Data validation
  rpc ValidateDataFeed(ValidationRequest) returns (ValidationResponse);
  
  // Health and metrics
  rpc GetServiceHealth(google.protobuf.Empty) returns (ServiceHealth);
  rpc GetServiceMetrics(google.protobuf.Empty) returns (ServiceMetrics);
}

// Request/Response Messages
message StreamRequest {
  repeated string symbols = 1;
  repeated DataType data_types = 2;
  DataFilter filter = 3;
  int32 buffer_size = 4;
}

message MarketDataEvent {
  string event_id = 1;
  google.protobuf.Timestamp timestamp = 2;
  string symbol = 3;
  DataType data_type = 4;
  MarketDataPayload payload = 5;
  DataQuality quality = 6;
  string provider = 7;
  map<string, string> metadata = 8;
}

message MarketDataPayload {
  oneof data {
    TradeData trade = 1;
    QuoteData quote = 2;
    BarData bar = 3;
    NewsData news = 4;
  }
}

message TradeData {
  double price = 1;
  double size = 2;
  google.protobuf.Timestamp timestamp = 3;
  string exchange = 4;
  int64 sequence = 5;
}

message QuoteData {
  double bid_price = 1;
  double bid_size = 2;
  double ask_price = 3;
  double ask_size = 4;
  google.protobuf.Timestamp timestamp = 5;
  string exchange = 6;
}

message BarData {
  double open = 1;
  double high = 2;
  double low = 3;
  double close = 4;
  double volume = 5;
  double vwap = 6;
  google.protobuf.Timestamp start_time = 7;
  google.protobuf.Timestamp end_time = 8;
  int32 trade_count = 9;
}

message DataQuality {
  double completeness_score = 1;  // 0.0 - 1.0
  double timeliness_score = 2;    // 0.0 - 1.0  
  double accuracy_score = 3;      // 0.0 - 1.0
  double overall_score = 4;       // 0.0 - 1.0
  repeated string issues = 5;
}

// Enums
enum DataType {
  DATA_TYPE_UNSPECIFIED = 0;
  DATA_TYPE_TRADE = 1;
  DATA_TYPE_QUOTE = 2;
  DATA_TYPE_BAR_1M = 3;
  DATA_TYPE_BAR_5M = 4;
  DATA_TYPE_BAR_1H = 5;
  DATA_TYPE_BAR_1D = 6;
  DATA_TYPE_NEWS = 7;
}
```

#### Error Codes

```protobuf
enum MarketDataErrorCode {
  MARKET_DATA_ERROR_UNSPECIFIED = 0;
  INVALID_SYMBOL = 1;
  PROVIDER_UNAVAILABLE = 2;  
  DATA_QUALITY_INSUFFICIENT = 3;
  RATE_LIMIT_EXCEEDED = 4;
  SUBSCRIPTION_FAILED = 5;
  HISTORICAL_DATA_NOT_AVAILABLE = 6;
}

message MarketDataError {
  MarketDataErrorCode code = 1;
  string message = 2;
  map<string, string> details = 3;
  bool retryable = 4;
  int32 retry_after_seconds = 5;
}
```

#### Mock Interface (Rust)

```rust
use async_trait::async_trait;
use mockall::mock;
use tonic::{Request, Response, Status};
use tokio_stream::Stream;

mock! {
    pub MarketDataServiceClient {}
    
    #[async_trait]
    impl MarketDataServiceTrait for MarketDataServiceClient {
        async fn stream_market_data(
            &self,
            request: Request<StreamRequest>
        ) -> Result<Response<Box<dyn Stream<Item = Result<MarketDataEvent, Status>> + Send>>, Status>;
        
        async fn get_historical_data(
            &self, 
            request: Request<HistoricalDataRequest>
        ) -> Result<Response<HistoricalDataResponse>, Status>;
        
        async fn get_data_quality_metrics(
            &self,
            request: Request<QualityMetricsRequest>
        ) -> Result<Response<DataQualityMetrics>, Status>;
    }
}

// Test helper functions
impl MockMarketDataServiceClient {
    pub fn expect_healthy_stream() -> Self {
        let mut mock = Self::new();
        mock.expect_stream_market_data()
            .returning(|_| {
                let stream = tokio_stream::iter(vec![
                    Ok(create_test_market_data_event("AAPL", 150.0)),
                    Ok(create_test_market_data_event("GOOGL", 2800.0)),
                ]);
                Ok(Response::new(Box::new(stream)))
            });
        mock
    }
    
    pub fn expect_provider_failure() -> Self {
        let mut mock = Self::new();
        mock.expect_stream_market_data()
            .returning(|_| {
                Err(Status::unavailable("Provider connection failed"))
            });
        mock
    }
}

// Test fixture generators
pub fn create_test_market_data_event(symbol: &str, price: f64) -> MarketDataEvent {
    MarketDataEvent {
        event_id: format!("test-{}-{}", symbol, price),
        timestamp: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
        symbol: symbol.to_string(),
        data_type: DataType::DataTypeTrade as i32,
        payload: Some(MarketDataPayload {
            data: Some(market_data_payload::Data::Trade(TradeData {
                price,
                size: 100.0,
                timestamp: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                exchange: "NASDAQ".to_string(),
                sequence: 12345,
            }))
        }),
        quality: Some(DataQuality {
            completeness_score: 1.0,
            timeliness_score: 0.98,
            accuracy_score: 0.99,
            overall_score: 0.99,
            issues: vec![],
        }),
        provider: "test-provider".to_string(),
        metadata: std::collections::HashMap::new(),
    }
}
```

### Feature Engineering Service Interface

#### Service Definition

```protobuf
syntax = "proto3";
package neural_trader.feature_engineering.v1;

service FeatureEngineeringService {
  // Feature calculation
  rpc CalculateFeatures(FeatureRequest) returns (FeatureResponse);
  rpc CalculateFeaturesStream(stream FeatureRequest) returns (stream FeatureResponse);
  
  // Feature management
  rpc GetAvailableIndicators(google.protobuf.Empty) returns (IndicatorList);
  rpc ValidateFeaturePipeline(PipelineConfig) returns (ValidationResponse);
  rpc GetFeatureMetadata(FeatureMetadataRequest) returns (FeatureMetadata);
  
  // Caching
  rpc GetCachedFeatures(CacheRequest) returns (CacheResponse);
  rpc InvalidateCache(CacheInvalidationRequest) returns (CacheInvalidationResponse);
}

message FeatureRequest {
  string request_id = 1;
  string symbol = 2;
  repeated IndicatorRequest indicators = 3;
  TimeWindow window = 4;
  map<string, string> parameters = 5;
}

message IndicatorRequest {
  string name = 1;  // "rsi", "macd", "bollinger_bands", etc.
  int32 period = 2;
  map<string, double> parameters = 3;
}

message FeatureResponse {
  string request_id = 1;
  string symbol = 2;
  repeated FeatureValue features = 3;
  google.protobuf.Timestamp calculated_at = 4;
  CalculationMetadata metadata = 5;
}

message FeatureValue {
  string name = 1;
  oneof value {
    double numeric_value = 2;
    string text_value = 3;
    bool boolean_value = 4;
    FeatureVector vector_value = 5;
  }
  double confidence = 6;
  map<string, string> attributes = 7;
}

message FeatureVector {
  repeated double values = 1;
  repeated string labels = 2;
}

message CalculationMetadata {
  int32 data_points_used = 1;
  double calculation_time_ms = 2;
  bool cache_hit = 3;
  repeated string warnings = 4;
}

// Technical Indicators
enum IndicatorType {
  INDICATOR_UNSPECIFIED = 0;
  RSI = 1;
  MACD = 2;
  BOLLINGER_BANDS = 3;
  MOVING_AVERAGE = 4;
  STOCHASTIC = 5;
  ATR = 6;
  VOLUME_PROFILE = 7;
  MOMENTUM = 8;
}
```

#### Mock Interface (Rust)

```rust
mock! {
    pub FeatureEngineeringServiceClient {}
    
    #[async_trait] 
    impl FeatureEngineeringServiceTrait for FeatureEngineeringServiceClient {
        async fn calculate_features(
            &self,
            request: Request<FeatureRequest>
        ) -> Result<Response<FeatureResponse>, Status>;
    }
}

impl MockFeatureEngineeringServiceClient {
    pub fn expect_successful_calculation() -> Self {
        let mut mock = Self::new();
        mock.expect_calculate_features()
            .returning(|req| {
                let request = req.into_inner();
                Ok(Response::new(FeatureResponse {
                    request_id: request.request_id,
                    symbol: request.symbol,
                    features: vec![
                        FeatureValue {
                            name: "rsi_14".to_string(),
                            value: Some(feature_value::Value::NumericValue(65.2)),
                            confidence: 0.95,
                            attributes: std::collections::HashMap::new(),
                        }
                    ],
                    calculated_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                    metadata: Some(CalculationMetadata {
                        data_points_used: 100,
                        calculation_time_ms: 2.5,
                        cache_hit: false,
                        warnings: vec![],
                    }),
                }))
            });
        mock
    }
}
```

### Model Management Service Interface

#### Service Definition

```protobuf
syntax = "proto3";
package neural_trader.model_management.v1;

service ModelManagementService {
  // Model training
  rpc TrainModel(TrainingRequest) returns (stream TrainingStatus);
  rpc GetTrainingJob(TrainingJobRequest) returns (TrainingJob);
  rpc CancelTrainingJob(CancelTrainingRequest) returns (CancelTrainingResponse);
  
  // Model deployment
  rpc DeployModel(DeploymentRequest) returns (DeploymentResponse);
  rpc UndeployModel(UndeploymentRequest) returns (UndeploymentResponse);
  rpc GetModelStatus(ModelStatusRequest) returns (ModelStatus);
  
  // Model inference
  rpc Predict(PredictionRequest) returns (PredictionResponse);
  rpc BatchPredict(BatchPredictionRequest) returns (BatchPredictionResponse);
  
  // Model management
  rpc ListModels(ListModelsRequest) returns (ModelList);
  rpc GetModelMetadata(ModelMetadataRequest) returns (ModelMetadata);
  rpc DeleteModel(DeleteModelRequest) returns (DeleteModelResponse);
  
  // A/B Testing
  rpc CreateExperiment(ExperimentRequest) returns (ExperimentResponse);
  rpc GetExperimentResults(ExperimentResultsRequest) returns (ExperimentResults);
}

message TrainingRequest {
  string model_name = 1;
  ModelType model_type = 2;
  TrainingConfig config = 3;
  DataConfig data_config = 4;
  HyperParameters hyperparameters = 5;
}

message TrainingConfig {
  int32 epochs = 1;
  double learning_rate = 2;
  int32 batch_size = 3;
  double validation_split = 4;
  bool early_stopping = 5;
  int32 patience = 6;
}

message TrainingStatus {
  string job_id = 1;
  TrainingStage stage = 2;
  int32 current_epoch = 3;
  double progress_percentage = 4;
  TrainingMetrics metrics = 5;
  string message = 6;
  google.protobuf.Timestamp timestamp = 7;
}

message TrainingMetrics {
  double loss = 1;
  double accuracy = 2;
  double val_loss = 3;
  double val_accuracy = 4;
  map<string, double> custom_metrics = 5;
}

enum TrainingStage {
  TRAINING_STAGE_UNSPECIFIED = 0;
  DATA_PREPARATION = 1;
  MODEL_INITIALIZATION = 2;
  TRAINING = 3;
  VALIDATION = 4;
  MODEL_SAVING = 5;
  COMPLETED = 6;
  FAILED = 7;
}

enum ModelType {
  MODEL_TYPE_UNSPECIFIED = 0;
  LSTM = 1;
  GRU = 2;
  TRANSFORMER = 3;
  MLP = 4;
  CNN_LSTM = 5;
  XG_BOOST = 6;
  RANDOM_FOREST = 7;
}

message PredictionRequest {
  string model_id = 1;
  string model_version = 2;
  repeated double features = 3;
  map<string, string> metadata = 4;
  PredictionConfig config = 5;
}

message PredictionResponse {
  string prediction_id = 1;
  repeated Prediction predictions = 2;
  double confidence = 3;
  int64 inference_time_us = 4;
  ModelInfo model_info = 5;
}

message Prediction {
  double value = 1;
  double confidence = 2;
  double lower_bound = 3;
  double upper_bound = 4;
  int32 horizon_steps = 5;
  map<string, double> feature_importance = 6;
}
```

#### Mock Interface (Rust)

```rust
mock! {
    pub ModelManagementServiceClient {}
    
    #[async_trait]
    impl ModelManagementServiceTrait for ModelManagementServiceClient {
        async fn predict(
            &self,
            request: Request<PredictionRequest>
        ) -> Result<Response<PredictionResponse>, Status>;
        
        async fn train_model(
            &self,
            request: Request<TrainingRequest>
        ) -> Result<Response<Box<dyn Stream<Item = Result<TrainingStatus, Status>> + Send>>, Status>;
    }
}

impl MockModelManagementServiceClient {
    pub fn expect_successful_prediction() -> Self {
        let mut mock = Self::new();
        mock.expect_predict()
            .returning(|req| {
                let request = req.into_inner();
                Ok(Response::new(PredictionResponse {
                    prediction_id: "pred-12345".to_string(),
                    predictions: vec![
                        Prediction {
                            value: 151.5,
                            confidence: 0.87,
                            lower_bound: 149.2,
                            upper_bound: 153.8,
                            horizon_steps: 60,
                            feature_importance: [
                                ("rsi_14".to_string(), 0.23),
                                ("macd".to_string(), 0.19),
                            ].into(),
                        }
                    ],
                    confidence: 0.87,
                    inference_time_us: 25_000,
                    model_info: Some(ModelInfo {
                        model_id: request.model_id,
                        version: request.model_version,
                        model_type: ModelType::Lstm as i32,
                        trained_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                    }),
                }))
            });
        mock
    }
}
```

### Trading Service Interface

#### Service Definition

```protobuf
syntax = "proto3";
package neural_trader.trading.v1;

service TradingService {
  // Order management
  rpc SubmitOrder(OrderRequest) returns (OrderResponse);
  rpc CancelOrder(CancelOrderRequest) returns (CancelOrderResponse);
  rpc ModifyOrder(ModifyOrderRequest) returns (ModifyOrderResponse);
  rpc GetOrderStatus(OrderStatusRequest) returns (OrderStatus);
  
  // Portfolio management
  rpc GetPortfolio(PortfolioRequest) returns (Portfolio);
  rpc GetPositions(PositionsRequest) returns (PositionList);
  rpc GetPerformance(PerformanceRequest) returns (PerformanceMetrics);
  
  // Risk management
  rpc ValidateRisk(RiskValidationRequest) returns (RiskValidationResponse);
  rpc GetRiskMetrics(RiskMetricsRequest) returns (RiskMetrics);
  rpc SetRiskLimits(RiskLimitsRequest) returns (RiskLimitsResponse);
  
  // Strategy management
  rpc GetTradingSignals(SignalRequest) returns (SignalResponse);
  rpc EnableStrategy(StrategyRequest) returns (StrategyResponse);
  rpc DisableStrategy(StrategyRequest) returns (StrategyResponse);
  
  // Emergency controls
  rpc EmergencyStop(EmergencyStopRequest) returns (EmergencyStopResponse);
  rpc ResumeTrading(ResumeRequest) returns (ResumeResponse);
}

message OrderRequest {
  string client_order_id = 1;
  string symbol = 2;
  OrderSide side = 3;
  double quantity = 4;
  OrderType order_type = 5;
  double price = 6;  // For limit orders
  double stop_price = 7;  // For stop orders
  TimeInForce time_in_force = 8;
  OrderSource source = 9;
  map<string, string> metadata = 10;
}

message OrderResponse {
  string order_id = 1;
  OrderState state = 2;
  string broker_order_id = 3;
  google.protobuf.Timestamp submitted_at = 4;
  repeated string warnings = 5;
  RiskValidationResult risk_validation = 6;
}

message RiskValidationRequest {
  OrderRequest order = 1;
  Portfolio current_portfolio = 2;
  map<string, double> current_positions = 3;
}

message RiskValidationResponse {
  bool approved = 1;
  double risk_score = 2;
  repeated string warnings = 3;
  repeated string errors = 4;
  RiskAssessment assessment = 5;
}

message RiskAssessment {
  double position_risk = 1;
  double portfolio_concentration = 2;
  double correlation_risk = 3;
  double drawdown_risk = 4;
  double liquidity_risk = 5;
  double overall_score = 6;
}

enum OrderSide {
  ORDER_SIDE_UNSPECIFIED = 0;
  BUY = 1;
  SELL = 2;
}

enum OrderType {
  ORDER_TYPE_UNSPECIFIED = 0;
  MARKET = 1;
  LIMIT = 2;
  STOP = 3;
  STOP_LIMIT = 4;
}

enum OrderState {
  ORDER_STATE_UNSPECIFIED = 0;
  PENDING = 1;
  SUBMITTED = 2;
  PARTIALLY_FILLED = 3;
  FILLED = 4;
  CANCELLED = 5;
  REJECTED = 6;
  EXPIRED = 7;
}

enum OrderSource {
  ORDER_SOURCE_UNSPECIFIED = 0;
  NEURAL_STRATEGY = 1;
  MANUAL = 2;
  RISK_MANAGEMENT = 3;
  EMERGENCY = 4;
}
```

#### Mock Interface (Rust)

```rust
mock! {
    pub TradingServiceClient {}
    
    #[async_trait]
    impl TradingServiceTrait for TradingServiceClient {
        async fn submit_order(
            &self,
            request: Request<OrderRequest>
        ) -> Result<Response<OrderResponse>, Status>;
        
        async fn validate_risk(
            &self,
            request: Request<RiskValidationRequest>
        ) -> Result<Response<RiskValidationResponse>, Status>;
    }
}

impl MockTradingServiceClient {
    pub fn expect_order_approved() -> Self {
        let mut mock = Self::new();
        mock.expect_validate_risk()
            .returning(|_| {
                Ok(Response::new(RiskValidationResponse {
                    approved: true,
                    risk_score: 0.3,
                    warnings: vec![],
                    errors: vec![],
                    assessment: Some(RiskAssessment {
                        position_risk: 0.02,
                        portfolio_concentration: 0.05,
                        correlation_risk: 0.15,
                        drawdown_risk: 0.08,
                        liquidity_risk: 0.01,
                        overall_score: 0.31,
                    }),
                }))
            });
        
        mock.expect_submit_order()
            .returning(|req| {
                let request = req.into_inner();
                Ok(Response::new(OrderResponse {
                    order_id: "order-12345".to_string(),
                    state: OrderState::Submitted as i32,
                    broker_order_id: "broker-67890".to_string(),
                    submitted_at: Some(prost_types::Timestamp::from(std::time::SystemTime::now())),
                    warnings: vec![],
                    risk_validation: Some(RiskValidationResult {
                        approved: true,
                        risk_score: 0.3,
                    }),
                }))
            });
        mock
    }
    
    pub fn expect_order_rejected() -> Self {
        let mut mock = Self::new();
        mock.expect_validate_risk()
            .returning(|_| {
                Ok(Response::new(RiskValidationResponse {
                    approved: false,
                    risk_score: 0.85,
                    warnings: vec!["High concentration risk".to_string()],
                    errors: vec!["Position size exceeds limit".to_string()],
                    assessment: Some(RiskAssessment {
                        position_risk: 0.08,
                        portfolio_concentration: 0.15,
                        correlation_risk: 0.12,
                        drawdown_risk: 0.05,
                        liquidity_risk: 0.02,
                        overall_score: 0.84,
                    }),
                }))
            });
        mock
    }
}
```

---

## Event Bus Interface Specifications

### Event Schema Framework

#### Base Event Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Neural Trader Event Schema",
  "type": "object",
  "properties": {
    "schema_info": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "version": { "type": "string", "pattern": "^\\d+\\.\\d+\\.\\d+$" },
        "format": { "type": "string", "enum": ["json", "avro", "protobuf"] },
        "compatibility": { "type": "string", "enum": ["backward", "forward", "full", "none"] }
      },
      "required": ["name", "version", "format", "compatibility"]
    },
    "metadata": {
      "type": "object",
      "properties": {
        "event_id": { "type": "string", "format": "uuid" },
        "correlation_id": { "type": "string", "format": "uuid" },
        "causation_id": { "type": "string", "format": "uuid" },
        "timestamp": { "type": "string", "format": "date-time" },
        "source": { "type": "string" },
        "event_type": { "type": "string" },
        "version": { "type": "string" },
        "trace_id": { "type": "string" }
      },
      "required": ["event_id", "timestamp", "source", "event_type"]
    },
    "payload": {
      "type": "object"
    }
  },
  "required": ["schema_info", "metadata", "payload"]
}
```

### Market Data Events

#### Market Data Event Schema

```json
{
  "schema_info": {
    "name": "market-data-event",
    "version": "1.0.0",
    "format": "json",
    "compatibility": "backward"
  },
  "metadata": {
    "event_id": "01HZXYZABC123456789",
    "timestamp": "2024-01-01T12:00:00.123Z",
    "source": "market-data-service",
    "event_type": "market_data_received"
  },
  "payload": {
    "symbol": "AAPL",
    "data_type": "trade",
    "exchange": "NASDAQ",
    "data": {
      "price": 150.25,
      "size": 1000,
      "timestamp": "2024-01-01T12:00:00.120Z",
      "sequence": 123456789
    },
    "quality": {
      "completeness_score": 1.0,
      "timeliness_score": 0.98,
      "accuracy_score": 0.99,
      "overall_score": 0.99
    },
    "provider_info": {
      "name": "alpaca",
      "feed_type": "sip",
      "latency_ms": 12
    }
  }
}
```

### Feature Events

#### Feature Calculated Event Schema

```json
{
  "schema_info": {
    "name": "features-calculated-event",
    "version": "1.0.0",
    "format": "json",
    "compatibility": "backward"
  },
  "metadata": {
    "event_id": "01HZXYZDEF123456789",
    "correlation_id": "01HZXYZABC123456789",
    "timestamp": "2024-01-01T12:00:00.125Z",
    "source": "feature-engineering-service",
    "event_type": "features_calculated"
  },
  "payload": {
    "symbol": "AAPL",
    "calculation_id": "calc-12345",
    "features": {
      "technical_indicators": {
        "rsi_14": {
          "value": 65.2,
          "confidence": 0.95,
          "timestamp": "2024-01-01T12:00:00.123Z"
        },
        "macd": {
          "value": 0.45,
          "signal": 0.38,
          "histogram": 0.07,
          "confidence": 0.92
        },
        "bollinger_bands": {
          "upper": 151.20,
          "middle": 149.85,
          "lower": 148.50,
          "width": 2.70,
          "percent_b": 0.15
        }
      },
      "market_regime": {
        "trend": "upward",
        "volatility": "normal",
        "volume_profile": "above_average",
        "confidence": 0.87
      }
    },
    "calculation_metadata": {
      "data_points_used": 100,
      "calculation_time_ms": 2.5,
      "cache_hit": false,
      "dependencies": ["market-data-event:01HZXYZABC123456789"]
    }
  }
}
```

### Prediction Events

#### Model Prediction Event Schema

```json
{
  "schema_info": {
    "name": "model-prediction-event", 
    "version": "1.0.0",
    "format": "json",
    "compatibility": "backward"
  },
  "metadata": {
    "event_id": "01HZXYZGHI123456789",
    "correlation_id": "01HZXYZDEF123456789",
    "timestamp": "2024-01-01T12:00:00.150Z",
    "source": "model-management-service",
    "event_type": "prediction_generated"
  },
  "payload": {
    "symbol": "AAPL",
    "prediction_id": "pred-67890",
    "model_info": {
      "model_id": "lstm-v1-20240101",
      "version": "1.2.3",
      "type": "lstm",
      "training_date": "2024-01-01T00:00:00Z"
    },
    "prediction": {
      "horizon_minutes": 60,
      "predicted_price": 151.50,
      "confidence": 0.87,
      "prediction_interval": {
        "lower_bound": 149.20,
        "upper_bound": 153.80,
        "confidence_level": 0.95
      },
      "probabilities": {
        "up_movement": 0.72,
        "down_movement": 0.28,
        "strong_up": 0.15,
        "strong_down": 0.08
      }
    },
    "feature_importance": {
      "rsi_14": 0.23,
      "macd": 0.19,
      "price_momentum_5m": 0.15,
      "volume_ratio": 0.12,
      "bollinger_position": 0.08
    },
    "inference_metadata": {
      "latency_ms": 23,
      "feature_count": 42,
      "model_load_time_ms": 1.2,
      "dependencies": ["features-calculated-event:01HZXYZDEF123456789"]
    }
  }
}
```

### Trading Events

#### Trading Signal Event Schema

```json
{
  "schema_info": {
    "name": "trading-signal-event",
    "version": "1.0.0", 
    "format": "json",
    "compatibility": "backward"
  },
  "metadata": {
    "event_id": "01HZXYZJKL123456789",
    "correlation_id": "01HZXYZGHI123456789",
    "timestamp": "2024-01-01T12:00:00.175Z",
    "source": "trading-service",
    "event_type": "trading_signal_generated"
  },
  "payload": {
    "symbol": "AAPL",
    "signal_id": "signal-98765",
    "strategy_info": {
      "name": "neural_enhanced_momentum",
      "version": "2.1.0",
      "parameters": {
        "lookback_period": 20,
        "momentum_threshold": 0.02,
        "confidence_threshold": 0.8
      }
    },
    "signal": {
      "action": "buy",
      "strength": "strong",
      "confidence": 0.89,
      "urgency": "normal",
      "reasoning": "Strong ML signal with momentum confirmation",
      "expected_holding_period": "4_hours"
    },
    "order_recommendation": {
      "order_type": "limit",
      "quantity": 100,
      "suggested_price": 150.50,
      "stop_loss": 147.50,
      "take_profit": 153.00,
      "position_sizing": {
        "method": "kelly_criterion",
        "risk_percentage": 0.02,
        "max_position_size": 100
      }
    },
    "risk_assessment": {
      "position_risk": 0.025,
      "portfolio_impact": 0.05,
      "correlation_risk": 0.12,
      "liquidity_risk": 0.01,
      "overall_risk_score": 0.32
    },
    "dependencies": ["model-prediction-event:01HZXYZGHI123456789"]
  }
}
```

#### Order Execution Event Schema

```json
{
  "schema_info": {
    "name": "order-execution-event",
    "version": "1.0.0",
    "format": "json", 
    "compatibility": "backward"
  },
  "metadata": {
    "event_id": "01HZXYZMNO123456789",
    "correlation_id": "01HZXYZJKL123456789",
    "timestamp": "2024-01-01T12:00:00.234Z",
    "source": "trading-service",
    "event_type": "order_executed"
  },
  "payload": {
    "order_info": {
      "client_order_id": "neural-001",
      "broker_order_id": "ALPACA-123456",
      "symbol": "AAPL",
      "side": "buy",
      "quantity": 100,
      "order_type": "limit",
      "limit_price": 150.50,
      "time_in_force": "day",
      "source": "neural_strategy"
    },
    "execution_details": {
      "status": "filled",
      "filled_quantity": 100,
      "average_fill_price": 150.48,
      "total_value": 15048.00,
      "commission": 1.00,
      "fills": [
        {
          "quantity": 50,
          "price": 150.47,
          "timestamp": "2024-01-01T12:00:00.225Z",
          "execution_id": "EXEC-001"
        },
        {
          "quantity": 50,
          "price": 150.49,
          "timestamp": "2024-01-01T12:00:00.230Z", 
          "execution_id": "EXEC-002"
        }
      ]
    },
    "portfolio_impact": {
      "new_position_size": 100,
      "position_value": 15048.00,
      "realized_pnl": 0.00,
      "unrealized_pnl": -2.00,
      "portfolio_weight": 0.025,
      "cash_impact": -15049.00
    },
    "execution_metadata": {
      "order_to_market_latency_ms": 45,
      "fill_to_report_latency_ms": 12,
      "total_execution_time_ms": 234,
      "broker": "alpaca",
      "routing_venue": "NASDAQ"
    }
  }
}
```

---

## Storage Interface Specifications

### Time Series Storage Interface

#### Trait Definition (Rust)

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub values: HashMap<String, f64>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct TimeSeriesQuery {
    pub symbols: Vec<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub fields: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub aggregation: Option<AggregationType>,
}

#[derive(Debug, Clone)]
pub enum AggregationType {
    None,
    Minute(i32),
    Hour(i32), 
    Day(i32),
    Custom(String),
}

#[async_trait]
pub trait TimeSeriesStorage: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// Write a batch of time series points
    async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<usize, Self::Error>;
    
    /// Read time series data within a time range
    async fn read_range(&self, query: TimeSeriesQuery) -> Result<Vec<TimeSeriesPoint>, Self::Error>;
    
    /// Get the latest data point for a symbol
    async fn get_latest(&self, symbol: &str) -> Result<Option<TimeSeriesPoint>, Self::Error>;
    
    /// Create or update an index for better query performance
    async fn create_index(&self, index_spec: IndexSpec) -> Result<(), Self::Error>;
    
    /// Delete data points (for data retention policies)
    async fn delete_range(&self, symbol: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<usize, Self::Error>;
    
    /// Health check for the storage system
    async fn health_check(&self) -> Result<StorageHealth, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct IndexSpec {
    pub name: String,
    pub columns: Vec<String>,
    pub index_type: IndexType,
}

#[derive(Debug, Clone)]
pub enum IndexType {
    BTree,
    Hash,
    GIN,
    GIST,
}

#[derive(Debug, Clone)]
pub struct StorageHealth {
    pub is_healthy: bool,
    pub latency_ms: f64,
    pub available_space_gb: f64,
    pub connection_pool_status: PoolStatus,
}

#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub active_connections: u32,
    pub idle_connections: u32,
    pub max_connections: u32,
}
```

#### Mock Implementation

```rust
use mockall::mock;

mock! {
    pub TimeSeriesStorageImpl {}
    
    #[async_trait]
    impl TimeSeriesStorage for TimeSeriesStorageImpl {
        type Error = Box<dyn std::error::Error + Send + Sync>;
        
        async fn write_batch(&self, points: Vec<TimeSeriesPoint>) -> Result<usize, Self::Error>;
        async fn read_range(&self, query: TimeSeriesQuery) -> Result<Vec<TimeSeriesPoint>, Self::Error>;
        async fn get_latest(&self, symbol: &str) -> Result<Option<TimeSeriesPoint>, Self::Error>;
        async fn create_index(&self, index_spec: IndexSpec) -> Result<(), Self::Error>;
        async fn delete_range(&self, symbol: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<usize, Self::Error>;
        async fn health_check(&self) -> Result<StorageHealth, Self::Error>;
    }
}

impl MockTimeSeriesStorageImpl {
    pub fn expect_healthy() -> Self {
        let mut mock = Self::new();
        
        mock.expect_write_batch()
            .returning(|points| Ok(points.len()));
            
        mock.expect_read_range()
            .returning(|query| {
                // Generate test data based on query
                let mut results = Vec::new();
                for symbol in &query.symbols {
                    results.push(TimeSeriesPoint {
                        timestamp: query.start_time,
                        symbol: symbol.clone(),
                        values: [("price".to_string(), 150.0)].into(),
                        metadata: HashMap::new(),
                    });
                }
                Ok(results)
            });
            
        mock.expect_health_check()
            .returning(|| Ok(StorageHealth {
                is_healthy: true,
                latency_ms: 2.5,
                available_space_gb: 1000.0,
                connection_pool_status: PoolStatus {
                    active_connections: 5,
                    idle_connections: 10,
                    max_connections: 20,
                },
            }));
            
        mock
    }
}
```

### Model Storage Interface

#### Trait Definition (Rust)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub model_id: String,
    pub name: String,
    pub version: String,
    pub model_type: ModelType,
    pub created_at: DateTime<Utc>,
    pub trained_at: DateTime<Utc>,
    pub file_size_bytes: u64,
    pub training_data_hash: String,
    pub performance_metrics: PerformanceMetrics,
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub accuracy: f64,
    pub precision: f64,
    pub recall: f64,
    pub f1_score: f64,
    pub mse: f64,
    pub mae: f64,
    pub custom_metrics: HashMap<String, f64>,
}

#[async_trait]
pub trait ModelStorage: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    
    /// Save a trained model with metadata
    async fn save_model(&self, model_data: &[u8], metadata: ModelMetadata) -> Result<String, Self::Error>;
    
    /// Load a model by ID
    async fn load_model(&self, model_id: &str) -> Result<(Vec<u8>, ModelMetadata), Self::Error>;
    
    /// List models with optional filtering
    async fn list_models(&self, filter: Option<ModelFilter>) -> Result<Vec<ModelMetadata>, Self::Error>;
    
    /// Update model metadata
    async fn update_metadata(&self, model_id: &str, metadata: ModelMetadata) -> Result<(), Self::Error>;
    
    /// Delete a model
    async fn delete_model(&self, model_id: &str) -> Result<(), Self::Error>;
    
    /// Check if a model exists
    async fn model_exists(&self, model_id: &str) -> Result<bool, Self::Error>;
    
    /// Get storage statistics
    async fn get_storage_stats(&self) -> Result<StorageStats, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct ModelFilter {
    pub name_pattern: Option<String>,
    pub model_type: Option<ModelType>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub tags: Option<HashMap<String, String>>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_models: u64,
    pub total_size_bytes: u64,
    pub available_space_bytes: u64,
    pub average_model_size_bytes: u64,
}
```

---

## Error Handling Framework

### Comprehensive Error Types

```rust
use thiserror::Error;
use tonic::{Code, Status};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },
    
    #[error("Resource not found: {resource_type} with id {resource_id}")]
    NotFound { resource_type: String, resource_id: String },
    
    #[error("Service unavailable: {service_name} - {reason}")]
    ServiceUnavailable { service_name: String, reason: String },
    
    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u32, window: String },
    
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },
    
    #[error("Authorization denied: {required_permission}")]
    AuthorizationDenied { required_permission: String },
    
    #[error("Data validation failed: {field} - {reason}")]
    ValidationFailed { field: String, reason: String },
    
    #[error("Internal server error: {message}")]
    Internal { message: String },
    
    #[error("External dependency error: {dependency} - {error}")]
    ExternalDependency { dependency: String, error: String },
    
    #[error("Timeout: operation took longer than {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}

impl ServiceError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, 
            ServiceError::ServiceUnavailable { .. } |
            ServiceError::ExternalDependency { .. } |
            ServiceError::Timeout { .. }
        )
    }
    
    pub fn retry_after_seconds(&self) -> Option<u32> {
        match self {
            ServiceError::RateLimitExceeded { .. } => Some(60),
            ServiceError::ServiceUnavailable { .. } => Some(30),
            _ => None,
        }
    }
}

impl From<ServiceError> for Status {
    fn from(err: ServiceError) -> Self {
        let (code, message) = match err {
            ServiceError::InvalidRequest { message } => (Code::InvalidArgument, message),
            ServiceError::NotFound { .. } => (Code::NotFound, err.to_string()),
            ServiceError::ServiceUnavailable { .. } => (Code::Unavailable, err.to_string()),
            ServiceError::RateLimitExceeded { .. } => (Code::ResourceExhausted, err.to_string()),
            ServiceError::AuthenticationFailed { .. } => (Code::Unauthenticated, err.to_string()),
            ServiceError::AuthorizationDenied { .. } => (Code::PermissionDenied, err.to_string()),
            ServiceError::ValidationFailed { .. } => (Code::InvalidArgument, err.to_string()),
            ServiceError::Internal { message } => (Code::Internal, message),
            ServiceError::ExternalDependency { .. } => (Code::Unavailable, err.to_string()),
            ServiceError::Timeout { .. } => (Code::DeadlineExceeded, err.to_string()),
        };
        
        Status::new(code, message)
    }
}
```

### Error Recovery Patterns

```rust
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

pub async fn retry_with_backoff<T, E, F, Fut>(
    operation: F,
    config: RetryConfig,
) -> Result<T, E>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 1;
    let mut delay = config.base_delay;
    
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(err) => {
                if attempt >= config.max_attempts {
                    return Err(err);
                }
                
                tracing::warn!(
                    attempt,
                    error = ?err,
                    delay_ms = delay.as_millis(),
                    "Operation failed, retrying"
                );
                
                sleep(delay).await;
                
                attempt += 1;
                delay = std::cmp::min(
                    Duration::from_millis(
                        (delay.as_millis() as f64 * config.backoff_multiplier) as u64
                    ),
                    config.max_delay,
                );
                
                if config.jitter {
                    use rand::Rng;
                    let jitter_ms = rand::thread_rng().gen_range(0..100);
                    delay += Duration::from_millis(jitter_ms);
                }
            }
        }
    }
}
```

---

## Testing Infrastructure

### Integration Test Framework

```rust
use tokio::sync::OnceCell;
use testcontainers::{clients::Cli, Container, Docker};

pub struct TestEnvironment {
    pub docker: Cli,
    pub postgres: Container<'static, testcontainers::images::postgres::Postgres>,
    pub redis: Container<'static, testcontainers::images::redis::Redis>,
    pub nats: Container<'static, testcontainers::images::generic::GenericImage>,
}

static TEST_ENV: OnceCell<TestEnvironment> = OnceCell::const_new();

impl TestEnvironment {
    pub async fn get() -> &'static TestEnvironment {
        TEST_ENV.get_or_init(|| async {
            let docker = Cli::default();
            
            let postgres = docker.run(testcontainers::images::postgres::Postgres::default());
            let redis = docker.run(testcontainers::images::redis::Redis::default());
            let nats = docker.run(
                testcontainers::images::generic::GenericImage::new("nats:latest")
                    .with_exposed_port(4222)
            );
            
            TestEnvironment {
                docker,
                postgres,
                redis,
                nats,
            }
        }).await
    }
    
    pub fn postgres_url(&self) -> String {
        let port = self.postgres.get_host_port_ipv4(5432);
        format!("postgresql://postgres:postgres@localhost:{}/test", port)
    }
    
    pub fn redis_url(&self) -> String {
        let port = self.redis.get_host_port_ipv4(6379);
        format!("redis://localhost:{}", port)
    }
    
    pub fn nats_url(&self) -> String {
        let port = self.nats.get_host_port_ipv4(4222);
        format!("nats://localhost:{}", port)
    }
}

#[macro_export]
macro_rules! integration_test {
    ($test_name:ident, $test_fn:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let env = TestEnvironment::get().await;
            $test_fn(env).await;
        }
    };
}

// Usage example
integration_test!(test_market_data_flow, |env| async move {
    let market_data_service = setup_market_data_service(&env.postgres_url()).await;
    let feature_service = setup_feature_service(&env.redis_url()).await;
    
    // Test data flow from market data to features
    let test_data = create_test_market_data();
    market_data_service.publish_data(test_data).await.unwrap();
    
    // Verify features were calculated
    let features = feature_service.get_latest_features("AAPL").await.unwrap();
    assert!(!features.is_empty());
});
```

### Performance Test Framework

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use tokio::runtime::Runtime;

fn benchmark_market_data_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("market_data_processing");
    
    for batch_size in [100, 1000, 10000] {
        group.bench_with_input(
            BenchmarkId::new("batch_processing", batch_size),
            &batch_size,
            |b, &size| {
                let test_data = generate_test_market_data(size);
                let service = setup_test_service();
                
                b.iter(|| {
                    rt.block_on(async {
                        service.process_batch(test_data.clone()).await.unwrap()
                    })
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_feature_calculation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("feature_calculation");
    
    let indicators = ["rsi", "macd", "bollinger_bands", "momentum"];
    
    for indicator in indicators {
        group.bench_with_input(
            BenchmarkId::new("indicator_calculation", indicator),
            &indicator,
            |b, &indicator_name| {
                let test_data = generate_price_data(1000);
                let calculator = setup_indicator_calculator(indicator_name);
                
                b.iter(|| {
                    rt.block_on(async {
                        calculator.calculate(test_data.clone()).await.unwrap()
                    })
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_market_data_processing,
    benchmark_feature_calculation
);
criterion_main!(benches);
```

---

## Conclusion

This interface specification provides a comprehensive foundation for building Neural Trader V2 with a focus on testability, maintainability, and reliability. Every interface is designed to be mock-friendly, includes comprehensive error handling, and supports the quality requirements outlined in the main requirements specification.

### Key Benefits

1. **Complete Test Coverage**: Every interface includes mock implementations for isolated testing
2. **Comprehensive Error Handling**: Typed errors with recovery strategies built-in
3. **Event-Driven Architecture**: Well-defined event schemas with versioning support
4. **Storage Abstraction**: Clean storage interfaces for easy testing and provider switching
5. **Performance Focus**: Built-in performance testing and benchmarking frameworks

The specification enables development teams to build robust, testable services that can be independently deployed and scaled while maintaining strong consistency guarantees across the system.

---

**Next Phase**: [Clean Architecture Implementation](clean-architecture.md) - Define the internal structure and patterns for each service