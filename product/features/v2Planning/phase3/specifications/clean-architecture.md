# Clean Architecture for Neural Trader V2

## Overview

Neural Trader V2 follows Clean Architecture principles with hexagonal/onion architecture patterns, ensuring high testability, maintainability, and clear separation of concerns. This greenfield implementation prioritizes dependency inversion and domain-centric design.

## Architecture Layers

### 1. Domain Layer (Core)
```
src/domain/
├── entities/           # Business entities with minimal dependencies
├── value_objects/      # Immutable value types
├── services/          # Domain services with business logic
├── repositories/      # Abstract repository contracts
├── events/           # Domain events
└── errors/           # Domain-specific error types
```

**Key Principles:**
- No external dependencies
- Pure business logic
- Framework-agnostic
- 100% unit testable

```rust
// Example: Domain Entity
#[derive(Debug, Clone, PartialEq)]
pub struct TradingSignal {
    id: SignalId,
    symbol: Symbol,
    signal_type: SignalType,
    confidence: Confidence,
    timestamp: Timestamp,
    metadata: SignalMetadata,
}

impl TradingSignal {
    pub fn new(
        symbol: Symbol,
        signal_type: SignalType,
        confidence: Confidence,
    ) -> Result<Self, DomainError> {
        // Domain validation logic
        if confidence.value() < 0.5 {
            return Err(DomainError::InvalidConfidence);
        }
        
        Ok(Self {
            id: SignalId::generate(),
            symbol,
            signal_type,
            confidence,
            timestamp: Timestamp::now(),
            metadata: SignalMetadata::default(),
        })
    }
}
```

### 2. Application Layer
```
src/application/
├── use_cases/         # Application use cases
├── commands/          # Command handlers
├── queries/          # Query handlers
├── dto/              # Data transfer objects
└── ports/            # Application ports (interfaces)
```

**Responsibilities:**
- Orchestrate domain objects
- Handle application-specific business rules
- Define ports for external dependencies
- Coordinate transactions

```rust
// Example: Use Case Implementation
#[async_trait]
pub trait ProcessMarketDataUseCase {
    async fn execute(&self, command: ProcessMarketDataCommand) 
        -> Result<ProcessMarketDataResponse, ApplicationError>;
}

pub struct ProcessMarketDataUseCaseImpl<R, E, N> {
    market_data_repo: R,
    event_publisher: E,
    neural_analyzer: N,
}

impl<R, E, N> ProcessMarketDataUseCaseImpl<R, E, N>
where
    R: MarketDataRepository + Send + Sync,
    E: EventPublisher + Send + Sync,
    N: NeuralAnalyzer + Send + Sync,
{
    pub fn new(
        market_data_repo: R,
        event_publisher: E,
        neural_analyzer: N,
    ) -> Self {
        Self {
            market_data_repo,
            event_publisher,
            neural_analyzer,
        }
    }
}

#[async_trait]
impl<R, E, N> ProcessMarketDataUseCase for ProcessMarketDataUseCaseImpl<R, E, N>
where
    R: MarketDataRepository + Send + Sync,
    E: EventPublisher + Send + Sync,
    N: NeuralAnalyzer + Send + Sync,
{
    async fn execute(&self, command: ProcessMarketDataCommand) 
        -> Result<ProcessMarketDataResponse, ApplicationError> {
        // 1. Validate input
        command.validate()?;
        
        // 2. Process through neural analyzer
        let analysis = self.neural_analyzer.analyze(command.data).await?;
        
        // 3. Store results
        let stored = self.market_data_repo.store(analysis).await?;
        
        // 4. Publish events
        self.event_publisher.publish(
            MarketDataProcessedEvent::new(stored.id, stored.analysis)
        ).await?;
        
        Ok(ProcessMarketDataResponse::from(stored))
    }
}
```

### 3. Infrastructure Layer
```
src/infrastructure/
├── repositories/      # Concrete repository implementations
├── adapters/         # External service adapters
├── config/           # Configuration management
├── database/         # Database-specific code
├── messaging/        # Message broker implementations
├── monitoring/       # Observability implementations
└── external/         # Third-party integrations
```

**Characteristics:**
- Implements application ports
- Handles external dependencies
- Framework-specific code
- Infrastructure concerns

```rust
// Example: Repository Implementation
pub struct PostgresMarketDataRepository {
    pool: PgPool,
    metrics: Arc<RepositoryMetrics>,
}

#[async_trait]
impl MarketDataRepository for PostgresMarketDataRepository {
    async fn store(&self, data: ProcessedMarketData) 
        -> Result<StoredMarketData, RepositoryError> {
        let timer = self.metrics.store_duration.start_timer();
        
        let result = sqlx::query!(
            r#"
            INSERT INTO market_data (id, symbol, price, volume, timestamp, neural_analysis)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
            data.id.as_uuid(),
            data.symbol.as_str(),
            data.price.as_decimal(),
            data.volume.as_i64(),
            data.timestamp.as_datetime(),
            serde_json::to_value(data.neural_analysis)?
        )
        .fetch_one(&self.pool)
        .await
        .map_err(RepositoryError::Database)?;
        
        timer.observe_duration();
        self.metrics.stores_total.inc();
        
        Ok(StoredMarketData::from_row(result))
    }
    
    async fn find_by_symbol(&self, symbol: Symbol, timeframe: TimeFrame) 
        -> Result<Vec<StoredMarketData>, RepositoryError> {
        // Implementation with proper error handling and metrics
        todo!("Implement query logic")
    }
}
```

