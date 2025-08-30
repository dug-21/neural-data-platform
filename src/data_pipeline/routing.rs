//! Multi-Scope Data Routing System
//!
//! This module implements efficient data routing based on different scopes:
//! - Symbol-specific: data affecting individual symbols
//! - Sector-wide: data affecting entire sectors  
//! - Market-wide: data affecting all symbols
//! - Geographic: region-specific data
//!
//! The routing system is designed for high-frequency trading scenarios
//! with optimized performance and memory usage.

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};

// Import existing types for integration
use crate::data::{TimeSeriesData, sector_mapper::{SectorId, SectorMapper}};

/// Data scope enumeration defining the breadth of data impact
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataScope {
    /// Data specific to a single symbol
    Symbol(String),
    /// Data affecting an entire sector
    Sector(SectorId),
    /// Data affecting the entire market
    Market,
    /// Data specific to a geographic region
    Geographic(GeographicRegion),
}

/// Geographic regions for data routing
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeographicRegion {
    NorthAmerica,
    Europe,
    Asia,
    EmergingMarkets,
    Global,
}

/// Data packet containing the data and its scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPacket {
    /// Unique identifier for this data packet
    pub id: String,
    /// The scope of this data's impact
    pub scope: DataScope,
    /// The actual time series data
    pub data: TimeSeriesData,
    /// Priority level for routing (1-10, higher = more urgent)
    pub priority: u8,
    /// Timestamp when the packet was created
    pub created_at: DateTime<Utc>,
    /// Optional metadata for routing decisions
    pub metadata: HashMap<String, serde_json::Value>,
    /// Source of the data (e.g., "polygon", "alpha_vantage")
    pub source: String,
}

/// Routing destination for data packets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDestination {
    /// Target symbols that should receive this data
    pub target_symbols: HashSet<String>,
    /// Sector IDs that should receive this data
    pub target_sectors: HashSet<SectorId>,
    /// Whether this should go to all symbols (market-wide)
    pub broadcast_all: bool,
    /// Geographic regions that should receive this data
    pub target_regions: HashSet<GeographicRegion>,
    /// Processing priority (1-10)
    pub priority: u8,
    /// Optional transformation hints
    pub transformation_hints: HashMap<String, String>,
}

/// Configuration for the multi-scope router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Maximum number of symbols to route to for sector-wide data
    pub max_symbols_per_sector: usize,
    /// Maximum number of symbols to route to for market-wide data
    pub max_symbols_for_market: usize,
    /// Enable geographic routing
    pub enable_geographic_routing: bool,
    /// Buffer size for high-priority packets
    pub high_priority_buffer_size: usize,
    /// Timeout for routing operations in milliseconds
    pub routing_timeout_ms: u64,
    /// Enable parallel routing
    pub enable_parallel_routing: bool,
    /// Maximum concurrent routing operations
    pub max_concurrent_routes: usize,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            max_symbols_per_sector: 50,
            max_symbols_for_market: 200,
            enable_geographic_routing: true,
            high_priority_buffer_size: 1000,
            routing_timeout_ms: 100,
            enable_parallel_routing: true,
            max_concurrent_routes: 10,
        }
    }
}

/// Routing performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RoutingMetrics {
    /// Total packets routed
    pub total_packets: u64,
    /// Packets routed by scope type
    pub packets_by_scope: HashMap<String, u64>,
    /// Average routing time in microseconds
    pub avg_routing_time_us: f64,
    /// High priority packets processed
    pub high_priority_packets: u64,
    /// Failed routing attempts
    pub failed_routes: u64,
    /// Current buffer utilization
    pub buffer_utilization: f64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Multi-scope data router implementation
