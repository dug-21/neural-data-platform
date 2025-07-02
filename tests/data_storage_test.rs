use autonomous_platform::data::storage::{TimescaleDBStorage, TimeSeriesData, PredictionData};
use chrono::{Utc, Duration};
use serde_json::json;

#[tokio::test]
async fn test_database_connection() {
    // Test that we can successfully connect to the database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await;
    assert!(storage.is_ok(), "Failed to connect to database: {:?}", storage.err());
}

#[tokio::test]
async fn test_create_tables() {
    // Test that we can create the necessary tables
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    let result = storage.create_tables().await;
    assert!(result.is_ok(), "Failed to create tables: {:?}", result.err());
}

#[tokio::test]
async fn test_store_time_series_data() {
    // Test storing a single time series data point
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    let data = TimeSeriesData {
        timestamp: Utc::now(),
        source: "test_source".to_string(),
        entity: "BTC/USD".to_string(),
        value: 42000.50,
        metadata: Some(json!({
            "exchange": "binance",
            "volume": 1234.56
        })),
    };
    
    let result = storage.store_time_series(&data).await;
    assert!(result.is_ok(), "Failed to store time series data: {:?}", result.err());
}

#[tokio::test]
async fn test_query_by_time_range() {
    // Test querying data by time range
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    // Insert test data
    let now = Utc::now();
    // Use unique entity for this test
    let entity = format!("ETH/USD_RANGE_{}", now.timestamp());
    
    let test_data = vec![
        TimeSeriesData {
            timestamp: now - Duration::minutes(5),
            source: "test_source".to_string(),
            entity: entity.clone(),
            value: 2800.0,
            metadata: None,
        },
        TimeSeriesData {
            timestamp: now - Duration::minutes(3),
            source: "test_source".to_string(),
            entity: entity.clone(),
            value: 2810.0,
            metadata: None,
        },
        TimeSeriesData {
            timestamp: now - Duration::minutes(1),
            source: "test_source".to_string(),
            entity: entity.clone(),
            value: 2805.0,
            metadata: None,
        },
    ];
    
    for data in test_data {
        storage.store_time_series(&data).await.unwrap();
    }
    
    // Query data from last 4 minutes
    let start = now - Duration::minutes(4);
    let end = now;
    let results = storage.query_range(&entity, start, end).await.unwrap();
    
    assert_eq!(results.len(), 2, "Expected 2 data points in range");
    assert!(results[0].value == 2810.0 || results[0].value == 2805.0);
}

#[tokio::test]
async fn test_store_neural_prediction() {
    // Test storing neural network predictions
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    let prediction = PredictionData {
        timestamp: Utc::now(),
        entity: "BTC/USD".to_string(),
        model_id: "lstm_v1".to_string(),
        prediction_value: 43000.0,
        confidence: 0.85,
        horizon_minutes: 60,
        features_used: Some(json!({
            "price_ma_7": 42500.0,
            "volume_ma_24h": 15000.0,
            "rsi": 65.5
        })),
    };
    
    let result = storage.store_prediction(&prediction).await;
    assert!(result.is_ok(), "Failed to store prediction: {:?}", result.err());
}

#[tokio::test]
async fn test_batch_operations() {
    // Test batch insert operations
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    // Create batch data
    let now = Utc::now();
    let mut batch_data = Vec::new();
    
    // Use unique entity for this test
    let entity = format!("XRP/USD_BATCH_{}", now.timestamp());
    
    for i in 0..100 {
        batch_data.push(TimeSeriesData {
            timestamp: now - Duration::seconds(i),
            source: "batch_test".to_string(),
            entity: entity.clone(),
            value: 0.50 + (i as f64 * 0.001),
            metadata: Some(json!({ "batch_id": i })),
        });
    }
    
    let result = storage.batch_insert(&batch_data).await;
    assert!(result.is_ok(), "Failed to batch insert: {:?}", result.err());
    
    // Verify all data was inserted
    let start = now - Duration::seconds(100);
    let results = storage.query_range(&entity, start, now).await.unwrap();
    assert_eq!(results.len(), 100, "Expected 100 data points after batch insert");
}

#[tokio::test]
async fn test_query_latest_predictions() {
    // Test querying latest predictions for an entity
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    // Insert multiple predictions
    let now = Utc::now();
    let predictions = vec![
        PredictionData {
            timestamp: now - Duration::minutes(10),
            entity: "BTC/USD".to_string(),
            model_id: "lstm_v1".to_string(),
            prediction_value: 42000.0,
            confidence: 0.80,
            horizon_minutes: 30,
            features_used: None,
        },
        PredictionData {
            timestamp: now - Duration::minutes(5),
            entity: "BTC/USD".to_string(),
            model_id: "lstm_v1".to_string(),
            prediction_value: 42500.0,
            confidence: 0.85,
            horizon_minutes: 30,
            features_used: None,
        },
        PredictionData {
            timestamp: now,
            entity: "BTC/USD".to_string(),
            model_id: "lstm_v1".to_string(),
            prediction_value: 43000.0,
            confidence: 0.90,
            horizon_minutes: 30,
            features_used: None,
        },
    ];
    
    for pred in predictions {
        storage.store_prediction(&pred).await.unwrap();
    }
    
    // Query latest prediction
    let latest = storage.get_latest_prediction("BTC/USD", "lstm_v1").await.unwrap();
    assert!(latest.is_some(), "Expected to find latest prediction");
    
    let latest_pred = latest.unwrap();
    assert_eq!(latest_pred.prediction_value, 43000.0);
    assert_eq!(latest_pred.confidence, 0.90);
}

#[tokio::test]
async fn test_cleanup_old_data() {
    // Test cleaning up old data
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://platform_user:platform_pass@localhost:5432/autonomous_platform".to_string());
    
    let storage = TimescaleDBStorage::new(&database_url).await.unwrap();
    storage.create_tables().await.unwrap();
    
    // Insert old and new data
    let now = Utc::now();
    
    // Use unique entity for this test
    let entity = format!("OLD/USD_CLEANUP_{}", now.timestamp());
    
    let old_data = vec![
        TimeSeriesData {
            timestamp: now - Duration::days(40),
            source: "old_source".to_string(),
            entity: entity.clone(),
            value: 100.0,
            metadata: None,
        },
        TimeSeriesData {
            timestamp: now - Duration::days(35),
            source: "old_source".to_string(),
            entity: entity.clone(),
            value: 110.0,
            metadata: None,
        },
    ];
    
    let new_data = TimeSeriesData {
        timestamp: now - Duration::days(10),
        source: "new_source".to_string(),
        entity: entity.clone(),
        value: 120.0,
        metadata: None,
    };
    
    for data in old_data {
        storage.store_time_series(&data).await.unwrap();
    }
    storage.store_time_series(&new_data).await.unwrap();
    
    // Note: cleanup_old_data deletes ALL data older than specified days,
    // not just for a specific entity. Since we might have other test data,
    // we can't assert the exact number of deleted records.
    let _ = storage.cleanup_old_data(30).await.unwrap();
    
    // Verify only new data remains for our entity
    let start = now - Duration::days(50);
    let results = storage.query_range(&entity, start, now).await.unwrap();
    assert_eq!(results.len(), 1, "Expected only 1 recent record to remain");
    assert_eq!(results[0].value, 120.0);
}