//! Integration tests for AIR-002 MQTT to Parquet pipeline
//!
//! Note: Some tests require an MQTT broker (use `docker compose up mosquitto` to start)
//!
//! Test Philosophy: These are integration tests that verify the complete data flow
//! from source to storage, ensuring components work together correctly.

use chrono::{Duration, Utc};
use neural_core::traits::{AggregationType, Store};
use neural_core::{HealthStatus, ParquetStore, TimeSeriesPoint};
use std::collections::HashMap;
use tempfile::TempDir;

// ========== BASIC STORAGE INTEGRATION TESTS ==========

/// Test that ParquetStore can write and query data correctly
#[tokio::test]
async fn test_parquet_write_and_query() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Write test points
    let now = Utc::now();
    let points = vec![
        TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 25.5,
            tags: HashMap::from([("metric".to_string(), "pm25".to_string())]),
            ndp_id: None,
            context: None,
        },
        TimeSeriesPoint {
            timestamp: now,
            location_id: "sensor-001".to_string(),
            value: 450.0,
            tags: HashMap::from([("metric".to_string(), "co2".to_string())]),
            ndp_id: None,
            context: None,
        },
    ];

    store.write_batch(points).await.unwrap();

    // Query and verify
    let results = store
        .query(
            "sensor-001",
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|p| (p.value - 25.5).abs() < 0.001));
    assert!(results.iter().any(|p| (p.value - 450.0).abs() < 0.001));
}

/// Test data persistence across store instances
#[tokio::test]
async fn test_data_persistence_after_restart() {
    let temp_dir = TempDir::new().unwrap();
    let now = Utc::now();

    // Write data with first store instance
    {
        let store = ParquetStore::new(temp_dir.path()).unwrap();
        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: "persistence-test".to_string(),
            value: 100.0,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };
        store.write_batch(vec![point]).await.unwrap();
    }

    // Create new store instance (simulates restart)
    {
        let store = ParquetStore::new(temp_dir.path()).unwrap();
        // Replay WAL if exists
        let _ = store.replay_wal().await;

        let results = store
            .query(
                "persistence-test",
                now - Duration::hours(1),
                now + Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert!(!results.is_empty(), "Data should persist after restart");
        assert_eq!(results[0].value, 100.0);
    }
}

/// Test health check returns accurate status
#[tokio::test]
async fn test_storage_health_check() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let health = store.health_check().await.unwrap();
    assert!(health.healthy, "Storage should be healthy");
    assert_eq!(
        health.details.get("storage_type"),
        Some(&"parquet".to_string())
    );
}

/// Test multiple locations are partitioned correctly
#[tokio::test]
async fn test_multi_location_partitioning() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write to multiple locations
    for i in 0..3 {
        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: format!("location-{}", i),
            value: i as f64 * 10.0,
            tags: HashMap::new(),
        };
        store.write_batch(vec![point]).await.unwrap();
    }

    // Query each location separately
    for i in 0..3 {
        let results = store
            .query(
                &format!("location-{}", i),
                now - Duration::hours(1),
                now + Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            results.len(),
            1,
            "Should have exactly one point for location-{}",
            i
        );
        assert_eq!(results[0].value, i as f64 * 10.0);
    }
}

