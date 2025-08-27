# Neural Trader V2 Testing Strategy

## SPARC Phase: Quality Assurance Framework

### Document Information
- **Version**: 2.0
- **Date**: 2025-08-23
- **Status**: Greenfield Testing Strategy
- **Scope**: Comprehensive testing approach for Neural Trader V2 from ground up
- **Quality Goal**: 90% test coverage minimum, zero-defect production deployment

---

## Executive Summary

This document defines a comprehensive testing strategy for Neural Trader V2, emphasizing quality-first development with test-driven design. Every component will be built with testability in mind, ensuring robust, maintainable, and reliable software from day one.

### Testing Philosophy

1. **Test-Driven Development**: Write tests before implementation
2. **Quality Gates**: No code deployment without passing all tests
3. **Comprehensive Coverage**: 90% minimum test coverage across all components
4. **Performance Validation**: Every service must meet performance benchmarks
5. **Chaos Engineering**: Fault tolerance built-in and tested regularly

---

## Testing Pyramid Strategy

### Test Distribution (90% Total Coverage)

```
         ┌─────────────────────────────────────────┐
         │           E2E Tests (10%)               │
         │    Critical user workflows only         │
         └─────────────────────────────────────────┘
                              ▲
                     ┌─────────────────┐
                     │ Integration (20%) │
                     │ Service contracts │
                     └─────────────────┘
                              ▲
            ┌───────────────────────────────────┐
            │          Unit Tests (70%)         │
            │    Isolated component testing     │
            └───────────────────────────────────┘
```

### Testing Pyramid Breakdown

#### Unit Tests (70% of total coverage)
- **Scope**: Individual functions, classes, and modules
- **Coverage Target**: 95% line coverage per service
- **Focus**: Business logic, edge cases, error conditions
- **Speed**: <1ms per test, run on every code change

#### Integration Tests (20% of total coverage)
- **Scope**: Service-to-service interactions
- **Coverage Target**: All gRPC interfaces and event flows
- **Focus**: Contract validation, data flow, error propagation
- **Speed**: <100ms per test, run on pull request

#### End-to-End Tests (10% of total coverage)
- **Scope**: Complete user workflows
- **Coverage Target**: Critical trading scenarios
- **Focus**: System behavior, performance under load
- **Speed**: <30s per test, run on release candidate

---

## Unit Testing Framework

### Testing Standards

#### Test Structure (Given-When-Then)

```rust
#[cfg(test)]
mod market_data_service_tests {
    use super::*;
    use mockall::predicate::*;
    use test_fixtures::*;

    #[tokio::test]
    async fn should_process_valid_market_data_successfully() {
        // Given - Set up test conditions
        let mock_provider = MockMarketDataProvider::new();
        let mock_storage = MockTimeSeriesStorage::expect_healthy();
        let service = MarketDataService::new(mock_provider, mock_storage);
        let test_data = create_test_market_data("AAPL", 150.0);

        // When - Execute the operation
        let result = service.process_market_data(test_data).await;

        // Then - Verify the outcome
        assert!(result.is_ok());
        let processed_data = result.unwrap();
        assert_eq!(processed_data.symbol, "AAPL");
        assert_eq!(processed_data.quality.overall_score, 0.99);
        assert!(processed_data.timestamp.is_some());
    }

    #[tokio::test]
    async fn should_handle_invalid_market_data_gracefully() {
        // Given - Invalid data conditions
        let mock_provider = MockMarketDataProvider::new();
        let mock_storage = MockTimeSeriesStorage::expect_healthy();
        let service = MarketDataService::new(mock_provider, mock_storage);
        let invalid_data = create_invalid_market_data(); // Negative price, invalid symbol

        // When - Process invalid data
        let result = service.process_market_data(invalid_data).await;

        // Then - Should return appropriate error
        assert!(result.is_err());
        match result.unwrap_err() {
            ServiceError::ValidationFailed { field, reason } => {
                assert_eq!(field, "price");
                assert!(reason.contains("must be positive"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[tokio::test]
    async fn should_retry_on_transient_storage_failures() {
        // Given - Storage fails temporarily
        let mock_provider = MockMarketDataProvider::new();
        let mut mock_storage = MockTimeSeriesStorage::new();
        
        // First call fails, second succeeds
        mock_storage
            .expect_write_batch()
            .times(1)
            .returning(|_| Err("Connection timeout".into()));
        
        mock_storage
            .expect_write_batch()
            .times(1)
            .returning(|data| Ok(data.len()));

        let service = MarketDataService::new(mock_provider, mock_storage);
        let test_data = create_test_market_data("AAPL", 150.0);

        // When - Process data with retry logic
        let result = service.process_market_data(test_data).await;

        // Then - Should succeed on retry
        assert!(result.is_ok());
    }
}
```

#### Property-Based Testing

```rust
use proptest::prelude::*;
use crate::domain::*;

proptest! {
    #[test]
    fn test_rsi_calculation_properties(
        prices in prop::collection::vec(1.0_f64..1000.0, 14..1000)
    ) {
        let rsi_calculator = RSI::new(14);
        let price_points: Vec<PricePoint> = prices
            .iter()
            .enumerate()
            .map(|(i, &price)| PricePoint {
                timestamp: Utc::now() + Duration::minutes(i as i64),
                price,
            })
            .collect();

        let result = rsi_calculator.calculate(&price_points);
        
        // RSI properties that must always hold
        prop_assert!(result.is_ok());
        let rsi_value = result.unwrap().value;
        prop_assert!(rsi_value >= 0.0 && rsi_value <= 100.0);
        
        // RSI should be between 30-70 for non-extreme price movements
        let price_volatility = calculate_volatility(&prices);
        if price_volatility < 0.1 {
            prop_assert!(rsi_value >= 20.0 && rsi_value <= 80.0);
        }
    }

    #[test] 
    fn test_position_sizing_never_exceeds_limits(
        account_balance in 1000.0_f64..1_000_000.0,
        risk_percentage in 0.01_f64..0.1,
        price in 1.0_f64..1000.0
    ) {
        let position_sizer = PositionSizer::new();
        let sizing_params = PositionSizingParams {
            account_balance,
            risk_percentage,
            stop_loss_distance: price * 0.05, // 5% stop loss
        };

        let position_size = position_sizer.calculate_position_size(price, sizing_params);

        // Position value should never exceed risk percentage of account
        let position_value = position_size * price;
        let max_allowed_risk = account_balance * risk_percentage;
        
        prop_assert!(position_value <= max_allowed_risk * 1.01); // Allow 1% tolerance for rounding
        prop_assert!(position_size >= 0.0);
    }
}
```

### Mock Testing Framework

#### Service Mocks with Expectation Patterns

