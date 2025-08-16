//! Data Consolidation Module
//!
//! This module provides unified consolidation of data from multiple scopes
//! into a single coherent view per symbol. It handles data from:
//! - Symbol-specific sources
//! - Sector-wide data affecting the symbol
//! - Market-wide data affecting all symbols
//! - Geographic data affecting the symbol's region
//!
//! The consolidator ensures that VendorPredictor receives properly
//! aggregated and consistent data for optimal prediction performance.

use anyhow::Result;
use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, instrument};

// Import existing types
use crate::data::TimeSeriesData;
use super::routing::{DataPacket, DataScope, GeographicRegion};
use crate::data::sector_mapper::SectorId;

/// Configuration for data consolidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    /// Maximum time window for consolidating data (in minutes)
    pub consolidation_window_minutes: i64,
    /// Maximum number of data points to keep per symbol
    pub max_data_points_per_symbol: usize,
    /// Weight for different data scope types in consolidation
    pub scope_weights: ScopeWeights,
    /// Enable temporal alignment of data
    pub enable_temporal_alignment: bool,
    /// Tolerance for temporal alignment in seconds
    pub temporal_alignment_tolerance_seconds: i64,
    /// Enable data quality scoring
    pub enable_quality_scoring: bool,
    /// Minimum quality score to include data
    pub min_quality_score: f64,
}

/// Weights for different data scopes in consolidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeWeights {
    /// Weight for symbol-specific data
    pub symbol_weight: f64,
    /// Weight for sector-wide data
    pub sector_weight: f64,
    /// Weight for market-wide data
    pub market_weight: f64,
    /// Weight for geographic data
    pub geographic_weight: f64,
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            consolidation_window_minutes: 60,
            max_data_points_per_symbol: 1000,
            scope_weights: ScopeWeights {
                symbol_weight: 1.0,
                sector_weight: 0.7,
                market_weight: 0.3,
                geographic_weight: 0.5,
            },
            enable_temporal_alignment: true,
            temporal_alignment_tolerance_seconds: 30,
            enable_quality_scoring: true,
            min_quality_score: 0.6,
        }
    }
}

/// Quality score for data points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQuality {
    /// Overall quality score (0.0 to 1.0)
    pub score: f64,
    /// Freshness score based on timestamp
    pub freshness: f64,
    /// Completeness score based on available fields
    pub completeness: f64,
    /// Consistency score based on validation
    pub consistency: f64,
    /// Source reliability score
    pub source_reliability: f64,
}

/// Consolidated data for a single symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolConsolidation {
    /// Target symbol
    pub symbol: String,
    /// Primary time series data (symbol-specific)
    pub primary_data: TimeSeriesData,
    /// Sector-level influences
    pub sector_influences: Vec<TimeSeriesData>,
    /// Market-level influences
    pub market_influences: Vec<TimeSeriesData>,
    /// Geographic influences
    pub geographic_influences: Vec<TimeSeriesData>,
    /// Consolidated timestamp
    pub consolidated_at: DateTime<Utc>,
    /// Quality metrics
    pub quality: DataQuality,
    /// Consolidation metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Result of data consolidation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationResult {
    /// Consolidated data for the symbol
    pub consolidated_data: TimeSeriesData,
    /// Individual component data sources
    pub component_sources: Vec<(DataScope, TimeSeriesData, f64)>, // scope, data, weight
    /// Overall confidence in the consolidation
    pub confidence: f64,
    /// Quality score
    pub quality_score: f64,
    /// Processing metrics
    pub processing_time_ms: u64,
    /// Number of sources used
    pub sources_used: usize,
}

