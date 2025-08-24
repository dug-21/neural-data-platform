# Rust Binary Separation Architecture (CORRECTED)

## Overview

This document details the CORRECT architecture: THREE RUST BINARIES with Rust workspace structure. NO layers within a single binary - instead, clear binary separation with embedded ruv-FANN and DAA Coordinators.

## Target Workspace Structure (Correct Architecture)

### Rust Workspace with 3 Binaries
```
Cargo.toml (workspace root)
├── neural-core/          # Shared library
│   ├── src/
│   │   ├── types.rs      # Common data types
│   │   ├── traits.rs     # Common interfaces
│   │   ├── events.rs     # Redis Streams client
│   │   └── ruv_fann.rs   # ruv-FANN integration
│   └── Cargo.toml
│
├── neural-ml-ops/        # ML training binary
│   ├── src/
│   │   ├── main.rs       # Training pipeline main
│   │   ├── trainer.rs    # ruv-FANN training
│   │   ├── features.rs   # Feature engineering
│   │   └── registry.rs   # Model storage
│   └── Cargo.toml
│
├── neural-trading/       # Trading execution binary
│   ├── src/
│   │   ├── main.rs       # Trading main with DAA
│   │   ├── daa.rs        # DAA Coordinator
│   │   ├── inference.rs  # Embedded ruv-FANN inference
│   │   ├── execution.rs  # Order execution
│   │   └── market.rs     # Market data processing
│   └── Cargo.toml
│
└── config-store/         # Separate gRPC service (existing)
    ├── src/main.rs       # Config storage service
    └── Cargo.toml
```

## Binary Interaction Architecture

### Binary Communication Pattern
```
┌─────────────────────────────────────────┐
│           NEURAL-ML-OPS                 │
│          (Training Binary)              │
├─────────────────────────────────────────┤
│ • Feature Engineering (Rust)            │
│ • ruv-FANN Model Training               │
│ • Model Registry (config-store)         │
│ • Event Publishing (Redis Streams)      │
│ • NO DAA Coordinator                    │
│ • NO Inference Engine                   │
└─────────────────────────────────────────┘
                    ↓
          [Redis Streams Events]
                    ↓
┌─────────────────────────────────────────┐
│          NEURAL-TRADING                 │
│         (Execution Binary)              │
├─────────────────────────────────────────┤
│ • DAA Coordinator (Decision Making)     │
│ • Embedded ruv-FANN Inference          │
│ • Market Data Processing                │
│ • Order Execution (Alpaca)             │
│ • Event Subscription (Redis Streams)   │
│ • NO Training                           │
└─────────────────────────────────────────┘
                    ↑
          [Shared neural-core library]
                    ↑
┌─────────────────────────────────────────┐
│            NEURAL-CORE                  │
│           (Shared Library)              │
├─────────────────────────────────────────┤
│ • Common Data Types                     │
│ • Event Streaming Traits               │
│ • ruv-FANN BaseModel<T> Integration     │
│ • Redis Streams Client                  │
│ • Serialization/Deserialization        │
└─────────────────────────────────────────┘

Dependencies: Both binaries depend on neural-core
```

## Binary Definitions

### 1. neural-core (Shared Library)

```rust
// neural-core/src/types.rs
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use ruv_fann::BaseModel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: Symbol,
    pub action: TradeAction,
    pub confidence: f64,
    pub timestamp: DateTime<Utc>,
    pub model_id: ModelId,
    pub reasoning: String, // DAA reasoning
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureVector {
    pub symbol: Symbol,
    pub timestamp: DateTime<Utc>,
    pub features: Vec<f64>,
    pub metadata: FeatureMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUpdateEvent {
    pub model_id: ModelId,
    pub version: String,
    pub config_store_path: String,
    pub performance_metrics: ModelMetrics,
}

impl TradingSignal {
    pub fn new(symbol: Symbol, action: TradeAction, confidence: f64) -> Result<Self, DomainError> {
        let confidence = Confidence::new(confidence)?;
        Ok(Self {
            symbol,
            action,
            confidence,
            timestamp: Utc::now(),
        })
    }
    
    pub fn should_execute(&self, risk_limits: &RiskLimits) -> bool {
        self.confidence.value() >= risk_limits.min_confidence
            && risk_limits.can_execute(&self.symbol)
    }
}

// Value Objects
#[derive(Debug, Clone)]
pub struct Confidence(f64);

impl Confidence {
    pub fn new(value: f64) -> Result<Self, DomainError> {
        if value < 0.0 || value > 1.0 {
            return Err(DomainError::InvalidConfidence(value));
        }
        Ok(Self(value))
    }
    
    pub fn value(&self) -> f64 {
        self.0
    }
}

// Domain Services
pub trait SignalGenerator {
    fn generate(&self, market_data: &MarketData) -> Result<TradingSignal, DomainError>;
}

pub trait RiskValidator {
    fn validate(&self, signal: &TradingSignal) -> Result<(), RiskViolation>;
}
```

