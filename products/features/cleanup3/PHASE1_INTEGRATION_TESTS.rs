// Phase 1: Critical Integration Tests for Neural Trader
// These 5 tests provide minimal protection during refactoring

use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::{Duration, Instant};

// Test 1: End-to-End Trading Decision
#[tokio::test]
async fn test_trading_decision_pipeline() -> Result<()> {
    // Initialize test system
    let test_system = TestSystem::start().await?;
    
    // Inject known market data for AAPL
    let market_data = vec![
        MarketData {
            symbol: "AAPL".to_string(),
            timestamp: Utc::now(),
            open: 100.0,
            high: 101.0,
            low: 99.0,
            close: 100.5,
            volume: 1000000,
        },
        MarketData {
            symbol: "AAPL".to_string(),
            timestamp: Utc::now() + chrono::Duration::hours(1),
            open: 100.5,
            high: 102.0,
            low: 100.0,
            close: 101.5,
            volume: 1200000,
        },
    ];
    
    test_system.inject_market_data(market_data).await?;
    
    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Verify a trading decision was made
    let decision = test_system.get_latest_decision("AAPL").await?;
    
    assert!(decision.is_some(), "Should produce a trading decision");
    if let Some(d) = decision {
        assert!(d.confidence >= 0.0 && d.confidence <= 1.0, "Confidence should be between 0 and 1");
        assert!(d.symbol == "AAPL", "Decision should be for AAPL");
    }
    
    test_system.cleanup().await?;
    Ok(())
}

// Test 2: Neural Model Training and Prediction
#[tokio::test]
async fn test_model_training_and_prediction() -> Result<()> {
    let test_system = TestSystem::start().await?;
    
    // Generate training data for XLF
    let training_data = generate_training_data("XLF", 100);
    
    // Train model
    let training_result = test_system
        .train_model("XLF", training_data)
        .await;
    
    assert!(training_result.is_ok(), "Model training should succeed");
    
    // Make prediction
    let current_data = vec![
        MarketData {
            symbol: "XLF".to_string(),
            timestamp: Utc::now(),
            open: 50.0,
            high: 51.0,
            low: 49.5,
            close: 50.5,
            volume: 500000,
        }
    ];
    
    let prediction = test_system
        .predict("XLF", current_data, 5) // 5 minute horizon
        .await;
    
    assert!(prediction.is_ok(), "Prediction should succeed");
    if let Ok(p) = prediction {
        assert!(p.value != 0.0, "Prediction should not be zero");
        assert!(p.confidence > 0.0, "Should have positive confidence");
    }
    
    test_system.cleanup().await?;
    Ok(())
}

// Test 3: Data Pipeline - Ingestion to Storage
#[tokio::test]
async fn test_data_ingestion_to_storage() -> Result<()> {
    let test_system = TestSystem::start().await?;
    
    // Clear any existing data for MSFT
    test_system.clear_symbol_data("MSFT").await?;
    
    // Ingest historical data
    let start_date = "2024-01-01";
    let end_date = "2024-01-31";
    
    test_system
        .ingest_historical_data("MSFT", start_date, end_date)
        .await?;
    
    // Query stored data
    let stored_data = test_system
        .query_data("MSFT", "2024-01-15", "2024-01-16")
        .await?;
    
    assert!(!stored_data.is_empty(), "Should have stored data");
    assert!(stored_data.len() >= 1, "Should have at least one day of data");
    
    // Verify data integrity
    for data_point in &stored_data {
        assert!(data_point.high >= data_point.low, "High should be >= Low");
        assert!(data_point.close >= data_point.low, "Close should be >= Low");
        assert!(data_point.close <= data_point.high, "Close should be <= High");
        assert!(data_point.volume > 0, "Volume should be positive");
    }
    
    test_system.cleanup().await?;
    Ok(())
}

// Test 4: Market Hours Trading vs Training Logic
#[tokio::test]
async fn test_market_hours_behavior() -> Result<()> {
    let test_system = TestSystem::start().await?;
    
    // Test during market hours (10:30 AM EST on a weekday)
    let market_open_time = "2024-01-15T10:30:00-05:00";
    test_system.set_system_time(market_open_time).await?;
    
    let status = test_system.get_system_status().await?;
    assert!(status.is_trading_active, "Should be trading during market hours");
    assert!(!status.is_training_priority, "Should not prioritize training during market hours");
    
    // Test after market hours (8:30 PM EST)
    let market_closed_time = "2024-01-15T20:30:00-05:00";
    test_system.set_system_time(market_closed_time).await?;
    
    let status = test_system.get_system_status().await?;
    assert!(!status.is_trading_active, "Should not be trading after hours");
    assert!(status.is_training_priority, "Should prioritize training after hours");
    
    // Test weekend (Saturday)
    let weekend_time = "2024-01-13T12:00:00-05:00";
    test_system.set_system_time(weekend_time).await?;
    
    let status = test_system.get_system_status().await?;
    assert!(!status.is_trading_active, "Should not be trading on weekend");
    assert!(status.is_training_priority, "Should train on weekend");
    
    test_system.cleanup().await?;
    Ok(())
}

