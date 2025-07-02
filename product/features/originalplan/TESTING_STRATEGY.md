# Testing Strategy

## Overview

The Neural Trading Platform requires comprehensive testing to ensure reliability, performance, and correctness of trading operations. This document outlines the testing strategy covering unit tests, integration tests, performance tests, and specialized trading system tests.

## Testing Pyramid

```
                    ┌─────────────┐
                    │    E2E      │ (5%)
                    │   Tests     │
                ┌───┴─────────────┴───┐
                │   Integration Tests │ (20%)
            ┌───┴─────────────────────┴───┐
            │       Unit Tests            │ (75%)
        └─────────────────────────────────┘
```

## 1. Unit Tests (75% of test suite)

### Test Structure
```
tests/unit/
├── agents/
│   ├── test_market_analyzer.rs
│   ├── test_risk_manager.rs
│   ├── test_portfolio_manager.rs
│   └── test_execution_agent.rs
├── neural/
│   ├── test_models.rs
│   ├── test_training.rs
│   └── test_inference.rs
├── trading/
│   ├── test_orders.rs
│   ├── test_positions.rs
│   └── test_execution.rs
├── data/
│   ├── test_providers.rs
│   ├── test_storage.rs
│   └── test_pipeline.rs
└── utils/
    ├── test_math.rs
    └── test_metrics.rs
```

### Agent Unit Tests

#### Market Analyzer Agent Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_nhits_prediction_latency() {
        // Arrange
        let mut agent = MarketAnalyzerAgent::new("test_analyzer".to_string())?;
        let market_data = create_test_market_data("AAPL", 150.25);
        
        // Act
        let start = std::time::Instant::now();
        let result = agent.analyze_market(&market_data).await?;
        let duration = start.elapsed();
        
        // Assert
        assert!(duration.as_millis() < 5, "Market analysis must complete in <5ms");
        assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
        assert!(!result.reasoning.is_empty());
    }
    
    #[tokio::test]
    async fn test_nhits_prediction_accuracy() {
        // Test with known data patterns
        let mut agent = MarketAnalyzerAgent::new("test_analyzer".to_string())?;
        
        // Create trending market data
        let trend_data = create_trending_market_data("AAPL", 150.0, 0.01, 100);
        let result = agent.analyze_market(&trend_data.last().unwrap()).await?;
        
        assert_eq!(result.recommendation, "bullish");
        assert!(result.confidence > 0.6);
    }
    
    #[tokio::test]
    async fn test_technical_indicators_calculation() {
        let agent = MarketAnalyzerAgent::new("test_analyzer".to_string())?;
        let market_data = create_test_market_data("AAPL", 150.25);
        
        let indicators = agent.calculate_indicators(&market_data).await?;
        
        assert!(indicators.rsi.is_some());
        assert!(indicators.rsi.unwrap() >= 0.0 && indicators.rsi.unwrap() <= 100.0);
        assert!(indicators.macd.is_some());
        assert!(indicators.ema_20.is_some());
    }
}
```

#### Risk Manager Agent Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_var_calculation_accuracy() {
        let mut agent = RiskManagerAgent::new("test_risk".to_string(), 100000.0)?;
        
        // Test with known portfolio
        let portfolio = create_test_portfolio(vec![
            ("AAPL", 100.0, 150.25),
            ("GOOGL", 50.0, 2750.50),
        ]);
        
        let var_forecast = agent.generate_var_forecast().await?;
        
        assert!(var_forecast.var_95 > 0.0);
        assert!(var_forecast.var_99 > var_forecast.var_95);
        assert!(var_forecast.probability_loss >= 0.0 && var_forecast.probability_loss <= 1.0);
    }
    
    #[tokio::test]
    async fn test_position_sizing_limits() {
        let agent = RiskManagerAgent::new("test_risk".to_string(), 100000.0)?;
        
        let analysis = AnalysisResult {
            symbol: "AAPL".to_string(),
            recommendation: "buy".to_string(),
            confidence: 0.8,
            // ... other fields
        };
        
        let decision = agent.assess_risk(&analysis).await?;
        
        // Should not exceed position limits
        if let Ok(size) = decision.get_recommended_size() {
            assert!(size <= 10000.0); // 10% of capital
        }
    }
    
    #[tokio::test]
    async fn test_risk_latency_requirement() {
        let mut agent = RiskManagerAgent::new("test_risk".to_string(), 100000.0)?;
        let analysis = create_test_analysis("AAPL", "buy", 0.75);
        
        let start = std::time::Instant::now();
        let _decision = agent.assess_risk(&analysis).await?;
        let duration = start.elapsed();
        
        assert!(duration.as_millis() < 10, "Risk assessment must complete in <10ms");
    }
}
```