### 4. Presentation Layer
```
src/presentation/
├── grpc/             # gRPC service implementations
├── rest/             # REST API handlers
├── websocket/        # WebSocket handlers
├── cli/              # Command-line interface
└── dto/              # Presentation DTOs
```

**Responsibilities:**
- Handle external requests
- Convert between external and internal formats
- Manage authentication/authorization
- API versioning

```rust
// Example: gRPC Service Implementation
pub struct MarketDataGrpcService<U> {
    process_use_case: U,
    metrics: Arc<GrpcMetrics>,
}

#[tonic::async_trait]
impl<U> market_data_service_server::MarketDataService for MarketDataGrpcService<U>
where
    U: ProcessMarketDataUseCase + Send + Sync + 'static,
{
    async fn process_market_data(
        &self,
        request: Request<ProcessMarketDataRequest>,
    ) -> Result<Response<ProcessMarketDataResponse>, Status> {
        let timer = self.metrics.request_duration
            .with_label_values(&["process_market_data"])
            .start_timer();
        
        let command = request.into_inner().try_into()
            .map_err(|e| Status::invalid_argument(format!("Invalid request: {}", e)))?;
        
        let result = self.process_use_case.execute(command).await
            .map_err(|e| {
                self.metrics.errors_total
                    .with_label_values(&["process_market_data", &e.error_type()])
                    .inc();
                Status::internal(format!("Processing failed: {}", e))
            })?;
        
        timer.observe_duration();
        self.metrics.requests_total
            .with_label_values(&["process_market_data", "success"])
            .inc();
        
        Ok(Response::new(result.into()))
    }
}
```

## Dependency Injection Pattern

### Service Container
```rust
pub struct ServiceContainer {
    // Repositories
    pub market_data_repo: Arc<dyn MarketDataRepository>,
    pub trading_repo: Arc<dyn TradingRepository>,
    pub neural_model_repo: Arc<dyn NeuralModelRepository>,
    pub config_repo: Arc<dyn ConfigRepository>,
    
    // Use Cases
    pub process_market_data: Arc<dyn ProcessMarketDataUseCase>,
    pub execute_trade: Arc<dyn ExecuteTradeUseCase>,
    pub train_model: Arc<dyn TrainModelUseCase>,
    pub manage_config: Arc<dyn ManageConfigUseCase>,
    
    // External Services
    pub event_publisher: Arc<dyn EventPublisher>,
    pub neural_analyzer: Arc<dyn NeuralAnalyzer>,
    pub metrics_collector: Arc<dyn MetricsCollector>,
}

impl ServiceContainer {
    pub async fn new(config: &Config) -> Result<Self, ContainerError> {
        // Infrastructure layer setup
        let database_pool = create_database_pool(&config.database).await?;
        let nats_client = create_nats_client(&config.messaging).await?;
        let metrics_registry = create_metrics_registry()?;
        
        // Repository implementations
        let market_data_repo = Arc::new(
            PostgresMarketDataRepository::new(database_pool.clone())
        );
        let trading_repo = Arc::new(
            PostgresTradingRepository::new(database_pool.clone())
        );
        
        // External service adapters
        let event_publisher = Arc::new(
            NatsEventPublisher::new(nats_client.clone())
        );
        let neural_analyzer = Arc::new(
            TorchNeuralAnalyzer::new(&config.neural_config)?
        );
        
        // Use case implementations
        let process_market_data = Arc::new(
            ProcessMarketDataUseCaseImpl::new(
                market_data_repo.clone(),
                event_publisher.clone(),
                neural_analyzer.clone(),
            )
        );
        
        Ok(Self {
            market_data_repo,
            trading_repo,
            process_market_data,
            event_publisher,
            neural_analyzer,
            // ... other dependencies
        })
    }
}
```

## Testing Strategy by Layer

