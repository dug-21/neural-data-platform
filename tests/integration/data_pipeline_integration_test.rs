//! Integration tests for the multi-scope data pipeline
//!
//! These tests verify that the data routing and consolidation system
//! works correctly with the existing VendorPredictor infrastructure.

use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::time::timeout;

use autonomous_platform::data_pipeline::{
    DataScope, DataPacket, MultiScopeRouter, DataConsolidator,
    RoutingConfig, ConsolidationConfig, GeographicRegion
};
use autonomous_platform::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorId}};

/// Helper function to create test time series data
fn create_test_data(symbol: &str, value: f64) -> TimeSeriesData {
    let mut data = TimeSeriesData::new(symbol.to_string(), Utc::now());
    data.open = value;
    data.high = value * 1.02;
    data.low = value * 0.98;
    data.close = value;
    data.volume = vec![100000.0];
    data.intervals = vec![1000]; // Add required intervals field
    data.add_value(value, Utc::now());
    data
}

/// Helper function to create test router
async fn create_test_router() -> Result<MultiScopeRouter> {
    let config = RoutingConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new()?);
    let router = MultiScopeRouter::new(config, sector_mapper);
    
    // Register some test symbols
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("GOOGL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("MSFT", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("SAP", GeographicRegion::Europe).await?;
    router.register_symbol("ASML", GeographicRegion::Europe).await?;
    router.register_symbol("TSM", GeographicRegion::Asia).await?;
    
    Ok(router)
}