/// Test batch writing efficiency
#[tokio::test]
async fn test_batch_write_performance() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Create 1000 points (simulating multiple batch flushes)
    let points: Vec<TimeSeriesPoint> = (0..1000)
        .map(|i| TimeSeriesPoint {
            timestamp: now + Duration::milliseconds(i as i64),
            location_id: "batch-test".to_string(),
            value: i as f64,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    let start = std::time::Instant::now();
    store.write_batch(points).await.unwrap();
    let elapsed = start.elapsed();

    // Should complete within reasonable time (adjust threshold as needed)
    assert!(
        elapsed.as_secs() < 5,
        "Batch write took too long: {:?}",
        elapsed
    );

    // Verify all points stored
    let results = store
        .query(
            "batch-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1000, "All points should be stored");
}

// ========== WAL (WRITE-AHEAD LOG) TESTS ==========

/// Test WAL replay correctness - data written to WAL survives restart
#[tokio::test]
async fn test_wal_replay_correctness() {
    let temp_dir = TempDir::new().unwrap();
    let now = Utc::now();

    // Write data and close store without clean shutdown
    {
        let store = ParquetStore::new(temp_dir.path()).unwrap();
        let points = vec![
            TimeSeriesPoint {
                timestamp: now,
                location_id: "wal-test".to_string(),
                value: 123.45,
                tags: HashMap::new(),
                ndp_id: None,
                context: None,
            },
            TimeSeriesPoint {
                timestamp: now + Duration::seconds(1),
                location_id: "wal-test".to_string(),
                value: 678.90,
                tags: HashMap::new(),
                ndp_id: None,
                context: None,
            },
        ];
        store.write_batch(points).await.unwrap();
    }

    // Restart and replay WAL
    {
        let store = ParquetStore::new(temp_dir.path()).unwrap();
        store.replay_wal().await.unwrap();

        let results = store
            .query(
                "wal-test",
                now - Duration::hours(1),
                now + Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 2, "Both WAL entries should be replayed");
        assert!(results.iter().any(|p| (p.value - 123.45).abs() < 0.001));
        assert!(results.iter().any(|p| (p.value - 678.90).abs() < 0.001));
    }
}

/// Test WAL handles empty replays gracefully
#[tokio::test]
async fn test_wal_replay_empty() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Replay on fresh store should not error
    let result = store.replay_wal().await;
    assert!(result.is_ok(), "Empty WAL replay should succeed");
}

// ========== AGGREGATION QUERY TESTS ==========

/// Test aggregation query - mean
#[tokio::test]
async fn test_aggregation_mean() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write points with known values
    let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
    let points: Vec<TimeSeriesPoint> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| TimeSeriesPoint {
            timestamp: now + Duration::minutes(i as i64),
            location_id: "agg-test".to_string(),
            value: v,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    // Query with mean aggregation
    let results = store
        .aggregate(
            "agg-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            AggregationType::Mean,
            Duration::hours(1),
        )
        .await
        .unwrap();

    assert!(!results.is_empty(), "Should have aggregated results");
    assert_eq!(results[0].value, 30.0, "Mean should be 30.0");
}

/// Test aggregation query - sum
#[tokio::test]
async fn test_aggregation_sum() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let points: Vec<TimeSeriesPoint> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| TimeSeriesPoint {
            timestamp: now + Duration::minutes(i as i64),
            location_id: "sum-test".to_string(),
            value: v,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    let results = store
        .aggregate(
            "sum-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            AggregationType::Sum,
            Duration::hours(1),
        )
        .await
        .unwrap();

    assert_eq!(results[0].value, 15.0, "Sum should be 15.0");
}

/// Test aggregation query - max
#[tokio::test]
async fn test_aggregation_max() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let values = vec![10.0, 50.0, 30.0, 20.0, 40.0];
    let points: Vec<TimeSeriesPoint> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| TimeSeriesPoint {
            timestamp: now + Duration::minutes(i as i64),
            location_id: "max-test".to_string(),
            value: v,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    let results = store
        .aggregate(
            "max-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            AggregationType::Max,
            Duration::hours(1),
        )
        .await
        .unwrap();

    assert_eq!(results[0].value, 50.0, "Max should be 50.0");
}

/// Test aggregation query - min
#[tokio::test]
async fn test_aggregation_min() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let values = vec![10.0, 50.0, 5.0, 20.0, 40.0];
    let points: Vec<TimeSeriesPoint> = values
        .iter()
        .enumerate()
        .map(|(i, &v)| TimeSeriesPoint {
            timestamp: now + Duration::minutes(i as i64),
            location_id: "min-test".to_string(),
            value: v,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    let results = store
        .aggregate(
            "min-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            AggregationType::Min,
            Duration::hours(1),
        )
        .await
        .unwrap();

    assert_eq!(results[0].value, 5.0, "Min should be 5.0");
}

// ========== TIME RANGE FILTERING TESTS ==========

/// Test time range filtering - exact boundaries
#[tokio::test]
async fn test_time_range_exact_boundaries() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let base_time = Utc::now();

    // Write 10 points at 1-minute intervals
    let points: Vec<TimeSeriesPoint> = (0..10)
        .map(|i| TimeSeriesPoint {
            timestamp: base_time + Duration::minutes(i),
            location_id: "time-test".to_string(),
            value: i as f64,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    // Query for minutes 3-7 (inclusive)
    let start = base_time + Duration::minutes(3);
    let end = base_time + Duration::minutes(7);

    let results = store.query("time-test", start, end, None).await.unwrap();

    // Should get points at minutes 3, 4, 5, 6, 7
    assert!(
        results.len() >= 4 && results.len() <= 5,
        "Should get 4-5 points in range, got {}",
        results.len()
    );

    for point in results {
        assert!(
            point.timestamp >= start && point.timestamp <= end,
            "Point timestamp should be within range"
        );
    }
}

/// Test time range filtering - no data in range
#[tokio::test]
async fn test_time_range_no_data() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let base_time = Utc::now();

    // Write data in the past
    let point = TimeSeriesPoint {
        timestamp: base_time - Duration::days(10),
        location_id: "empty-test".to_string(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };
    store.write_batch(vec![point]).await.unwrap();

    // Query for future data
    let start = base_time + Duration::days(1);
    let end = base_time + Duration::days(2);

    let results = store.query("empty-test", start, end, None).await.unwrap();

    assert_eq!(
        results.len(),
        0,
        "Should return empty results for future time range"
    );
}

/// Test time range filtering - cross-day boundaries
#[tokio::test]
async fn test_time_range_cross_day_boundaries() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let base_time = Utc::now();

    // Write points across multiple days
    let points: Vec<TimeSeriesPoint> = (0..3)
        .map(|i| TimeSeriesPoint {
            timestamp: base_time + Duration::days(i),
            location_id: "multiday-test".to_string(),
            value: i as f64 * 100.0,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    store.write_batch(points).await.unwrap();

    // Query across all days
    let start = base_time - Duration::hours(1);
    let end = base_time + Duration::days(3);

    let results = store
        .query("multiday-test", start, end, None)
        .await
        .unwrap();

    assert_eq!(
        results.len(),
        3,
        "Should retrieve all points across multiple days"
    );
}

// ========== INVALID INPUT HANDLING TESTS ==========

/// Test handling of empty location_id
#[tokio::test]
async fn test_invalid_empty_location_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: "".to_string(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };

    // Should handle empty location_id gracefully
    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle empty location_id");
}

/// Test handling of NaN values
#[tokio::test]
async fn test_invalid_nan_values() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: "nan-test".to_string(),
        value: f64::NAN,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };

    // Should handle NaN values
    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle NaN values");
}

/// Test handling of infinite values
#[tokio::test]
async fn test_invalid_infinity_values() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: "inf-test".to_string(),
        value: f64::INFINITY,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };

    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle infinity values");
}