```rust
// Market Data Provider Mock
mock! {
    pub MarketDataProvider {}

    #[async_trait]
    impl MarketDataProviderTrait for MarketDataProvider {
        async fn connect(&self) -> Result<DataStream, ProviderError>;
        async fn subscribe(&self, symbols: Vec<String>) -> Result<(), ProviderError>;
        async fn get_historical_data(&self, request: HistoricalDataRequest) -> Result<Vec<MarketDataPoint>, ProviderError>;
    }
}

impl MockMarketDataProvider {
    // Helper for common success scenario
    pub fn expect_healthy_connection() -> Self {
        let mut mock = Self::new();
        mock.expect_connect()
            .times(1)
            .returning(|| Ok(create_test_data_stream()));
        
        mock.expect_subscribe()
            .with(predicate::always())
            .returning(|_| Ok(()));
        
        mock
    }

    // Helper for provider failure scenarios
    pub fn expect_connection_failure() -> Self {
        let mut mock = Self::new();
        mock.expect_connect()
            .times(1)
            .returning(|| Err(ProviderError::ConnectionFailed("Network timeout".to_string())));
        mock
    }

    // Helper for data quality testing
    pub fn expect_degraded_data_quality() -> Self {
        let mut mock = Self::new();
        mock.expect_connect()
            .returning(|| Ok(create_degraded_data_stream())); // Missing fields, delayed data
        mock
    }
}

// Feature Engineering Service Mock
mock! {
    pub FeatureEngineeringService {}

    #[async_trait]
    impl FeatureEngineeringServiceTrait for FeatureEngineeringService {
        async fn calculate_features(&self, request: FeatureRequest) -> Result<FeatureResponse, ServiceError>;
        async fn validate_pipeline(&self, config: PipelineConfig) -> Result<ValidationResult, ServiceError>;
    }
}

impl MockFeatureEngineeringService {
    pub fn expect_successful_calculation() -> Self {
        let mut mock = Self::new();
        mock.expect_calculate_features()
            .returning(|request| {
                Ok(FeatureResponse {
                    request_id: request.request_id,
                    symbol: request.symbol,
                    features: create_test_features_for_symbol(&request.symbol),
                    calculation_time_ms: 2.5,
                    cache_hit: false,
                })
            });
        mock
    }

    pub fn expect_calculation_timeout() -> Self {
        let mut mock = Self::new();
        mock.expect_calculate_features()
            .returning(|_| Err(ServiceError::Timeout { timeout_ms: 5000 }));
        mock
    }
}
```

#### Test Data Factories

```rust
pub struct TestDataFactory;

impl TestDataFactory {
    pub fn create_market_data(symbol: &str, base_price: f64) -> MarketDataEvent {
        MarketDataEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            symbol: symbol.to_string(),
            data_type: DataType::Trade,
            payload: MarketDataPayload::Trade(TradeData {
                price: base_price,
                size: 100.0,
                timestamp: Utc::now(),
                exchange: "NASDAQ".to_string(),
                sequence: rand::thread_rng().gen_range(1000000..9999999),
            }),
            quality: DataQuality {
                completeness_score: 1.0,
                timeliness_score: 0.98,
                accuracy_score: 0.99,
                overall_score: 0.99,
                issues: vec![],
            },
            provider: "test-provider".to_string(),
            metadata: HashMap::new(),
        }
    }

    pub fn create_feature_vector(symbol: &str) -> FeatureVector {
        FeatureVector {
            symbol: symbol.to_string(),
            features: hashmap! {
                "rsi_14".to_string() => 65.2,
                "macd".to_string() => 0.45,
                "macd_signal".to_string() => 0.38,
                "macd_histogram".to_string() => 0.07,
                "bollinger_upper".to_string() => 151.20,
                "bollinger_middle".to_string() => 149.85,
                "bollinger_lower".to_string() => 148.50,
                "volume_sma_20".to_string() => 1_500_000.0,
                "price_momentum_5m".to_string() => 0.02,
            },
            timestamp: Utc::now(),
            confidence: 0.95,
        }
    }

    pub fn create_prediction(symbol: &str, current_price: f64) -> PredictionResult {
        PredictionResult {
            symbol: symbol.to_string(),
            model_id: "test-lstm-v1".to_string(),
            predicted_price: current_price * 1.02, // 2% increase
            confidence: 0.87,
            prediction_interval: PredictionInterval {
                lower_bound: current_price * 0.98,
                upper_bound: current_price * 1.06,
                confidence_level: 0.95,
            },
            horizon_minutes: 60,
            timestamp: Utc::now(),
            feature_importance: hashmap! {
                "rsi_14".to_string() => 0.23,
                "macd".to_string() => 0.19,
                "price_momentum_5m".to_string() => 0.15,
            },
        }
    }

    pub fn create_trading_signal(symbol: &str, action: SignalAction) -> TradingSignal {
        TradingSignal {
            signal_id: Uuid::new_v4().to_string(),
            symbol: symbol.to_string(),
            action,
            strength: SignalStrength::Strong,
            confidence: 0.89,
            reasoning: "Strong ML signal with technical confirmation".to_string(),
            recommended_quantity: 100,
            suggested_price: 150.50,
            stop_loss: 147.50,
            take_profit: 153.00,
            risk_assessment: RiskAssessment {
                position_risk: 0.025,
                portfolio_impact: 0.05,
                overall_risk_score: 0.32,
            },
            timestamp: Utc::now(),
        }
    }
}
```

---

## Integration Testing Framework

### Service Integration Tests

#### gRPC Service Integration

```rust
use testcontainers::{clients::Cli, images::generic::GenericImage, Container, Docker};
use tonic::{transport::Channel, Request};

#[tokio::test]
async fn test_market_data_to_feature_integration() {
    // Setup test environment
    let docker = Cli::default();
    let postgres = docker.run(testcontainers::images::postgres::Postgres::default());
    let redis = docker.run(testcontainers::images::redis::Redis::default());
    let nats = docker.run(GenericImage::new("nats:latest").with_exposed_port(4222));

    let postgres_url = format!(
        "postgresql://postgres:postgres@localhost:{}/test", 
        postgres.get_host_port_ipv4(5432)
    );
    
    // Start services
    let market_data_service = start_market_data_service(&postgres_url).await;
    let feature_service = start_feature_engineering_service().await;
    let event_bus = setup_event_bus(&nats).await;

    // Test data flow
    let test_symbol = "AAPL";
    let market_data = TestDataFactory::create_market_data(test_symbol, 150.0);
    
    // Publish market data
    market_data_service.publish_data(market_data.clone()).await.unwrap();
    
    // Wait for event propagation (with timeout)
    let features = timeout(
        Duration::from_secs(5),
        wait_for_features(&feature_service, test_symbol)
    ).await.unwrap().unwrap();
    
    // Verify features were calculated correctly
    assert!(!features.features.is_empty());
    assert!(features.features.contains_key("rsi_14"));
    assert!(features.features.contains_key("macd"));
    assert_eq!(features.symbol, test_symbol);
    assert!(features.calculation_time_ms < 100.0); // Performance requirement
    
    // Cleanup
    cleanup_test_environment().await;
}

#[tokio::test]
async fn test_feature_to_prediction_integration() {
    let env = setup_integration_environment().await;
    
    let feature_service = start_feature_engineering_service().await;
    let model_service = start_model_management_service().await;
    
    // Deploy test model
    let model_deployment = deploy_test_model(&model_service).await.unwrap();
    assert_eq!(model_deployment.status, "deployed");
    
    // Calculate features
    let feature_request = FeatureRequest {
        symbol: "AAPL".to_string(),
        indicators: vec![
            IndicatorRequest { name: "rsi".to_string(), period: 14 },
            IndicatorRequest { name: "macd".to_string(), period: 12 },
        ],
    };
    
    let features = feature_service.calculate_features(feature_request).await.unwrap();
    
    // Generate prediction from features
    let prediction_request = PredictionRequest {
        model_id: model_deployment.model_id,
        features: features.features,
        config: PredictionConfig {
            horizon_minutes: 60,
            confidence_threshold: 0.5,
        },
    };
    
    let prediction = model_service.predict(prediction_request).await.unwrap();
    
    // Verify prediction quality
    assert!(prediction.confidence >= 0.5);
    assert!(prediction.inference_time_us < 50_000); // <50ms requirement
    assert!(!prediction.feature_importance.is_empty());
    
    cleanup_integration_environment(env).await;
}
```

