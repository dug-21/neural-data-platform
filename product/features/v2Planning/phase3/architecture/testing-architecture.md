# Testing Architecture - V2 MVP Layer-Based Testing Strategy

## Executive Summary

This document outlines a comprehensive testing strategy for the layered V2 MVP architecture, ensuring each layer is independently testable while maintaining integration test coverage. The strategy emphasizes dependency injection, mocking, and clear testing boundaries that align with the architectural layers.

## Testing Philosophy

### Core Principles

1. **Layer Independence**: Each layer can be tested in isolation
2. **Fast Feedback**: Unit tests run in <1 second, integration tests in <10 seconds  
3. **Reliable Tests**: Tests are deterministic and don't depend on external state
4. **Maintainable**: Tests are easy to understand and modify
5. **Representative**: Tests reflect real-world usage patterns

### Testing Pyramid

```
                    🔺
                   /E2E\           <- 5% (Critical user journeys)
                  /     \
                 /  API  \         <- 15% (Layer integration)  
                /         \
               /Integration\       <- 30% (Component interaction)
              /             \
             /     Unit      \     <- 50% (Business logic)
            /_________________\
```

## Layer-Specific Testing Strategies

### Domain Layer Testing (`src/domain/`)

**Characteristics**:
- **Pure Functions**: No external dependencies
- **Fast Execution**: All tests run in memory
- **Comprehensive Coverage**: 95%+ code coverage target

**Testing Approach**:
```rust
// src/domain/services/risk_calculator.rs
use crate::domain::entities::{TradingPosition, MarketData, RiskAssessment};
use crate::domain::value_objects::{Price, Volume, Symbol};

pub struct RiskCalculator;

impl RiskCalculator {
    pub fn calculate_position_risk(
        &self,
        position: &TradingPosition,
        current_market_data: &MarketData,
        historical_volatility: f64,
    ) -> RiskAssessment {
        // Pure business logic - no external dependencies
        let price_change = (current_market_data.price().value() - position.entry_price().value()) 
            / position.entry_price().value();
        
        let volatility_risk = historical_volatility * position.size();
        let correlation_risk = self.calculate_correlation_risk(position, current_market_data);
        
        RiskAssessment::new(
            price_change.abs(),
            volatility_risk,
            correlation_risk,
        )
    }
    
    fn calculate_correlation_risk(&self, position: &TradingPosition, market_data: &MarketData) -> f64 {
        // Business logic for correlation risk
        0.05 // Simplified for example
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::test_fixtures::*;
    
    #[test]
    fn test_calculate_position_risk_long_profitable() {
        // Arrange
        let risk_calculator = RiskCalculator;
        let position = TradingPositionBuilder::new()
            .symbol(Symbol::new("AAPL").unwrap())
            .entry_price(Price::new(150.00).unwrap())
            .size(100.0)
            .side(PositionSide::Long)
            .build();
            
        let market_data = MarketDataBuilder::new()
            .symbol(Symbol::new("AAPL").unwrap())
            .price(Price::new(160.00).unwrap())
            .volume(Volume::new(1000000.0).unwrap())
            .build();
        
        let historical_volatility = 0.25; // 25% annualized
        
        // Act
        let risk_assessment = risk_calculator.calculate_position_risk(
            &position, 
            &market_data, 
            historical_volatility
        );
        
        // Assert
        assert!(risk_assessment.price_risk() > 0.0);
        assert!(risk_assessment.volatility_risk() > 0.0);
        assert_eq!(risk_assessment.overall_risk_score(), /* expected value */);
    }
    
    #[test]
    fn test_calculate_position_risk_edge_cases() {
        let risk_calculator = RiskCalculator;
        
        // Test zero position size
        let zero_position = TradingPositionBuilder::new()
            .size(0.0)
            .build();
            
        let risk = risk_calculator.calculate_position_risk(&zero_position, &market_data(), 0.25);
        assert_eq!(risk.volatility_risk(), 0.0);
        
        // Test extreme volatility
        let high_vol_risk = risk_calculator.calculate_position_risk(&standard_position(), &market_data(), 2.0);
        assert!(high_vol_risk.volatility_risk() > risk.volatility_risk());
    }
    
    #[test]
    fn test_risk_calculation_mathematical_properties() {
        let risk_calculator = RiskCalculator;
        let position = standard_position();
        let market_data = market_data();
        
        // Risk should be proportional to volatility
        let low_vol_risk = risk_calculator.calculate_position_risk(&position, &market_data, 0.1);
        let high_vol_risk = risk_calculator.calculate_position_risk(&position, &market_data, 0.2);
        
        assert!(high_vol_risk.volatility_risk() > low_vol_risk.volatility_risk());
        assert_eq!(high_vol_risk.volatility_risk(), low_vol_risk.volatility_risk() * 2.0);
    }
}

// Domain test fixtures
mod test_fixtures {
    use super::*;
    
    pub struct TradingPositionBuilder {
        symbol: Option<Symbol>,
        entry_price: Option<Price>,
        size: f64,
        side: PositionSide,
    }
    
    impl TradingPositionBuilder {
        pub fn new() -> Self {
            Self {
                symbol: None,
                entry_price: None,
                size: 100.0,
                side: PositionSide::Long,
            }
        }
        
        pub fn symbol(mut self, symbol: Symbol) -> Self {
            self.symbol = Some(symbol);
            self
        }
        
        pub fn entry_price(mut self, price: Price) -> Self {
            self.entry_price = Some(price);
            self
        }
        
        pub fn size(mut self, size: f64) -> Self {
            self.size = size;
            self
        }
        
        pub fn build(self) -> TradingPosition {
            TradingPosition::new(
                self.symbol.unwrap_or_else(|| Symbol::new("AAPL").unwrap()),
                self.entry_price.unwrap_or_else(|| Price::new(100.0).unwrap()),
                self.size,
                self.side,
                chrono::Utc::now(),
            )
        }
    }
    
    pub fn standard_position() -> TradingPosition {
        TradingPositionBuilder::new().build()
    }
    
    pub fn market_data() -> MarketData {
        MarketDataBuilder::new().build()
    }
}
```

