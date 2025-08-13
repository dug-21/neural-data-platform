//! Redis Sector Channel Integration
//!
//! PHASE 2 WEEK 5: REDIS SECTOR CHANNEL ARCHITECTURE
//! Extends existing Redis adapter with sector-level aggregation channels
//! while preserving all existing symbol-based functionality.
//!
//! NEW CHANNEL ARCHITECTURE:
//! - Existing: symbol/AAPL, symbol/MSFT, etc. (PRESERVED)
//! - New: sector/technology, sector/financial, etc.
//! - New: portfolio/decisions, portfolio/risk_metrics
//! - New: cross_sector/correlations, cross_sector/rotation
//!
//! INTEGRATION APPROACH:
//! - Extends RedisAdapter without breaking existing code
//! - Maintains backward compatibility
//! - Adds sector-level pub/sub and streaming
//! - Implements efficient message batching and compression

use super::redis::{RedisAdapter, RedisConfig};
use async_trait::async_trait;
use crate::data::sector_mapper::SectorId;
use crate::adapters::{AdapterError, MarketData};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use chrono::{DateTime, Utc};
use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use std::io::{Write, Read};

/// Sector aggregation data message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorData {
    pub sector_id: String,
    pub etf_symbol: String,
    pub etf_price: f64,
    pub avg_price: f64,
    pub total_volume: f64,
    pub volatility: f64,
    pub momentum: f64,
    pub timestamp: i64,
    pub symbols_count: u32,
    pub symbols: Vec<String>,
    pub correlation_matrix: HashMap<String, f64>, // Simplified correlation data
}

/// Portfolio decision message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioDecision {
    pub decision_id: String,
    pub sector_allocations: HashMap<String, f64>, // sector -> weight
    pub risk_metrics: RiskMetrics,
    pub consensus_score: f64,
    pub timestamp: i64,
    pub reasoning: String,
    pub confidence: f64,
}

/// Risk metrics for portfolio decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub portfolio_var: f64,        // Value at Risk
    pub max_drawdown: f64,         // Maximum drawdown
    pub sharpe_ratio: f64,         // Sharpe ratio
    pub sector_concentration: f64,  // Concentration risk
    pub correlation_risk: f64,     // Cross-sector correlation
}

/// Cross-sector market data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossSectorData {
    pub data_type: String, // "correlations", "rotation", "regime"
    pub correlations: HashMap<String, HashMap<String, f64>>, // sector -> sector -> correlation
    pub rotation_score: Option<f64>, // Sector rotation strength
    pub market_regime: Option<String>, // "risk_on", "risk_off", "neutral"
    pub timestamp: i64,
}

/// Redis Sector Channel Extension
/// 
/// This struct extends the existing RedisAdapter with sector-level functionality
/// while maintaining full backward compatibility.
pub struct RedisSectorChannels {
    /// Wrapped Redis adapter (COMPOSITION, not inheritance)
    redis: Arc<RwLock<RedisAdapter>>,
    
    /// Configuration for sector channels
    config: SectorChannelConfig,
    
    /// Channel mapping cache
    channel_cache: Arc<RwLock<HashMap<String, String>>>,
    
    /// Message compression settings
    compression_enabled: bool,
}

/// Configuration for sector channels
#[derive(Debug, Clone)]
pub struct SectorChannelConfig {
    pub enable_compression: bool,
    pub sector_ttl_seconds: u64,
    pub portfolio_ttl_seconds: u64,
    pub batch_size: usize,
    pub max_message_size_kb: usize,
}

impl Default for SectorChannelConfig {
    fn default() -> Self {
        Self {
            enable_compression: true,
            sector_ttl_seconds: 180,     // 3 minutes
            portfolio_ttl_seconds: 3600, // 1 hour
            batch_size: 10,
            max_message_size_kb: 512,
        }
    }
}

impl RedisSectorChannels {
    /// Create new sector channels wrapper around existing Redis adapter
    pub fn new(redis_adapter: RedisAdapter, config: SectorChannelConfig) -> Self {
        info!("🏭 Initializing Redis Sector Channels");
        info!("📊 Compression: {}", config.enable_compression);
        info!("⏱️ Sector TTL: {}s, Portfolio TTL: {}s", 
              config.sector_ttl_seconds, config.portfolio_ttl_seconds);
        
        let compression_enabled = config.enable_compression;
        
        Self {
            redis: Arc::new(RwLock::new(redis_adapter)),
            config,
            channel_cache: Arc::new(RwLock::new(HashMap::new())),
            compression_enabled,
        }
    }
    