// Test 5: Performance Baseline
#[tokio::test]
async fn test_performance_baseline() -> Result<()> {
    let test_system = TestSystem::start().await?;
    
    // Prepare test data
    let symbols = vec!["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"];
    let data_per_symbol = 50;
    
    for symbol in &symbols {
        let data = generate_training_data(symbol, data_per_symbol);
        test_system.inject_market_data(data).await?;
    }
    
    // Measure prediction performance
    let start = Instant::now();
    
    let mut predictions = Vec::new();
    for symbol in &symbols {
        for _ in 0..20 {  // 20 predictions per symbol = 100 total
            let pred = test_system.predict_simple(symbol).await?;
            predictions.push(pred);
        }
    }
    
    let duration = start.elapsed();
    
    // Performance assertions
    assert_eq!(predictions.len(), 100, "Should complete all predictions");
    assert!(
        duration.as_secs() < 2,
        "100 predictions should complete in < 2 seconds, took {:?}",
        duration
    );
    
    // Verify prediction quality
    let valid_predictions = predictions.iter()
        .filter(|p| p.value != 0.0 && p.confidence > 0.0)
        .count();
    
    assert!(
        valid_predictions > 90,
        "At least 90% of predictions should be valid"
    );
    
    println!("Performance baseline: 100 predictions in {:?}", duration);
    
    test_system.cleanup().await?;
    Ok(())
}

// ============================================================================
// Test Infrastructure
// ============================================================================

struct TestSystem {
    db: PgPool,
    redis: redis::Client,
    base_url: String,
}

impl TestSystem {
    async fn start() -> Result<Self> {
        // Start test containers using testcontainers-rs or docker-compose
        let db = start_test_postgres().await?;
        let redis = start_test_redis().await?;
        
        // Start the application
        let base_url = start_test_application(&db, &redis).await?;
        
        Ok(Self {
            db,
            redis,
            base_url,
        })
    }
    
    async fn inject_market_data(&self, data: Vec<MarketData>) -> Result<()> {
        // Direct database insertion for testing
        for item in data {
            sqlx::query!(
                r#"
                INSERT INTO market_data (symbol, timestamp, open, high, low, close, volume)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (symbol, timestamp) DO UPDATE
                SET open = $3, high = $4, low = $5, close = $6, volume = $7
                "#,
                item.symbol,
                item.timestamp,
                item.open,
                item.high,
                item.low,
                item.close,
                item.volume as i64
            )
            .execute(&self.db)
            .await?;
        }
        Ok(())
    }
    
    async fn get_latest_decision(&self, symbol: &str) -> Result<Option<TradingDecision>> {
        // Query via HTTP API or direct DB
        let response = reqwest::get(&format!("{}/api/decisions/{}", self.base_url, symbol))
            .await?
            .json::<Option<TradingDecision>>()
            .await?;
        Ok(response)
    }
    
    async fn train_model(&self, symbol: &str, data: Vec<MarketData>) -> Result<()> {
        self.inject_market_data(data).await?;
        
        // Trigger training
        let response = reqwest::post(&format!("{}/api/train/{}", self.base_url, symbol))
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Training failed with status: {}", response.status());
        }
        