pub struct MultiScopeRouter {
    /// Configuration
    config: RoutingConfig,
    /// Sector mapper for symbol-to-sector lookups
    sector_mapper: Arc<SectorMapper>,
    /// Geographic symbol mapping
    geographic_mapping: Arc<DashMap<String, GeographicRegion>>,
    /// Active symbols registry
    active_symbols: Arc<DashMap<String, DateTime<Utc>>>,
    /// Sector-to-symbols mapping cache
    sector_symbols_cache: Arc<DashMap<SectorId, HashSet<String>>>,
    /// Performance metrics
    metrics: Arc<RwLock<RoutingMetrics>>,
    /// High priority packet buffer
    priority_buffer: Arc<DashMap<String, DataPacket>>,
}

impl MultiScopeRouter {
    /// Create a new multi-scope router
    pub fn new(
        config: RoutingConfig,
        sector_mapper: Arc<SectorMapper>,
    ) -> Self {
        Self {
            config,
            sector_mapper,
            geographic_mapping: Arc::new(DashMap::new()),
            active_symbols: Arc::new(DashMap::new()),
            sector_symbols_cache: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(RoutingMetrics::default())),
            priority_buffer: Arc::new(DashMap::new()),
        }
    }

    /// Register a symbol as active for routing
    pub async fn register_symbol(&self, symbol: &str, region: &GeographicRegion) -> Result<()> {
        self.active_symbols.insert(symbol.to_string(), Utc::now());
        self.geographic_mapping.insert(symbol.to_string(), region.clone());
        
        // Update sector cache
        if let Ok(sector_info) = self.sector_mapper.get_sector(symbol) {
            let sector_id = SectorId::from_str(&sector_info.id).unwrap_or(SectorId::Technology);
            self.sector_symbols_cache
                .entry(sector_id)
                .or_insert_with(HashSet::new)
                .insert(symbol.to_string());
        }
        
        debug!("Registered symbol {} for routing in region {:?}", symbol, region);
        Ok(())
    }

    /// Route a data packet by its scope
    #[instrument(skip(self, packet), fields(packet_id = %packet.id, scope = ?packet.scope))]
    pub async fn route_by_scope(&self, packet: DataPacket) -> Result<RouteDestination> {
        let start_time = std::time::Instant::now();
        
        // Handle high priority packets
        if packet.priority >= 8 {
            self.handle_high_priority_packet(&packet).await?;
        }
        
        let destination = match &packet.scope {
            DataScope::Symbol(symbol) => self.route_to_symbol(symbol.clone()).await?,
            DataScope::Sector(sector) => self.route_to_sector_symbols(*sector).await?,
            DataScope::Market => self.route_to_all_symbols().await?,
            DataScope::Geographic(region) => self.route_to_geo_symbols(region.clone()).await?,
        };
        
        // Update metrics
        self.update_routing_metrics(&packet, start_time.elapsed()).await;
        
        info!("Routed packet {} to {} targets", 
              packet.id, destination.target_symbols.len());
        
        Ok(destination)
    }

    /// Route data to a specific symbol
    async fn route_to_symbol(&self, symbol: String) -> Result<RouteDestination> {
        let mut target_symbols = HashSet::new();
        target_symbols.insert(symbol.clone());
        
        // Get the symbol's sector for additional context
        let target_sectors = if let Ok(sector_info) = self.sector_mapper.get_sector(&symbol) {
            let sector_id = SectorId::from_str(&sector_info.id).unwrap_or(SectorId::Technology);
            let mut sectors = HashSet::new();
            sectors.insert(sector_id);
            sectors
        } else {
            HashSet::new()
        };
        
        // Get the symbol's region
        let target_regions = self.geographic_mapping
            .get(&symbol)
            .map(|region| {
                let mut regions = HashSet::new();
                regions.insert(region.clone());
                regions
            })
            .unwrap_or_default();
        
        Ok(RouteDestination {
            target_symbols,
            target_sectors,
            broadcast_all: false,
            target_regions,
            priority: 5, // Normal priority for symbol-specific data
            transformation_hints: HashMap::new(),
        })
    }

    /// Route data to all symbols in a sector
    async fn route_to_sector_symbols(&self, sector: SectorId) -> Result<RouteDestination> {
        let target_symbols = self.sector_symbols_cache
            .get(&sector)
            .map(|symbols| symbols.clone())
            .unwrap_or_default();
        
        // Limit the number of symbols to avoid overwhelming the system
        let limited_symbols: HashSet<String> = target_symbols
            .into_iter()
            .take(self.config.max_symbols_per_sector)
            .collect();
        
        let mut target_sectors = HashSet::new();
        target_sectors.insert(sector);
        
        Ok(RouteDestination {
            target_symbols: limited_symbols,
            target_sectors,
            broadcast_all: false,
            target_regions: HashSet::new(),
            priority: 6, // Higher priority for sector-wide data
            transformation_hints: {
                let mut hints = HashMap::new();
                hints.insert("scope".to_string(), "sector".to_string());
                hints.insert("sector_id".to_string(), sector.as_str().to_string());
                hints
            },
        })
    }

    /// Route data to all active symbols (market-wide)
    async fn route_to_all_symbols(&self) -> Result<RouteDestination> {
        let all_symbols: HashSet<String> = self.active_symbols
            .iter()
            .map(|entry| entry.key().clone())
            .take(self.config.max_symbols_for_market)
            .collect();
        
        // Include all sectors
        let target_sectors: HashSet<SectorId> = [
            SectorId::Technology,
            SectorId::Financial,
            SectorId::Healthcare,
            SectorId::Energy,
            SectorId::ConsumerDiscretionary,
            SectorId::ConsumerStaples,
            SectorId::Industrials,
            SectorId::Materials,
            SectorId::Utilities,
            SectorId::RealEstate,
        ].iter().copied().collect();
        
        Ok(RouteDestination {
            target_symbols: all_symbols,
            target_sectors,
            broadcast_all: true,
            target_regions: HashSet::new(),
            priority: 7, // High priority for market-wide data
            transformation_hints: {
                let mut hints = HashMap::new();
                hints.insert("scope".to_string(), "market".to_string());
                hints.insert("broadcast".to_string(), "true".to_string());
                hints
            },
        })
    }

    /// Route data to symbols in a geographic region
    async fn route_to_geo_symbols(&self, region: GeographicRegion) -> Result<RouteDestination> {
        if !self.config.enable_geographic_routing {
            warn!("Geographic routing is disabled");
            return Ok(RouteDestination {
                target_symbols: HashSet::new(),
                target_sectors: HashSet::new(),
                broadcast_all: false,
                target_regions: HashSet::new(),
                priority: 1,
                transformation_hints: HashMap::new(),
            });
        }
        
        let target_symbols: HashSet<String> = self.geographic_mapping
            .iter()
            .filter(|entry| *entry.value() == region)
            .map(|entry| entry.key().clone())
            .collect();
        
        let mut target_regions = HashSet::new();
        target_regions.insert(region.clone());
        
        Ok(RouteDestination {
            target_symbols,
            target_sectors: HashSet::new(),
            broadcast_all: false,
            target_regions,
            priority: 5, // Normal priority for geographic data
            transformation_hints: {
                let mut hints = HashMap::new();
                hints.insert("scope".to_string(), "geographic".to_string());
                hints.insert("region".to_string(), format!("{:?}", region));
                hints
            },
        })
    }

    /// Handle high priority packets with special buffering
    async fn handle_high_priority_packet(&self, packet: &DataPacket) -> Result<()> {
        if self.priority_buffer.len() >= self.config.high_priority_buffer_size {
            // Remove oldest entry to make space
            if let Some((oldest_key, _)) = self.priority_buffer
                .iter()
                .min_by_key(|entry| entry.value().created_at)
                .map(|entry| (entry.key().clone(), entry.value().clone())) {
                self.priority_buffer.remove(&oldest_key);
            }
        }
        
        self.priority_buffer.insert(packet.id.clone(), packet.clone());
        debug!("Buffered high priority packet: {}", packet.id);
        Ok(())
    }

    /// Update routing performance metrics
    async fn update_routing_metrics(&self, packet: &DataPacket, duration: std::time::Duration) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_packets += 1;
        
        // Update scope-specific counters
        let scope_key = match &packet.scope {
            DataScope::Symbol(_) => "symbol",
            DataScope::Sector(_) => "sector", 
            DataScope::Market => "market",
            DataScope::Geographic(_) => "geographic",
        };
        *metrics.packets_by_scope.entry(scope_key.to_string()).or_insert(0) += 1;
        
        // Update average routing time
        let duration_us = duration.as_micros() as f64;
        if metrics.avg_routing_time_us == 0.0 {
            metrics.avg_routing_time_us = duration_us;
        } else {
            metrics.avg_routing_time_us = (metrics.avg_routing_time_us * 0.9) + (duration_us * 0.1);
        }
        
        // Update high priority counter
        if packet.priority >= 8 {
            metrics.high_priority_packets += 1;
        }
        
        // Update buffer utilization
        metrics.buffer_utilization = self.priority_buffer.len() as f64 
            / self.config.high_priority_buffer_size as f64;
        
        metrics.last_updated = Utc::now();
    }

    /// Get current routing metrics
    pub async fn get_metrics(&self) -> RoutingMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Get active symbols count
    pub fn get_active_symbols_count(&self) -> usize {
        self.active_symbols.len()
    }

    /// Get symbols in a specific sector
    pub fn get_sector_symbols(&self, sector: SectorId) -> Option<HashSet<String>> {
        self.sector_symbols_cache.get(&sector).map(|symbols| symbols.clone())
    }

    /// Clear the high priority buffer
    pub async fn clear_priority_buffer(&self) {
        self.priority_buffer.clear();
        debug!("Cleared high priority packet buffer");
    }

    /// Get buffer status
    pub fn get_buffer_status(&self) -> (usize, usize) {
        (self.priority_buffer.len(), self.config.high_priority_buffer_size)
    }

    /// Create a data packet from TimeSeriesData
    pub fn create_packet(
        &self,
        mut data: TimeSeriesData,
        scope: DataScope,
        priority: u8,
        source: String,
    ) -> DataPacket {
        // Ensure intervals field is populated for compatibility
        if data.intervals.is_empty() {
            data.intervals = vec![1000]; // Default 1 second intervals
        }
        
        DataPacket {
            id: format!("pkt_{}_{}", Utc::now().timestamp_nanos_opt().unwrap_or(0), data.symbol),
            scope,
            data,
            priority,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            source,
        }
    }
}