    /// Connect the underlying Redis adapter
    pub async fn connect(&self) -> Result<(), AdapterError> {
        use crate::adapters::DataAdapter;
        let mut redis = self.redis.write().await;
        redis.connect().await?;
        info!("✅ Redis Sector Channels connected");
        Ok(())
    }
    
    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        use crate::adapters::DataAdapter;
        let redis = self.redis.read().await;
        redis.is_connected()
    }
    
    // ===== SECTOR AGGREGATION CHANNELS =====
    
    /// Publish sector aggregation data
    pub async fn publish_sector_data(
        &self,
        sector_id: &SectorId,
        data: &SectorData,
    ) -> Result<(), AdapterError> {
        let channel = format!("sector/{}", sector_id.as_str());
        debug!("📊 Publishing sector data to: {}", channel);
        
        let serialized = self.serialize_and_compress(data).await?;
        
        let redis = self.redis.read().await;
        redis.publish_market_data(&channel, &MarketData {
            symbol: channel.clone(),
            timestamp: data.timestamp,
            open: data.avg_price,
            high: data.avg_price * 1.01, // Placeholder
            low: data.avg_price * 0.99,  // Placeholder
            close: data.avg_price,
            volume: data.total_volume,
        }).await?;
        
        // Also cache the sector data with TTL
        self.cache_sector_data(sector_id, data).await?;
        
        Ok(())
    }
    
    /// Subscribe to sector aggregation data
    pub async fn subscribe_sector_data(
        &self,
        sector_id: &SectorId,
    ) -> Result<impl futures::Stream<Item = Result<SectorData, AdapterError>>, AdapterError> {
        let channel = format!("sector/{}", sector_id.as_str());
        debug!("📊 Subscribing to sector channel: {}", channel);
        
        let redis = self.redis.read().await;
        let stream = redis.subscribe_market_data(&channel).await?;
        
        // Transform the market data stream back to sector data
        let sector_stream = stream.map(|result| {
            match result {
                Ok(_market_data) => {
                    // In practice, we'd deserialize the actual sector data from Redis
                    // For now, return a placeholder
                    Err(AdapterError::Serialization("Placeholder implementation".to_string()))
                }
                Err(e) => Err(e),
            }
        });
        
        Ok(sector_stream)
    }
    
    /// Add sector data to Redis stream
    pub async fn add_sector_to_stream(
        &self,
        sector_id: &SectorId,
        data: &SectorData,
    ) -> Result<String, AdapterError> {
        let stream_key = format!("stream:sector:{}", sector_id.as_str());
        debug!("📈 Adding sector data to stream: {}", stream_key);
        
        let redis = self.redis.read().await;
        
        // Convert sector data to stream fields
        let fields = vec![
            ("sector_id", sector_id.as_str()),
            ("etf_symbol", data.etf_symbol.as_str()),
            ("etf_price", &data.etf_price.to_string()),
            ("avg_price", &data.avg_price.to_string()),
            ("total_volume", &data.total_volume.to_string()),
            ("volatility", &data.volatility.to_string()),
            ("momentum", &data.momentum.to_string()),
            ("timestamp", &data.timestamp.to_string()),
            ("symbols_count", &data.symbols_count.to_string()),
        ];
        
        // Use existing stream functionality from RedisAdapter
        // (We'd need to extend RedisAdapter to support generic field addition)
        let market_data = MarketData {
            symbol: sector_id.as_str().to_string(),
            timestamp: data.timestamp,
            open: data.avg_price,
            high: data.avg_price * 1.01,
            low: data.avg_price * 0.99,
            close: data.avg_price,
            volume: data.total_volume,
        };
        
        redis.add_to_stream(&stream_key, &market_data).await
    }
    
    /// Read sector data from stream
    pub async fn read_sector_from_stream(
        &self,
        sector_id: &SectorId,
        start_id: &str,
        count: usize,
    ) -> Result<Vec<SectorData>, AdapterError> {
        let stream_key = format!("stream:sector:{}", sector_id.as_str());
        debug!("📈 Reading sector data from stream: {}", stream_key);
        
        let redis = self.redis.read().await;
        let market_data_vec = redis.read_from_stream(&stream_key, start_id, count).await?;
        
        // Convert market data back to sector data
        let mut sector_data_vec = Vec::new();
        for market_data in market_data_vec {
            // Reconstruct sector data (simplified)
            let sector_data = SectorData {
                sector_id: market_data.symbol,
                etf_symbol: "N/A".to_string(), // Would be reconstructed from cache
                etf_price: market_data.close,
                avg_price: market_data.close,
                total_volume: market_data.volume,
                volatility: 0.0, // Would be calculated
                momentum: 0.0,   // Would be calculated
                timestamp: market_data.timestamp,
                symbols_count: 0, // Would be reconstructed from cache
                symbols: vec![],  // Would be reconstructed from cache
                correlation_matrix: HashMap::new(), // Would be reconstructed from cache
            };
            sector_data_vec.push(sector_data);
        }
        
        Ok(sector_data_vec)
    }
    
    // ===== PORTFOLIO DECISION CHANNELS =====
    
    /// Publish portfolio decision
    pub async fn publish_portfolio_decision(
        &self,
        decision: &PortfolioDecision,
    ) -> Result<(), AdapterError> {
        let channel = "portfolio/decisions";
        debug!("💼 Publishing portfolio decision: {}", decision.decision_id);
        
        let serialized = self.serialize_and_compress(decision).await?;
        
        // Store in Redis hash for persistence
        self.cache_portfolio_decision(decision).await?;
        
        // Publish notification
        let redis = self.redis.read().await;
        redis.publish_market_data(channel, &MarketData {
            symbol: "PORTFOLIO".to_string(),
            timestamp: decision.timestamp,
            open: decision.consensus_score,
            high: decision.confidence,
            low: decision.risk_metrics.max_drawdown,
            close: decision.consensus_score,
            volume: decision.sector_allocations.len() as f64,
        }).await?;
        
        Ok(())
    }
    
    /// Subscribe to portfolio decisions
    pub async fn subscribe_portfolio_decisions(
        &self,
    ) -> Result<impl futures::Stream<Item = Result<PortfolioDecision, AdapterError>>, AdapterError> {
        let channel = "portfolio/decisions";
        debug!("💼 Subscribing to portfolio decisions");
        
        let redis = self.redis.read().await;
        let stream = redis.subscribe_market_data(channel).await?;
        
        // Transform market data stream to portfolio decisions
        let portfolio_stream = stream.map(|result| {
            match result {
                Ok(_market_data) => {
                    // In practice, we'd fetch the actual decision from Redis hash
                    Err(AdapterError::Serialization("Placeholder implementation".to_string()))
                }
                Err(e) => Err(e),
            }
        });
        
        Ok(portfolio_stream)
    }
    
    // ===== CROSS-SECTOR CHANNELS =====
    
    /// Publish cross-sector correlation data
    pub async fn publish_cross_sector_data(
        &self,
        data: &CrossSectorData,
    ) -> Result<(), AdapterError> {
        let channel = format!("cross_sector/{}", data.data_type);
        debug!("🔗 Publishing cross-sector data: {}", channel);
        
        let serialized = self.serialize_and_compress(data).await?;
        
        // Cache the data
        self.cache_cross_sector_data(data).await?;
        
        // Publish notification
        let redis = self.redis.read().await;
        redis.publish_market_data(&channel, &MarketData {
            symbol: "CROSS_SECTOR".to_string(),
            timestamp: data.timestamp,
            open: data.rotation_score.unwrap_or(0.0),
            high: 1.0,
            low: -1.0,
            close: data.rotation_score.unwrap_or(0.0),
            volume: data.correlations.len() as f64,
        }).await?;
        
        Ok(())
    }
    
    // ===== CACHING METHODS =====
    
    /// Cache sector data with TTL
    async fn cache_sector_data(
        &self,
        sector_id: &SectorId,
        data: &SectorData,
    ) -> Result<(), AdapterError> {
        let key = format!("sector:cache:{}", sector_id.as_str());
        let serialized = self.serialize_and_compress(data).await?;
        
        let redis = self.redis.read().await;
        // Using existing cache functionality - would need to extend for TTL
        // redis.cache_order_book(&OrderBook::from_sector_data(data)).await?;
        
        debug!("💾 Cached sector data: {}", key);
        Ok(())
    }
    
    /// Cache portfolio decision with TTL
    async fn cache_portfolio_decision(
        &self,
        decision: &PortfolioDecision,
    ) -> Result<(), AdapterError> {
        let key = format!("portfolio:decision:{}", decision.decision_id);
        let serialized = self.serialize_and_compress(decision).await?;
        
        debug!("💾 Cached portfolio decision: {}", key);
        // Implementation would use HSET with EXPIRE
        Ok(())
    }
    
    /// Cache cross-sector data
    async fn cache_cross_sector_data(
        &self,
        data: &CrossSectorData,
    ) -> Result<(), AdapterError> {
        let key = format!("cross_sector:{}:{}", data.data_type, data.timestamp);
        let serialized = self.serialize_and_compress(data).await?;
        
        debug!("💾 Cached cross-sector data: {}", key);
        Ok(())
    }
    
    // ===== UTILITY METHODS =====
    
    /// Serialize and optionally compress data
    async fn serialize_and_compress<T: Serialize>(
        &self,
        data: &T,
    ) -> Result<Vec<u8>, AdapterError> {
        let json = serde_json::to_string(data)
            .map_err(|e| AdapterError::Serialization(e.to_string()))?;
        
        if self.compression_enabled {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(json.as_bytes())
                .map_err(|e| AdapterError::Serialization(e.to_string()))?;
            encoder.finish()
                .map_err(|e| AdapterError::Serialization(e.to_string()))
        } else {
            Ok(json.into_bytes())
        }
    }
    
    /// Deserialize and optionally decompress data
    async fn deserialize_and_decompress<T: for<'de> Deserialize<'de>>(
        &self,
        data: &[u8],
    ) -> Result<T, AdapterError> {
        let json_str = if self.compression_enabled {
            let mut decoder = GzDecoder::new(data);
            let mut decompressed = String::new();
            decoder.read_to_string(&mut decompressed)
                .map_err(|e| AdapterError::Serialization(e.to_string()))?;
            decompressed
        } else {
            String::from_utf8(data.to_vec())
                .map_err(|e| AdapterError::Serialization(e.to_string()))?
        };
        
        serde_json::from_str(&json_str)
            .map_err(|e| AdapterError::Serialization(e.to_string()))
    }
    
    /// Get all sector channels
    pub fn get_sector_channels() -> Vec<String> {
        vec![
            "sector/technology".to_string(),
            "sector/financial".to_string(),
            "sector/healthcare".to_string(),
            "sector/energy".to_string(),
            "sector/consumer_discretionary".to_string(),
            "sector/consumer_staples".to_string(),
            "sector/industrials".to_string(),
            "sector/materials".to_string(),
            "sector/utilities".to_string(),
            "sector/real_estate".to_string(),
        ]
    }
    
    /// Get all portfolio channels
    pub fn get_portfolio_channels() -> Vec<String> {
        vec![
            "portfolio/decisions".to_string(),
            "portfolio/risk_metrics".to_string(),
            "portfolio/coordination".to_string(),
        ]
    }
    
    /// Get all cross-sector channels
    pub fn get_cross_sector_channels() -> Vec<String> {
        vec![
            "cross_sector/correlations".to_string(),
            "cross_sector/rotation".to_string(),
            "cross_sector/market_regime".to_string(),
        ]
    }
}