### Application Layer Testing (`src/application/`)

**Characteristics**:
- **Use Case Focus**: Test business scenarios
- **Dependency Injection**: Mock external dependencies
- **Behavior Verification**: Ensure correct interactions

**Testing Approach**:
```rust
// src/application/use_cases/execute_trading_decision.rs
use mockall::predicate::*;
use crate::application::ports::*;
use crate::domain::services::*;

pub struct ExecuteTradingDecisionUseCase {
    config_service: Arc<dyn ConfigService>,
    market_data_repository: Arc<dyn MarketDataRepository>,
    position_repository: Arc<dyn PositionRepository>,
    broker_service: Arc<dyn BrokerService>,
    risk_calculator: Arc<RiskCalculator>,
}

impl ExecuteTradingDecisionUseCase {
    pub async fn execute(&self, command: TradingDecisionCommand) -> Result<TradingDecisionResult> {
        // Get current configuration
        let trading_config = self.config_service.get::<TradingConfig>("trading").await?;
        
        // Validate trading hours
        if !trading_config.is_market_open() {
            return Ok(TradingDecisionResult::MarketClosed);
        }
        
        // Get market data
        let market_data = self.market_data_repository
            .get_latest_data(&command.symbol)
            .await?;
        
        // Get current position  
        let current_position = self.position_repository
            .get_position(&command.symbol)
            .await?;
        
        // Calculate risk
        let risk_assessment = self.risk_calculator.calculate_position_risk(
            &current_position.unwrap_or_default(),
            &market_data,
            command.volatility,
        );
        
        // Make trading decision based on signal and risk
        if risk_assessment.is_acceptable() && command.signal.is_strong() {
            let order = self.create_order(&command, &trading_config, &risk_assessment)?;
            let order_id = self.broker_service.submit_order(&order).await?;
            
            Ok(TradingDecisionResult::OrderSubmitted { order_id })
        } else {
            Ok(TradingDecisionResult::NoAction { 
                reason: format!("Risk: {}, Signal: {}", risk_assessment, command.signal)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::mocks::*;
    use crate::domain::test_fixtures::*;
    
    #[tokio::test]
    async fn test_execute_trading_decision_successful_order() {
        // Arrange
        let mut mock_config = MockConfigService::new();
        let mut mock_market_repo = MockMarketDataRepository::new();
        let mut mock_position_repo = MockPositionRepository::new();
        let mut mock_broker = MockBrokerService::new();
        let risk_calculator = Arc::new(RiskCalculator);
        
        // Setup mock expectations
        mock_config
            .expect_get::<TradingConfig>()
            .with(eq("trading"))
            .returning(|_| Ok(trading_config_market_open()));
            
        mock_market_repo
            .expect_get_latest_data()
            .with(eq(Symbol::new("AAPL").unwrap()))
            .returning(|_| Ok(bullish_market_data()));
            
        mock_position_repo
            .expect_get_position()
            .with(eq(Symbol::new("AAPL").unwrap()))
            .returning(|_| Ok(None)); // No current position
            
        mock_broker
            .expect_submit_order()
            .withf(|order| order.symbol() == &Symbol::new("AAPL").unwrap())
            .returning(|_| Ok(OrderId::new()));
        
        let use_case = ExecuteTradingDecisionUseCase::new(
            Arc::new(mock_config),
            Arc::new(mock_market_repo),
            Arc::new(mock_position_repo),
            Arc::new(mock_broker),
            risk_calculator,
        );
        
        let command = TradingDecisionCommand {
            symbol: Symbol::new("AAPL").unwrap(),
            signal: TradingSignal::StrongBuy { confidence: 0.8 },
            volatility: 0.15,
        };
        
        // Act
        let result = use_case.execute(command).await.unwrap();
        
        // Assert
        match result {
            TradingDecisionResult::OrderSubmitted { order_id } => {
                assert!(!order_id.is_nil());
            }
            _ => panic!("Expected OrderSubmitted result"),
        }
    }
    
    #[tokio::test]
    async fn test_execute_trading_decision_market_closed() {
        // Arrange - market closed scenario
        let mut mock_config = MockConfigService::new();
        mock_config
            .expect_get::<TradingConfig>()
            .returning(|_| Ok(trading_config_market_closed()));
        
        let use_case = ExecuteTradingDecisionUseCase::new(
            Arc::new(mock_config),
            // Other mocks not needed for this test
            Arc::new(MockMarketDataRepository::new()),
            Arc::new(MockPositionRepository::new()),
            Arc::new(MockBrokerService::new()),
            Arc::new(RiskCalculator),
        );
        
        // Act
        let result = use_case.execute(trading_decision_command()).await.unwrap();
        
        // Assert
        assert!(matches!(result, TradingDecisionResult::MarketClosed));
    }
    
    #[tokio::test]
    async fn test_execute_trading_decision_high_risk_rejection() {
        // Test that high-risk scenarios are rejected appropriately
        let use_case = setup_use_case_with_high_risk_scenario();
        
        let result = use_case.execute(high_risk_command()).await.unwrap();
        
        match result {
            TradingDecisionResult::NoAction { reason } => {
                assert!(reason.contains("Risk"));
            }
            _ => panic!("Expected NoAction due to high risk"),
        }
    }
}
```