### Neural Network Unit Tests

```rust
#[cfg(test)]
mod neural_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_nhits_model_initialization() {
        let model = NHITSModel::builder()
            .horizon(24)
            .input_size(50)
            .hierarchical_levels(3)
            .build()?;
            
        assert_eq!(model.horizon(), 24);
        assert_eq!(model.input_size(), 50);
    }
    
    #[tokio::test]
    async fn test_model_prediction_shape() {
        let model = NHITSModel::builder().build()?;
        let input = vec![1.0; 50];
        
        let prediction = model.predict(&input).await?;
        
        assert_eq!(prediction.minute_level.len(), 24);
        assert_eq!(prediction.hourly_level.len(), 24);
        assert_eq!(prediction.daily_level.len(), 24);
    }
    
    #[tokio::test]
    async fn test_training_pipeline() {
        let mut pipeline = NeuralTrainingPipeline::new()?;
        let training_data = create_test_training_data(1000);
        
        let result = pipeline.train(&training_data).await?;
        
        assert!(result.final_loss < result.initial_loss);
        assert!(result.validation_accuracy > 0.5);
    }
}
```

### Trading Engine Unit Tests

```rust
#[cfg(test)]
mod trading_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_order_creation() {
        let order = Order::builder()
            .symbol("AAPL")
            .side(OrderSide::Buy)
            .quantity(100.0)
            .order_type(OrderType::Market)
            .build()?;
            
        assert_eq!(order.symbol(), "AAPL");
        assert_eq!(order.quantity(), 100.0);
        assert_eq!(order.status(), OrderStatus::Pending);
    }
    
    #[tokio::test]
    async fn test_position_calculation() {
        let mut position = Position::new("AAPL", PositionSide::Long);
        
        position.add_execution(100.0, 150.00)?;
        position.add_execution(50.0, 151.00)?;
        
        assert_eq!(position.quantity(), 150.0);
        assert_eq!(position.average_price(), 150.33); // Weighted average
    }
    
    #[tokio::test]
    async fn test_portfolio_pnl_calculation() {
        let mut portfolio = Portfolio::new("test_account");
        
        portfolio.add_position("AAPL", 100.0, 150.00);
        portfolio.update_market_price("AAPL", 155.00);
        
        let unrealized_pnl = portfolio.unrealized_pnl();
        assert_eq!(unrealized_pnl, 500.0); // (155 - 150) * 100
    }
}
```

## 2. Integration Tests (20% of test suite)

### Test Structure
```
tests/integration/
├── test_daa_orchestration.rs
├── test_trading_workflow.rs
├── test_data_pipeline.rs
├── test_neural_integration.rs
└── test_mcp_coordination.rs
```

### DAA Orchestration Tests

