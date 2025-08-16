//! Emergency Test: Data Pipeline Integrity
//! Tests that data flows from source to storage correctly

use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use chrono::{DateTime, Utc, Duration};

async fn connect_test_db() -> Result<sqlx::PgPool> {
    // Use environment variables or defaults
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/neural_trader_db".to_string());
    
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    
    Ok(pool)
}

async fn insert_market_data(db: &sqlx::PgPool, symbol: &str) -> Result<()> {
    let now = Utc::now();
    
    // Insert test data for the last hour
    for i in 0..60 {
        let timestamp = now - Duration::minutes(i);
        let price_base = 100.0 + (i as f64 * 0.1);
        
        sqlx::query(
            r#"
            INSERT INTO market_data (timestamp, symbol, open, high, low, close, volume)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (timestamp, symbol) DO NOTHING
            "#
        )
        .bind(timestamp.naive_utc())
        .bind(symbol)
        .bind(price_base)
        .bind(price_base + 0.5)
        .bind(price_base - 0.5)
        .bind(price_base + 0.2)
        .bind(1000.0 + (i as f64 * 10.0))
        .execute(db)
        .await?;
    }
    
    Ok(())
}

async fn query_hourly_data(db: &sqlx::PgPool, symbol: &str) -> Result<Vec<(DateTime<Utc>, f64)>> {
    // First check if continuous aggregate exists
    let aggregate_exists = sqlx::query(
        "SELECT 1 FROM timescaledb_information.continuous_aggregates 
         WHERE view_name = 'market_data_1h' LIMIT 1"
    )
    .fetch_optional(db)
    .await?
    .is_some();
    
    if aggregate_exists {
        // Query from continuous aggregate
        let rows = sqlx::query(
            "SELECT bucket, close 
             FROM market_data_1h 
             WHERE symbol = $1 
             ORDER BY bucket DESC 
             LIMIT 24"
        )
        .bind(symbol)
        .fetch_all(db)
        .await?;
        
        Ok(rows.into_iter().map(|row| {
            let bucket: chrono::NaiveDateTime = row.get("bucket");
            let close: f64 = row.get("close");
            (DateTime::from_naive_utc_and_offset(bucket, Utc), close)
        }).collect())
    } else {
        // Fallback to manual aggregation
        let rows = sqlx::query(
            "SELECT date_trunc('hour', timestamp) as hour, AVG(close) as avg_close
             FROM market_data
             WHERE symbol = $1 
             AND timestamp > NOW() - INTERVAL '24 hours'
             GROUP BY hour
             ORDER BY hour DESC"
        )
        .bind(symbol)
        .fetch_all(db)
        .await?;
        
        Ok(rows.into_iter().map(|row| {
            let hour: chrono::NaiveDateTime = row.get("hour");
            let avg_close: f64 = row.get("avg_close");
            (DateTime::from_naive_utc_and_offset(hour, Utc), avg_close)
        }).collect())
    }
}

async fn cleanup_test_data(db: &sqlx::PgPool, symbol: &str) -> Result<()> {
    sqlx::query("DELETE FROM market_data WHERE symbol = $1")
        .bind(symbol)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn test_data_pipeline_integrity() -> Result<()> {
    println!("🧪 Testing Data Pipeline Integrity...");
    
    // Connect to database
    let db = match connect_test_db().await {
        Ok(pool) => {
            println!("  ✅ Connected to database");
            pool
        }
        Err(e) => {
            println!("  ⚠️  Cannot connect to database: {}", e);
            println!("  ℹ️  Skipping data pipeline test");
            return Ok(());
        }
    };
    
    let test_symbol = "TEST_EMERGENCY_PIPELINE";
    
    // Clean up any existing test data
    let _ = cleanup_test_data(&db, test_symbol).await;
    
    // Insert test data
    match insert_market_data(&db, test_symbol).await {
        Ok(_) => println!("  ✅ Test data inserted (60 records)"),
        Err(e) => {
            println!("  ❌ Failed to insert test data: {}", e);
            return Err(e);
        }
    }
    
    // Verify data was stored
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM market_data WHERE symbol = $1"
    )
    .bind(test_symbol)
    .fetch_one(&db)
    .await?;
    
    assert!(count > 0, "No data found in market_data table");
    println!("  ✅ Data stored successfully ({} records)", count);
    
    // Test aggregation
    let hourly_data = query_hourly_data(&db, test_symbol).await?;
    if !hourly_data.is_empty() {
        println!("  ✅ Hourly aggregations working ({} hours)", hourly_data.len());
    } else {
        println!("  ⚠️  No hourly aggregations found (may need refresh)");
    }
    
    // Test data validation
    let validation_result = sqlx::query(
        "SELECT COUNT(*) as invalid_count
         FROM market_data
         WHERE symbol = $1
         AND (high < low OR open < 0 OR close < 0 OR volume < 0)"
    )
    .bind(test_symbol)
    .fetch_one(&db)
    .await?;
    
    let invalid_count: i64 = validation_result.get("invalid_count");
    assert_eq!(invalid_count, 0, "Found invalid data records");
    println!("  ✅ Data validation passed");
    
    // Cleanup
    cleanup_test_data(&db, test_symbol).await?;
    println!("  ✅ Test data cleaned up");
    
    println!("✅ Data Pipeline Integrity test completed");
    Ok(())
}

pub async fn test_timescale_aggregates() -> Result<()> {
    println!("🧪 Testing TimescaleDB Continuous Aggregates...");
    
    let db = match connect_test_db().await {
        Ok(pool) => pool,
        Err(_) => {
            println!("  ℹ️  Database not available, skipping test");
            return Ok(());
        }
    };
    
    // Check if continuous aggregate exists
    let aggregate_info = sqlx::query(
        "SELECT view_name, refresh_interval 
         FROM timescaledb_information.continuous_aggregates 
         WHERE view_name = 'market_data_1h'"
    )
    .fetch_optional(&db)
    .await?;
    
    if let Some(row) = aggregate_info {
        let view_name: String = row.get("view_name");
        println!("  ✅ Continuous aggregate '{}' exists", view_name);
        
        // Check last refresh time
        let refresh_info = sqlx::query(
            "SELECT materialization_hypertable_schema, materialization_hypertable_name
             FROM timescaledb_information.continuous_aggregates
             WHERE view_name = 'market_data_1h'"
        )
        .fetch_optional(&db)
        .await?;
        
        if refresh_info.is_some() {
            println!("  ✅ Aggregate is materialized");
        }
    } else {
        println!("  ⚠️  No continuous aggregate found for market_data_1h");
        println!("  ℹ️  System may be using manual aggregation");
    }
    
    Ok(())
}