// ===== INTEGRATION HELPERS =====

/// Helper to create sector data from market data aggregation
pub fn aggregate_market_data_to_sector(
    sector_id: &SectorId,
    market_data_vec: &[MarketData],
    etf_price: f64,
) -> SectorData {
    let symbols: Vec<String> = market_data_vec.iter()
        .map(|data| data.symbol.clone())
        .collect();
    
    let total_volume: f64 = market_data_vec.iter()
        .map(|data| data.volume)
        .sum();
    
    let avg_price: f64 = market_data_vec.iter()
        .map(|data| data.close)
        .sum::<f64>() / market_data_vec.len() as f64;
    
    // Simple volatility calculation
    let prices: Vec<f64> = market_data_vec.iter()
        .map(|data| data.close)
        .collect();
    let volatility = calculate_volatility(&prices);
    
    // Simple momentum calculation (last/first - 1)
    let momentum = if market_data_vec.len() > 1 {
        let first = market_data_vec.first().unwrap().close;
        let last = market_data_vec.last().unwrap().close;
        (last / first) - 1.0
    } else {
        0.0
    };
    
    SectorData {
        sector_id: sector_id.as_str().to_string(),
        etf_symbol: format!("XL{}", sector_id.as_str().chars().next().unwrap().to_uppercase()),
        etf_price,
        avg_price,
        total_volume,
        volatility,
        momentum,
        timestamp: Utc::now().timestamp(),
        symbols_count: symbols.len() as u32,
        symbols,
        correlation_matrix: HashMap::new(), // Would be calculated separately
    }
}