```rust
#[tokio::test]
async fn test_full_daa_decision_flow() {
    // Arrange
    let mut orchestrator = create_test_orchestrator().await;
    let market_data = create_test_market_data("AAPL", 150.25);
    
    // Act - Full agent coordination flow
    let market_analysis = orchestrator.get_agent("market_analyzer")
        .analyze(&market_data).await?;
        
    let risk_decision = orchestrator.get_agent("risk_manager")
        .assess_risk(&market_analysis).await?;
        
    let portfolio_decision = orchestrator.get_agent("portfolio_manager")
        .optimize_allocation(&market_analysis, &risk_decision).await?;
        
    let execution_result = orchestrator.get_agent("execution_agent")
        .execute(&portfolio_decision).await?;
    
    // Assert
    assert!(market_analysis.confidence > 0.0);
    assert_ne!(risk_decision.action, "error");
    assert!(execution_result.success);
    
    // Verify agent coordination
    assert!(orchestrator.coordination_successful());
}

#[tokio::test]
async fn test_agent_failure_recovery() {
    let mut orchestrator = create_test_orchestrator().await;
    
    // Simulate agent failure
    orchestrator.simulate_agent_failure("risk_manager");
    
    let market_data = create_test_market_data("AAPL", 150.25);
    let result = orchestrator.process_market_data(&market_data).await;
    
    // Should gracefully handle failure and use fallback
    assert!(result.is_ok());
    assert!(result.unwrap().used_fallback);
}
```

### Trading Workflow Tests

```rust
#[tokio::test]
async fn test_end_to_end_trade_execution() {
    // Arrange
    let platform = TradingPlatform::new_test().await;
    let initial_balance = platform.get_account_balance().await?;
    
    // Act - Submit buy order
    let order_id = platform.submit_order(Order {
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        quantity: 10.0,
        order_type: OrderType::Market,
        ..Default::default()
    }).await?;
    
    // Wait for execution
    platform.wait_for_order_fill(order_id, Duration::from_secs(5)).await?;
    
    // Verify position created
    let positions = platform.get_positions().await?;
    let aapl_position = positions.iter()
        .find(|p| p.symbol == "AAPL")
        .expect("AAPL position should exist");
        
    assert_eq!(aapl_position.quantity, 10.0);
    
    // Verify balance updated
    let new_balance = platform.get_account_balance().await?;
    assert!(new_balance < initial_balance);
}

#[tokio::test] 
async fn test_risk_limit_enforcement() {
    let platform = TradingPlatform::new_test().await;
    
    // Set low risk limits
    platform.set_risk_limits(RiskLimits {
        max_position_size: 1000.0,
        max_daily_loss: 500.0,
        ..Default::default()
    }).await?;
    
    // Try to submit large order
    let result = platform.submit_order(Order {
        symbol: "AAPL".to_string(),
        side: OrderSide::Buy,
        quantity: 100.0, // Would exceed position limit
        order_type: OrderType::Market,
        ..Default::default()
    }).await;
    
    // Should be rejected
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("risk limit"));
}
```

### Data Pipeline Tests

```rust
#[tokio::test]
async fn test_data_ingestion_pipeline() {
    let pipeline = DataPipeline::new_test().await;
    
    // Start data ingestion
    pipeline.start_ingestion().await?;
    
    // Simulate market data from multiple sources
    let iex_data = create_mock_iex_data("AAPL", 100);
    let alpaca_data = create_mock_alpaca_data("AAPL", 100);
    
    pipeline.ingest_data("iex", iex_data).await?;
    pipeline.ingest_data("alpaca", alpaca_data).await?;
    
    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Verify data stored correctly
    let stored_data = pipeline.get_stored_data("AAPL", "1m").await?;
    assert!(!stored_data.is_empty());
    
    // Verify data quality
    assert!(stored_data.iter().all(|d| d.quality_score >= 0.8));
}

#[tokio::test]
async fn test_real_time_data_latency() {
    let pipeline = DataPipeline::new_test().await;
    let mut receiver = pipeline.subscribe_to_real_time("AAPL").await?;
    
    // Send test data
    let test_data = create_test_tick_data("AAPL", 150.25);
    let send_time = Utc::now();
    
    pipeline.send_real_time_data(test_data).await?;
    
    // Receive and measure latency
    let received_data = tokio::time::timeout(
        Duration::from_millis(100),
        receiver.recv()
    ).await??;
    
    let receive_time = Utc::now();
    let latency = receive_time - send_time;
    
    assert!(latency.num_milliseconds() < 50, "Real-time data latency too high");
}
```