### Infrastructure Layer Testing (`src/infrastructure/`)

**Characteristics**:
- **Integration Focus**: Test external service interactions
- **Container-Based**: Use Docker for external dependencies
- **Environment Isolation**: Each test gets clean state

**Testing Approach**:
```rust
// src/infrastructure/persistence/timescale_market_repository.rs
#[cfg(test)]
mod tests {
    use super::*;
    use testcontainers::*;
    use testcontainers::clients::Cli;
    use testcontainers::images::postgres::Postgres;
    
    struct TestContext {
        container: Container<'static, Postgres>,
        repository: TimescaleMarketRepository,
    }
    
    impl TestContext {
        async fn new() -> Self {
            let docker = Cli::default();
            let container = docker.run(
                Postgres::default()
                    .with_env_var("POSTGRES_DB", "test_db")
                    .with_env_var("POSTGRES_USER", "test_user")
                    .with_env_var("POSTGRES_PASSWORD", "test_pass")
            );
            
            let port = container.get_host_port_ipv4(5432);
            let database_config = DatabaseConfig {
                host: "localhost".to_string(),
                port,
                database: "test_db".to_string(),
                username: "test_user".to_string(),
                password: "test_pass".to_string(),
                max_connections: 5,
                connection_timeout_seconds: 30,
            };
            
            let repository = TimescaleMarketRepository::new_with_config(database_config).await.unwrap();
            
            // Run migrations
            repository.run_migrations().await.unwrap();
            
            Self { container, repository }
        }
        
        async fn cleanup(&self) {
            // Container cleanup is automatic via Drop
        }
    }
    
    #[tokio::test]
    async fn test_store_and_retrieve_market_data() {
        let ctx = TestContext::new().await;
        
        // Arrange
        let market_data = MarketData::new(
            Symbol::new("AAPL").unwrap(),
            Price::new(150.00).unwrap(),
            Volume::new(1000000.0).unwrap(),
            chrono::Utc::now(),
        );
        
        // Act - Store
        ctx.repository.store_market_data(&market_data).await.unwrap();
        
        // Act - Retrieve
        let retrieved_data = ctx.repository
            .get_latest_data(&market_data.symbol())
            .await
            .unwrap();
        
        // Assert
        assert_eq!(retrieved_data.symbol(), market_data.symbol());
        assert_eq!(retrieved_data.price(), market_data.price());
        assert_eq!(retrieved_data.volume(), market_data.volume());
        
        ctx.cleanup().await;
    }
    
    #[tokio::test]
    async fn test_get_historical_data_time_range() {
        let ctx = TestContext::new().await;
        
        // Arrange - Insert test data with different timestamps
        let base_time = chrono::Utc::now() - chrono::Duration::hours(24);
        let symbol = Symbol::new("GOOGL").unwrap();
        
        for i in 0..24 {
            let market_data = MarketData::new(
                symbol.clone(),
                Price::new(2000.0 + (i as f64)).unwrap(),
                Volume::new(100000.0).unwrap(),
                base_time + chrono::Duration::hours(i),
            );
            ctx.repository.store_market_data(&market_data).await.unwrap();
        }
        
        // Act - Get last 12 hours
        let time_range = TimeRange::new(
            base_time + chrono::Duration::hours(12),
            chrono::Utc::now()
        );
        
        let historical_data = ctx.repository
            .get_historical_data(&symbol, time_range)
            .await
            .unwrap();
        
        // Assert
        assert_eq!(historical_data.len(), 12);
        assert!(historical_data.iter().all(|data| data.symbol() == &symbol));
        
        // Verify chronological order
        for i in 1..historical_data.len() {
            assert!(historical_data[i].timestamp() > historical_data[i-1].timestamp());
        }
        
        ctx.cleanup().await;
    }
    
    #[tokio::test]
    async fn test_concurrent_operations() {
        let ctx = TestContext::new().await;
        
        // Test concurrent reads and writes
        let symbol = Symbol::new("AMZN").unwrap();
        let repository = Arc::new(ctx.repository);
        
        // Spawn concurrent tasks
        let mut tasks = vec![];
        
        for i in 0..10 {
            let repo = repository.clone();
            let sym = symbol.clone();
            
            tasks.push(tokio::spawn(async move {
                let market_data = MarketData::new(
                    sym.clone(),
                    Price::new(3000.0 + (i as f64)).unwrap(),
                    Volume::new(50000.0).unwrap(),
                    chrono::Utc::now(),
                );
                repo.store_market_data(&market_data).await
            }));
        }
        
        // Wait for all tasks
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        
        // Verify all data was stored
        let all_data = repository.get_all_data_for_symbol(&symbol).await.unwrap();
        assert_eq!(all_data.len(), 10);
        
        ctx.cleanup().await;
    }
}

// Config Store Service Integration Tests
// src/infrastructure/config/config_store_service.rs
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_config_store_service_full_integration() {
        // Arrange - Create temporary config file
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test-config.toml");
        
        std::fs::write(&config_path, r#"
            [trading]
            max_position_size = 10000.0
            risk_tolerance = 0.02
            enable_paper_trading = true
            allowed_symbols = ["AAPL", "GOOGL", "MSFT"]
            
            [database]
            host = "localhost"
            port = 5432
            database = "neural_trader_test"
            username = "test_user"
            password = "test_pass"
            max_connections = 10
        "#).unwrap();
        
        // Act - Initialize service
        let service = ConfigStoreService::new_with_file(config_path).await.unwrap();
        
        // Test configuration loading
        let trading_config: TradingConfig = service.get("trading").await.unwrap();
        assert_eq!(trading_config.max_position_size, 10000.0);
        assert_eq!(trading_config.allowed_symbols.len(), 3);
        
        let db_config: DatabaseConfig = service.get("database").await.unwrap();
        assert_eq!(db_config.host, "localhost");
        assert_eq!(db_config.max_connections, 10);
        
        // Test configuration updates
        let mut updated_trading = trading_config.clone();
        updated_trading.max_position_size = 15000.0;
        
        service.set("trading", &updated_trading).await.unwrap();
        
        let reloaded_config: TradingConfig = service.get("trading").await.unwrap();
        assert_eq!(reloaded_config.max_position_size, 15000.0);
        
        // Test caching behavior
        let start_time = std::time::Instant::now();
        let _cached_config: TradingConfig = service.get("trading").await.unwrap();
        let cache_duration = start_time.elapsed();
        
        assert!(cache_duration < std::time::Duration::from_millis(10)); // Should be very fast
    }
    
    #[tokio::test] 
    async fn test_config_validation_integration() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid-config.toml");
        
        // Invalid configuration
        std::fs::write(&config_path, r#"
            [trading]
            max_position_size = -1000.0    # Invalid: negative
            risk_tolerance = 1.5           # Invalid: > 1.0
            allowed_symbols = []           # Invalid: empty array
        "#).unwrap();
        
        // Should fail validation during service creation
        let result = ConfigStoreService::new_with_file(config_path).await;
        assert!(result.is_err());
    }
}
```