### 2. neural-ml-ops (Training Binary)

```rust
// neural-ml-ops/src/main.rs
use neural_core::{FeatureVector, ModelUpdateEvent, RedisStreamPublisher};
use ruv_fann::BaseModel;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut trainer = MLOpsTrainer::new().await?;
    
    // Training pipeline
    trainer.run_training_pipeline().await?;
    
    // Feature computation pipeline
    trainer.run_feature_pipeline().await?;
    
    Ok(())
}

pub struct MLOpsTrainer {
    feature_engine: FeatureEngine,
    model_trainer: FANNModelTrainer,
    model_registry: ConfigStoreRegistry,
    event_publisher: RedisStreamPublisher,
    
    // NO DAA Coordinator (only in neural-trading)
    // NO inference engine (only in neural-trading)
}

impl MLOpsTrainer {
    pub async fn train_and_deploy_model(&mut self, config: TrainingConfig) -> Result<()> {
        // 1. Prepare training data
        let training_data = self.feature_engine.prepare_training_data(&config).await?;
        
        // 2. Train ruv-FANN model
        let model = self.model_trainer.train_fann_model(&training_data).await?;
        
        // 3. Store in config-store
        let model_id = self.model_registry.store_model(model, &config).await?;
        
        // 4. Publish update event to neural-trading
        self.event_publisher.publish_model_update(ModelUpdateEvent {
            model_id,
            version: config.version,
            config_store_path: format!("/models/{}", model_id),
            performance_metrics: training_result.metrics,
        }).await?;
        
        Ok(())
    }
}

impl<R, G, E, S> GenerateTradingSignalUseCase<R, G, E, S>
where
    R: MarketDataRepository,
    G: SignalGenerator,
    E: EventPublisher,
    S: SignalRepository,
{
    pub async fn execute(&self, request: SignalRequest) -> Result<SignalResponse, ApplicationError> {
        // 1. Fetch market data
        let market_data = self.market_repo
            .get_latest(&request.symbol)
            .await
            .map_err(ApplicationError::Repository)?;
        
        // 2. Generate signal using domain logic
        let signal = self.signal_generator
            .generate(&market_data)
            .map_err(ApplicationError::Domain)?;
        
        // 3. Persist signal
        self.signal_repo
            .save(&signal)
            .await
            .map_err(ApplicationError::Repository)?;
        
        // 4. Publish event
        self.event_publisher
            .publish(SignalGeneratedEvent::from(&signal))
            .await
            .map_err(ApplicationError::EventPublishing)?;
        
        Ok(SignalResponse::from(signal))
    }
}

// Application Services
pub struct TradingApplicationService {
    generate_signal: GenerateTradingSignalUseCase,
    execute_trade: ExecuteTradeUseCase,
    calculate_risk: CalculateRiskUseCase,
}

impl TradingApplicationService {
    pub fn new(/* dependencies */) -> Self {
        // Wire up use cases
    }
}
```

### 3. neural-trading (Execution Binary)

