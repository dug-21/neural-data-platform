# Rust Layer Separation Architecture

## Overview

This document details the architectural refactoring of the Neural Trader Rust application (`/src`) into clean, testable layers following Domain-Driven Design and Clean Architecture principles.

## Current State Analysis

### Existing Structure Problems
```
/src/
├── neural/           # Mixed ML logic and infrastructure
├── action_layer/     # Trading + external APIs mixed
├── features/         # Feature extraction + domain logic
├── monitoring/       # Cross-cutting concerns
├── adapters/         # External integrations
├── config/          # Configuration scattered
└── main.rs          # Monolithic initialization
```

**Issues:**
- Domain logic mixed with infrastructure
- Direct dependencies on external services
- Configuration via environment variables
- Difficult to test in isolation
- Circular dependencies

## Target Architecture

### Layer Structure
```
┌────────────────────────────────────────────┐
│          Presentation Layer                │
│    (gRPC Services, REST APIs, WebSocket)   │
├────────────────────────────────────────────┤
│          Application Layer                 │
│    (Use Cases, Service Orchestration)      │
├────────────────────────────────────────────┤
│            Domain Layer                    │
│    (Entities, Value Objects, Domain Logic) │
├────────────────────────────────────────────┤
│         Infrastructure Layer               │
│    (Repositories, External Services, DB)   │
└────────────────────────────────────────────┘

Dependencies: ↓ (inward only)
```

## Layer Definitions

### 1. Domain Layer (Core Business Logic)

```rust
// src/domain/mod.rs
pub mod trading;
pub mod neural;
pub mod risk;
pub mod market;

// src/domain/trading/signal.rs
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    symbol: Symbol,
    action: TradeAction,
    confidence: Confidence,
    timestamp: DateTime<Utc>,
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

### 2. Application Layer (Use Cases)

```rust
// src/application/use_cases/generate_trading_signal.rs
use crate::domain::trading::{TradingSignal, SignalGenerator};
use crate::domain::market::MarketDataRepository;
use crate::application::ports::{EventPublisher, SignalRepository};

pub struct GenerateTradingSignalUseCase<R, G, E, S>
where
    R: MarketDataRepository,
    G: SignalGenerator,
    E: EventPublisher,
    S: SignalRepository,
{
    market_repo: R,
    signal_generator: G,
    event_publisher: E,
    signal_repo: S,
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

### 3. Infrastructure Layer (External Integrations)

```rust
// src/infrastructure/persistence/postgres_signal_repository.rs
use async_trait::async_trait;
use sqlx::PgPool;
use crate::domain::trading::TradingSignal;
use crate::application::ports::SignalRepository;

pub struct PostgresSignalRepository {
    pool: PgPool,
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