### Presentation Layer Testing (`src/presentation/`)

**Characteristics**:
- **API Contract Testing**: Ensure API follows OpenAPI spec
- **Integration Testing**: Test request/response flows
- **Authentication & Authorization**: Security testing

**Testing Approach**:
```rust
// src/presentation/api/trading_api.rs
#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use tower::ServiceExt;
    use crate::application::mocks::*;
    
    async fn create_test_server() -> TestServer {
        let mock_container = create_mock_application_container().await;
        let app = create_trading_api_router(Arc::new(mock_container));
        TestServer::new(app).unwrap()
    }
    
    #[tokio::test]
    async fn test_submit_trading_order_success() {
        let server = create_test_server().await;
        
        let order_request = json!({
            "symbol": "AAPL",
            "side": "buy",
            "quantity": 100.0,
            "order_type": "market"
        });
        
        let response = server
            .post("/api/v1/orders")
            .json(&order_request)
            .await;
        
        response.assert_status_ok();
        
        let order_response: OrderResponse = response.json();
        assert!(!order_response.order_id.is_empty());
        assert_eq!(order_response.status, "submitted");
    }
    
    #[tokio::test]
    async fn test_submit_trading_order_validation_error() {
        let server = create_test_server().await;
        
        // Invalid order - negative quantity
        let invalid_order = json!({
            "symbol": "AAPL",
            "side": "buy", 
            "quantity": -100.0,
            "order_type": "market"
        });
        
        let response = server
            .post("/api/v1/orders")
            .json(&invalid_order)
            .await;
        
        response.assert_status_bad_request();
        
        let error_response: ErrorResponse = response.json();
        assert!(error_response.message.contains("quantity"));
    }
    
    #[tokio::test]
    async fn test_get_portfolio_status() {
        let server = create_test_server().await;
        
        let response = server
            .get("/api/v1/portfolio")
            .await;
        
        response.assert_status_ok();
        
        let portfolio: PortfolioResponse = response.json();
        assert!(portfolio.total_value >= 0.0);
        assert!(!portfolio.positions.is_empty());
    }
    
    #[tokio::test]
    async fn test_websocket_connection() {
        let server = create_test_server().await;
        
        let mut websocket = server
            .get_websocket("/ws")
            .await;
        
        // Test subscription to market data
        websocket.send_text(json!({
            "action": "subscribe",
            "channel": "market_data",
            "symbol": "AAPL"
        }).to_string()).await;
        
        // Should receive confirmation
        let message = websocket.recv_text().await.unwrap();
        let response: serde_json::Value = serde_json::from_str(&message).unwrap();
        
        assert_eq!(response["type"], "subscription_confirmed");
        assert_eq!(response["channel"], "market_data");
    }
    
    #[tokio::test]
    async fn test_rate_limiting() {
        let server = create_test_server().await;
        
        // Make multiple rapid requests
        let mut responses = vec![];
        for _ in 0..20 {
            let response = server.get("/api/v1/health").await;
            responses.push(response);
        }
        
        // First few should succeed
        assert!(responses[0..5].iter().all(|r| r.status_code() == 200));
        
        // Later requests should be rate limited
        let rate_limited = responses.iter()
            .any(|r| r.status_code() == 429);
        assert!(rate_limited);
    }
}
```