#### Event Flow Integration Tests

```rust
#[tokio::test]
async fn test_complete_trading_event_flow() {
    let env = setup_full_system().await;
    
    // Setup event listeners
    let mut market_data_events = subscribe_to_events("market-data-stream").await;
    let mut feature_events = subscribe_to_events("features-stream").await;
    let mut prediction_events = subscribe_to_events("predictions-stream").await;
    let mut trading_events = subscribe_to_events("trading-signals-stream").await;
    
    // Inject market data
    let market_data = TestDataFactory::create_market_data("AAPL", 150.0);
    publish_market_data(market_data.clone()).await.unwrap();
    
    // Verify event cascade
    // 1. Market data event received
    let market_event = timeout(Duration::from_secs(1), market_data_events.next())
        .await.unwrap().unwrap();
    assert_eq!(market_event.symbol, "AAPL");
    
    // 2. Features calculated event received
    let feature_event = timeout(Duration::from_secs(2), feature_events.next())
        .await.unwrap().unwrap();
    assert_eq!(feature_event.correlation_id, market_event.event_id);
    assert!(!feature_event.features.is_empty());
    
    // 3. Prediction generated event received
    let prediction_event = timeout(Duration::from_secs(3), prediction_events.next())
        .await.unwrap().unwrap();
    assert_eq!(prediction_event.correlation_id, feature_event.event_id);
    assert!(prediction_event.confidence > 0.0);
    
    // 4. Trading signal generated (if prediction is strong enough)
    if prediction_event.confidence > 0.8 {
        let trading_event = timeout(Duration::from_secs(1), trading_events.next())
            .await.unwrap().unwrap();
        assert_eq!(trading_event.correlation_id, prediction_event.event_id);
        assert!(matches!(trading_event.signal.action, SignalAction::Buy | SignalAction::Sell));
    }
    
    cleanup_full_system(env).await;
}
```

### Database Integration Testing

```rust
#[tokio::test]
async fn test_time_series_storage_performance() {
    let postgres_container = setup_test_postgres().await;
    let storage = TimeSeriesStorageImpl::new(&postgres_container.connection_string()).await.unwrap();
    
    // Generate large dataset for performance testing
    let test_data: Vec<TimeSeriesPoint> = (0..10_000)
        .map(|i| TimeSeriesPoint {
            timestamp: Utc::now() + Duration::minutes(i),
            symbol: format!("TEST{}", i % 100), // 100 different symbols
            values: hashmap! {
                "price".to_string() => 100.0 + (i as f64 * 0.01),
                "volume".to_string() => 1000.0 + (i as f64 * 10.0),
            },
            metadata: HashMap::new(),
        })
        .collect();
    
    // Test batch write performance
    let start = Instant::now();
    let result = storage.write_batch(test_data.clone()).await;
    let write_duration = start.elapsed();
    
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), test_data.len());
    assert!(write_duration < Duration::from_millis(1000)); // <1s for 10K records
    
    // Test query performance
    let query = TimeSeriesQuery {
        symbols: vec!["TEST1".to_string(), "TEST2".to_string()],
        start_time: Utc::now() - Duration::hours(1),
        end_time: Utc::now(),
        limit: Some(1000),
        aggregation: Some(AggregationType::Minute(1)),
    };
    
    let start = Instant::now();
    let query_result = storage.read_range(query).await;
    let query_duration = start.elapsed();
    
    assert!(query_result.is_ok());
    assert!(query_duration < Duration::from_millis(100)); // <100ms query
    assert!(!query_result.unwrap().is_empty());
}

#[tokio::test] 
async fn test_model_storage_versioning() {
    let storage = setup_test_model_storage().await;
    
    // Store multiple versions of the same model
    let model_v1 = create_test_model_data("lstm-v1");
    let metadata_v1 = ModelMetadata {
        name: "test-model".to_string(),
        version: "1.0.0".to_string(),
        model_type: ModelType::LSTM,
        created_at: Utc::now(),
        trained_at: Utc::now(),
        file_size_bytes: model_v1.len() as u64,
        performance_metrics: PerformanceMetrics {
            accuracy: 0.85,
            precision: 0.83,
            recall: 0.87,
            f1_score: 0.85,
            mse: 0.02,
            mae: 0.015,
            custom_metrics: HashMap::new(),
        },
        tags: hashmap! {
            "environment".to_string() => "test".to_string(),
            "algorithm".to_string() => "lstm".to_string(),
        },
    };
    
    let model_id_v1 = storage.save_model(&model_v1, metadata_v1.clone()).await.unwrap();
    
    // Store version 2.0.0
    let model_v2 = create_test_model_data("lstm-v2");
    let mut metadata_v2 = metadata_v1.clone();
    metadata_v2.version = "2.0.0".to_string();
    metadata_v2.performance_metrics.accuracy = 0.87; // Improved
    
    let model_id_v2 = storage.save_model(&model_v2, metadata_v2.clone()).await.unwrap();
    
    // Test versioning queries
    let models = storage.list_models(Some(ModelFilter {
        name_pattern: Some("test-model".to_string()),
        ..Default::default()
    })).await.unwrap();
    
    assert_eq!(models.len(), 2);
    assert!(models.iter().any(|m| m.version == "1.0.0"));
    assert!(models.iter().any(|m| m.version == "2.0.0"));
    
    // Test loading specific versions
    let (loaded_model_v1, loaded_metadata_v1) = storage.load_model(&model_id_v1).await.unwrap();
    assert_eq!(loaded_model_v1, model_v1);
    assert_eq!(loaded_metadata_v1.version, "1.0.0");
    assert_eq!(loaded_metadata_v1.performance_metrics.accuracy, 0.85);
    
    let (loaded_model_v2, loaded_metadata_v2) = storage.load_model(&model_id_v2).await.unwrap();
    assert_eq!(loaded_model_v2, model_v2);
    assert_eq!(loaded_metadata_v2.version, "2.0.0");
    assert_eq!(loaded_metadata_v2.performance_metrics.accuracy, 0.87);
}
```

---

## Performance Testing Framework

### Load Testing with k6

#### Market Data Processing Load Test

