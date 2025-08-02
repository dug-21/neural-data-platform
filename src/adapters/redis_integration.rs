//! Redis Integration Bridge
//!
//! PHASE 2 WEEK 5: REDIS INTEGRATION LAYER
//! Bridges existing Redis adapter with new sector channel functionality.
//! Provides unified interface while maintaining backward compatibility.
//!
//! INTEGRATION STRATEGY:
//! 1. Extends existing RedisAdapter module structure
//! 2. Adds sector channel manager as optional component
//! 3. Maintains all existing symbol-based functionality
//! 4. Provides factory methods for easy setup

use super::redis::{RedisAdapter, RedisConfig};
use super::redis_sector_channels::{RedisSectorChannels, SectorChannelConfig, SectorData, PortfolioDecision, CrossSectorData};
use crate::data::sector_mapper::{SectorId, SectorMapper};
use crate::adapters::{AdapterError, MarketData};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};
use std::collections::HashMap;

/// Unified Redis Integration Manager
/// 
/// Provides a single interface for both traditional symbol-based operations
/// and new sector-based aggregation channels.
pub struct RedisIntegration {
    /// Core Redis adapter for symbol operations (PRESERVED)
    symbol_redis: Arc<RwLock<RedisAdapter>>,
    
    /// Sector channel manager (NEW)
    sector_channels: Option<Arc<RedisSectorChannels>>,
    
    /// Sector mapper for symbol->sector resolution
    sector_mapper: Option<Arc<SectorMapper>>,
    
    /// Configuration
    config: RedisIntegrationConfig,
    
    /// Connection status tracking
    connected: Arc<RwLock<bool>>,
}

/// Configuration for Redis integration
#[derive(Debug, Clone)]
pub struct RedisIntegrationConfig {
    pub redis_config: RedisConfig,
    pub sector_config: SectorChannelConfig,
    pub enable_sector_channels: bool,
    pub enable_dual_publishing: bool,    // Publish to both symbol and sector channels
    pub symbol_to_sector_ratio: f64,     // Resource allocation ratio
}

impl Default for RedisIntegrationConfig {
    fn default() -> Self {
        Self {
            redis_config: RedisConfig::default(),
            sector_config: SectorChannelConfig::default(),
            enable_sector_channels: true,
            enable_dual_publishing: true,
            symbol_to_sector_ratio: 0.7, // 70% symbol, 30% sector
        }
    }
}