## Test Infrastructure

### Test Data Management

```rust
// src/test_utils/fixtures.rs
pub struct TestDataBuilder {
    database: TestDatabase,
    redis: TestRedis,
}

impl TestDataBuilder {
    pub async fn new() -> Self {
        let database = TestDatabase::setup().await;
        let redis = TestRedis::setup().await;
        
        Self { database, redis }
    }
    
    pub async fn with_market_data(&self, symbol: &str, days: usize) -> &Self {
        for i in 0..days {
            let market_data = MarketData::new(
                Symbol::new(symbol).unwrap(),
                Price::new(100.0 + (i as f64)).unwrap(),
                Volume::new(1000000.0).unwrap(),
                chrono::Utc::now() - chrono::Duration::days(days as i64 - i as i64),
            );
            
            self.database.insert_market_data(&market_data).await.unwrap();
        }
        self
    }
    
    pub async fn with_positions(&self, positions: Vec<TradingPosition>) -> &Self {
        for position in positions {
            self.database.insert_position(&position).await.unwrap();
        }
        self
    }
    
    pub async fn cleanup(&self) {
        self.database.cleanup().await;
        self.redis.cleanup().await;
    }
}

// Usage in tests
#[tokio::test]
async fn test_portfolio_analysis_with_real_data() {
    let test_data = TestDataBuilder::new().await
        .with_market_data("AAPL", 30)
        .with_market_data("GOOGL", 30)
        .with_positions(vec![
            long_position("AAPL", 100.0),
            short_position("GOOGL", 50.0),
        ]).await;
    
    // Run actual test
    let portfolio_service = PortfolioService::new(test_data.repository());
    let analysis = portfolio_service.analyze_portfolio().await.unwrap();
    
    assert!(analysis.total_value > 0.0);
    
    test_data.cleanup().await;
}
```