```rust
// neural-trading/src/main.rs
use neural_core::{TradingSignal, FeatureVector, ModelUpdateEvent};
use neural_core::{RedisStreamConsumer, RedisStreamPublisher};
use ruv_fann::BaseModel;
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut trading_engine = TradingEngine::new().await?;
    
    // Main trading loop with DAA coordination
    trading_engine.run().await?;
    
    Ok(())
}

pub struct TradingEngine {
    // DAA Coordinator (ONLY in domain binaries)
    daa_coordinator: DAACoordinator,
    
    // Embedded ruv-FANN inference
    model_cache: HashMap<ModelId, BaseModel<f64>>,
    inference_engine: EmbeddedFANNInference,
    
    // Event handling
    feature_subscriber: RedisStreamConsumer,
    model_subscriber: RedisStreamConsumer,
    signal_publisher: RedisStreamPublisher,
    
    // Trading execution
    order_executor: AlpacaOrderExecutor,
    market_data_processor: MarketDataProcessor,
}

impl TradingEngine {
    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                // Features from neural-ml-ops
                features = self.feature_subscriber.next() => {
                    if let Some(feature_event) = features {
                        self.process_features(feature_event).await?;
                    }
                }
                
                // Model updates from neural-ml-ops
                model_update = self.model_subscriber.next() => {
                    if let Some(update) = model_update {
                        self.hot_reload_model(update).await?;
                    }
                }
            }
        }
    }
    
    async fn process_features(&mut self, features: FeatureVector) -> Result<()> {
        // 1. Embedded ruv-FANN inference (< 1ms, no network calls)
        let model = self.model_cache.get(&features.model_id).unwrap();
        let prediction = self.inference_engine.predict(model, &features.features)?;
        
        // 2. DAA Coordinator makes trading decision
        let decision = self.daa_coordinator.coordinate_trading_decision(
            prediction,
            features.symbol,
            self.get_market_context()
        )?;
        
        // 3. Execute trading action if warranted
        if let Some(action) = decision {
            self.order_executor.execute(action).await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl SignalRepository for PostgresSignalRepository {
    async fn save(&self, signal: &TradingSignal) -> Result<(), RepositoryError> {
        sqlx::query!(
            r#"
            INSERT INTO trading_signals (symbol, action, confidence, timestamp)
            VALUES ($1, $2, $3, $4)
            "#,
            signal.symbol().as_str(),
            signal.action().as_str(),
            signal.confidence().value(),
            signal.timestamp()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    async fn find_by_id(&self, id: Uuid) -> Result<Option<TradingSignal>, RepositoryError> {
        // Implementation
    }
}

// src/infrastructure/config_store/client.rs
use crate::config_store_proto::{ConfigStoreServiceClient, GetConfigRequest};

pub struct ConfigStoreAdapter {
    client: ConfigStoreServiceClient<tonic::transport::Channel>,
}

impl ConfigStoreAdapter {
    pub async fn get_trading_config(&self) -> Result<TradingConfig, ConfigError> {
        let request = GetConfigRequest {
            namespace_path: "/neural-trader/trading".to_string(),
            key: "config".to_string(),
            ..Default::default()
        };
        
        let response = self.client
            .get_config(request)
            .await
            .map_err(|e| ConfigError::Grpc(e))?;
        
        let config: TradingConfig = serde_json::from_str(&response.into_inner().value)
            .map_err(|e| ConfigError::Deserialization(e))?;
        
        Ok(config)
    }
}

// src/infrastructure/external/alpaca_adapter.rs
use crate::domain::market::{Order, OrderExecutor};

pub struct AlpacaOrderExecutor {
    client: AlpacaClient,
    config: AlpacaConfig,
}

#[async_trait]
impl OrderExecutor for AlpacaOrderExecutor {
    async fn execute(&self, order: Order) -> Result<ExecutionResult, ExecutionError> {
        let alpaca_order = self.map_to_alpaca_order(order);
        let result = self.client.place_order(alpaca_order).await?;
        Ok(self.map_to_execution_result(result))
    }
}
```

### 4. Presentation Layer (APIs)

```rust
// src/presentation/grpc/trading_service.rs
use tonic::{Request, Response, Status};
use crate::proto::trading::{
    trading_service_server::TradingService,
    GenerateSignalRequest,
    GenerateSignalResponse,
};
use crate::application::TradingApplicationService;

pub struct TradingServiceImpl {
    app_service: TradingApplicationService,
}

#[tonic::async_trait]
impl TradingService for TradingServiceImpl {
    async fn generate_signal(
        &self,
        request: Request<GenerateSignalRequest>,
    ) -> Result<Response<GenerateSignalResponse>, Status> {
        let req = request.into_inner();
        
        // Map gRPC request to application request
        let app_request = SignalRequest {
            symbol: req.symbol.parse().map_err(|e| Status::invalid_argument(e))?,
            parameters: self.map_parameters(req.parameters),
        };
        
        // Execute use case
        let result = self.app_service
            .generate_signal(app_request)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        
        // Map response
        let response = GenerateSignalResponse {
            signal: Some(self.map_to_proto_signal(result.signal)),
            metadata: self.map_metadata(result.metadata),
        };
        
        Ok(Response::new(response))
    }
}

// src/presentation/rest/health.rs
use axum::{response::Json, http::StatusCode};
use serde_json::json;

pub async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    // Check all layer health
    Ok(Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
```

## Dependency Injection