## 3. Performance Tests

### Latency Tests

```rust
#[tokio::test]
async fn test_agent_latency_requirements() {
    let agents = create_all_agents().await;
    let test_data = create_test_market_data("AAPL", 150.25);
    
    // Market Analyzer: <5ms
    let start = Instant::now();
    agents.market_analyzer.analyze(&test_data).await?;
    assert!(start.elapsed().as_millis() < 5);
    
    // Risk Manager: <10ms
    let start = Instant::now();
    agents.risk_manager.assess_risk(&create_test_analysis()).await?;
    assert!(start.elapsed().as_millis() < 10);
    
    // Portfolio Manager: <20ms
    let start = Instant::now();
    agents.portfolio_manager.optimize(&create_test_portfolio()).await?;
    assert!(start.elapsed().as_millis() < 20);
    
    // Execution Agent: <1ms
    let start = Instant::now();
    agents.execution_agent.execute(&create_test_order()).await?;
    assert!(start.elapsed().as_millis() < 1);
}
```

### Throughput Tests

```rust
#[tokio::test]
async fn test_order_processing_throughput() {
    let platform = TradingPlatform::new_test().await;
    let num_orders = 1000;
    let orders: Vec<Order> = (0..num_orders)
        .map(|i| create_test_order(format!("SYM{}", i)))
        .collect();
    
    let start = Instant::now();
    
    let results = futures::future::join_all(
        orders.into_iter().map(|order| platform.submit_order(order))
    ).await;
    
    let duration = start.elapsed();
    let throughput = num_orders as f64 / duration.as_secs_f64();
    
    assert!(throughput > 100.0, "Order throughput should exceed 100 orders/second");
    assert!(results.iter().all(|r| r.is_ok()), "All orders should be processed successfully");
}
```

### Memory Tests

```rust
#[tokio::test]
async fn test_memory_usage_under_load() {
    let platform = TradingPlatform::new_test().await;
    let initial_memory = get_memory_usage();
    
    // Generate heavy load
    for _ in 0..10000 {
        let data = create_large_market_data_batch(100);
        platform.process_market_data(data).await?;
    }
    
    // Force garbage collection
    std::hint::black_box(());
    
    let final_memory = get_memory_usage();
    let memory_growth = final_memory - initial_memory;
    
    assert!(memory_growth < 100_000_000, "Memory growth should be <100MB under load");
}
```

## 4. End-to-End Tests (5% of test suite)

```rust
#[tokio::test]
async fn test_complete_trading_session() {
    // This test simulates a complete trading session
    let platform = TradingPlatform::new_test().await;
    
    // 1. Start platform and agents
    platform.start().await?;
    platform.start_agents().await?;
    
    // 2. Begin market data simulation
    let market_sim = MarketDataSimulator::new();
    market_sim.simulate_trading_day("AAPL", Duration::from_secs(30)).await;
    
    // 3. Let the system trade for simulated period
    tokio::time::sleep(Duration::from_secs(30)).await;
    
    // 4. Verify system performed as expected
    let trades = platform.get_completed_trades().await?;
    let portfolio = platform.get_portfolio().await?;
    
    assert!(!trades.is_empty(), "System should have executed trades");
    assert!(portfolio.total_value > 0.0, "Portfolio should have value");
    
    // 5. Verify all agents remained healthy
    let agent_health = platform.get_agent_health().await?;
    assert!(agent_health.iter().all(|h| h.status == "healthy"));
    
    // 6. Shutdown gracefully
    platform.shutdown().await?;
}
```

