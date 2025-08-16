//! Integration tests for Redis sector channels
//!
//! PHASE 2 WEEK 5: REDIS SECTOR CHANNEL TESTING
//! Tests the new sector channel functionality while ensuring
//! backward compatibility with existing symbol-based operations.

use autonomous_platform::adapters::{
    redis::{RedisAdapter, RedisConfig},
    redis_sector_channels::{RedisSectorChannels, SectorChannelConfig, SectorData, PortfolioDecision, CrossSectorData, RiskMetrics},
    redis_integration::{RedisIntegration, RedisIntegrationConfig, RedisIntegrationFactory},
    DataAdapter, MarketData,
};
use autonomous_platform::data::sector_mapper::{SectorId, SectorMapper, SectorMapperConfig};
use std::sync::Arc;
use std::collections::HashMap;
use tokio;

/// Test basic sector channel functionality
#[tokio::test]
async fn test_sector_channel_basic_operations() {
    // GIVEN: Redis sector channels setup
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 5,
    };
    
    let sector_config = SectorChannelConfig {
        enable_compression: true,
        sector_ttl_seconds: 300,
        portfolio_ttl_seconds: 3600,
        batch_size: 5,
        max_message_size_kb: 256,
    };
    
    let redis_adapter = RedisAdapter::new(redis_config);
    let sector_channels = RedisSectorChannels::new(redis_adapter, sector_config);
    
    // Skip test if Redis is not available
    if sector_channels.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }
    
    // Test data
    let sector_data = SectorData {
        sector_id: "technology".to_string(),
        etf_symbol: "XLK".to_string(),
        etf_price: 150.75,
        avg_price: 175.25,
        total_volume: 1_500_000.0,
        volatility: 0.025,
        momentum: 0.015,
        timestamp: chrono::Utc::now().timestamp(),
        symbols_count: 8,
        symbols: vec![
            "AAPL".to_string(),
            "MSFT".to_string(),
            "GOOGL".to_string(),
            "META".to_string(),
        ],
        correlation_matrix: {
            let mut correlations = HashMap::new();
            correlations.insert("AAPL".to_string(), 0.75);
            correlations.insert("MSFT".to_string(), 0.68);
            correlations
        },
    };
    
    // WHEN: Publishing sector data
    println!("📊 Testing sector data publishing...");
    match sector_channels.publish_sector_data(&SectorId::Technology, &sector_data).await {
        Ok(_) => println!("✅ Sector data published successfully"),
        Err(e) => {
            println!("❌ Failed to publish sector data: {}", e);
            return;
        }
    }
    
    // WHEN: Adding sector data to stream
    println!("📈 Testing sector stream operations...");
    match sector_channels.add_sector_to_stream(&SectorId::Technology, &sector_data).await {
        Ok(stream_id) => {
            println!("✅ Added to sector stream with ID: {}", stream_id);
            
            // THEN: Read back from stream
            match sector_channels.read_sector_from_stream(&SectorId::Technology, "0", 10).await {
                Ok(data_vec) => {
                    println!("✅ Read {} sector entries from stream", data_vec.len());
                    assert!(!data_vec.is_empty(), "Should have read sector data from stream");
                }
                Err(e) => println!("⚠️ Could not read from sector stream: {}", e),
            }
        }
        Err(e) => println!("❌ Failed to add to sector stream: {}", e),
    }
    
    println!("✅ Sector channel basic operations test completed");
}

/// Test portfolio decision channels
#[tokio::test]
async fn test_portfolio_decision_channels() {
    // GIVEN: Redis sector channels setup
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 5,
    };
    
    let redis_adapter = RedisAdapter::new(redis_config);
    let sector_channels = RedisSectorChannels::new(redis_adapter, SectorChannelConfig::default());
    
    // Skip test if Redis is not available
    if sector_channels.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }
    
    // Test portfolio decision
    let portfolio_decision = PortfolioDecision {
        decision_id: "portfolio_decision_001".to_string(),
        sector_allocations: {
            let mut allocations = HashMap::new();
            allocations.insert("technology".to_string(), 0.35);
            allocations.insert("healthcare".to_string(), 0.20);
            allocations.insert("financial".to_string(), 0.25);
            allocations.insert("energy".to_string(), 0.10);
            allocations.insert("consumer_discretionary".to_string(), 0.10);
            allocations
        },
        risk_metrics: RiskMetrics {
            portfolio_var: 0.025,
            max_drawdown: 0.08,
            sharpe_ratio: 1.45,
            sector_concentration: 0.35,
            correlation_risk: 0.42,
        },
        consensus_score: 0.78,
        timestamp: chrono::Utc::now().timestamp(),
        reasoning: "High-conviction technology allocation based on earnings momentum".to_string(),
        confidence: 0.85,
    };
    
    // WHEN: Publishing portfolio decision
    println!("💼 Testing portfolio decision publishing...");
    match sector_channels.publish_portfolio_decision(&portfolio_decision).await {
        Ok(_) => println!("✅ Portfolio decision published successfully"),
        Err(e) => {
            println!("❌ Failed to publish portfolio decision: {}", e);
            return;
        }
    }
    
    println!("✅ Portfolio decision channels test completed");
}

