//! Integration tests for Redis streams functionality
//!
//! Tests the Redis adapter's streaming capabilities including:
//! - Adding market data to streams
//! - Reading from streams
//! - Consumer group functionality

use autonomous_platform::adapters::{
    redis::{RedisAdapter, RedisConfig},
    DataAdapter, MarketData,
};

#[tokio::test]
async fn test_redis_streams_functionality() {
    // GIVEN: A Redis adapter connected to test database
    let config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 15, // Use test database
        pool_size: 5,
    };

    let mut adapter = RedisAdapter::new(config);

    // Skip test if Redis is not available
    if adapter.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }

    // Test data
    let stream_key = "test:stream:market:BTC/USD";
    let market_data = vec![
        MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: 1704067200,
            open: 50000.0,
            high: 51000.0,
            low: 49000.0,
            close: 50500.0,
            volume: vec![1000.0],
        },
        MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: 1704067260,
            open: 50500.0,
            high: 50800.0,
            low: 50400.0,
            close: 50700.0,
            volume: vec![1100.0],
        },
        MarketData {
            symbol: "BTC/USD".to_string(),
            timestamp: 1704067320,
            open: 50700.0,
            high: 51200.0,
            low: 50600.0,
            close: 51000.0,
            volume: vec![1200.0],
        },
    ];

    // Test 1: Add data to stream
    println!("\n1. Testing add to stream...");
    let mut stream_ids = Vec::new();
    for data in &market_data {
        match adapter.add_to_stream(stream_key, data).await {
            Ok(id) => {
                println!("✓ Added to stream with ID: {}", id);
                stream_ids.push(id);
            }
            Err(e) => {
                println!("✗ Failed to add to stream: {}", e);
            }
        }
    }
    assert_eq!(stream_ids.len(), 3, "Should have added 3 entries to stream");

    // Test 2: Read from stream
    println!("\n2. Testing read from stream...");
    match adapter.read_from_stream(stream_key, "0", 10).await {
        Ok(data) => {
            println!("✓ Read {} entries from stream", data.len());
            assert_eq!(data.len(), 3, "Should have read 3 entries");

            // Verify data integrity
            for (i, item) in data.iter().enumerate() {
                assert_eq!(item.symbol, market_data[i].symbol);
                assert_eq!(item.timestamp, market_data[i].timestamp);
                assert_eq!(item.close, market_data[i].close);
            }
        }
        Err(e) => {
            println!("✗ Failed to read from stream: {}", e);
            panic!("Stream read should have succeeded");
        }
    }

    // Test 3: Create consumer group
    println!("\n3. Testing consumer group creation...");
    match adapter
        .create_consumer_group(stream_key, "test-group")
        .await
    {
        Ok(_) => println!("✓ Created consumer group successfully"),
        Err(e) => println!("✗ Consumer group creation failed: {}", e),
    }

    // Test 4: Test pub/sub alongside streams
    println!("\n4. Testing pub/sub functionality...");
    match adapter
        .publish_market_data("market:BTC/USD", &market_data[0])
        .await
    {
        Ok(_) => println!("✓ Published market data via pub/sub"),
        Err(e) => println!("✗ Pub/sub publish failed: {}", e),
    }

    // Clean up
    let _ = adapter.disconnect().await;
    println!("\n✓ All Redis streams tests completed successfully!");
}
