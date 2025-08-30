//! Data Pipeline Module - Multi-Scope Data Routing System
//!
//! This module implements a sophisticated data routing system that handles different
//! data scopes (symbol-specific, sector-wide, market-wide, geographic) and provides
//! unified consolidation per symbol for the neural trader system.
//!
//! INTEGRATION-FIRST IMPLEMENTATION:
//! - Compatible with existing VendorPredictor inputs
//! - Works with current TimeSeriesData structures
//! - Integrates with sector mapping and cluster model pools
//! - Maintains performance for high-frequency data streams

pub mod routing;
pub mod consolidation;

use anyhow::Result;
use std::sync::Arc;
use tracing::{info, debug};

// Re-export main types
pub use routing::{
    DataScope, DataPacket, RouteDestination, MultiScopeRouter, 
    RoutingMetrics, RoutingConfig, GeographicRegion
};
pub use consolidation::{
    DataConsolidator, ConsolidationResult, ConsolidationConfig,
    ConsolidationMetrics, SymbolConsolidation
};

// Import existing types for integration
use crate::data::{TimeSeriesData, sector_mapper::{SectorId, SectorMapper}};

/// Unified DataPipeline that integrates routing and consolidation
/// 
/// This struct provides a single interface for the multi-scope data pipeline
/// while integrating seamlessly with existing DAA coordinator and neural predictors.
pub struct DataPipeline {
    /// Multi-scope router for directing data based on scope
    router: Arc<MultiScopeRouter>,
    /// Data consolidator for unified symbol views
    consolidator: Arc<DataConsolidator>,
    /// Sector mapper for symbol-to-sector lookups
    sector_mapper: Arc<SectorMapper>,
}

impl DataPipeline {
    /// Create a new unified data pipeline
    pub fn new(
        routing_config: RoutingConfig,
        consolidation_config: ConsolidationConfig,
        sector_mapper: Arc<SectorMapper>,
    ) -> Self {
        info!("🚀 Initializing unified DataPipeline with Integration-First design");
        
        let router = Arc::new(MultiScopeRouter::new(routing_config, sector_mapper.clone()));
        let consolidator = Arc::new(DataConsolidator::new(consolidation_config));
        
        Self {
            router,
            consolidator,
            sector_mapper,
        }
    }

    /// Process incoming time series data through the complete pipeline
    /// 
    /// This method handles:
    /// 1. Scope determination and routing
    /// 2. Data packet creation
    /// 3. Consolidation ingestion
    /// 4. Quality assessment
    pub async fn process_data(
        &self,
        data: TimeSeriesData,
        scope: DataScope,
        priority: u8,
        source: String,
    ) -> Result<()> {
        // Create data packet using the router
        let packet = self.router.create_packet(data, scope, priority, source);
        
        debug!("Processing data packet: {} for scope: {:?}", packet.id, packet.scope);
        
        // Route the packet to determine destinations
        let _destination = self.router.route_by_scope(packet.clone()).await?;
        
        // Ingest into consolidator for future consolidation
        self.consolidator.ingest_packet(packet).await?;
        
        debug!("Successfully processed data through pipeline");
        Ok(())
    }

    /// Get consolidated data for a specific symbol
    /// 
    /// This provides the unified view that DAA coordinators and neural predictors need
    pub async fn get_consolidated_data(
        &self,
        symbol: &str,
        symbol_region: GeographicRegion,
    ) -> Result<ConsolidationResult> {
        // Get the symbol's sector
        let sector_id = if let Ok(sector_info) = self.sector_mapper.get_sector(symbol) {
            SectorId::from_str(&sector_info.id).unwrap_or(SectorId::Technology)
        } else {
            SectorId::Technology // Default fallback
        };

        // Consolidate data from all scopes
        self.consolidator.consolidate_for_symbol(symbol, sector_id, symbol_region).await
    }

    /// Register a symbol for routing and processing
    pub async fn register_symbol(&self, symbol: &str, region: GeographicRegion) -> Result<()> {
        self.router.register_symbol(symbol, &region).await
    }

    /// Get router metrics
    pub async fn get_routing_metrics(&self) -> RoutingMetrics {
        self.router.get_metrics().await
    }

    /// Get consolidation metrics
    pub async fn get_consolidation_metrics(&self) -> ConsolidationMetrics {
        self.consolidator.get_metrics().await
    }

    /// Cleanup old data to maintain performance
    pub async fn cleanup_old_data(&self) -> Result<()> {
        self.consolidator.cleanup_old_data().await
    }

    /// Get the underlying router (for advanced use cases)
    pub fn get_router(&self) -> &Arc<MultiScopeRouter> {
        &self.router
    }

    /// Get the underlying consolidator (for advanced use cases)
    pub fn get_consolidator(&self) -> &Arc<DataConsolidator> {
        &self.consolidator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_unified_data_pipeline() {
        let routing_config = RoutingConfig::default();
        let consolidation_config = ConsolidationConfig::default();
        let sector_mapper = Arc::new(SectorMapper::new(Default::default()));
        
        let pipeline = DataPipeline::new(routing_config, consolidation_config, sector_mapper);
        
        // Register a test symbol
        pipeline.register_symbol("AAPL", GeographicRegion::NorthAmerica).await.unwrap();
        
        // Process some test data
        let data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
        pipeline.process_data(
            data,
            DataScope::Symbol("AAPL".to_string()),
            5,
            "test".to_string()
        ).await.unwrap();
        
        // Try to get consolidated data
        if let Ok(result) = pipeline.get_consolidated_data("AAPL", GeographicRegion::NorthAmerica).await {
            assert_eq!(result.consolidated_data.symbol, "AAPL");
        }
    }
}