#[tokio::test]
async fn test_end_to_end_data_pipeline() -> Result<()> {
    // Create router and consolidator
    let router = create_test_router().await?;
    let consolidator = DataConsolidator::new(ConsolidationConfig::default());
    
    // Create different types of data packets
    let symbol_data = create_test_data("AAPL", 150.0);
    let symbol_packet = router.create_packet(
        symbol_data,
        DataScope::Symbol("AAPL".to_string()),
        5,
        "polygon".to_string(),
    );
    
    let sector_data = create_test_data("TECH_SECTOR", 1000.0);
    let sector_packet = router.create_packet(
        sector_data,
        DataScope::Sector(SectorId::Technology),
        6,
        "sector_analysis".to_string(),
    );
    
    let market_data = create_test_data("SPY", 400.0);
    let market_packet = router.create_packet(
        market_data,
        DataScope::Market,
        7,
        "market_data".to_string(),
    );
    
    let geo_data = create_test_data("NORTH_AMERICA", 500.0);
    let geo_packet = router.create_packet(
        geo_data,
        DataScope::Geographic(GeographicRegion::NorthAmerica),
        5,
        "economic_data".to_string(),
    );
    
    // Test routing
    let symbol_route = router.route_by_scope(symbol_packet.clone()).await?;
    assert_eq!(symbol_route.target_symbols.len(), 1);
    assert!(symbol_route.target_symbols.contains("AAPL"));
    
    let sector_route = router.route_by_scope(sector_packet.clone()).await?;
    assert!(sector_route.target_symbols.len() > 0); // Should route to tech symbols
    assert!(sector_route.target_sectors.contains(&SectorId::Technology));
    
    let market_route = router.route_by_scope(market_packet.clone()).await?;
    assert!(market_route.broadcast_all);
    assert!(market_route.target_symbols.len() >= 3); // Should include all registered symbols
    
    let geo_route = router.route_by_scope(geo_packet.clone()).await?;
    assert!(geo_route.target_regions.contains(&GeographicRegion::NorthAmerica));
    
    // Test consolidation
    consolidator.ingest_packet(symbol_packet).await?;
    consolidator.ingest_packet(sector_packet).await?;
    consolidator.ingest_packet(market_packet).await?;
    consolidator.ingest_packet(geo_packet).await?;
    
    // Wait a bit for ingestion
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    
    // Consolidate data for AAPL
    let consolidation_result = consolidator.consolidate_for_symbol(
        "AAPL",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await?;
    
    assert_eq!(consolidation_result.consolidated_data.symbol, "AAPL");
    assert!(consolidation_result.sources_used >= 1); // Should have at least symbol data
    assert!(consolidation_result.confidence > 0.0);
    assert!(consolidation_result.quality_score > 0.0);
    
    // Verify the consolidated data contains expected metadata
    assert!(consolidation_result.consolidated_data.metadata_map.contains_key("consolidation_sources"));
    assert!(consolidation_result.consolidated_data.metadata_map.contains_key("consolidation_timestamp"));
    
    Ok(())
}

#[tokio::test]
async fn test_high_priority_routing() -> Result<()> {
    let router = create_test_router().await?;
    
    // Create high priority urgent market data
    let urgent_data = create_test_data("URGENT_NEWS", 100.0);
    let urgent_packet = router.create_packet(
        urgent_data,
        DataScope::Market,
        9, // High priority
        "breaking_news".to_string(),
    );
    
    // Route the high priority packet
    let route = router.route_by_scope(urgent_packet).await?;
    
    assert!(route.broadcast_all);
    assert_eq!(route.priority, 7); // Market data gets high priority
    
    // Check that the packet was buffered
    let (buffer_count, _) = router.get_buffer_status();
    assert_eq!(buffer_count, 1);
    
    Ok(())
}

#[tokio::test]
async fn test_geographic_routing() -> Result<()> {
    let router = create_test_router().await?;
    
    // Create Europe-specific economic data
    let eu_data = create_test_data("ECB_RATES", 0.05);
    let eu_packet = router.create_packet(
        eu_data,
        DataScope::Geographic(GeographicRegion::Europe),
        6,
        "ecb".to_string(),
    );
    
    // Route the packet
    let route = router.route_by_scope(eu_packet).await?;
    
    // Should only target European symbols
    assert!(route.target_symbols.contains("SAP") || route.target_symbols.contains("ASML"));
    assert!(!route.target_symbols.contains("AAPL"));
    assert!(route.target_regions.contains(&GeographicRegion::Europe));
    
    Ok(())
}

#[tokio::test]
async fn test_sector_based_routing() -> Result<()> {
    let router = create_test_router().await?;
    
    // Create technology sector news
    let tech_data = create_test_data("AI_BREAKTHROUGH", 200.0);
    let tech_packet = router.create_packet(
        tech_data,
        DataScope::Sector(SectorId::Technology),
        6,
        "tech_news".to_string(),
    );
    
    // Route the packet
    let route = router.route_by_scope(tech_packet).await?;
    
    // Should target tech symbols (AAPL, GOOGL, MSFT are all tech)
    assert!(route.target_symbols.len() > 0);
    assert!(route.target_sectors.contains(&SectorId::Technology));
    assert!(!route.broadcast_all);
    
    Ok(())
}

#[tokio::test]
async fn test_data_quality_filtering() -> Result<()> {
    let consolidator = DataConsolidator::new(ConsolidationConfig {
        min_quality_score: 0.8, // High quality threshold
        ..Default::default()
    });
    
    // Create low quality data (missing required fields)
    let mut low_quality_data = TimeSeriesData::new("BAD_DATA".to_string(), Utc::now());
    // Don't set OHLC values (will result in low completeness score)
    
    let router = create_test_router().await?;
    let low_quality_packet = router.create_packet(
        low_quality_data,
        DataScope::Symbol("BAD_DATA".to_string()),
        5,
        "unreliable_source".to_string(),
    );
    
    // Ingest should succeed but data should be filtered out
    consolidator.ingest_packet(low_quality_packet).await?;
    
    // Create high quality data
    let high_quality_data = create_test_data("GOOD_DATA", 100.0);
    let high_quality_packet = router.create_packet(
        high_quality_data,
        DataScope::Symbol("GOOD_DATA".to_string()),
        5,
        "polygon".to_string(),
    );
    
    consolidator.ingest_packet(high_quality_packet).await?;
    
    // Try to consolidate - should only use high quality data
    let result = consolidator.consolidate_for_symbol(
        "GOOD_DATA",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await;
    
    // Should succeed with the high quality data
    assert!(result.is_ok());
    
    Ok(())
}

#[tokio::test]
async fn test_temporal_alignment() -> Result<()> {
    let config = ConsolidationConfig {
        enable_temporal_alignment: true,
        temporal_alignment_tolerance_seconds: 60, // 1 minute buckets
        ..Default::default()
    };
    let consolidator = DataConsolidator::new(config);
    
    // Create data at slightly different times
    let router = create_test_router().await?;
    
    let now = Utc::now();
    let mut data1 = create_test_data("AAPL", 150.0);
    data1.timestamp = now;
    
    let mut data2 = create_test_data("SECTOR_DATA", 100.0);
    data2.timestamp = now + Duration::seconds(30); // 30 seconds later
    
    let packet1 = router.create_packet(
        data1,
        DataScope::Symbol("AAPL".to_string()),
        5,
        "source1".to_string(),
    );
    
    let packet2 = router.create_packet(
        data2,
        DataScope::Sector(SectorId::Technology),
        6,
        "source2".to_string(),
    );
    
    consolidator.ingest_packet(packet1).await?;
    consolidator.ingest_packet(packet2).await?;
    
    // Consolidate - should align timestamps
    let result = consolidator.consolidate_for_symbol(
        "AAPL",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await?;
    
    // Verify temporal alignment applied
    assert!(result.consolidated_data.metadata_map.contains_key("consolidation_timestamp"));
    
    Ok(())
}

#[tokio::test]
async fn test_performance_metrics() -> Result<()> {
    let router = create_test_router().await?;
    let consolidator = DataConsolidator::new(ConsolidationConfig::default());
    
    // Process multiple packets to generate metrics
    for i in 0..10 {
        let data = create_test_data(&format!("TEST_{}", i), 100.0 + i as f64);
        let packet = router.create_packet(
            data,
            DataScope::Symbol(format!("TEST_{}", i)),
            5,
            "test".to_string(),
        );
        
        // Route and ingest
        let _route = router.route_by_scope(packet.clone()).await?;
        consolidator.ingest_packet(packet).await?;
    }
    
    // Get routing metrics
    let routing_metrics = router.get_metrics().await;
    assert_eq!(routing_metrics.total_packets, 10);
    assert!(routing_metrics.avg_routing_time_us > 0.0);
    assert_eq!(routing_metrics.packets_by_scope.get("symbol").unwrap(), &10);
    
    // Perform a consolidation to generate consolidation metrics
    let _result = consolidator.consolidate_for_symbol(
        "TEST_0",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await?;
    
    let consolidation_metrics = consolidator.get_metrics().await;
    assert_eq!(consolidation_metrics.total_consolidations, 1);
    assert!(consolidation_metrics.avg_consolidation_time_ms > 0.0);
    
    Ok(())
}

#[tokio::test]
async fn test_vendor_predictor_compatibility() -> Result<()> {
    // This test verifies that the consolidated data format is compatible
    // with VendorPredictor input requirements
    
    let router = create_test_router().await?;
    let consolidator = DataConsolidator::new(ConsolidationConfig::default());
    
    // Create realistic market data
    let symbol_data = create_test_data("AAPL", 150.25);
    let packet = router.create_packet(
        symbol_data,
        DataScope::Symbol("AAPL".to_string()),
        5,
        "polygon".to_string(),
    );
    
    consolidator.ingest_packet(packet).await?;
    
    let result = consolidator.consolidate_for_symbol(
        "AAPL",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await?;
    
    let consolidated_data = &result.consolidated_data;
    
    // Verify VendorPredictor compatible fields
    assert_eq!(consolidated_data.symbol, "AAPL");
    assert!(consolidated_data.close > 0.0);
    assert!(consolidated_data.high >= consolidated_data.low);
    assert!(!consolidated_data.volume.is_empty());
    assert!(!consolidated_data.values.is_empty());
    assert_eq!(consolidated_data.values.len(), consolidated_data.timestamps.len());
    
    // Verify enhanced fields for vendor conversion
    assert!(consolidated_data.metadata_map.contains_key("consolidation_sources"));
    
    // Test data validation
    assert!(consolidated_data.validate().is_ok());
    
    Ok(())
}

#[tokio::test]
async fn test_cleanup_old_data() -> Result<()> {
    let config = ConsolidationConfig {
        consolidation_window_minutes: 1, // Very short window for testing
        ..Default::default()
    };
    let consolidator = DataConsolidator::new(config);
    
    // Create old data
    let router = create_test_router().await?;
    let old_data = create_test_data("OLD", 100.0);
    let mut old_packet = router.create_packet(
        old_data,
        DataScope::Symbol("OLD".to_string()),
        5,
        "test".to_string(),
    );
    
    // Make it old
    old_packet.created_at = Utc::now() - Duration::minutes(5);
    
    // Create new data
    let new_data = create_test_data("NEW", 200.0);
    let new_packet = router.create_packet(
        new_data,
        DataScope::Symbol("NEW".to_string()),
        5,
        "test".to_string(),
    );
    
    consolidator.ingest_packet(old_packet).await?;
    consolidator.ingest_packet(new_packet).await?;
    
    // Cleanup old data
    consolidator.cleanup_old_data().await?;
    
    // Try to consolidate - should work for new data but not have old data
    let result = consolidator.consolidate_for_symbol(
        "NEW",
        SectorId::Technology,
        GeographicRegion::NorthAmerica,
    ).await;
    
    assert!(result.is_ok());
    
    Ok(())
}