### Mock Factories

```rust
// src/application/ports/mocks/mod.rs
use mockall::mock;

// Generate mocks for all application ports
mock! {
    pub ConfigService {}
    #[async_trait]
    impl ConfigService for ConfigService {
        async fn get<T>(&self, key: &str) -> Result<T>
        where T: serde::de::DeserializeOwned + Send + Sync;
        async fn set<T>(&self, key: &str, value: &T) -> Result<()>
        where T: serde::Serialize + Send + Sync;
    }
}

mock! {
    pub MarketDataRepository {}
    #[async_trait]
    impl MarketDataRepository for MarketDataRepository {
        async fn get_latest_data(&self, symbol: &Symbol) -> Result<MarketData>;
        async fn store_market_data(&self, data: &MarketData) -> Result<()>;
        async fn get_historical_data(&self, symbol: &Symbol, range: TimeRange) -> Result<Vec<MarketData>>;
    }
}

// Mock factory for common scenarios
pub struct MockFactory;

impl MockFactory {
    pub fn trading_config_market_open() -> TradingConfig {
        TradingConfig {
            max_position_size: 10000.0,
            risk_tolerance: 0.02,
            enable_paper_trading: true,
            stop_loss_percentage: 0.02,
            allowed_symbols: vec!["AAPL".to_string(), "GOOGL".to_string()],
            trading_hours: TradingHours::always_open(), // For testing
        }
    }
    
    pub fn bullish_market_data() -> MarketData {
        MarketData::new(
            Symbol::new("AAPL").unwrap(),
            Price::new(160.0).unwrap(),
            Volume::new(2000000.0).unwrap(),
            chrono::Utc::now(),
        )
    }
    
    pub fn create_mock_application_container() -> MockApplicationContainer {
        let mut mock_container = MockApplicationContainer::new();
        
        // Set up common mock behaviors
        mock_container
            .expect_get_config_service()
            .returning(|| {
                let mut mock_config = MockConfigService::new();
                mock_config
                    .expect_get::<TradingConfig>()
                    .returning(|_| Ok(MockFactory::trading_config_market_open()));
                Arc::new(mock_config)
            });
        
        mock_container
    }
}
```

