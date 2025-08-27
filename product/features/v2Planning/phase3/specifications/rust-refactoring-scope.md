# Rust Application Refactoring Scope - Phase 3

## Overview

This document defines the precise scope for Phase 3: refactoring the existing Rust application in `/src` into standardized architectural layers while integrating config-store for configuration management.

## Scope Definition

### In Scope

#### 1. Rust Application Code (`/src`)
- All Rust modules and components currently in `/src`
- Business logic currently mixed across files
- Configuration management code
- Service implementations
- API endpoints and handlers

#### 2. Architectural Refactoring
```
Current Structure:          Target Structure:
/src                   →    /src
├── neural/            →    ├── domain/
├── action_layer/      →    │   ├── trading/
├── features/          →    │   ├── neural/
├── monitoring/        →    │   └── risk/
├── adapters/          →    ├── application/
├── config/            →    │   ├── use_cases/
└── main.rs            →    │   └── services/
                            ├── infrastructure/
                            │   ├── config_store/
                            │   ├── persistence/
                            │   └── external/
                            └── presentation/
                                ├── grpc/
                                └── rest/
```

#### 3. Config-Store Integration
- Replace all `std::env::var()` calls with config-store
- Create configuration schemas for each service
- Implement `ServiceConfig<T>` for all components
- Use ConfigStoreService gRPC client

#### 4. Testing Improvements
- Extract interfaces (traits) for all components
- Implement dependency injection
- Create mock implementations
- Unit tests for each layer
- Integration tests for layer boundaries

### Out of Scope

#### 1. Data Ingestion Layer
- **Assumption**: Redis Streams is already in place
- No changes to market data streaming
- No changes to data pipeline infrastructure
- Keep existing data flow patterns

#### 2. External Infrastructure
- No changes to Redis deployment
- No changes to TimescaleDB
- No changes to monitoring infrastructure
- No Kubernetes/Docker changes

#### 3. Non-Rust Code
- Python components remain unchanged
- Proto files (except for config-store integration)
- Shell scripts and CI/CD pipelines

## Component Classification

### Domain Layer Components
```rust
// Core business entities and logic
/src/domain/
├── trading/
│   ├── signal.rs         // TradingSignal entity
│   ├── strategy.rs       // Strategy logic
│   └── risk.rs          // Risk calculations
├── neural/
│   ├── model.rs         // Neural model domain
│   ├── prediction.rs    // Prediction logic
│   └── training.rs      // Training rules
└── market/
    ├── quote.rs         // Market data entities
    └── order.rs         // Order entities
```

### Application Layer Components
```rust
// Use cases and orchestration
/src/application/
├── use_cases/
│   ├── generate_signal.rs
│   ├── execute_trade.rs
│   └── train_model.rs
└── services/
    ├── trading_service.rs
    ├── ml_service.rs
    └── risk_service.rs
```

### Infrastructure Layer Components
```rust
// External integrations and adapters
/src/infrastructure/
├── config_store/
│   ├── client.rs        // ConfigStoreService client
│   └── schemas.rs       // Configuration schemas
├── persistence/
│   ├── postgres.rs      // Database adapters
│   └── redis.rs        // Cache adapters
└── external/
    ├── alpaca.rs       // Broker integration
    └── market_data.rs  // Market data adapters
```

### Presentation Layer Components
```rust
// API and service interfaces
/src/presentation/
├── grpc/
│   ├── trading_service.rs
│   └── ml_service.rs
└── rest/
    ├── health.rs
    └── metrics.rs
```

## Configuration Migration

### Current Configuration (Environment Variables)
```rust
// Before - scattered throughout codebase
let db_url = env::var("DATABASE_URL")?;
let api_key = env::var("ALPACA_API_KEY")?;
let model_path = env::var("MODEL_PATH")?;
```

### Target Configuration (Config-Store)
```rust
// After - centralized configuration
#[derive(Serialize, Deserialize)]
struct TradingConfig {
    database_url: String,
    alpaca_api_key: String,
    model_path: PathBuf,
    risk_limits: RiskLimits,
}

let config_store = ConfigStoreClient::new("grpc://config-store:50051");
let config: TradingConfig = config_store
    .get_config("/neural-trader/trading")
    .await?;
```

## Testing Strategy

### Layer Testing
```rust
// Domain Layer - Pure unit tests
#[test]
fn test_trading_signal_validation() {
    let signal = TradingSignal::new("AAPL", Action::Buy, 0.85);
    assert!(signal.is_valid());
}

// Application Layer - Mock infrastructure
#[test]
async fn test_generate_signal_use_case() {
    let mock_repo = MockSignalRepository::new();
    let use_case = GenerateSignalUseCase::new(mock_repo);
    let result = use_case.execute(request).await;
    assert!(result.is_ok());
}

// Infrastructure Layer - Integration tests
#[test]
async fn test_config_store_integration() {
    let store = ConfigStoreClient::new("grpc://localhost:50051");
    let config = store.get_config("/test").await;
    assert!(config.is_ok());
}
```

## Refactoring Phases

### Phase 1: Extract Domain Layer (Week 1-2)
- Identify pure business logic
- Create domain entities
- Extract business rules
- No external dependencies

### Phase 2: Build Application Layer (Week 3-4)
- Create use cases
- Implement orchestration
- Define service interfaces
- Mock infrastructure

### Phase 3: Refactor Infrastructure (Week 5-6)
- Implement config-store client
- Create repository adapters
- External service adapters
- Database migrations

### Phase 4: Update Presentation (Week 7-8)
- Refactor gRPC services
- Update REST endpoints
- Wire dependency injection
- Integration testing

## Success Criteria

### Code Quality Metrics
- [ ] 90% test coverage for domain layer
- [ ] 80% test coverage for application layer
- [ ] All components use dependency injection
- [ ] Zero environment variable usage

### Architecture Metrics
- [ ] Clear layer boundaries enforced
- [ ] No circular dependencies
- [ ] All configuration via config-store
- [ ] Each layer independently testable

### Functionality Preservation
- [ ] All existing features working
- [ ] No performance degradation
- [ ] Backward compatible APIs
- [ ] Zero downtime refactoring

## Risk Mitigation

### Incremental Refactoring
- Feature flags for gradual rollout
- Parallel run of old and new code
- Component-by-component migration
- Continuous testing

### Configuration Migration
- Dual configuration support initially
- Gradual migration to config-store
- Fallback to environment variables
- Configuration validation

### Testing Coverage
- Write tests before refactoring
- Maintain integration test suite
- Performance benchmarking
- Regression testing

## Deliverables

### Code Deliverables
1. Refactored `/src` with clean layers
2. Config-store integration
3. Comprehensive test suite
4. Mock implementations

### Documentation Deliverables
1. Architecture documentation
2. Configuration schemas
3. API documentation
4. Testing guide

### Quality Deliverables
1. Test coverage reports
2. Dependency analysis
3. Performance benchmarks
4. Code review checklist