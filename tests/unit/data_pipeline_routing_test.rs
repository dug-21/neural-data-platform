//! Unit tests for multi-scope data routing

use anyhow::Result;
use chrono::Utc;
use std::sync::Arc;

use autonomous_platform::data_pipeline::routing::{
    DataScope, DataPacket, MultiScopeRouter, RoutingConfig, GeographicRegion
};
use autonomous_platform::data::{TimeSeriesData, sector_mapper::{SectorMapper, SectorId}};

async fn create_minimal_router() -> Result<MultiScopeRouter> {
    let config = RoutingConfig {
        max_symbols_per_sector: 5,
        max_symbols_for_market: 10,
        enable_geographic_routing: true,
        high_priority_buffer_size: 100,
        routing_timeout_ms: 1000,
        enable_parallel_routing: true,
        max_concurrent_routes: 5,
    };
    
    let sector_mapper = Arc::new(SectorMapper::new()?);
    Ok(MultiScopeRouter::new(config, sector_mapper))
}

#[tokio::test]
async fn test_symbol_routing() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Register test symbol
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    
    // Create symbol-specific packet
    let mut data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
    data.intervals = vec![1000];
    let packet = router.create_packet(
        data,
        DataScope::Symbol("AAPL".to_string()),
        5,
        "test".to_string(),
    );
    
    // Route the packet
    let destination = router.route_by_scope(packet).await?;
    
    // Verify routing
    assert_eq!(destination.target_symbols.len(), 1);
    assert!(destination.target_symbols.contains("AAPL"));
    assert!(!destination.broadcast_all);
    assert_eq!(destination.priority, 5);
    
    Ok(())
}

#[tokio::test]
async fn test_sector_routing() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Register multiple tech symbols
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("GOOGL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("MSFT", GeographicRegion::NorthAmerica).await?;
    
    // Create sector-specific packet
    let data = TimeSeriesData::new("TECH_NEWS".to_string(), Utc::now());
    let packet = router.create_packet(
        data,
        DataScope::Sector(SectorId::Technology),
        6,
        "test".to_string(),
    );
    
    // Route the packet
    let destination = router.route_by_scope(packet).await?;
    
    // Verify routing
    assert!(destination.target_symbols.len() > 0);
    assert!(destination.target_sectors.contains(&SectorId::Technology));
    assert!(!destination.broadcast_all);
    assert_eq!(destination.priority, 6);
    assert!(destination.transformation_hints.contains_key("scope"));
    
    Ok(())
}

#[tokio::test]
async fn test_market_wide_routing() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Register symbols from different sectors
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("JPM", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("JNJ", GeographicRegion::NorthAmerica).await?;
    
    // Create market-wide packet
    let data = TimeSeriesData::new("MARKET_NEWS".to_string(), Utc::now());
    let packet = router.create_packet(
        data,
        DataScope::Market,
        7,
        "test".to_string(),
    );
    
    // Route the packet
    let destination = router.route_by_scope(packet).await?;
    
    // Verify routing
    assert_eq!(destination.target_symbols.len(), 3);
    assert!(destination.broadcast_all);
    assert_eq!(destination.priority, 7);
    assert!(destination.target_sectors.len() > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_geographic_routing() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Register symbols in different regions
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("SAP", GeographicRegion::Europe).await?;
    router.register_symbol("TSM", GeographicRegion::Asia).await?;
    
    // Create Europe-specific packet
    let data = TimeSeriesData::new("EU_POLICY".to_string(), Utc::now());
    let packet = router.create_packet(
        data,
        DataScope::Geographic(GeographicRegion::Europe),
        5,
        "test".to_string(),
    );
    
    // Route the packet
    let destination = router.route_by_scope(packet).await?;
    
    // Verify routing
    assert_eq!(destination.target_symbols.len(), 1);
    assert!(destination.target_symbols.contains("SAP"));
    assert!(!destination.target_symbols.contains("AAPL"));
    assert!(!destination.target_symbols.contains("TSM"));
    assert!(destination.target_regions.contains(&GeographicRegion::Europe));
    
    Ok(())
}

#[tokio::test]
async fn test_high_priority_buffering() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Create high priority packet
    let data = TimeSeriesData::new("URGENT".to_string(), Utc::now());
    let packet = router.create_packet(
        data,
        DataScope::Market,
        9, // High priority
        "test".to_string(),
    );
    
    // Route the packet
    let _destination = router.route_by_scope(packet).await?;
    
    // Check buffer status
    let (buffer_count, buffer_capacity) = router.get_buffer_status();
    assert_eq!(buffer_count, 1);
    assert_eq!(buffer_capacity, 100);
    
    Ok(())
}