## 5. Property-Based Testing

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_portfolio_value_invariants(
        trades in vec(arbitrary_trade(), 0..100)
    ) {
        let mut portfolio = Portfolio::new("test");
        let initial_cash = 100000.0;
        portfolio.set_cash(initial_cash);
        
        let mut total_spent = 0.0;
        for trade in trades {
            if portfolio.can_execute_trade(&trade) {
                total_spent += trade.value();
                portfolio.execute_trade(trade)?;
            }
        }
        
        // Portfolio value should equal cash + positions value
        let portfolio_value = portfolio.total_value();
        let cash_plus_positions = portfolio.cash() + portfolio.positions_value();
        
        prop_assert_eq!(portfolio_value, cash_plus_positions);
        
        // Total spent should not exceed initial cash (no leverage)
        prop_assert!(total_spent <= initial_cash);
    }
}

fn arbitrary_trade() -> impl Strategy<Value = Trade> {
    (
        "[A-Z]{3,4}",  // Symbol
        prop::bool::ANY,  // Buy/Sell
        1u32..1000,   // Quantity  
        1.0..1000.0   // Price
    ).prop_map(|(symbol, is_buy, quantity, price)| {
        Trade {
            symbol,
            side: if is_buy { TradeSide::Buy } else { TradeSide::Sell },
            quantity: quantity as f64,
            price,
        }
    })
}
```

## 6. Test Data Management

### Test Fixtures

```rust
// tests/fixtures/market_data.rs
pub fn create_test_market_data(symbol: &str, price: f64) -> MarketData {
    MarketData {
        timestamp: Utc::now(),
        symbol: symbol.to_string(),
        price,
        volume: 10000.0,
        high: price * 1.01,
        low: price * 0.99,
        open: price * 0.995,
        bid: Some(price - 0.01),
        ask: Some(price + 0.01),
        spread: Some(0.02),
    }
}

pub fn create_trending_market_data(
    symbol: &str, 
    start_price: f64, 
    trend: f64, 
    count: usize
) -> Vec<MarketData> {
    (0..count)
        .map(|i| {
            let price = start_price * (1.0 + trend * i as f64);
            create_test_market_data(symbol, price)
        })
        .collect()
}
```

### Database Testing

```rust
#[tokio::test]
async fn test_with_test_database() {
    let test_db = TestDatabase::new().await;
    
    // Run test with isolated database
    let result = test_with_database(&test_db.url).await;
    
    // Cleanup automatic when test_db drops
    assert!(result.is_ok());
}

pub struct TestDatabase {
    container: Container,
    pub url: String,
}

impl TestDatabase {
    async fn new() -> Self {
        // Start test database container
        let container = start_test_timescaledb().await;
        let url = format!("postgresql://test:test@localhost:{}/test", 
                         container.port());
        
        // Run migrations
        run_migrations(&url).await.unwrap();
        
        Self { container, url }
    }
}
```

## 7. Testing Tools and Infrastructure

### Test Configuration

```toml
# Cargo.toml test configuration
[dev-dependencies]
tokio-test = "0.4"
proptest = "1.0"
wiremock = "0.5"
testcontainers = "0.14"
criterion = "0.5"

[features]
test-utils = []

[[bench]]
name = "neural_inference"
harness = false

[[bench]]
name = "trading_engine"
harness = false
```

### Test Scripts

```bash
#!/bin/bash
# scripts/run-tests.sh

set -e

echo "Running unit tests..."
cargo test --lib

echo "Running integration tests..."
cargo test --test '*' --features test-utils

echo "Running benchmarks..."
cargo bench

echo "Running property tests..."
cargo test --features proptest

echo "Generating coverage report..."
cargo tarpaulin --out Html

echo "All tests completed successfully!"
```

### Continuous Integration

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg15
        env:
          POSTGRES_PASSWORD: test
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
    - uses: actions/checkout@v2
    
    - name: Setup Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
    
    - name: Run tests
      run: |
        cargo test --verbose
        
    - name: Run benchmarks
      run: |
        cargo bench --verbose
        
    - name: Generate coverage
      uses: actions-rs/tarpaulin@v0.1
      with:
        args: '--out Xml'
        
    - name: Upload coverage
      uses: codecov/codecov-action@v1
```

This comprehensive testing strategy ensures the neural trading platform is reliable, performant, and correct under all conditions.