### Performance Testing

```rust
// src/tests/performance/mod.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokio::runtime::Runtime;

fn benchmark_risk_calculation(c: &mut Criterion) {
    let risk_calculator = RiskCalculator::new();
    let position = standard_position();
    let market_data = market_data();
    
    c.bench_function("risk_calculation", |b| {
        b.iter(|| {
            risk_calculator.calculate_position_risk(
                black_box(&position),
                black_box(&market_data),
                black_box(0.25),
            )
        })
    });
}

fn benchmark_config_loading(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("config_loading_cached", |b| {
        b.to_async(&rt).iter(|| async {
            let config_service = create_test_config_service().await;
            let _config: TradingConfig = config_service.get("trading").await.unwrap();
        })
    });
}

fn benchmark_market_data_storage(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    c.bench_function("market_data_batch_insert", |b| {
        b.to_async(&rt).iter(|| async {
            let repository = create_test_repository().await;
            let market_data_batch = generate_market_data_batch(1000);
            
            for data in market_data_batch {
                repository.store_market_data(&data).await.unwrap();
            }
        })
    });
}

criterion_group!(
    benches,
    benchmark_risk_calculation,
    benchmark_config_loading,
    benchmark_market_data_storage
);
criterion_main!(benches);
```

## Continuous Integration Testing

### GitHub Actions Workflow