### Domain Layer Testing
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    
    #[test]
    fn should_create_valid_trading_signal() {
        // Given
        let symbol = Symbol::new("AAPL").unwrap();
        let signal_type = SignalType::Buy;
        let confidence = Confidence::new(0.85).unwrap();
        
        // When
        let signal = TradingSignal::new(symbol, signal_type, confidence);
        
        // Then
        assert!(signal.is_ok());
        let signal = signal.unwrap();
        assert_eq!(signal.symbol().as_str(), "AAPL");
        assert_eq!(signal.confidence().value(), 0.85);
    }
    
    proptest! {
        #[test]
        fn confidence_validation_property(confidence in 0.0f64..1.0f64) {
            let symbol = Symbol::new("TEST").unwrap();
            let signal_type = SignalType::Buy;
            let confidence = Confidence::new(confidence);
            
            if confidence.is_ok() {
                let result = TradingSignal::new(
                    symbol, 
                    signal_type, 
                    confidence.unwrap()
                );
                prop_assert!(result.is_ok());
            }
        }
    }
}
```

### Application Layer Testing
```rust
#[tokio::test]
async fn should_process_market_data_successfully() {
    // Given
    let mut mock_repo = MockMarketDataRepository::new();
    let mut mock_publisher = MockEventPublisher::new();
    let mut mock_analyzer = MockNeuralAnalyzer::new();
    
    mock_analyzer
        .expect_analyze()
        .with(predicate::always())
        .times(1)
        .returning(|data| Ok(create_test_analysis(data)));
    
    mock_repo
        .expect_store()
        .with(predicate::always())
        .times(1)
        .returning(|data| Ok(create_stored_data(data)));
    
    mock_publisher
        .expect_publish()
        .with(predicate::always())
        .times(1)
        .returning(|_| Ok(()));
    
    let use_case = ProcessMarketDataUseCaseImpl::new(
        mock_repo,
        mock_publisher,
        mock_analyzer,
    );
    
    let command = create_test_command();
    
    // When
    let result = use_case.execute(command).await;
    
    // Then
    assert!(result.is_ok());
}
```

### Infrastructure Layer Testing
```rust
#[tokio::test]
async fn postgres_repository_integration_test() {
    // Given
    let container = TestContainers::postgres().await;
    let pool = create_test_pool(container.connection_string()).await;
    let repo = PostgresMarketDataRepository::new(pool);
    
    let test_data = create_test_market_data();
    
    // When
    let stored = repo.store(test_data.clone()).await;
    
    // Then
    assert!(stored.is_ok());
    let stored = stored.unwrap();
    assert_eq!(stored.symbol, test_data.symbol);
    
    // Verify retrieval
    let retrieved = repo.find_by_id(stored.id).await;
    assert!(retrieved.is_ok());
    assert_eq!(retrieved.unwrap().id, stored.id);
}
```

## Error Handling Architecture

### Error Types Hierarchy
```rust
// Domain errors
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("Invalid confidence value: {0}")]
    InvalidConfidence(f64),
    
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
    
    #[error("Insufficient market data")]
    InsufficientData,
}

// Application errors
#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    
    #[error("Repository error: {0}")]
    Repository(#[from] RepositoryError),
    
    #[error("External service error: {0}")]
    ExternalService(#[from] ExternalServiceError),
    
    #[error("Validation error: {0}")]
    Validation(String),
}

// Infrastructure errors
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

## Performance Considerations

### Metrics Collection
```rust
pub struct ServiceMetrics {
    pub requests_total: CounterVec,
    pub request_duration: HistogramVec,
    pub active_connections: Gauge,
    pub errors_total: CounterVec,
    pub processing_queue_size: Gauge,
}

impl ServiceMetrics {
    pub fn new() -> Result<Self, MetricsError> {
        let requests_total = CounterVec::new(
            Opts::new("requests_total", "Total requests processed"),
            &["service", "method", "status"]
        )?;
        
        let request_duration = HistogramVec::new(
            HistogramOpts::new("request_duration_seconds", "Request duration"),
            &["service", "method"]
        )?;
        
        // Register metrics...
        
        Ok(Self {
            requests_total,
            request_duration,
            active_connections,
            errors_total,
            processing_queue_size,
        })
    }
}
```

## Configuration Management

### Hierarchical Configuration
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub messaging: MessagingConfig,
    pub neural: NeuralConfig,
    pub api: ApiConfig,
    pub monitoring: MonitoringConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::Environment::with_prefix("NEURAL_TRADER"))
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name(&format!(
                "config/{}", 
                std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".into())
            )).required(false))
            .build()?;
            
        Ok(config.try_deserialize()?)
    }
}
```

## Next Steps

1. **Implement Service Boundaries**: Define clear contracts between services
2. **Add Circuit Breakers**: Implement fault tolerance patterns
3. **Create Integration Tests**: End-to-end testing scenarios
4. **Performance Benchmarks**: Establish performance baselines
5. **Deployment Architecture**: Container orchestration and scaling

## Quality Gates

- [ ] All dependencies point inward (dependency inversion)
- [ ] No circular dependencies between layers
- [ ] Domain layer has no external dependencies
- [ ] All interfaces have mock implementations
- [ ] 90%+ test coverage across all layers
- [ ] Integration tests for all repositories
- [ ] Performance tests for all use cases
- [ ] Error handling covers all failure modes