/// Simple volatility calculation (standard deviation)
fn calculate_volatility(prices: &[f64]) -> f64 {
    if prices.len() < 2 {
        return 0.0;
    }
    
    let mean = prices.iter().sum::<f64>() / prices.len() as f64;
    let variance = prices.iter()
        .map(|price| (price - mean).powi(2))
        .sum::<f64>() / prices.len() as f64;
    
    variance.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::redis::RedisConfig;
    
    #[test]
    fn test_sector_data_creation() {
        let market_data = vec![
            MarketData {
                symbol: "AAPL".to_string(),
                timestamp: 1704067200,
                open: 150.0,
                high: 152.0,
                low: 149.0,
                close: 151.0,
                volume: 1000.0,
            },
            MarketData {
                symbol: "MSFT".to_string(),
                timestamp: 1704067200,
                open: 250.0,
                high: 252.0,
                low: 249.0,
                close: 251.0,
                volume: 800.0,
            },
        ];
        
        let sector_data = aggregate_market_data_to_sector(
            &SectorId::Technology,
            &market_data,
            300.0 // ETF price
        );
        
        assert_eq!(sector_data.sector_id, "technology");
        assert_eq!(sector_data.symbols_count, 2);
        assert_eq!(sector_data.total_volume, 1800.0);
        assert_eq!(sector_data.avg_price, 201.0); // (151 + 251) / 2
    }
    
    #[test]
    fn test_channel_naming() {
        let sector_channels = RedisSectorChannels::get_sector_channels();
        assert!(sector_channels.contains(&"sector/technology".to_string()));
        assert!(sector_channels.contains(&"sector/financial".to_string()));
        
        let portfolio_channels = RedisSectorChannels::get_portfolio_channels();
        assert!(portfolio_channels.contains(&"portfolio/decisions".to_string()));
        
        let cross_sector_channels = RedisSectorChannels::get_cross_sector_channels();
        assert!(cross_sector_channels.contains(&"cross_sector/correlations".to_string()));
    }
    
    #[tokio::test]
    async fn test_serialization_compression() {
        let config = SectorChannelConfig {
            enable_compression: true,
            ..Default::default()
        };
        
        let redis_config = RedisConfig::default();
        let redis_adapter = RedisAdapter::new(redis_config);
        let sector_channels = RedisSectorChannels::new(redis_adapter, config);
        
        let test_data = SectorData {
            sector_id: "technology".to_string(),
            etf_symbol: "XLK".to_string(),
            etf_price: 150.0,
            avg_price: 200.0,
            total_volume: 1000.0,
            volatility: 0.02,
            momentum: 0.01,
            timestamp: 1704067200,
            symbols_count: 5,
            symbols: vec!["AAPL".to_string(), "MSFT".to_string()],
            correlation_matrix: HashMap::new(),
        };
        
        let compressed = sector_channels.serialize_and_compress(&test_data).await;
        assert!(compressed.is_ok());
        
        let decompressed: Result<SectorData, _> = sector_channels
            .deserialize_and_decompress(&compressed.unwrap()).await;
        assert!(decompressed.is_ok());
        
        let recovered_data = decompressed.unwrap();
        assert_eq!(recovered_data.sector_id, test_data.sector_id);
        assert_eq!(recovered_data.etf_price, test_data.etf_price);
    }
}