#[tokio::test]
async fn test_routing_metrics() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Register a symbol
    router.register_symbol("TEST", GeographicRegion::NorthAmerica).await?;
    
    // Route multiple packets of different types
    let symbol_data = TimeSeriesData::new("TEST".to_string(), Utc::now());
    let symbol_packet = router.create_packet(
        symbol_data,
        DataScope::Symbol("TEST".to_string()),
        5,
        "test".to_string(),
    );
    
    let market_data = TimeSeriesData::new("MARKET".to_string(), Utc::now());
    let market_packet = router.create_packet(
        market_data,
        DataScope::Market,
        7,
        "test".to_string(),
    );
    
    let sector_data = TimeSeriesData::new("TECH".to_string(), Utc::now());
    let sector_packet = router.create_packet(
        sector_data,
        DataScope::Sector(SectorId::Technology),
        6,
        "test".to_string(),
    );
    
    // Route all packets
    router.route_by_scope(symbol_packet).await?;
    router.route_by_scope(market_packet).await?;
    router.route_by_scope(sector_packet).await?;
    
    // Check metrics
    let metrics = router.get_metrics().await;
    assert_eq!(metrics.total_packets, 3);
    assert_eq!(metrics.packets_by_scope.get("symbol").unwrap(), &1);
    assert_eq!(metrics.packets_by_scope.get("market").unwrap(), &1);
    assert_eq!(metrics.packets_by_scope.get("sector").unwrap(), &1);
    assert!(metrics.avg_routing_time_us > 0.0);
    
    Ok(())
}

#[tokio::test]
async fn test_packet_creation() -> Result<()> {
    let router = create_minimal_router().await?;
    
    let mut data = TimeSeriesData::new("TEST".to_string(), Utc::now());
    data.intervals = vec![1000]; // Add required intervals field
    let packet = router.create_packet(
        data.clone(),
        DataScope::Symbol("TEST".to_string()),
        5,
        "test_source".to_string(),
    );
    
    // Verify packet structure
    assert!(packet.id.starts_with("pkt_"));
    assert!(packet.id.contains("TEST"));
    assert_eq!(packet.scope, DataScope::Symbol("TEST".to_string()));
    assert_eq!(packet.data.symbol, data.symbol);
    assert_eq!(packet.priority, 5);
    assert_eq!(packet.source, "test_source");
    assert!(packet.metadata.is_empty());
    
    Ok(())
}

#[tokio::test]
async fn test_buffer_management() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Fill up the high priority buffer
    for i in 0..5 {
        let data = TimeSeriesData::new(format!("URGENT_{}", i), Utc::now());
        let packet = router.create_packet(
            data,
            DataScope::Market,
            9, // High priority
            "test".to_string(),
        );
        
        router.route_by_scope(packet).await?;
    }
    
    // Check buffer is filling up
    let (buffer_count, _) = router.get_buffer_status();
    assert_eq!(buffer_count, 5);
    
    // Clear buffer
    router.clear_priority_buffer().await;
    let (buffer_count_after, _) = router.get_buffer_status();
    assert_eq!(buffer_count_after, 0);
    
    Ok(())
}

#[tokio::test]
async fn test_sector_id_conversion() -> Result<()> {
    // Test SectorId::from_str functionality
    assert_eq!(SectorId::from_str("technology")?, SectorId::Technology);
    assert_eq!(SectorId::from_str("financial")?, SectorId::Financial);
    assert_eq!(SectorId::from_str("healthcare")?, SectorId::Healthcare);
    assert_eq!(SectorId::from_str("energy")?, SectorId::Energy);
    
    // Test invalid sector
    assert!(SectorId::from_str("invalid_sector").is_err());
    
    Ok(())
}

#[tokio::test]
async fn test_active_symbols_management() -> Result<()> {
    let router = create_minimal_router().await?;
    
    // Initially no symbols
    assert_eq!(router.get_active_symbols_count(), 0);
    
    // Register symbols
    router.register_symbol("AAPL", GeographicRegion::NorthAmerica).await?;
    router.register_symbol("GOOGL", GeographicRegion::NorthAmerica).await?;
    
    // Check count
    assert_eq!(router.get_active_symbols_count(), 2);
    
    // Check sector symbols
    let tech_symbols = router.get_sector_symbols(SectorId::Technology);
    assert!(tech_symbols.is_some());
    let symbols = tech_symbols.unwrap();
    assert!(symbols.contains("AAPL") || symbols.contains("GOOGL"));
    
    Ok(())
}