/// Test handling of reversed time range (end before start)
#[tokio::test]
async fn test_invalid_reversed_time_range() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write some data
    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: "reverse-test".to_string(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };
    store.write_batch(vec![point]).await.unwrap();

    // Query with reversed range
    let start = now + Duration::hours(1);
    let end = now - Duration::hours(1);

    let results = store.query("reverse-test", start, end, None).await.unwrap();

    // Should return empty results or handle gracefully
    assert_eq!(
        results.len(),
        0,
        "Reversed time range should return no results"
    );
}

/// Test handling of empty batch write
#[tokio::test]
async fn test_invalid_empty_batch() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    let result = store.write_batch(vec![]).await;
    assert!(result.is_ok(), "Empty batch should be handled gracefully");
}

// ========== CONCURRENT ACCESS TESTS ==========

/// Test concurrent writes to different locations
#[tokio::test]
async fn test_concurrent_writes_different_locations() {
    let temp_dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
    let now = Utc::now();

    // Spawn concurrent write tasks
    let mut handles = vec![];

    for i in 0..5 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            let point = TimeSeriesPoint {
                timestamp: now,
                location_id: format!("concurrent-{}", i),
                value: i as f64,
                tags: HashMap::new(),
            };
            store_clone.write_batch(vec![point]).await
        });
        handles.push(handle);
    }

    // Wait for all writes to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent write should succeed");
    }

    // Verify all data was written
    for i in 0..5 {
        let results = store
            .query(
                &format!("concurrent-{}", i),
                now - Duration::hours(1),
                now + Duration::hours(1),
                None,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1, "Each location should have one point");
    }
}