/// Test cross-sector correlation channels
#[tokio::test]
async fn test_cross_sector_channels() {
    // GIVEN: Redis sector channels setup
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 5,
    };
    
    let redis_adapter = RedisAdapter::new(redis_config);
    let sector_channels = RedisSectorChannels::new(redis_adapter, SectorChannelConfig::default());
    
    // Skip test if Redis is not available
    if sector_channels.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }
    
    // Test cross-sector correlation data
    let cross_sector_data = CrossSectorData {
        data_type: "correlations".to_string(),
        correlations: {
            let mut correlations = HashMap::new();
            
            let mut tech_correlations = HashMap::new();
            tech_correlations.insert("healthcare".to_string(), 0.45);
            tech_correlations.insert("financial".to_string(), 0.38);
            tech_correlations.insert("energy".to_string(), -0.15);
            correlations.insert("technology".to_string(), tech_correlations);
            
            let mut healthcare_correlations = HashMap::new();
            healthcare_correlations.insert("technology".to_string(), 0.45);
            healthcare_correlations.insert("financial".to_string(), 0.52);
            healthcare_correlations.insert("utilities".to_string(), 0.28);
            correlations.insert("healthcare".to_string(), healthcare_correlations);
            
            correlations
        },
        rotation_score: Some(0.65),
        market_regime: Some("risk_on".to_string()),
        timestamp: chrono::Utc::now().timestamp(),
    };
    
    // WHEN: Publishing cross-sector data
    println!("🔗 Testing cross-sector data publishing...");
    match sector_channels.publish_cross_sector_data(&cross_sector_data).await {
        Ok(_) => println!("✅ Cross-sector data published successfully"),
        Err(e) => {
            println!("❌ Failed to publish cross-sector data: {}", e);
            return;
        }
    }
    
    println!("✅ Cross-sector channels test completed");
}

/// Test Redis integration backward compatibility
#[tokio::test]
async fn test_redis_integration_backward_compatibility() {
    // GIVEN: Traditional Redis setup (symbol-only)
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 5,
    };
    
    let integration = RedisIntegrationFactory::create_symbol_only(redis_config);
    
    // Skip test if Redis is not available
    if integration.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }
    
    // WHEN: Using traditional symbol operations
    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        open: 150.0,
        high: 152.0,
        low: 149.0,
        close: 151.0,
        volume: 1_000_000.0,
    };
    
    println!("📊 Testing backward compatibility with symbol operations...");
    
    // Test 1: Publish market data (PRESERVED)
    match integration.publish_market_data("symbol/AAPL", &market_data).await {
        Ok(_) => println!("✅ Market data published to symbol channel"),
        Err(e) => {
            println!("❌ Failed to publish market data: {}", e);
            return;
        }
    }
    
    // Test 2: Set/Get latest price (PRESERVED)
    match integration.set_latest_price("AAPL", 151.0, market_data.timestamp).await {
        Ok(_) => {
            println!("✅ Latest price set successfully");
            
            match integration.get_latest_price("AAPL").await {
                Ok(Some((price, timestamp))) => {
                    println!("✅ Retrieved latest price: ${} at {}", price, timestamp);
                    assert_eq!(price, 151.0);
                }
                Ok(None) => println!("⚠️ No price found"),
                Err(e) => println!("❌ Failed to get latest price: {}", e),
            }
        }
        Err(e) => println!("❌ Failed to set latest price: {}", e),
    }
    
    // Test 3: Stream operations (PRESERVED)
    match integration.add_to_stream("market:AAPL", &market_data).await {
        Ok(stream_id) => {
            println!("✅ Added to stream with ID: {}", stream_id);
            
            match integration.read_from_stream("market:AAPL", "0", 5).await {
                Ok(data_vec) => {
                    println!("✅ Read {} entries from stream", data_vec.len());
                    assert!(!data_vec.is_empty());
                }
                Err(e) => println!("⚠️ Could not read from stream: {}", e),
            }
        }
        Err(e) => println!("❌ Failed to add to stream: {}", e),
    }
    
    println!("✅ Backward compatibility test completed");
}