### Wire Everything Together
```rust
// src/main.rs
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize infrastructure
    let config_store = Arc::new(ConfigStoreAdapter::new().await?);
    let trading_config = config_store.get_trading_config().await?;
    
    let db_pool = PgPool::connect(&trading_config.database_url).await?;
    let redis_client = redis::Client::open(trading_config.redis_url)?;
    
    // Create repositories
    let signal_repo = Arc::new(PostgresSignalRepository::new(db_pool.clone()));
    let market_repo = Arc::new(RedisMarketDataRepository::new(redis_client.clone()));
    
    // Create domain services
    let signal_generator = Arc::new(NeuralSignalGenerator::new(
        config_store.get_neural_config().await?
    ));
    let risk_validator = Arc::new(DefaultRiskValidator::new(
        config_store.get_risk_config().await?
    ));
    
    // Create event publisher
    let event_publisher = Arc::new(RedisEventPublisher::new(redis_client));
    
    // Wire up use cases
    let generate_signal_use_case = GenerateTradingSignalUseCase::new(
        market_repo.clone(),
        signal_generator.clone(),
        event_publisher.clone(),
        signal_repo.clone(),
    );
    
    // Create application service
    let app_service = TradingApplicationService::new(
        generate_signal_use_case,
        // ... other use cases
    );
    
    // Start gRPC server
    let trading_service = TradingServiceImpl::new(app_service);
    
    Server::builder()
        .add_service(TradingServiceServer::new(trading_service))
        .serve("[::1]:50051".parse()?)
        .await?;
    
    Ok(())
}
```

## Testing Strategy

### Domain Layer Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_trading_signal_creation() {
        let signal = TradingSignal::new(
            Symbol::new("AAPL").unwrap(),
            TradeAction::Buy,
            0.85,
        );
        
        assert!(signal.is_ok());
        let signal = signal.unwrap();
        assert_eq!(signal.confidence().value(), 0.85);
    }
    
    #[test]
    fn test_invalid_confidence() {
        let signal = TradingSignal::new(
            Symbol::new("AAPL").unwrap(),
            TradeAction::Buy,
            1.5, // Invalid
        );
        
        assert!(matches!(signal, Err(DomainError::InvalidConfidence(_))));
    }
}
```

### Application Layer Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    
    mock! {
        MarketRepo {}
        
        #[async_trait]
        impl MarketDataRepository for MarketRepo {
            async fn get_latest(&self, symbol: &Symbol) -> Result<MarketData, RepositoryError>;
        }
    }
    
    #[tokio::test]
    async fn test_generate_signal_use_case() {
        let mut mock_market_repo = MockMarketRepo::new();
        mock_market_repo
            .expect_get_latest()
            .returning(|_| Ok(MarketData::default()));
        
        let use_case = GenerateTradingSignalUseCase::new(
            mock_market_repo,
            MockSignalGenerator::new(),
            MockEventPublisher::new(),
            MockSignalRepository::new(),
        );
        
        let request = SignalRequest {
            symbol: Symbol::new("AAPL").unwrap(),
            parameters: Default::default(),
        };
        
        let result = use_case.execute(request).await;
        assert!(result.is_ok());
    }
}
```

### Infrastructure Layer Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::{clients, images::postgres::Postgres};
    
    #[tokio::test]
    async fn test_postgres_signal_repository() {
        let docker = clients::Cli::default();
        let postgres = docker.run(Postgres::default());
        
        let connection_string = format!(
            "postgres://postgres:postgres@localhost:{}",
            postgres.get_host_port(5432)
        );
        
        let pool = PgPool::connect(&connection_string).await.unwrap();
        let repo = PostgresSignalRepository::new(pool);
        
        let signal = TradingSignal::new(
            Symbol::new("AAPL").unwrap(),
            TradeAction::Buy,
            0.85,
        ).unwrap();
        
        let result = repo.save(&signal).await;
        assert!(result.is_ok());
    }
}
```

## Refactoring Steps

### Step 1: Create Layer Structure
```bash
mkdir -p src/{domain,application,infrastructure,presentation}
mkdir -p src/domain/{trading,neural,market,risk}
mkdir -p src/application/{use_cases,ports,services}
mkdir -p src/infrastructure/{persistence,config_store,external,events}
mkdir -p src/presentation/{grpc,rest,websocket}
```

### Step 2: Extract Domain Entities
1. Identify pure business logic
2. Create value objects
3. Define domain services
4. Remove all external dependencies

### Step 3: Build Application Layer
1. Define use cases
2. Create port interfaces
3. Implement service orchestration
4. Add application error handling

### Step 4: Refactor Infrastructure
1. Implement repository adapters
2. Create external service adapters
3. Add config-store client
4. Implement event publishers

### Step 5: Update Presentation
1. Refactor gRPC services
2. Update REST endpoints
3. Add WebSocket handlers
4. Implement error mapping

## Benefits

### Testability
- Each layer can be tested independently
- Mock implementations for all interfaces
- No external dependencies in tests
- Fast unit test execution

### Maintainability
- Clear separation of concerns
- Single responsibility principle
- Easy to understand and modify
- Reduced coupling

### Flexibility
- Easy to swap implementations
- Technology agnostic domain
- Adaptable to changes
- Extensible architecture

## Conclusion

This layer separation provides:
- **Clean boundaries** between concerns
- **Testable components** at every level
- **Flexible architecture** for future changes
- **Maintainable codebase** with clear responsibilities