```javascript
import http from 'k6/http';
import ws from 'k6/ws';
import { check, sleep } from 'k6';
import { Counter, Rate, Trend } from 'k6/metrics';

// Custom metrics
const marketDataProcessed = new Counter('market_data_processed');
const processingErrors = new Rate('processing_errors');
const processingTime = new Trend('processing_time_ms');

export let options = {
  stages: [
    { duration: '1m', target: 100 },   // Ramp up to 100 concurrent connections
    { duration: '5m', target: 1000 },  // Scale to 1000 connections
    { duration: '10m', target: 1000 }, // Hold at 1000 for 10 minutes
    { duration: '2m', target: 0 },     // Ramp down
  ],
  thresholds: {
    'processing_time_ms': ['p(95)<10'], // 95% under 10ms
    'processing_errors': ['rate<0.01'], // Error rate under 1%
    'market_data_processed': ['count>500000'], // Process at least 500K events
  },
};

export default function () {
  const wsUrl = 'ws://market-data-service:8080/stream';
  
  const response = ws.connect(wsUrl, {
    tags: { test: 'market_data_load' },
  }, function (socket) {
    
    socket.on('open', function() {
      // Subscribe to market data
      socket.send(JSON.stringify({
        action: 'subscribe',
        symbols: ['AAPL', 'GOOGL', 'MSFT', 'TSLA', 'NVDA']
      }));
    });

    socket.on('message', function (data) {
      const startTime = Date.now();
      
      try {
        const marketData = JSON.parse(data);
        
        // Validate market data structure
        const isValid = check(marketData, {
          'has event_id': (data) => data.event_id !== undefined,
          'has timestamp': (data) => data.timestamp !== undefined,
          'has symbol': (data) => data.symbol !== undefined,
          'has valid price': (data) => data.payload.price > 0,
          'has quality score': (data) => data.quality.overall_score >= 0.5,
        });
        
        if (isValid) {
          marketDataProcessed.add(1);
        } else {
          processingErrors.add(1);
        }
        
        const processTime = Date.now() - startTime;
        processingTime.add(processTime);
        
      } catch (e) {
        processingErrors.add(1);
        console.error('Failed to parse market data:', e);
      }
    });

    socket.on('error', function (e) {
      processingErrors.add(1);
      console.error('WebSocket error:', e);
    });

    // Keep connection alive for test duration
    socket.setTimeout(() => {}, 30000);
  });
  
  sleep(1);
}

export function handleSummary(data) {
  return {
    'market-data-load-test.json': JSON.stringify(data, null, 2),
    'stdout': textSummary(data, { indent: ' ', enableColors: true }),
  };
}
```

#### Model Inference Performance Test

```javascript
import http from 'k6/http';
import { check } from 'k6';
import { Counter, Trend } from 'k6/metrics';

const predictionsProcessed = new Counter('predictions_processed');
const inferenceLatency = new Trend('inference_latency_ms');

export let options = {
  stages: [
    { duration: '30s', target: 50 },
    { duration: '2m', target: 500 },
    { duration: '5m', target: 1000 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    'inference_latency_ms': ['p(95)<50'], // 95% under 50ms
    'http_req_failed': ['rate<0.01'],
    'predictions_processed': ['count>100000'],
  },
};

export default function () {
  const predictionRequest = {
    model_id: 'lstm-v1',
    model_version: '1.2.3',
    features: generateRandomFeatures(),
    config: {
      horizon_minutes: 60,
      confidence_threshold: 0.5,
    },
  };

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
    timeout: '60s',
  };

  const startTime = Date.now();
  const response = http.post('http://model-service:8080/predict', JSON.stringify(predictionRequest), params);
  const latency = Date.now() - startTime;

  const success = check(response, {
    'prediction returned': (r) => r.status === 200,
    'has prediction value': (r) => JSON.parse(r.body).predictions.length > 0,
    'confidence within range': (r) => {
      const body = JSON.parse(r.body);
      return body.confidence >= 0.0 && body.confidence <= 1.0;
    },
    'inference time acceptable': (r) => {
      const body = JSON.parse(r.body);
      return body.inference_time_us < 50000; // <50ms
    },
  });

  if (success) {
    predictionsProcessed.add(1);
  }
  
  inferenceLatency.add(latency);
}

function generateRandomFeatures() {
  return {
    rsi_14: Math.random() * 100,
    macd: Math.random() * 2 - 1,
    bollinger_position: Math.random(),
    volume_ratio: Math.random() * 3,
    price_momentum_5m: Math.random() * 0.1 - 0.05,
    volatility: Math.random() * 0.5,
  };
}
```

### Rust Performance Benchmarks

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use tokio::runtime::Runtime;