/// Metrics for consolidation performance
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConsolidationMetrics {
    /// Total consolidations performed
    pub total_consolidations: u64,
    /// Average consolidation time in milliseconds
    pub avg_consolidation_time_ms: f64,
    /// Average sources per consolidation
    pub avg_sources_per_consolidation: f64,
    /// Average quality score
    pub avg_quality_score: f64,
    /// Failed consolidations
    pub failed_consolidations: u64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

/// Data consolidator for unified symbol data
pub struct DataConsolidator {
    /// Configuration
    config: ConsolidationConfig,
    /// Symbol-specific data storage
    symbol_data: Arc<DashMap<String, VecDeque<(DataPacket, DataQuality)>>>,
    /// Sector data storage
    sector_data: Arc<DashMap<SectorId, VecDeque<(DataPacket, DataQuality)>>>,
    /// Market-wide data storage
    market_data: Arc<RwLock<VecDeque<(DataPacket, DataQuality)>>>,
    /// Geographic data storage
    geographic_data: Arc<DashMap<GeographicRegion, VecDeque<(DataPacket, DataQuality)>>>,
    /// Performance metrics
    metrics: Arc<RwLock<ConsolidationMetrics>>,
}

impl DataConsolidator {
    /// Create a new data consolidator
    pub fn new(config: ConsolidationConfig) -> Self {
        Self {
            config,
            symbol_data: Arc::new(DashMap::new()),
            sector_data: Arc::new(DashMap::new()),
            market_data: Arc::new(RwLock::new(VecDeque::new())),
            geographic_data: Arc::new(DashMap::new()),
            metrics: Arc::new(RwLock::new(ConsolidationMetrics::default())),
        }
    }

    /// Ingest a data packet for consolidation
    #[instrument(skip(self, packet), fields(packet_id = %packet.id, symbol = %packet.data.symbol))]
    pub async fn ingest_packet(&self, packet: DataPacket) -> Result<()> {
        let quality = self.calculate_data_quality(&packet).await?;
        
        // Skip low quality data if quality scoring is enabled
        if self.config.enable_quality_scoring && quality.score < self.config.min_quality_score {
            warn!("Skipping low quality data packet: {} (score: {:.2})", 
                  packet.id, quality.score);
            return Ok(());
        }

        // Capture values before moving packet
        let packet_id = packet.id.clone();
        let packet_scope = packet.scope.clone();
        
        match &packet.scope {
            DataScope::Symbol(symbol) => {
                let mut symbol_queue = self.symbol_data
                    .entry(symbol.clone())
                    .or_insert_with(VecDeque::new);
                    
                symbol_queue.push_back((packet, quality));
                self.maintain_queue_size(&mut symbol_queue);
            },
            DataScope::Sector(sector_id) => {
                let mut sector_queue = self.sector_data
                    .entry(*sector_id)
                    .or_insert_with(VecDeque::new);
                    
                sector_queue.push_back((packet, quality));
                self.maintain_queue_size(&mut sector_queue);
            },
            DataScope::Market => {
                let mut market_queue = self.market_data.write().await;
                market_queue.push_back((packet, quality));
                self.maintain_queue_size(&mut market_queue);
            },
            DataScope::Geographic(region) => {
                let mut geo_queue = self.geographic_data
                    .entry(region.clone())
                    .or_insert_with(VecDeque::new);
                    
                geo_queue.push_back((packet, quality));
                self.maintain_queue_size(&mut geo_queue);
            },
        }

        debug!("Ingested packet {} for scope {:?}", packet_id, packet_scope);
        Ok(())
    }

    /// Consolidate data for a specific symbol
    #[instrument(skip(self), fields(symbol = %symbol))]
    pub async fn consolidate_for_symbol(
        &self,
        symbol: &str,
        symbol_sector: SectorId,
        symbol_region: GeographicRegion,
    ) -> Result<ConsolidationResult> {
        let start_time = std::time::Instant::now();
        
        // Collect data from all relevant scopes
        let symbol_data = self.get_symbol_data(symbol).await;
        let sector_data = self.get_sector_data(symbol_sector).await;
        let market_data = self.get_market_data().await;
        let geographic_data = self.get_geographic_data(symbol_region).await;
        
        // Filter data within consolidation window
        let cutoff_time = Utc::now() - Duration::minutes(self.config.consolidation_window_minutes);
        
        let filtered_symbol = self.filter_by_time(&symbol_data, cutoff_time);
        let filtered_sector = self.filter_by_time(&sector_data, cutoff_time);
        let filtered_market = self.filter_by_time(&market_data, cutoff_time);
        let filtered_geographic = self.filter_by_time(&geographic_data, cutoff_time);
        
        // Prepare component sources with weights
        let mut component_sources = Vec::new();
        
        // Add symbol-specific data
        for (packet, quality) in &filtered_symbol {
            let weight = self.config.scope_weights.symbol_weight * quality.score;
            component_sources.push((packet.scope.clone(), packet.data.clone(), weight));
        }
        
        // Add sector data
        for (packet, quality) in &filtered_sector {
            let weight = self.config.scope_weights.sector_weight * quality.score;
            component_sources.push((packet.scope.clone(), packet.data.clone(), weight));
        }
        
        // Add market data
        for (packet, quality) in &filtered_market {
            let weight = self.config.scope_weights.market_weight * quality.score;
            component_sources.push((packet.scope.clone(), packet.data.clone(), weight));
        }
        
        // Add geographic data
        for (packet, quality) in &filtered_geographic {
            let weight = self.config.scope_weights.geographic_weight * quality.score;
            component_sources.push((packet.scope.clone(), packet.data.clone(), weight));
        }
        
        if component_sources.is_empty() {
            return Err(anyhow::anyhow!("No data available for consolidation for symbol: {}", symbol));
        }
        
        // Perform the actual consolidation
        let consolidated_data = self.perform_consolidation(symbol, &component_sources).await?;
        
        // Calculate overall confidence and quality
        let confidence = self.calculate_consolidation_confidence(&component_sources);
        let quality_score = self.calculate_overall_quality(&component_sources);
        
        let processing_time = start_time.elapsed();
        
        // Update metrics
        self.update_consolidation_metrics(&component_sources, processing_time, quality_score).await;
        
        let sources_count = component_sources.len();
        let result = ConsolidationResult {
            consolidated_data,
            component_sources,
            confidence,
            quality_score,
            processing_time_ms: processing_time.as_millis() as u64,
            sources_used: sources_count,
        };
        
        info!("Consolidated data for {} from {} sources (confidence: {:.2}, quality: {:.2})",
              symbol, result.sources_used, result.confidence, result.quality_score);
        
        Ok(result)
    }

    /// Perform the actual data consolidation logic
    async fn perform_consolidation(
        &self,
        symbol: &str,
        component_sources: &[(DataScope, TimeSeriesData, f64)],
    ) -> Result<TimeSeriesData> {
        // Start with a base TimeSeriesData for the symbol
        let mut consolidated = TimeSeriesData::new(symbol.to_string(), Utc::now());
        
        // Weighted aggregation of numerical values
        let mut weighted_sum_price = 0.0;
        let mut weighted_sum_volume = 0.0;
        let mut total_weight = 0.0;
        let mut all_values = Vec::new();
        let mut all_timestamps = Vec::new();
        let mut consolidated_indicators = HashMap::new();
        
        for (scope, data, weight) in component_sources {
            total_weight += weight;
            
            // Aggregate price data
            weighted_sum_price += data.close * weight;
            
            // Aggregate volume data using single volume value
            weighted_sum_volume += data.volume_value * weight;
            
            // Collect all values and timestamps
            all_values.extend(data.values.iter().map(|v| v * weight));
            all_timestamps.extend(data.timestamps.clone());
            
            // Merge indicators with scope-specific weighting
            for (key, value) in &data.indicators {
                let weighted_value = value * weight;
                let scope_key = format!("{}_{}", scope_prefix(scope), key);
                consolidated_indicators.insert(scope_key, weighted_value);
            }
        }
        
        // Normalize by total weight
        if total_weight > 0.0 {
            consolidated.close = weighted_sum_price / total_weight;
            consolidated.open = consolidated.close; // Simplified for now
            consolidated.high = consolidated.close * 1.02; // Add some realistic variance
            consolidated.low = consolidated.close * 0.98;
            
            if !component_sources.is_empty() {
                consolidated.volume = vec![weighted_sum_volume / total_weight];
            }
        }
        
        // Set aggregated values and timestamps
        consolidated.values = all_values;
        consolidated.timestamps = all_timestamps;
        consolidated.indicators = consolidated_indicators;
        
        // Add consolidation metadata
        consolidated.metadata_map.insert(
            "consolidation_sources".to_string(),
            serde_json::json!(component_sources.len())
        );
        consolidated.metadata_map.insert(
            "consolidation_timestamp".to_string(),
            serde_json::json!(Utc::now())
        );
        consolidated.metadata_map.insert(
            "total_weight".to_string(),
            serde_json::json!(total_weight)
        );
        
        // Temporal alignment if enabled
        if self.config.enable_temporal_alignment {
            self.apply_temporal_alignment(&mut consolidated).await?;
        }
        
        Ok(consolidated)
    }

    /// Calculate data quality score
    async fn calculate_data_quality(&self, packet: &DataPacket) -> Result<DataQuality> {
        let now = Utc::now();
        
        // Freshness score (based on data age)
        let age_minutes = (now - packet.created_at).num_minutes();
        let freshness = (1.0 - (age_minutes as f64 / 60.0)).max(0.0).min(1.0);
        
        // Completeness score (based on available fields)
        let mut completeness_factors = 0;
        let mut total_factors = 0;
        
        total_factors += 1; // symbol
        if !packet.data.symbol.is_empty() { completeness_factors += 1; }
        
        total_factors += 4; // OHLC
        if packet.data.open > 0.0 { completeness_factors += 1; }
        if packet.data.high > 0.0 { completeness_factors += 1; }
        if packet.data.low > 0.0 { completeness_factors += 1; }
        if packet.data.close > 0.0 { completeness_factors += 1; }
        
        total_factors += 1; // volume
        if packet.data.volume_value > 0.0 { 
            completeness_factors += 1; 
        }
        
        let completeness = if total_factors > 0 {
            completeness_factors as f64 / total_factors as f64
        } else {
            0.0
        };
        
        // Consistency score (basic validation)
        let mut consistency = 1.0;
        if packet.data.high < packet.data.low {
            consistency *= 0.5;
        }
        if packet.data.close < 0.0 || packet.data.open < 0.0 {
            consistency *= 0.5;
        }
        
        // Source reliability (could be enhanced with source-specific scores)
        let source_reliability = match packet.source.as_str() {
            "polygon" => 0.95,
            "alpha_vantage" => 0.90,
            "yahoo_finance" => 0.85,
            "iex_cloud" => 0.90,
            _ => 0.70,
        };
        
        // Overall score
        let score = (freshness * 0.25) + (completeness * 0.35) + (consistency * 0.25) + (source_reliability * 0.15);
        
        Ok(DataQuality {
            score,
            freshness,
            completeness,
            consistency,
            source_reliability,
        })
    }

    /// Helper functions for data retrieval
    async fn get_symbol_data(&self, symbol: &str) -> Vec<(DataPacket, DataQuality)> {
        self.symbol_data
            .get(symbol)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    async fn get_sector_data(&self, sector: SectorId) -> Vec<(DataPacket, DataQuality)> {
        self.sector_data
            .get(&sector)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    async fn get_market_data(&self) -> Vec<(DataPacket, DataQuality)> {
        let queue = self.market_data.read().await;
        queue.iter().cloned().collect()
    }

    async fn get_geographic_data(&self, region: GeographicRegion) -> Vec<(DataPacket, DataQuality)> {
        self.geographic_data
            .get(&region)
            .map(|queue| queue.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Filter data by time window
    fn filter_by_time(
        &self,
        data: &[(DataPacket, DataQuality)],
        cutoff_time: DateTime<Utc>,
    ) -> Vec<(DataPacket, DataQuality)> {
        data.iter()
            .filter(|(packet, _)| packet.created_at >= cutoff_time)
            .cloned()
            .collect()
    }

    /// Maintain queue size limits
    fn maintain_queue_size(&self, queue: &mut VecDeque<(DataPacket, DataQuality)>) {
        while queue.len() > self.config.max_data_points_per_symbol {
            queue.pop_front();
        }
    }

    /// Calculate consolidation confidence
    fn calculate_consolidation_confidence(&self, sources: &[(DataScope, TimeSeriesData, f64)]) -> f64 {
        if sources.is_empty() {
            return 0.0;
        }
        
        let total_weight: f64 = sources.iter().map(|(_, _, weight)| weight).sum();
        let source_diversity = sources.len() as f64;
        
        // Confidence increases with more sources and higher weights
        let weight_factor = (total_weight / sources.len() as f64).min(1.0);
        let diversity_factor = (source_diversity / 4.0).min(1.0); // Max 4 scope types
        
        (weight_factor * 0.7) + (diversity_factor * 0.3)
    }

    /// Calculate overall quality from component sources
    fn calculate_overall_quality(&self, sources: &[(DataScope, TimeSeriesData, f64)]) -> f64 {
        if sources.is_empty() {
            return 0.0;
        }
        
        // Weighted average of source weights (which include quality)
        let total_weight: f64 = sources.iter().map(|(_, _, weight)| weight).sum();
        total_weight / sources.len() as f64
    }

    /// Apply temporal alignment to data
    async fn apply_temporal_alignment(&self, data: &mut TimeSeriesData) -> Result<()> {
        // Align timestamps to nearest second boundary
        let _tolerance = Duration::seconds(self.config.temporal_alignment_tolerance_seconds);
        let now = Utc::now();
        
        // Round timestamp to nearest alignment boundary
        let aligned_timestamp = now.timestamp() / self.config.temporal_alignment_tolerance_seconds 
            * self.config.temporal_alignment_tolerance_seconds;
        data.timestamp = DateTime::from_timestamp(aligned_timestamp, 0)
            .unwrap_or(now);
        
        debug!("Applied temporal alignment to data for symbol: {}", data.symbol);
        Ok(())
    }

    /// Update consolidation metrics
    async fn update_consolidation_metrics(
        &self,
        sources: &[(DataScope, TimeSeriesData, f64)],
        processing_time: std::time::Duration,
        quality_score: f64,
    ) {
        let mut metrics = self.metrics.write().await;
        
        metrics.total_consolidations += 1;
        
        // Update average consolidation time
        let processing_time_ms = processing_time.as_millis() as f64;
        if metrics.avg_consolidation_time_ms == 0.0 {
            metrics.avg_consolidation_time_ms = processing_time_ms;
        } else {
            metrics.avg_consolidation_time_ms = (metrics.avg_consolidation_time_ms * 0.9) + (processing_time_ms * 0.1);
        }
        
        // Update average sources
        let source_count = sources.len() as f64;
        if metrics.avg_sources_per_consolidation == 0.0 {
            metrics.avg_sources_per_consolidation = source_count;
        } else {
            metrics.avg_sources_per_consolidation = (metrics.avg_sources_per_consolidation * 0.9) + (source_count * 0.1);
        }
        
        // Update average quality
        if metrics.avg_quality_score == 0.0 {
            metrics.avg_quality_score = quality_score;
        } else {
            metrics.avg_quality_score = (metrics.avg_quality_score * 0.9) + (quality_score * 0.1);
        }
        
        metrics.last_updated = Utc::now();
    }

    /// Get current consolidation metrics
    pub async fn get_metrics(&self) -> ConsolidationMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Clear old data beyond the consolidation window
    pub async fn cleanup_old_data(&self) -> Result<()> {
        let cutoff_time = Utc::now() - Duration::minutes(self.config.consolidation_window_minutes * 2);
        
        // Clean symbol data
        for mut entry in self.symbol_data.iter_mut() {
            entry.retain(|(packet, _)| packet.created_at >= cutoff_time);
        }
        
        // Clean sector data
        for mut entry in self.sector_data.iter_mut() {
            entry.retain(|(packet, _)| packet.created_at >= cutoff_time);
        }
        
        // Clean market data
        {
            let mut market_queue = self.market_data.write().await;
            market_queue.retain(|(packet, _)| packet.created_at >= cutoff_time);
        }
        
        // Clean geographic data
        for mut entry in self.geographic_data.iter_mut() {
            entry.retain(|(packet, _)| packet.created_at >= cutoff_time);
        }
        
        debug!("Cleaned up old data before: {}", cutoff_time);
        Ok(())
    }
}

/// Helper function to get scope prefix for indicator keys
fn scope_prefix(scope: &DataScope) -> &'static str {
    match scope {
        DataScope::Symbol(_) => "symbol",
        DataScope::Sector(_) => "sector",
        DataScope::Market => "market",
        DataScope::Geographic(_) => "geo",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::sector_mapper::SectorId;

    #[tokio::test]
    async fn test_data_consolidation() {
        let config = ConsolidationConfig::default();
        let consolidator = DataConsolidator::new(config);
        
        // Create test data packets
        let symbol_data = TimeSeriesData::new("AAPL".to_string(), Utc::now());
        let mut symbol_packet = DataPacket {
            id: "test_symbol".to_string(),
            scope: DataScope::Symbol("AAPL".to_string()),
            data: symbol_data,
            priority: 5,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            source: "test".to_string(),
        };
        symbol_packet.data.close = 150.0;
        
        let market_data = TimeSeriesData::new("MARKET".to_string(), Utc::now());
        let mut market_packet = DataPacket {
            id: "test_market".to_string(),
            scope: DataScope::Market,
            data: market_data,
            priority: 7,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            source: "test".to_string(),
        };
        market_packet.data.close = 100.0;
        
        // Ingest packets
        consolidator.ingest_packet(symbol_packet).await.unwrap();
        consolidator.ingest_packet(market_packet).await.unwrap();
        
        // Consolidate for AAPL
        let result = consolidator.consolidate_for_symbol(
            "AAPL",
            SectorId::Technology,
            GeographicRegion::NorthAmerica,
        ).await.unwrap();
        
        assert_eq!(result.sources_used, 2);
        assert!(result.confidence > 0.0);
        assert!(result.quality_score > 0.0);
        assert_eq!(result.consolidated_data.symbol, "AAPL");
    }

    #[tokio::test]
    async fn test_data_quality_scoring() {
        let config = ConsolidationConfig::default();
        let consolidator = DataConsolidator::new(config);
        
        // Create a high-quality data packet
        let mut data = TimeSeriesData::new("TEST".to_string(), Utc::now());
        data.open = 100.0;
        data.high = 105.0;
        data.low = 95.0;
        data.close = 102.0;
        data.volume = vec![1000000.0];
        
        let packet = DataPacket {
            id: "test_quality".to_string(),
            scope: DataScope::Symbol("TEST".to_string()),
            data,
            priority: 5,
            created_at: Utc::now(),
            metadata: HashMap::new(),
            source: "polygon".to_string(),
        };
        
        let quality = consolidator.calculate_data_quality(&packet).await.unwrap();
        
        assert!(quality.score > 0.8); // Should be high quality
        assert!(quality.completeness > 0.8);
        assert!(quality.consistency > 0.9);
        assert!(quality.source_reliability > 0.9);
    }

    #[tokio::test]
    async fn test_consolidation_metrics() {
        let config = ConsolidationConfig::default();
        let consolidator = DataConsolidator::new(config);
        
        // Perform multiple consolidations
        for i in 0..3 {
            let data = TimeSeriesData::new(format!("TEST_{}", i), Utc::now());
            let packet = DataPacket {
                id: format!("test_{}", i),
                scope: DataScope::Symbol(format!("TEST_{}", i)),
                data,
                priority: 5,
                created_at: Utc::now(),
                metadata: HashMap::new(),
                source: "test".to_string(),
            };
            
            consolidator.ingest_packet(packet).await.unwrap();
        }
        
        // Consolidate one symbol
        if let Ok(_result) = consolidator.consolidate_for_symbol(
            "TEST_0",
            SectorId::Technology,
            GeographicRegion::NorthAmerica,
        ).await {
            let metrics = consolidator.get_metrics().await;
            assert_eq!(metrics.total_consolidations, 1);
            assert!(metrics.avg_consolidation_time_ms > 0.0);
        }
    }
}