impl RedisIntegration {
    /// Create new Redis integration manager
    pub fn new(config: RedisIntegrationConfig) -> Self {
        info!("🔄 Initializing Redis Integration Manager");
        info!("📊 Sector channels enabled: {}", config.enable_sector_channels);
        info!("🔀 Dual publishing enabled: {}", config.enable_dual_publishing);
        
        let symbol_redis_adapter = RedisAdapter::new(config.redis_config.clone());
        
        Self {
            symbol_redis: Arc::new(RwLock::new(symbol_redis_adapter)),
            sector_channels: None,
            sector_mapper: None,
            config,
            connected: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Initialize with sector mapper
    pub fn with_sector_mapper(mut self, sector_mapper: Arc<SectorMapper>) -> Self {
        self.sector_mapper = Some(sector_mapper);
        
        if self.config.enable_sector_channels {
            // Create sector channels wrapper
            let symbol_redis_clone = {
                // Clone the inner RedisAdapter for the sector channels
                let redis_guard = futures::executor::block_on(self.symbol_redis.read());
                redis_guard.clone() // Assuming RedisAdapter implements Clone
            };
            
            let sector_channels = RedisSectorChannels::new(
                symbol_redis_clone, 
                self.config.sector_config.clone()
            );
            self.sector_channels = Some(Arc::new(sector_channels));
            
            info!("✅ Sector channels initialized with sector mapper");
        }
        
        self
    }
    
    /// Connect to all Redis components
    pub async fn connect(&self) -> Result<(), AdapterError> {
        use crate::adapters::DataAdapter;
        info!("🔌 Connecting Redis Integration Manager...");
        
        // Connect symbol Redis
        {
            let mut symbol_redis = self.symbol_redis.write().await;
            symbol_redis.connect().await?;
        }
        
        // Connect sector channels if enabled
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.connect().await?;
        }
        
        *self.connected.write().await = true;
        info!("✅ Redis Integration Manager connected");
        Ok(())
    }
    
    /// Check connection status
    pub async fn is_connected(&self) -> bool {
        *self.connected.read().await
    }
    
    /// Disconnect all components
    pub async fn disconnect(&self) -> Result<(), AdapterError> {
        use crate::adapters::DataAdapter;
        info!("🔌 Disconnecting Redis Integration Manager...");
        
        {
            let mut symbol_redis = self.symbol_redis.write().await;
            symbol_redis.disconnect().await?;
        }
        
        *self.connected.write().await = false;
        info!("✅ Redis Integration Manager disconnected");
        Ok(())
    }
    
    // ===== SYMBOL OPERATIONS (PRESERVED) =====
    
    /// Publish market data (PRESERVED functionality + optional sector aggregation)
    pub async fn publish_market_data(
        &self,
        channel: &str,
        data: &MarketData,
    ) -> Result<(), AdapterError> {
        debug!("📊 Publishing market data: {} -> {}", data.symbol, channel);
        
        // 1. ALWAYS publish to symbol channel (PRESERVED)
        {
            let symbol_redis = self.symbol_redis.read().await;
            symbol_redis.publish_market_data(channel, data).await?;
        }
        
        // 2. OPTIONALLY aggregate to sector channel (NEW)
        if self.config.enable_dual_publishing {
            self.aggregate_and_publish_to_sector(data).await?;
        }
        
        Ok(())
    }
    
    /// Subscribe to market data (PRESERVED)
    pub async fn subscribe_market_data(
        &self,
        channel: &str,
    ) -> Result<impl futures::Stream<Item = Result<MarketData, AdapterError>>, AdapterError> {
        debug!("📊 Subscribing to market data: {}", channel);
        
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.subscribe_market_data(channel).await
    }
    
    /// Cache order book (PRESERVED)
    pub async fn cache_order_book(
        &self,
        order_book: &crate::adapters::OrderBook,
    ) -> Result<(), AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.cache_order_book(order_book).await
    }
    