fn benchmark_technical_indicators(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("technical_indicators");
    
    // Test different data sizes
    for data_size in [100, 1000, 5000, 10000] {
        let price_data = generate_price_data(data_size);
        
        // Benchmark RSI calculation
        group.bench_with_input(
            BenchmarkId::new("rsi_calculation", data_size),
            &data_size,
            |b, &_size| {
                let rsi_calculator = RSI::new(14);
                b.iter(|| {
                    rt.block_on(async {
                        rsi_calculator.calculate(&price_data).unwrap()
                    })
                });
            },
        );
        
        // Benchmark MACD calculation
        group.bench_with_input(
            BenchmarkId::new("macd_calculation", data_size),
            &data_size,
            |b, &_size| {
                let macd_calculator = MACD::new(12, 26, 9);
                b.iter(|| {
                    rt.block_on(async {
                        macd_calculator.calculate(&price_data).unwrap()
                    })
                });
            },
        );
        
        // Benchmark Bollinger Bands calculation
        group.bench_with_input(
            BenchmarkId::new("bollinger_calculation", data_size),
            &data_size,
            |b, &_size| {
                let bollinger_calculator = BollingerBands::new(20, 2.0);
                b.iter(|| {
                    rt.block_on(async {
                        bollinger_calculator.calculate(&price_data).unwrap()
                    })
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_risk_calculations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("risk_calculations");
    
    let portfolio = create_test_portfolio_with_positions(100);
    let orders = generate_test_orders(10);
    
    group.bench_function("portfolio_risk_assessment", |b| {
        let risk_engine = RiskEngine::new(create_default_risk_limits());
        b.iter(|| {
            rt.block_on(async {
                for order in &orders {
                    risk_engine.validate_order(order, &portfolio).await.unwrap();
                }
            })
        });
    });
    
    group.bench_function("position_correlation_calculation", |b| {
        let correlation_engine = CorrelationEngine::new();
        b.iter(|| {
            rt.block_on(async {
                correlation_engine.calculate_portfolio_correlation(&portfolio).await.unwrap()
            })
        });
    });
    
    group.finish();
}

fn benchmark_data_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    
    let market_data_events: Vec<MarketDataEvent> = (0..1000)
        .map(|i| TestDataFactory::create_market_data(&format!("SYMBOL{}", i), 100.0 + i as f64))
        .collect();
    
    // Benchmark JSON serialization
    group.bench_function("json_serialize_market_data", |b| {
        b.iter(|| {
            serde_json::to_string(&market_data_events).unwrap()
        });
    });
    
    // Benchmark Protocol Buffers serialization  
    group.bench_function("protobuf_serialize_market_data", |b| {
        let proto_events: Vec<proto::MarketDataEvent> = market_data_events
            .iter()
            .map(|e| e.clone().into())
            .collect();
        b.iter(|| {
            proto_events.iter().map(|e| e.encode_to_vec()).collect::<Vec<_>>()
        });
    });
    
    group.finish();
}

criterion_group!(
    benches, 
    benchmark_technical_indicators,
    benchmark_risk_calculations,
    benchmark_data_serialization
);
criterion_main!(benches);
```

---

## End-to-End Testing Framework

### Trading Workflow E2E Tests

```rust
#[tokio::test]
async fn test_complete_automated_trading_workflow() {
    // Setup complete system
    let system = setup_full_neural_trader_system().await;
    
    // Deploy models
    let model_deployment = system.model_service
        .deploy_model(create_test_lstm_model())
        .await.unwrap();
    
    // Configure trading strategy
    system.trading_service
        .enable_strategy("neural_enhanced_momentum", StrategyConfig {
            confidence_threshold: 0.8,
            position_size_percentage: 0.02,
            stop_loss_percentage: 0.05,
            take_profit_percentage: 0.1,
        })
        .await.unwrap();
    
    // Set risk limits
    system.trading_service
        .set_risk_limits(RiskLimits {
            max_position_size: 1000.0,
            max_portfolio_risk: 0.1,
            max_single_position_risk: 0.05,
            max_correlation_risk: 0.3,
        })
        .await.unwrap();
    
    // Inject strong buy signal data sequence
    let strong_buy_sequence = generate_strong_buy_signal_data("AAPL");
    
    for market_data in strong_buy_sequence {
        system.market_data_service
            .publish_data(market_data)
            .await.unwrap();
        
        // Small delay to allow processing
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    // Wait for trading decision (with timeout)
    let trading_decision = timeout(
        Duration::from_secs(30),
        wait_for_trading_decision(&system.event_bus, "AAPL")
    ).await.unwrap().unwrap();
    
    // Verify trading decision
    assert_eq!(trading_decision.symbol, "AAPL");
    assert!(matches!(trading_decision.signal.action, SignalAction::Buy));
    assert!(trading_decision.signal.confidence >= 0.8);
    assert!(trading_decision.risk_assessment.approved);
    
    // Verify order was submitted
    let order_execution = timeout(
        Duration::from_secs(10),
        wait_for_order_execution(&system.event_bus, &trading_decision.signal_id)
    ).await.unwrap().unwrap();
    
    // Verify order execution
    assert_eq!(order_execution.order_info.symbol, "AAPL");
    assert_eq!(order_execution.order_info.side, "buy");
    assert!(order_execution.execution_details.status == "filled" || 
           order_execution.execution_details.status == "partially_filled");
    
    // Verify portfolio update
    let portfolio = system.trading_service
        .get_portfolio()
        .await.unwrap();
    
    let aapl_position = portfolio.positions
        .iter()
        .find(|p| p.symbol == "AAPL")
        .expect("AAPL position should exist");
    
    assert!(aapl_position.quantity > 0.0);
    assert!(aapl_position.market_value > 0.0);
    
    // Verify audit trail
    let audit_logs = system.audit_service
        .get_trading_logs_for_symbol("AAPL", Utc::now() - Duration::hours(1), Utc::now())
        .await.unwrap();
    
    assert!(!audit_logs.is_empty());
    assert!(audit_logs.iter().any(|log| log.event_type == "trading_signal_generated"));
    assert!(audit_logs.iter().any(|log| log.event_type == "order_submitted"));
    assert!(audit_logs.iter().any(|log| log.event_type == "order_executed"));
    
    cleanup_full_system(system).await;
}

#[tokio::test]
async fn test_risk_management_prevents_dangerous_trades() {
    let system = setup_full_neural_trader_system().await;
    
    // Set very restrictive risk limits
    system.trading_service
        .set_risk_limits(RiskLimits {
            max_position_size: 10.0, // Very small
            max_portfolio_risk: 0.01, // Very conservative  
            max_single_position_risk: 0.005,
            max_correlation_risk: 0.1,
        })
        .await.unwrap();
    
    // Try to generate large position signal
    let large_position_signal = TradingSignal {
        symbol: "AAPL".to_string(),
        action: SignalAction::Buy,
        recommended_quantity: 1000, // Exceeds max_position_size
        confidence: 0.95, // High confidence
        // ... other fields
    };
    
    // Attempt to execute the signal
    let execution_result = system.trading_service
        .process_trading_signal(large_position_signal)
        .await;
    
    // Should be rejected by risk management
    assert!(execution_result.is_err() || !execution_result.unwrap().approved);
    
    // Verify no order was placed
    let portfolio = system.trading_service.get_portfolio().await.unwrap();
    assert!(portfolio.positions.is_empty() || 
           !portfolio.positions.iter().any(|p| p.symbol == "AAPL"));
    
    // Verify risk rejection was logged
    let audit_logs = system.audit_service
        .get_risk_rejection_logs(Utc::now() - Duration::minutes(5), Utc::now())
        .await.unwrap();
    
    assert!(!audit_logs.is_empty());
    assert!(audit_logs.iter().any(|log| 
        log.rejection_reason.contains("position size exceeds limit")
    ));
    
    cleanup_full_system(system).await;
}
```

### System Resilience E2E Tests

```rust
#[tokio::test]
async fn test_system_handles_service_failures_gracefully() {
    let mut system = setup_full_neural_trader_system().await;
    
    // Start normal operation
    system.start_market_data_feed("AAPL").await.unwrap();
    
    // Wait for normal processing
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Kill feature engineering service
    system.kill_service("feature-engineering").await;
    
    // System should continue with cached features
    let cached_features = timeout(
        Duration::from_secs(10),
        wait_for_cached_features(&system.event_bus, "AAPL")
    ).await.unwrap().unwrap();
    
    assert!(cached_features.metadata.cache_hit);
    assert!(!cached_features.features.is_empty());
    
    // Restart feature engineering service
    system.restart_service("feature-engineering").await.unwrap();
    
    // Wait for service to recover
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Verify fresh feature calculation
    let fresh_features = timeout(
        Duration::from_secs(10),
        wait_for_fresh_features(&system.event_bus, "AAPL")
    ).await.unwrap().unwrap();
    
    assert!(!fresh_features.metadata.cache_hit);
    assert!(fresh_features.calculation_time_ms < 100.0);
    
    cleanup_full_system(system).await;
}

#[tokio::test]
async fn test_data_provider_failover() {
    let system = setup_neural_trader_with_multiple_providers().await;
    
    // Start with primary provider (Alpaca)
    system.market_data_service
        .set_primary_provider("alpaca")
        .await.unwrap();
    
    // Verify data flow from primary
    let primary_data = wait_for_market_data(&system.event_bus, "AAPL").await.unwrap();
    assert_eq!(primary_data.provider, "alpaca");
    
    // Simulate primary provider failure
    system.simulate_provider_failure("alpaca").await;
    
    // System should automatically failover to secondary (Polygon)
    let failover_data = timeout(
        Duration::from_secs(10),
        wait_for_market_data(&system.event_bus, "AAPL")
    ).await.unwrap().unwrap();
    
    assert_eq!(failover_data.provider, "polygon");
    assert!(failover_data.quality.overall_score >= 0.8); // Still good quality
    
    // Verify failover event was logged
    let system_logs = system.monitoring_service
        .get_system_events(Utc::now() - Duration::minutes(5), Utc::now())
        .await.unwrap();
    
    assert!(system_logs.iter().any(|log| 
        log.event_type == "provider_failover" && 
        log.details.contains("alpaca -> polygon")
    ));
    
    cleanup_full_system(system).await;
}
```

---

## Chaos Engineering Framework

### Fault Injection Testing

```rust
use chaos_monkey::{ChaosScenario, FaultType};

#[tokio::test]
async fn chaos_test_random_service_failures() {
    let system = setup_full_neural_trader_system().await;
    let chaos_config = ChaosConfig {
        duration: Duration::from_minutes(10),
        failure_rate: 0.1, // 10% failure rate
        recovery_time: Duration::from_seconds(30),
        target_services: vec![
            "market-data-service",
            "feature-engineering-service", 
            "model-management-service",
            "trading-service",
        ],
    };
    
    // Start chaos monkey
    let chaos_monkey = ChaosMonkey::new(chaos_config);
    let chaos_handle = tokio::spawn(chaos_monkey.run());
    
    // Run normal trading operations during chaos
    let trading_handle = tokio::spawn(async move {
        for _ in 0..100 {
            // Inject market data
            let market_data = TestDataFactory::create_market_data("AAPL", 150.0);
            let _ = system.market_data_service.publish_data(market_data).await;
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    
    // Let chaos and trading run concurrently
    let (chaos_result, trading_result) = tokio::join!(chaos_handle, trading_handle);
    
    // Verify system remained stable
    assert!(chaos_result.is_ok());
    assert!(trading_result.is_ok());
    
    // Check system metrics
    let metrics = system.monitoring_service.get_system_metrics().await.unwrap();
    
    // System should maintain minimum availability
    assert!(metrics.overall_availability >= 0.95); // 95% minimum during chaos
    assert!(metrics.average_response_time_ms < 100.0);
    assert!(metrics.error_rate < 0.05); // Less than 5% error rate
    
    // Verify automatic recovery occurred
    let recovery_events = system.monitoring_service
        .get_recovery_events(Utc::now() - Duration::minutes(15), Utc::now())
        .await.unwrap();
    
    assert!(!recovery_events.is_empty());
    assert!(recovery_events.iter().all(|event| 
        event.recovery_time_seconds < 60 // All services recovered within 1 minute
    ));
    
    cleanup_full_system(system).await;
}

#[tokio::test]
async fn chaos_test_network_partitions() {
    let system = setup_distributed_neural_trader_system().await;
    
    let partition_scenario = NetworkPartitionScenario {
        duration: Duration::from_minutes(5),
        partition_type: PartitionType::SplitBrain, // Split services into two groups
        affected_services: vec![
            ("market-data-service", "group-a"),
            ("feature-engineering-service", "group-a"), 
            ("model-management-service", "group-b"),
            ("trading-service", "group-b"),
        ],
    };
    
    // Apply network partition
    system.network_chaos.apply_partition(partition_scenario).await;
    
    // During partition, both groups should continue operating
    // but with degraded functionality
    
    // Group A (market data + features) should continue processing
    let market_data = TestDataFactory::create_market_data("AAPL", 150.0);
    let group_a_result = timeout(
        Duration::from_secs(10),
        system.feature_service.calculate_features_from_market_data(market_data)
    ).await;
    
    assert!(group_a_result.is_ok()); // Should work within group
    
    // Group B (models + trading) should use cached data
    let cached_prediction = timeout(
        Duration::from_secs(10),
        system.model_service.get_cached_prediction("AAPL")
    ).await;
    
    assert!(cached_prediction.is_ok()); // Should fallback to cache
    
    // Heal network partition
    system.network_chaos.heal_partition().await;
    
    // Wait for services to reconnect
    tokio::time::sleep(Duration::from_secs(30)).await;
    
    // Verify full functionality restored
    let end_to_end_test = timeout(
        Duration::from_secs(60),
        test_complete_trading_workflow(&system)
    ).await;
    
    assert!(end_to_end_test.is_ok());
    
    cleanup_distributed_system(system).await;
}
```

### Load + Chaos Testing

```rust
#[tokio::test]
async fn stress_test_with_concurrent_chaos() {
    let system = setup_full_neural_trader_system().await;
    
    // Configure high load
    let load_config = LoadConfig {
        concurrent_users: 1000,
        requests_per_second: 10000,
        duration: Duration::from_minutes(15),
        symbols: vec!["AAPL", "GOOGL", "MSFT", "TSLA", "NVDA"],
    };
    
    // Configure chaos
    let chaos_config = ChaosConfig {
        duration: Duration::from_minutes(15),
        failure_rate: 0.15, // Higher failure rate under load
        recovery_time: Duration::from_seconds(45),
        fault_types: vec![
            FaultType::ServiceCrash,
            FaultType::NetworkLatency(Duration::from_millis(500)),
            FaultType::DiskFull,
            FaultType::MemoryExhaustion,
        ],
        target_services: vec![
            "market-data-service",
            "feature-engineering-service",
            "model-management-service", 
            "trading-service",
        ],
    };
    
    // Start load generator
    let load_handle = tokio::spawn(async move {
        let load_generator = LoadGenerator::new(load_config);
        load_generator.run().await
    });
    
    // Start chaos monkey
    let chaos_handle = tokio::spawn(async move {
        let chaos_monkey = ChaosMonkey::new(chaos_config);
        chaos_monkey.run().await
    });
    
    // Monitor system during stress + chaos
    let monitoring_handle = tokio::spawn(async move {
        let mut metrics_collector = MetricsCollector::new();
        
        for _ in 0..900 { // 15 minutes * 60 seconds
            let metrics = system.monitoring_service.get_real_time_metrics().await.unwrap();
            metrics_collector.record(metrics);
            
            // Verify critical invariants
            assert!(metrics.market_data_processing_rate > 5000); // At least 50% of target
            assert!(metrics.prediction_latency_p95_ms < 100); // Allow degraded but usable latency
            assert!(metrics.order_execution_success_rate > 0.95); // 95% success rate minimum
            
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        
        metrics_collector.finalize()
    });
    
    // Wait for all tests to complete
    let (load_result, chaos_result, monitoring_result) = 
        tokio::join!(load_handle, chaos_handle, monitoring_handle);
    
    // Verify results
    assert!(load_result.is_ok());
    assert!(chaos_result.is_ok());
    let final_metrics = monitoring_result.unwrap();
    
    // System should maintain minimum performance under stress + chaos
    assert!(final_metrics.average_availability >= 0.90); // 90% minimum
    assert!(final_metrics.average_throughput >= 7500); // 75% of target throughput
    assert!(final_metrics.max_recovery_time_seconds < 120); // 2 minute max recovery
    
    // Generate stress test report
    let report = StressTestReport {
        test_duration: Duration::from_minutes(15),
        peak_load_rps: 10000,
        chaos_events: chaos_result.unwrap().events,
        performance_metrics: final_metrics,
        success_criteria_met: true,
    };
    
    save_stress_test_report(report).await;
    cleanup_full_system(system).await;
}
```

---

## Quality Gates & CI/CD Integration

### Pre-Merge Quality Gates

```yaml
# .github/workflows/quality-gates.yml
name: Quality Gates

on:
  pull_request:
    branches: [main, develop]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run unit tests
        run: cargo test --all --lib
        env:
          RUST_LOG: debug
      
      - name: Generate coverage report
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --all --out xml --output-dir target/coverage
      
      - name: Check coverage threshold
        run: |
          coverage=$(grep -o 'line-rate="[^"]*"' target/coverage/cobertura.xml | head -1 | sed 's/line-rate="//' | sed 's/"//')
          echo "Coverage: $coverage"
          if (( $(echo "$coverage < 0.90" | bc -l) )); then
            echo "Coverage $coverage is below 90% threshold"
            exit 1
          fi

  integration-tests:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: timescale/timescaledb:latest-pg14
        env:
          POSTGRES_PASSWORD: postgres
      redis:
        image: redis:alpine
      nats:
        image: nats:latest
    
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
      
      - name: Run integration tests
        run: cargo test --all --test '*' 
        env:
          DATABASE_URL: postgresql://postgres:postgres@localhost:5432/test
          REDIS_URL: redis://localhost:6379
          NATS_URL: nats://localhost:4222

  performance-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
      
      - name: Run performance benchmarks
        run: |
          cargo bench --all
          # Check that benchmarks don't regress by more than 10%
          python scripts/check-benchmark-regression.py
      
      - name: Load test critical endpoints
        run: |
          docker-compose -f docker-compose.test.yml up -d
          k6 run tests/load/market-data-load-test.js
          k6 run tests/load/model-inference-load-test.js

  security-scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Security audit
        run: |
          cargo install cargo-audit
          cargo audit --deny warnings
      
      - name: Dependency vulnerability scan
        uses: securecodewarrior/github-action-add-sarif@v1
        with:
          sarif-file: 'security-scan-results.sarif'

  static-analysis:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Clippy analysis
        run: cargo clippy --all -- -D warnings
      
      - name: Format check
        run: cargo fmt --all -- --check

  quality-gate-summary:
    needs: [unit-tests, integration-tests, performance-tests, security-scan, static-analysis]
    runs-on: ubuntu-latest
    if: always()
    steps:
      - name: Quality gate result
        run: |
          echo "Unit Tests: ${{ needs.unit-tests.result }}"
          echo "Integration Tests: ${{ needs.integration-tests.result }}"
          echo "Performance Tests: ${{ needs.performance-tests.result }}"
          echo "Security Scan: ${{ needs.security-scan.result }}"
          echo "Static Analysis: ${{ needs.static-analysis.result }}"
          
          if [[ "${{ needs.unit-tests.result }}" != "success" ]] || \
             [[ "${{ needs.integration-tests.result }}" != "success" ]] || \
             [[ "${{ needs.performance-tests.result }}" != "success" ]] || \
             [[ "${{ needs.security-scan.result }}" != "success" ]] || \
             [[ "${{ needs.static-analysis.result }}" != "success" ]]; then
            echo "Quality gates failed!"
            exit 1
          fi
          
          echo "All quality gates passed! ✅"
```

### Pre-Production Quality Gates

```yaml
# .github/workflows/pre-production-gates.yml
name: Pre-Production Quality Gates

on:
  push:
    branches: [release/*]

jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Deploy to staging
        run: |
          kubectl apply -k deploy/staging/
          kubectl wait --for=condition=available deployment --all --timeout=300s
      
      - name: Run E2E tests
        run: |
          pytest tests/e2e/ -v --junit-xml=e2e-results.xml
        env:
          STAGING_URL: https://staging-neural-trader.example.com
      
      - name: Upload E2E results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: e2e-test-results
          path: e2e-results.xml

  chaos-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run chaos engineering tests
        run: |
          # Deploy to chaos testing environment
          kubectl apply -k deploy/chaos-test/
          
          # Install chaos mesh
          curl -sSL https://mirrors.chaos-mesh.org/v2.4.3/install.sh | bash -s -- --local kind
          
          # Run chaos scenarios
          kubectl apply -f tests/chaos/network-partition.yaml
          kubectl apply -f tests/chaos/pod-failure.yaml
          kubectl apply -f tests/chaos/io-delay.yaml
          
          # Wait and verify system stability
          sleep 600  # 10 minutes of chaos
          
          # Verify system recovered
          python tests/chaos/verify-recovery.py

  load-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run comprehensive load tests
        run: |
          # Start load testing environment
          docker-compose -f docker-compose.load-test.yml up -d
          
          # Run extended load tests (1 hour)
          k6 run --duration=1h tests/load/comprehensive-load-test.js
          
          # Verify SLA compliance
          python scripts/verify-sla-compliance.py load-test-results.json

  security-penetration-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Deploy to security test environment
        run: |
          kubectl apply -k deploy/security-test/
      
      - name: Run penetration tests
        run: |
          # OWASP ZAP automated security testing
          docker run -v $(pwd):/zap/wrk/:rw -t owasp/zap2docker-stable zap-full-scan.py \
            -t https://security-test-neural-trader.example.com \
            -x security-report.xml
      
      - name: Security compliance check
        run: |
          python scripts/check-security-compliance.py security-report.xml

  production-readiness-check:
    needs: [e2e-tests, chaos-tests, load-tests, security-penetration-test]
    runs-on: ubuntu-latest
    steps:
      - name: Generate readiness report
        run: |
          echo "# Production Readiness Report" > readiness-report.md
          echo "" >> readiness-report.md
          echo "## Quality Gate Results" >> readiness-report.md
          echo "- E2E Tests: ${{ needs.e2e-tests.result }}" >> readiness-report.md
          echo "- Chaos Tests: ${{ needs.chaos-tests.result }}" >> readiness-report.md  
          echo "- Load Tests: ${{ needs.load-tests.result }}" >> readiness-report.md
          echo "- Security Tests: ${{ needs.security-penetration-test.result }}" >> readiness-report.md
          
          if [[ "${{ needs.e2e-tests.result }}" == "success" ]] && \
             [[ "${{ needs.chaos-tests.result }}" == "success" ]] && \
             [[ "${{ needs.load-tests.result }}" == "success" ]] && \
             [[ "${{ needs.security-penetration-test.result }}" == "success" ]]; then
            echo "" >> readiness-report.md
            echo "✅ **APPROVED FOR PRODUCTION DEPLOYMENT**" >> readiness-report.md
          else
            echo "" >> readiness-report.md
            echo "❌ **NOT READY FOR PRODUCTION**" >> readiness-report.md
            exit 1
          fi
      
      - name: Upload readiness report
        uses: actions/upload-artifact@v3
        with:
          name: production-readiness-report
          path: readiness-report.md
```

---

## Test Data Management

### Test Data Generation

```rust
pub struct TestDataGenerator {
    rng: StdRng,
}

impl TestDataGenerator {
    pub fn new_with_seed(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }

    pub fn generate_realistic_price_series(
        &mut self, 
        symbol: &str,
        start_price: f64,
        num_points: usize,
        volatility: f64,
    ) -> Vec<PricePoint> {
        let mut prices = Vec::with_capacity(num_points);
        let mut current_price = start_price;
        let start_time = Utc::now() - Duration::minutes(num_points as i64);
        
        for i in 0..num_points {
            // Generate realistic price movement using geometric Brownian motion
            let dt = 1.0 / (252.0 * 24.0 * 60.0); // 1 minute intervals
            let random_shock = self.rng.gen::<f64>() - 0.5;
            let drift = 0.0001; // Small positive drift
            
            let price_change = current_price * (drift * dt + volatility * (dt.sqrt()) * random_shock);
            current_price += price_change;
            
            // Ensure price doesn't go negative
            current_price = current_price.max(0.01);
            
            prices.push(PricePoint {
                timestamp: start_time + Duration::minutes(i as i64),
                symbol: symbol.to_string(),
                price: current_price,
                volume: self.generate_realistic_volume(current_price),
            });
        }
        
        prices
    }

    pub fn generate_market_regime_data(&mut self, regime: MarketRegime) -> Vec<PricePoint> {
        match regime {
            MarketRegime::Trending => {
                self.generate_trending_market_data("AAPL", 150.0, 1000, 0.02, 0.001)
            }
            MarketRegime::Sideways => {
                self.generate_sideways_market_data("AAPL", 150.0, 1000, 0.01)
            }
            MarketRegime::Volatile => {
                self.generate_volatile_market_data("AAPL", 150.0, 1000, 0.05)
            }
        }
    }

    fn generate_trending_market_data(
        &mut self,
        symbol: &str,
        start_price: f64,
        num_points: usize,
        volatility: f64,
        trend_strength: f64,
    ) -> Vec<PricePoint> {
        let mut prices = Vec::with_capacity(num_points);
        let mut current_price = start_price;
        let start_time = Utc::now() - Duration::minutes(num_points as i64);
        
        for i in 0..num_points {
            // Add trend component
            let trend_component = trend_strength * (i as f64 / num_points as f64);
            
            // Add noise
            let noise = self.rng.gen_range(-volatility..volatility);
            
            current_price = start_price * (1.0 + trend_component + noise);
            current_price = current_price.max(0.01);
            
            prices.push(PricePoint {
                timestamp: start_time + Duration::minutes(i as i64),
                symbol: symbol.to_string(), 
                price: current_price,
                volume: self.generate_realistic_volume(current_price),
            });
        }
        
        prices
    }

    fn generate_realistic_volume(&mut self, price: f64) -> f64 {
        // Volume tends to be higher during price movements
        let base_volume = 1_000_000.0;
        let price_impact = (price / 100.0).ln().abs() * 100_000.0;
        let random_factor = self.rng.gen_range(0.5..1.5);
        
        (base_volume + price_impact) * random_factor
    }
}

pub enum MarketRegime {
    Trending,
    Sideways, 
    Volatile,
}
```

### Test Environment Management

```rust
pub struct TestEnvironmentManager {
    containers: HashMap<String, Container<'static, GenericImage>>,
    docker: Cli,
}

impl TestEnvironmentManager {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
            docker: Cli::default(),
        }
    }

    pub async fn setup_complete_environment(&mut self) -> Result<TestEnvironment, TestError> {
        // Start all required infrastructure
        let postgres = self.start_postgres().await?;
        let redis = self.start_redis().await?;
        let nats = self.start_nats().await?;
        let prometheus = self.start_prometheus().await?;
        
        // Wait for services to be ready
        self.wait_for_service_health(&postgres, 5432).await?;
        self.wait_for_service_health(&redis, 6379).await?;
        self.wait_for_service_health(&nats, 4222).await?;
        
        // Initialize databases and schemas
        self.initialize_test_schema(&postgres).await?;
        self.initialize_redis_streams(&redis).await?;
        
        Ok(TestEnvironment {
            postgres_url: self.get_connection_string(&postgres, "postgresql", "test"),
            redis_url: self.get_connection_string(&redis, "redis", ""),
            nats_url: self.get_connection_string(&nats, "nats", ""),
            prometheus_url: self.get_connection_string(&prometheus, "http", ""),
        })
    }

    async fn start_postgres(&mut self) -> Result<String, TestError> {
        let postgres = self.docker.run(
            testcontainers::images::postgres::Postgres::default()
                .with_env_var("POSTGRES_DB", "test")
                .with_env_var("POSTGRES_USER", "test")
                .with_env_var("POSTGRES_PASSWORD", "test")
        );
        
        let container_id = postgres.id().to_string();
        self.containers.insert("postgres".to_string(), postgres);
        Ok(container_id)
    }

    async fn initialize_test_schema(&self, container_id: &str) -> Result<(), TestError> {
        let connection_string = self.get_postgres_connection_string(container_id);
        let pool = PgPool::connect(&connection_string).await?;
        
        // Run migrations
        sqlx::migrate!("./migrations").run(&pool).await?;
        
        // Insert test data
        sqlx::query!(
            r#"
            INSERT INTO symbols (symbol, name, sector, market_cap) 
            VALUES 
                ('AAPL', 'Apple Inc', 'Technology', 3000000000000),
                ('GOOGL', 'Alphabet Inc', 'Technology', 2000000000000),
                ('MSFT', 'Microsoft Corp', 'Technology', 2800000000000)
            "#
        )
        .execute(&pool)
        .await?;
        
        Ok(())
    }

    pub async fn cleanup(&mut self) {
        // Stop all containers
        for (name, container) in self.containers.drain() {
            tracing::info!("Stopping container: {}", name);
            // Container is automatically stopped when dropped
        }
    }
}

// Helper for managing test lifecycle
#[macro_export]
macro_rules! test_with_environment {
    ($test_name:ident, $test_fn:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let mut env_manager = TestEnvironmentManager::new();
            let env = env_manager.setup_complete_environment().await
                .expect("Failed to setup test environment");
            
            let result = std::panic::AssertUnwindSafe($test_fn(env))
                .catch_unwind()
                .await;
            
            env_manager.cleanup().await;
            
            match result {
                Ok(Ok(())) => (),
                Ok(Err(e)) => panic!("Test failed: {:?}", e),
                Err(e) => panic!("Test panicked: {:?}", e),
            }
        }
    };
}
```

---

## Conclusion

This comprehensive testing strategy ensures Neural Trader V2 will be built with quality and reliability from the ground up. The multi-layered testing approach provides confidence that the system will perform correctly under all conditions.

### Key Benefits

1. **90% Test Coverage**: Comprehensive testing across all layers
2. **Performance Validation**: Every component benchmarked and validated  
3. **Chaos Engineering**: Built-in fault tolerance testing
4. **Quality Gates**: Automated quality enforcement in CI/CD
5. **Real-world Testing**: Realistic test data and scenarios

### Success Metrics

- **Zero production bugs** in critical trading paths
- **99.9% availability** during market hours
- **Sub-50ms latency** for all critical operations
- **Automated deployment** with confidence
- **Comprehensive coverage** of all failure scenarios

This testing strategy provides the foundation for building a production-ready, enterprise-grade trading system with the quality standards required for financial applications.

---

**Next Phase**: [Clean Architecture Implementation](clean-architecture.md) - Define internal service structure and patterns