// Helper function for sector string conversion (avoiding duplicate impl)
fn parse_sector_from_string(s: &str) -> Result<SectorId> {
    match s.to_lowercase().as_str() {
        "technology" => Ok(SectorId::Technology),
        "financial" => Ok(SectorId::Financial),
        "healthcare" => Ok(SectorId::Healthcare),
        "energy" => Ok(SectorId::Energy),
        "consumer_discretionary" => Ok(SectorId::ConsumerDiscretionary),
        "consumer_staples" => Ok(SectorId::ConsumerStaples),
        "industrials" => Ok(SectorId::Industrials),
        "materials" => Ok(SectorId::Materials),
        "utilities" => Ok(SectorId::Utilities),
        "real_estate" => Ok(SectorId::RealEstate),
        _ => Err(anyhow::anyhow!("Unknown sector: {}", s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sector_mapper::SectorMapper;

    async fn create_test_router() -> MultiScopeRouter {
        let config = RoutingConfig::default();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        MultiScopeRouter::new(config, sector_mapper)
    }

    #[tokio::test]
    async fn test_symbol_routing() {
        let router = create_test_router().await;
        
        // Register a test symbol
        router.register_symbol("AAPL", &GeographicRegion::NorthAmerica).await.unwrap();
        
        // Create a symbol-specific data packet
        let data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
        let packet = router.create_packet(
            data,
            DataScope::Symbol("AAPL".to_string()),
            5,
            "test".to_string(),
        );
        
        // Route the packet
        let destination = router.route_by_scope(packet).await.unwrap();
        
        assert_eq!(destination.target_symbols.len(), 1);
        assert!(destination.target_symbols.contains("AAPL"));
        assert!(!destination.broadcast_all);
    }

    #[tokio::test]
    async fn test_market_wide_routing() {
        let router = create_test_router().await;
        
        // Register multiple symbols
        router.register_symbol("AAPL", &GeographicRegion::NorthAmerica).await.unwrap();
        router.register_symbol("GOOGL", &GeographicRegion::NorthAmerica).await.unwrap();
        router.register_symbol("MSFT", &GeographicRegion::NorthAmerica).await.unwrap();
        
        // Create a market-wide data packet
        let data = TimeSeriesData::new("MARKET".to_string(), Utc::now());
        let packet = router.create_packet(
            data,
            DataScope::Market,
            7,
            "test".to_string(),
        );
        
        // Route the packet
        let destination = router.route_by_scope(packet).await.unwrap();
        
        assert_eq!(destination.target_symbols.len(), 3);
        assert!(destination.broadcast_all);
        assert_eq!(destination.priority, 7);
    }

    #[tokio::test]
    async fn test_geographic_routing() {
        let router = create_test_router().await;
        
        // Register symbols in different regions
        router.register_symbol("AAPL", &GeographicRegion::NorthAmerica).await.unwrap();
        router.register_symbol("SAP", &GeographicRegion::Europe).await.unwrap();
        router.register_symbol("TSM", &GeographicRegion::Asia).await.unwrap();
        
        // Create a Europe-specific data packet
        let data = TimeSeriesData::new("EUR_NEWS".to_string(), Utc::now());
        let packet = router.create_packet(
            data,
            DataScope::Geographic(GeographicRegion::Europe),
            6,
            "test".to_string(),
        );
        
        // Route the packet
        let destination = router.route_by_scope(packet).await.unwrap();
        
        assert_eq!(destination.target_symbols.len(), 1);
        assert!(destination.target_symbols.contains("SAP"));
        assert!(destination.target_regions.contains(&GeographicRegion::Europe));
    }

    #[tokio::test]
    async fn test_high_priority_buffering() {
        let router = create_test_router().await;
        
        // Create a high priority packet
        let data = TimeSeriesData::new("URGENT".to_string(), Utc::now());
        let packet = router.create_packet(
            data,
            DataScope::Market,
            9, // High priority
            "test".to_string(),
        );
        
        // Route the packet (should trigger buffering)
        let _destination = router.route_by_scope(packet).await.unwrap();
        
        // Check buffer status
        let (buffer_count, buffer_capacity) = router.get_buffer_status();
        assert_eq!(buffer_count, 1);
        assert_eq!(buffer_capacity, router.config.high_priority_buffer_size);
    }

    #[tokio::test]
    async fn test_routing_metrics() {
        let router = create_test_router().await;
        
        // Register a symbol
        router.register_symbol("TEST", &GeographicRegion::NorthAmerica).await.unwrap();
        
        // Route multiple packets
        for i in 0..5 {
            let data = TimeSeriesData::new(format!("TEST_{}", i), Utc::now());
            let packet = router.create_packet(
                data,
                DataScope::Symbol(format!("TEST_{}", i)),
                5,
                "test".to_string(),
            );
            router.route_by_scope(packet).await.unwrap();
        }
        
        // Check metrics
        let metrics = router.get_metrics().await;
        assert_eq!(metrics.total_packets, 5);
        assert_eq!(metrics.packets_by_scope.get("symbol").unwrap(), &5);
        assert!(metrics.avg_routing_time_us > 0.0);
    }
}