    /// Get cached order book (PRESERVED)
    pub async fn get_order_book(
        &self,
        symbol: &str,
    ) -> Result<Option<crate::adapters::OrderBook>, AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.get_order_book(symbol).await
    }
    
    /// Store latest price (PRESERVED)
    pub async fn set_latest_price(
        &self,
        symbol: &str,
        price: f64,
        timestamp: i64,
    ) -> Result<(), AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.set_latest_price(symbol, price, timestamp).await
    }
    
    /// Get latest price (PRESERVED)
    pub async fn get_latest_price(
        &self,
        symbol: &str,
    ) -> Result<Option<(f64, i64)>, AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.get_latest_price(symbol).await
    }
    
    // ===== STREAM OPERATIONS (PRESERVED) =====
    
    /// Add to stream (PRESERVED)
    pub async fn add_to_stream(
        &self,
        stream_key: &str,
        data: &MarketData,
    ) -> Result<String, AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.add_to_stream(stream_key, data).await
    }
    
    /// Read from stream (PRESERVED)
    pub async fn read_from_stream(
        &self,
        stream_key: &str,
        start_id: &str,
        count: usize,
    ) -> Result<Vec<MarketData>, AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.read_from_stream(stream_key, start_id, count).await
    }
    
    /// Create consumer group (PRESERVED)
    pub async fn create_consumer_group(
        &self,
        stream_key: &str,
        group_name: &str,
    ) -> Result<(), AdapterError> {
        let symbol_redis = self.symbol_redis.read().await;
        symbol_redis.create_consumer_group(stream_key, group_name).await
    }
    
    // ===== SECTOR OPERATIONS (NEW) =====
    
    /// Publish sector aggregation data
    pub async fn publish_sector_data(
        &self,
        sector_id: &SectorId,
        data: &SectorData,
    ) -> Result<(), AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.publish_sector_data(sector_id, data).await
        } else {
            warn!("Sector channels not enabled - cannot publish sector data");
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Subscribe to sector data
    pub async fn subscribe_sector_data(
        &self,
        sector_id: &SectorId,
    ) -> Result<impl futures::Stream<Item = Result<SectorData, AdapterError>>, AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.subscribe_sector_data(sector_id).await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Add sector data to stream
    pub async fn add_sector_to_stream(
        &self,
        sector_id: &SectorId,
        data: &SectorData,
    ) -> Result<String, AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.add_sector_to_stream(sector_id, data).await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Read sector data from stream
    pub async fn read_sector_from_stream(
        &self,
        sector_id: &SectorId,
        start_id: &str,
        count: usize,
    ) -> Result<Vec<SectorData>, AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.read_sector_from_stream(sector_id, start_id, count).await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Publish portfolio decision
    pub async fn publish_portfolio_decision(
        &self,
        decision: &PortfolioDecision,
    ) -> Result<(), AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.publish_portfolio_decision(decision).await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Subscribe to portfolio decisions
    pub async fn subscribe_portfolio_decisions(
        &self,
    ) -> Result<impl futures::Stream<Item = Result<PortfolioDecision, AdapterError>>, AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.subscribe_portfolio_decisions().await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    /// Publish cross-sector data
    pub async fn publish_cross_sector_data(
        &self,
        data: &CrossSectorData,
    ) -> Result<(), AdapterError> {
        if let Some(sector_channels) = &self.sector_channels {
            sector_channels.publish_cross_sector_data(data).await
        } else {
            Err(AdapterError::Connection("Sector channels not initialized".to_string()))
        }
    }
    
    // ===== AGGREGATION LOGIC (NEW) =====
    
    /// Aggregate market data to sector and publish
    async fn aggregate_and_publish_to_sector(
        &self,
        market_data: &MarketData,
    ) -> Result<(), AdapterError> {
        if let Some(sector_mapper) = &self.sector_mapper {
            if let Some(sector_channels) = &self.sector_channels {
                // Get sector for this symbol
                match sector_mapper.get_sector(&market_data.symbol) {
                    Ok(sector_info) => {
                        debug!("📊 Aggregating {} to sector: {}", 
                               market_data.symbol, sector_info.sector_id.as_str());
                        
                        // Create sector data (simplified - in practice would aggregate multiple symbols)
                        let sector_data = super::redis_sector_channels::aggregate_market_data_to_sector(
                            &sector_info.sector_id,
                            &[market_data.clone()],
                            market_data.close * 1.1 // Placeholder ETF price
                        );
                        
                        // Publish to sector channel
                        sector_channels.publish_sector_data(&sector_info.sector_id, &sector_data).await?;
                    }
                    Err(e) => {
                        debug!("⚠️ Could not map symbol {} to sector: {}", market_data.symbol, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Get all available channels
    pub fn get_all_channels(&self) -> HashMap<String, Vec<String>> {
        let mut channels = HashMap::new();
        
        // Symbol channels (existing pattern)
        channels.insert("symbol".to_string(), vec![
            "symbol/AAPL".to_string(),
            "symbol/MSFT".to_string(),
            "symbol/GOOGL".to_string(),
            // Add more as needed
        ]);
        
        if self.sector_channels.is_some() {
            // Sector channels (new)
            channels.insert("sector".to_string(), 
                           RedisSectorChannels::get_sector_channels());
            
            // Portfolio channels (new)
            channels.insert("portfolio".to_string(), 
                           RedisSectorChannels::get_portfolio_channels());
            
            // Cross-sector channels (new)
            channels.insert("cross_sector".to_string(), 
                           RedisSectorChannels::get_cross_sector_channels());
        }
        
        channels
    }
    
    /// Health check for all components
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let mut health = HashMap::new();
        
        // Check symbol Redis
        let symbol_connected = {
            use crate::adapters::DataAdapter;
            let redis = self.symbol_redis.read().await;
            redis.is_connected()
        };
        health.insert("symbol_redis".to_string(), symbol_connected);
        
        // Check sector channels
        if let Some(sector_channels) = &self.sector_channels {
            let sector_connected = sector_channels.is_connected().await;
            health.insert("sector_channels".to_string(), sector_connected);
        }
        
        // Overall health
        let overall_health = health.values().all(|&connected| connected);
        health.insert("overall".to_string(), overall_health);
        
        health
    }
}

// ===== FACTORY METHODS =====

/// Factory for easy Redis integration setup
pub struct RedisIntegrationFactory;

impl RedisIntegrationFactory {
    /// Create symbol-only Redis integration (backward compatible)
    pub fn create_symbol_only(redis_config: RedisConfig) -> RedisIntegration {
        let config = RedisIntegrationConfig {
            redis_config,
            enable_sector_channels: false,
            enable_dual_publishing: false,
            ..Default::default()
        };
        
        RedisIntegration::new(config)
    }
    
    /// Create full Redis integration with sectors
    pub fn create_with_sectors(
        redis_config: RedisConfig,
        sector_config: SectorChannelConfig,
        sector_mapper: Arc<SectorMapper>,
    ) -> RedisIntegration {
        let config = RedisIntegrationConfig {
            redis_config,
            sector_config,
            enable_sector_channels: true,
            enable_dual_publishing: true,
            ..Default::default()
        };
        
        RedisIntegration::new(config).with_sector_mapper(sector_mapper)
    }
    
    /// Create with custom configuration
    pub fn create_custom(
        config: RedisIntegrationConfig,
        sector_mapper: Option<Arc<SectorMapper>>,
    ) -> RedisIntegration {
        let mut integration = RedisIntegration::new(config);
        
        if let Some(mapper) = sector_mapper {
            integration = integration.with_sector_mapper(mapper);
        }
        
        integration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sector_mapper::SectorMapperConfig;
    
    #[tokio::test]
    async fn test_symbol_only_integration() {
        let redis_config = RedisConfig::default();
        let integration = RedisIntegrationFactory::create_symbol_only(redis_config);
        
        // Should not have sector channels
        assert!(integration.sector_channels.is_none());
        assert!(integration.sector_mapper.is_none());
        assert!(!integration.config.enable_sector_channels);
    }
    
    #[tokio::test]
    async fn test_full_integration_creation() {
        let redis_config = RedisConfig::default();
        let sector_config = SectorChannelConfig::default();
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        
        let integration = RedisIntegrationFactory::create_with_sectors(
            redis_config,
            sector_config,
            sector_mapper
        );
        
        // Should have all components
        assert!(integration.sector_channels.is_some());
        assert!(integration.sector_mapper.is_some());
        assert!(integration.config.enable_sector_channels);
        assert!(integration.config.enable_dual_publishing);
    }
    
    #[test]
    fn test_channel_listing() {
        let config = RedisIntegrationConfig::default();
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let integration = RedisIntegration::new(config).with_sector_mapper(sector_mapper);
        
        let channels = integration.get_all_channels();
        
        // Should have all channel types
        assert!(channels.contains_key("symbol"));
        assert!(channels.contains_key("sector"));
        assert!(channels.contains_key("portfolio"));
        assert!(channels.contains_key("cross_sector"));
        
        // Check sector channels
        let sector_channels = channels.get("sector").unwrap();
        assert!(sector_channels.contains(&"sector/technology".to_string()));
        assert!(sector_channels.contains(&"sector/financial".to_string()));
    }
}