```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: timescale/timescaledb:latest-pg14
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
      
      redis:
        image: redis:7
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
    - uses: actions/checkout@v3
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        components: rustfmt, clippy
        override: true
    
    - name: Cache Cargo
      uses: actions/cache@v3
      with:
        path: ~/.cargo
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Run Unit Tests
      run: cargo test --lib --bins
      env:
        DATABASE_URL: postgres://postgres:postgres@localhost/test_db
        REDIS_URL: redis://localhost:6379
    
    - name: Run Integration Tests
      run: cargo test --test integration_tests
      env:
        DATABASE_URL: postgres://postgres:postgres@localhost/test_db
        REDIS_URL: redis://localhost:6379
    
    - name: Run Performance Tests
      run: cargo test --release --test performance_tests
    
    - name: Generate Coverage Report
      run: |
        cargo install cargo-tarpaulin
        cargo tarpaulin --out xml --output-dir coverage/
    
    - name: Upload Coverage
      uses: codecov/codecov-action@v3
      with:
        file: coverage/cobertura.xml
```

## Testing Metrics & Quality Gates

### Coverage Targets
- **Domain Layer**: 95% line coverage (business logic critical)
- **Application Layer**: 90% line coverage (use cases)
- **Infrastructure Layer**: 85% line coverage (external integration complexity)
- **Presentation Layer**: 80% line coverage (API contracts)

### Performance Targets
- **Unit Tests**: < 1 second total execution time
- **Integration Tests**: < 30 seconds total execution time  
- **E2E Tests**: < 5 minutes total execution time
- **Memory Usage**: < 100MB during test execution

### Quality Gates
```rust
// tests/quality_gates.rs
#[test]
fn test_dependency_architecture_compliance() {
    use std::process::Command;
    
    // Verify no domain layer dependencies on infrastructure
    let output = Command::new("cargo")
        .args(["tree", "--duplicates", "--format", "{p}"])
        .output()
        .unwrap();
        
    let dependencies = String::from_utf8(output.stdout).unwrap();
    
    // Domain layer should only depend on std and serde
    assert!(!dependencies.contains("domain -> tokio"));
    assert!(!dependencies.contains("domain -> sqlx"));
    assert!(!dependencies.contains("domain -> redis"));
}

#[test] 
fn test_no_unwrap_in_production_code() {
    use std::process::Command;
    
    let output = Command::new("grep")
        .args(["-r", ".unwrap()", "src/", "--exclude-dir=test"])
        .output()
        .unwrap();
    
    let unwrap_usages = String::from_utf8(output.stdout).unwrap();
    
    // Should only find unwraps in test code or very specific cases
    let allowed_unwraps = unwrap_usages.lines().count();
    assert!(allowed_unwraps < 5, "Too many .unwrap() calls in production code: {}", allowed_unwraps);
}
```

## Migration Testing Strategy

### Parallel Testing During Migration

```rust
// tests/migration_compatibility.rs
/// Test that both old and new implementations produce the same results
#[tokio::test]
async fn test_config_migration_compatibility() {
    // Setup both old and new config systems
    let legacy_config = load_legacy_config().await;
    let new_config_service = ConfigStoreService::new().await.unwrap();
    
    // Test that they return equivalent configurations
    let legacy_trading = legacy_config.get_trading_config();
    let new_trading: TradingConfig = new_config_service.get("trading").await.unwrap();
    
    assert_configs_equivalent(&legacy_trading, &new_trading);
}

/// Test that migration doesn't break existing functionality
#[tokio::test]
async fn test_end_to_end_compatibility() {
    // Run complete trading scenario with new architecture
    let new_system_result = run_trading_scenario_new().await;
    
    // Compare with expected behavior
    assert!(new_system_result.orders_executed > 0);
    assert!(new_system_result.risk_checks_passed);
    assert!(new_system_result.performance_acceptable);
}
```

## Summary

This testing architecture provides:

1. **Comprehensive Coverage**: Each layer tested independently and in integration
2. **Fast Feedback**: Unit tests complete in seconds
3. **Reliable Results**: Tests don't depend on external state or timing
4. **Easy Maintenance**: Clear patterns and helper utilities
5. **Quality Assurance**: Automated quality gates and performance benchmarks

The strategy ensures that the V2 MVP refactoring maintains functionality while improving code quality, testability, and maintainability. The layer-based approach makes it easy to identify and fix issues at the appropriate architectural level.