/// Test concurrent reads of same location
#[tokio::test]
async fn test_concurrent_reads_same_location() {
    let temp_dir = TempDir::new().unwrap();
    let store = std::sync::Arc::new(ParquetStore::new(temp_dir.path()).unwrap());
    let now = Utc::now();

    // Write test data
    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: "read-test".to_string(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };
    store.write_batch(vec![point]).await.unwrap();

    // Spawn concurrent read tasks
    let mut handles = vec![];

    for _ in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone
                .query(
                    "read-test",
                    now - Duration::hours(1),
                    now + Duration::hours(1),
                    None,
                )
                .await
        });
        handles.push(handle);
    }

    // All reads should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent read should succeed");
        assert_eq!(result.unwrap().len(), 1);
    }
}

// ========== STRESS TESTS ==========

/// Test large batch size (simulating AIR-002 batch size of 100)
#[tokio::test]
async fn test_air002_batch_size() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write exactly 100 points (AIR-002 batch size)
    let points: Vec<TimeSeriesPoint> = (0..100)
        .map(|i| TimeSeriesPoint {
            timestamp: now + Duration::seconds(i),
            location_id: "air002-test".to_string(),
            value: i as f64,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        })
        .collect();

    let start = std::time::Instant::now();
    store.write_batch(points).await.unwrap();
    let elapsed = start.elapsed();

    // Should complete within 5 seconds (AIR-002 timeout)
    assert!(
        elapsed.as_secs() < 5,
        "Batch should complete within timeout"
    );

    let results = store
        .query(
            "air002-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 100, "Should have all 100 points");
}

/// Test multiple batches in sequence
#[tokio::test]
async fn test_multiple_sequential_batches() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    // Write 10 batches of 100 points each
    for batch_num in 0..10 {
        let points: Vec<TimeSeriesPoint> = (0..100)
            .map(|i| TimeSeriesPoint {
                timestamp: now + Duration::seconds((batch_num * 100 + i) as i64),
                location_id: "sequential-test".to_string(),
                value: (batch_num * 100 + i) as f64,
                tags: HashMap::new(),
                ndp_id: None,
                context: None,
            })
            .collect();

        store.write_batch(points).await.unwrap();
    }

    let results = store
        .query(
            "sequential-test",
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(
        results.len(),
        1000,
        "Should have all 1000 points from 10 batches"
    );
}

// ========== EDGE CASE TESTS ==========

/// Test query for non-existent location
#[tokio::test]
async fn test_query_nonexistent_location() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let results = store
        .query(
            "does-not-exist",
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(
        results.len(),
        0,
        "Non-existent location should return empty results"
    );
}

/// Test very long location IDs
#[tokio::test]
async fn test_long_location_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let long_id = "a".repeat(500);
    let point = TimeSeriesPoint {
        timestamp: now,
        location_id: long_id.clone(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };

    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle long location IDs");

    let results = store
        .query(
            &long_id,
            now - Duration::hours(1),
            now + Duration::hours(1),
            None,
        )
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
}

/// Test special characters in location IDs
#[tokio::test]
async fn test_special_characters_in_location_id() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();
    let now = Utc::now();

    let special_ids = vec![
        "sensor/with/slashes",
        "sensor-with-dashes",
        "sensor_with_underscores",
        "sensor.with.dots",
    ];

    for id in &special_ids {
        let point = TimeSeriesPoint {
            timestamp: now,
            location_id: id.to_string(),
            value: 42.0,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        };

        let result = store.write_batch(vec![point]).await;
        assert!(
            result.is_ok(),
            "Should handle special characters in ID: {}",
            id
        );
    }
}

/// Test extreme timestamp values
#[tokio::test]
async fn test_extreme_timestamp_values() {
    let temp_dir = TempDir::new().unwrap();
    let store = ParquetStore::new(temp_dir.path()).unwrap();

    // Test far future timestamp
    let far_future = Utc::now() + Duration::days(365 * 100); // 100 years
    let point = TimeSeriesPoint {
        timestamp: far_future,
        location_id: "future-test".to_string(),
        value: 42.0,
        tags: HashMap::new(),
        ndp_id: None,
        context: None,
    };

    let result = store.write_batch(vec![point]).await;
    assert!(result.is_ok(), "Should handle far future timestamps");
}