        Ok(())
    }
    
    async fn predict(&self, symbol: &str, current_data: Vec<MarketData>, horizon: i32) -> Result<Prediction> {
        self.inject_market_data(current_data).await?;
        
        let response = reqwest::get(&format!("{}/api/predict/{}?horizon={}", self.base_url, symbol, horizon))
            .await?
            .json::<Prediction>()
            .await?;
        
        Ok(response)
    }
    
    async fn predict_simple(&self, symbol: &str) -> Result<Prediction> {
        let response = reqwest::get(&format!("{}/api/predict/{}", self.base_url, symbol))
            .await?
            .json::<Prediction>()
            .await?;
        
        Ok(response)
    }
    
    async fn clear_symbol_data(&self, symbol: &str) -> Result<()> {
        sqlx::query!("DELETE FROM market_data WHERE symbol = $1", symbol)
            .execute(&self.db)
            .await?;
        Ok(())
    }
    
    async fn ingest_historical_data(&self, symbol: &str, start: &str, end: &str) -> Result<()> {
        let response = reqwest::post(&format!("{}/api/ingest", self.base_url))
            .json(&serde_json::json!({
                "symbol": symbol,
                "start_date": start,
                "end_date": end
            }))
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Ingestion failed with status: {}", response.status());
        }
        
        // Wait for ingestion to complete
        tokio::time::sleep(Duration::from_secs(5)).await;
        
        Ok(())
    }
    
    async fn query_data(&self, symbol: &str, start: &str, end: &str) -> Result<Vec<MarketData>> {
        let rows = sqlx::query_as!(
            MarketData,
            r#"
            SELECT symbol, timestamp, open, high, low, close, volume
            FROM market_data
            WHERE symbol = $1 
            AND timestamp >= $2::timestamp 
            AND timestamp <= $3::timestamp
            ORDER BY timestamp
            "#,
            symbol,
            start,
            end
        )
        .fetch_all(&self.db)
        .await?;
        
        Ok(rows)
    }
    
    async fn set_system_time(&self, time_str: &str) -> Result<()> {
        // For testing, we might mock the time or use a test endpoint
        let response = reqwest::post(&format!("{}/api/test/set-time", self.base_url))
            .json(&serde_json::json!({ "time": time_str }))
            .send()
            .await?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to set system time");
        }
        
        Ok(())
    }
    
    async fn get_system_status(&self) -> Result<SystemStatus> {
        let response = reqwest::get(&format!("{}/api/status", self.base_url))
            .await?
            .json::<SystemStatus>()
            .await?;
        
        Ok(response)
    }
    
    async fn cleanup(&self) -> Result<()> {
        // Clean up test data
        sqlx::query!("DELETE FROM market_data WHERE symbol LIKE 'TEST_%'")
            .execute(&self.db)
            .await?;
        
        // Close connections
        self.db.close().await;
        
        Ok(())
    }
}

// ============================================================================
// Test Data Structures
// ============================================================================

#[derive(Debug, Clone, sqlx::FromRow)]
struct MarketData {
    symbol: String,
    timestamp: DateTime<Utc>,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
}

#[derive(Debug, serde::Deserialize)]
struct TradingDecision {
    symbol: String,
    action: String,  // "buy", "sell", "hold"
    confidence: f64,
    reason: String,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize)]
struct Prediction {
    symbol: String,
    value: f64,
    confidence: f64,
    horizon: i32,
}

#[derive(Debug, serde::Deserialize)]
struct SystemStatus {
    is_trading_active: bool,
    is_training_priority: bool,
    models_loaded: usize,
    last_prediction: Option<DateTime<Utc>>,
}

// ============================================================================
// Test Helpers
// ============================================================================

fn generate_training_data(symbol: &str, count: usize) -> Vec<MarketData> {
    let mut data = Vec::new();
    let mut price = 100.0;
    let base_time = Utc::now() - chrono::Duration::days(30);
    
    for i in 0..count {
        // Random walk for price
        price *= 1.0 + (rand::random::<f64>() - 0.5) * 0.02;
        
        let high = price * (1.0 + rand::random::<f64>() * 0.01);
        let low = price * (1.0 - rand::random::<f64>() * 0.01);
        let close = low + (high - low) * rand::random::<f64>();
        
        data.push(MarketData {
            symbol: symbol.to_string(),
            timestamp: base_time + chrono::Duration::hours(i as i64),
            open: price,
            high,
            low,
            close,
            volume: (1000000.0 * rand::random::<f64>()) as i64,
        });
        
        price = close;  // Next open is previous close
    }
    
    data
}

async fn start_test_postgres() -> Result<PgPool> {
    // Use testcontainers or docker-compose to start PostgreSQL
    // For now, assume test database is running
    let database_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://test:test@localhost:5433/neural_trader_test".to_string());
    
    let pool = PgPool::connect(&database_url).await?;
    
    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;
    
    Ok(pool)
}

async fn start_test_redis() -> Result<redis::Client> {
    let redis_url = std::env::var("TEST_REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6380".to_string());
    
    let client = redis::Client::open(redis_url)?;
    
    // Test connection
    let mut conn = client.get_async_connection().await?;
    redis::cmd("PING").query_async::<_, String>(&mut conn).await?;
    
    Ok(client)
}

async fn start_test_application(db: &PgPool, redis: &redis::Client) -> Result<String> {
    // Start the application in test mode
    // This would typically spawn the binary or use a test harness
    
    // For integration tests, we might want to:
    // 1. Start the actual binary with test configuration
    // 2. Use a test harness that initializes the app
    // 3. Use docker-compose to start the full stack
    
    let port = 8081;  // Test port
    let base_url = format!("http://localhost:{}", port);
    
    // Wait for app to be ready
    for _ in 0..30 {
        if reqwest::get(&format!("{}/health", base_url)).await.is_ok() {
            return Ok(base_url);
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    
    anyhow::bail!("Application failed to start")
}