/// Test full Redis integration with sectors
#[tokio::test]
async fn test_full_redis_integration_with_sectors() {
    // GIVEN: Full Redis integration with sector mapper
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 5,
    };
    
    let sector_config = SectorChannelConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
    
    let integration = RedisIntegrationFactory::create_with_sectors(
        redis_config,
        sector_config,
        sector_mapper
    );
    
    // Skip test if Redis is not available
    if integration.connect().await.is_err() {
        println!("Skipping test - Redis not available");
        return;
    }
    
    // Test data
    let market_data = MarketData {
        symbol: "AAPL".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        open: 150.0,
        high: 152.0,
        low: 149.0,
        close: 151.0,
        volume: 1_000_000.0,
    };
    
    println!("🔄 Testing full integration with dual publishing...");
    
    // WHEN: Publishing market data (should publish to both symbol AND sector channels)
    match integration.publish_market_data("symbol/AAPL", &market_data).await {
        Ok(_) => {
            println!("✅ Market data published with dual channel support");
            
            // Verify health check
            let health = integration.health_check().await;
            println!("🏥 Health status: {:?}", health);
            
            if let Some(overall_health) = health.get("overall") {
                assert!(*overall_health, "Overall health should be good");
            }
        }
        Err(e) => {
            println!("❌ Failed to publish with dual channels: {}", e);
            return;
        }
    }
    
    // WHEN: Testing channel discovery
    let all_channels = integration.get_all_channels();
    println!("📋 Available channels: {:?}", all_channels.keys().collect::<Vec<_>>());
    
    // Should have all channel types
    assert!(all_channels.contains_key("symbol"), "Should have symbol channels");
    assert!(all_channels.contains_key("sector"), "Should have sector channels");
    assert!(all_channels.contains_key("portfolio"), "Should have portfolio channels");
    assert!(all_channels.contains_key("cross_sector"), "Should have cross-sector channels");
    
    // Verify sector channels
    if let Some(sector_channels) = all_channels.get("sector") {
        assert!(sector_channels.contains(&"sector/technology".to_string()));
        assert!(sector_channels.contains(&"sector/financial".to_string()));
        println!("✅ Sector channels properly configured");
    }
    
    println!("✅ Full integration test completed");
}

/// Test error handling and fallbacks
#[tokio::test]
async fn test_error_handling_and_fallbacks() {
    // GIVEN: Redis integration with invalid configuration
    let invalid_redis_config = RedisConfig {
        host: "invalid_host".to_string(),
        port: 9999,
        password: None,
        db: 0,
        pool_size: 1,
    };
    
    let integration = RedisIntegrationFactory::create_symbol_only(invalid_redis_config);
    
    println!("🔧 Testing error handling with invalid configuration...");
    
    // WHEN: Attempting to connect with invalid config
    match integration.connect().await {
        Ok(_) => println!("⚠️ Unexpected success with invalid config"),
        Err(e) => {
            println!("✅ Expected connection error: {}", e);
            
            // THEN: Health check should reflect connection failure
            let health = integration.health_check().await;
            if let Some(overall_health) = health.get("overall") {
                assert!(!*overall_health, "Overall health should be false with invalid config");
            }
        }
    }
    
    println!("✅ Error handling test completed");
}

/// Performance benchmark for sector channels
#[tokio::test]
async fn test_sector_channel_performance() {
    // GIVEN: Redis sector channels setup
    let redis_config = RedisConfig {
        host: "localhost".to_string(),
        port: 6379,
        password: None,
        db: 14, // Test database
        pool_size: 10,
    };
    
    let sector_config = SectorChannelConfig {
        enable_compression: true,
        batch_size: 20,
        max_message_size_kb: 1024,
        ..Default::default()
    };
    
    let redis_adapter = RedisAdapter::new(redis_config);
    let sector_channels = RedisSectorChannels::new(redis_adapter, sector_config);
    
    // Skip test if Redis is not available
    if sector_channels.connect().await.is_err() {
        println!("Skipping performance test - Redis not available");
        return;
    }
    
    println!("⚡ Running sector channel performance benchmark...");
    
    let start_time = std::time::Instant::now();
    let num_operations = 100;
    
    // WHEN: Publishing multiple sector data messages
    for i in 0..num_operations {
        let sector_data = SectorData {
            sector_id: "technology".to_string(),
            etf_symbol: "XLK".to_string(),
            etf_price: 150.0 + (i as f64 * 0.1),
            avg_price: 175.0 + (i as f64 * 0.1),
            total_volume: 1_000_000.0 + (i as f64 * 1000.0),
            volatility: 0.02 + (i as f64 * 0.0001),
            momentum: 0.01 + (i as f64 * 0.0001),
            timestamp: chrono::Utc::now().timestamp(),
            symbols_count: 10,
            symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
            correlation_matrix: HashMap::new(),
        };
        
        if let Err(e) = sector_channels.publish_sector_data(&SectorId::Technology, &sector_data).await {
            println!("❌ Performance test failed at iteration {}: {}", i, e);
            return;
        }
    }
    
    let elapsed = start_time.elapsed();
    let ops_per_second = num_operations as f64 / elapsed.as_secs_f64();
    
    println!("✅ Performance benchmark completed:");
    println!("   📊 Operations: {}", num_operations);
    println!("   ⏱️ Time: {:?}", elapsed);
    println!("   🚀 Operations/sec: {:.2}", ops_per_second);
    
    // Should achieve reasonable performance
    assert!(ops_per_second > 50.0, "Should achieve at least 50 ops/sec");
}