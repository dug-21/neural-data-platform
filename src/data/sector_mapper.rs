//! Sector Mapping and Aggregation System
//!
//! Maps individual symbols to sectors for efficient model sharing
//! and provides sector-level feature aggregation.

use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Sector identifier enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SectorId {
    Technology,
    FinancialServices,
    Healthcare,
    Energy,
    ConsumerDiscretionary,
    ConsumerStaples,
    Industrials,
    Materials,
    Utilities,
    RealEstate,
    Communication,
    Custom(u32), // For custom sectors
}

impl SectorId {
    pub fn as_str(&self) -> &str {
        match self {
            SectorId::Technology => "technology",
            SectorId::FinancialServices => "financial_services",
            SectorId::Healthcare => "healthcare",
            SectorId::Energy => "energy",
            SectorId::ConsumerDiscretionary => "consumer_discretionary",
            SectorId::ConsumerStaples => "consumer_staples",
            SectorId::Industrials => "industrials",
            SectorId::Materials => "materials",
            SectorId::Utilities => "utilities",
            SectorId::RealEstate => "real_estate",
            SectorId::Communication => "communication",
            SectorId::Custom(_) => "custom",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "technology" | "tech" => Some(SectorId::Technology),
            "financial_services" | "financial" | "finance" => Some(SectorId::FinancialServices),
            "healthcare" | "health" => Some(SectorId::Healthcare),
            "energy" => Some(SectorId::Energy),
            "consumer_discretionary" => Some(SectorId::ConsumerDiscretionary),
            "consumer_staples" => Some(SectorId::ConsumerStaples),
            "industrials" => Some(SectorId::Industrials),
            "materials" => Some(SectorId::Materials),
            "utilities" => Some(SectorId::Utilities),
            "real_estate" | "realestate" => Some(SectorId::RealEstate),
            "communication" | "comm" => Some(SectorId::Communication),
            _ => None,
        }
    }
}

/// Market cap tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketCapTier {
    LargeCap,   // > $10B
    MidCap,     // $2B - $10B
    SmallCap,   // < $2B
}

/// Sector information for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorInfo {
    pub id: String, // Using String for flexibility instead of enum
    pub sector_id: SectorId,
    pub sub_sector: Option<String>,
    pub market_cap_tier: MarketCapTier,
    pub weight_in_sector: f64, // 0.0 to 1.0
    pub correlation_group: Option<String>,
}

/// Sector ETF mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorETF {
    pub sector_id: SectorId,
    pub etf_symbol: String,
    pub description: String,
}

/// Sector mapper configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorMapperConfig {
    pub enable_dynamic_updates: bool,
    pub cache_ttl_seconds: u64,
    pub default_sector: String,
}

impl Default for SectorMapperConfig {
    fn default() -> Self {
        Self {
            enable_dynamic_updates: true,
            cache_ttl_seconds: 3600,
            default_sector: "technology".to_string(),
        }
    }
}

/// Main sector mapper struct
pub struct SectorMapper {
    /// Symbol to sector mappings
    symbol_sectors: Arc<DashMap<String, SectorInfo>>,
    
    /// Sector ETF representatives
    sector_etfs: Arc<DashMap<SectorId, String>>,
    
    /// Dynamic sector updates
    sector_updates: Arc<RwLock<Vec<SectorUpdate>>>,
    
    /// Configuration
    config: SectorMapperConfig,
}

/// Dynamic sector update record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorUpdate {
    pub symbol: String,
    pub old_sector: Option<SectorId>,
    pub new_sector: SectorId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub reason: String,
}

impl SectorMapper {
    /// Create new sector mapper
    pub fn new(config: SectorMapperConfig) -> Self {
        info!("Initializing SectorMapper with default mappings");
        
        let mapper = Self {
            symbol_sectors: Arc::new(DashMap::new()),
            sector_etfs: Arc::new(DashMap::new()),
            sector_updates: Arc::new(RwLock::new(Vec::new())),
            config,
        };
        
        // Initialize default mappings
        mapper.init_default_mappings();
        mapper
    }
    
    /// Initialize default sector mappings
    fn init_default_mappings(&self) {
        // Technology sector
        self.add_symbol_mapping("AAPL", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Consumer Electronics".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.22,
            correlation_group: Some("FAANG".to_string()),
        });
        
        self.add_symbol_mapping("MSFT", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Software".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.21,
            correlation_group: None,
        });
        
        self.add_symbol_mapping("GOOGL", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            sub_sector: Some("Internet Services".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.10,
            correlation_group: Some("FAANG".to_string()),
        });
        
        // Financial sector
        self.add_symbol_mapping("JPM", SectorInfo {
            id: "financial_services".to_string(),
            sector_id: SectorId::FinancialServices,
            sub_sector: Some("Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.13,
            correlation_group: Some("big_banks".to_string()),
        });
        
        self.add_symbol_mapping("BAC", SectorInfo {
            id: "financial_services".to_string(),
            sector_id: SectorId::FinancialServices,
            sub_sector: Some("Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.09,
            correlation_group: Some("big_banks".to_string()),
        });
        
        // Healthcare sector
        self.add_symbol_mapping("JNJ", SectorInfo {
            id: "healthcare".to_string(),
            sector_id: SectorId::Healthcare,
            sub_sector: Some("Pharmaceuticals".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.12,
            correlation_group: None,
        });
        
        // Add sector ETF mappings
        self.sector_etfs.insert(SectorId::Technology, "XLK".to_string());
        self.sector_etfs.insert(SectorId::FinancialServices, "XLF".to_string());
        self.sector_etfs.insert(SectorId::Healthcare, "XLV".to_string());
        self.sector_etfs.insert(SectorId::Energy, "XLE".to_string());
        self.sector_etfs.insert(SectorId::ConsumerDiscretionary, "XLY".to_string());
        
        info!("Initialized default sector mappings for {} symbols", self.symbol_sectors.len());
    }
    
    /// Add symbol mapping
    pub fn add_symbol_mapping(&self, symbol: &str, info: SectorInfo) {
        debug!("Adding sector mapping for {}: {:?}", symbol, info.sector_id);
        self.symbol_sectors.insert(symbol.to_string(), info);
    }
    
    /// Get sector for a symbol
    pub fn get_sector(&self, symbol: &str) -> Result<SectorInfo> {
        self.symbol_sectors
            .get(symbol)
            .map(|entry| entry.clone())
            .ok_or_else(|| {
                // If not found, use default sector
                warn!("Symbol {} not found in sector mapping, using default", symbol);
                let default_sector = SectorInfo {
                    id: self.config.default_sector.clone(),
                    sector_id: SectorId::from_str(&self.config.default_sector)
                        .unwrap_or(SectorId::Technology),
                    sub_sector: None,
                    market_cap_tier: MarketCapTier::MidCap,
                    weight_in_sector: 0.01,
                    correlation_group: None,
                };
                self.symbol_sectors.insert(symbol.to_string(), default_sector.clone());
                anyhow!("").context(format!("Using default sector for {}", symbol))
            })
            .or_else(|_| {
                self.symbol_sectors
                    .get(symbol)
                    .map(|entry| entry.clone())
                    .ok_or_else(|| anyhow!("Failed to get sector for symbol: {}", symbol))
            })
    }
    
    /// Get all symbols in a sector
    pub fn get_symbols_in_sector(&self, sector: &SectorId) -> Vec<String> {
        self.symbol_sectors
            .iter()
            .filter(|entry| &entry.value().sector_id == sector)
            .map(|entry| entry.key().clone())
            .collect()
    }
    
    /// Get sector ETF representative
    pub fn get_sector_etf(&self, sector: &SectorId) -> Option<String> {
        self.sector_etfs.get(sector).map(|entry| entry.clone())
    }
    
    /// Load sector mappings from configuration file
    pub async fn load_from_config(&mut self, config_path: &Path) -> Result<()> {
        info!("Loading sector mappings from: {:?}", config_path);
        
        // This would load from a TOML or JSON file
        // For now, we're using hardcoded defaults
        
        Ok(())
    }
    
    /// Update sector for a symbol dynamically
    pub async fn update_sector(
        &self,
        symbol: &str,
        new_sector: SectorId,
        reason: &str,
    ) -> Result<()> {
        let old_sector = self.symbol_sectors
            .get(symbol)
            .map(|entry| entry.sector_id);
        
        // Update the mapping
        if let Some(mut entry) = self.symbol_sectors.get_mut(symbol) {
            entry.sector_id = new_sector;
            entry.id = new_sector.as_str().to_string();
        }
        
        // Record the update
        let update = SectorUpdate {
            symbol: symbol.to_string(),
            old_sector,
            new_sector,
            timestamp: chrono::Utc::now(),
            reason: reason.to_string(),
        };
        
        let mut updates = self.sector_updates.write().await;
        updates.push(update);
        
        info!("Updated sector for {} to {:?}: {}", symbol, new_sector, reason);
        Ok(())
    }
    
    /// Get sector aggregation statistics
    pub fn get_sector_stats(&self) -> HashMap<SectorId, SectorStats> {
        let mut stats = HashMap::new();
        
        for entry in self.symbol_sectors.iter() {
            let sector = entry.value().sector_id;
            let stat = stats.entry(sector).or_insert(SectorStats::default());
            stat.symbol_count += 1;
            stat.total_weight += entry.value().weight_in_sector;
            
            match entry.value().market_cap_tier {
                MarketCapTier::LargeCap => stat.large_cap_count += 1,
                MarketCapTier::MidCap => stat.mid_cap_count += 1,
                MarketCapTier::SmallCap => stat.small_cap_count += 1,
            }
        }
        
        stats
    }
}

/// Sector statistics
#[derive(Debug, Default, Clone)]
pub struct SectorStats {
    pub symbol_count: usize,
    pub total_weight: f64,
    pub large_cap_count: usize,
    pub mid_cap_count: usize,
    pub small_cap_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sector_id_conversion() {
        assert_eq!(SectorId::from_str("technology"), Some(SectorId::Technology));
        assert_eq!(SectorId::from_str("TECH"), Some(SectorId::Technology));
        assert_eq!(SectorId::from_str("financial"), Some(SectorId::FinancialServices));
        assert_eq!(SectorId::from_str("unknown"), None);
    }
    
    #[test]
    fn test_sector_mapper_creation() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        // Check default mappings exist
        let aapl_sector = mapper.get_sector("AAPL");
        assert!(aapl_sector.is_ok());
        
        let sector_info = aapl_sector.unwrap();
        assert_eq!(sector_info.sector_id, SectorId::Technology);
        assert_eq!(sector_info.sub_sector, Some("Consumer Electronics".to_string()));
    }
    
    #[test]
    fn test_get_symbols_in_sector() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        let tech_symbols = mapper.get_symbols_in_sector(&SectorId::Technology);
        assert!(tech_symbols.contains(&"AAPL".to_string()));
        assert!(tech_symbols.contains(&"MSFT".to_string()));
        assert!(tech_symbols.contains(&"GOOGL".to_string()));
    }
    
    #[test]
    fn test_sector_etf_mapping() {
        let mapper = SectorMapper::new(SectorMapperConfig::default());
        
        let tech_etf = mapper.get_sector_etf(&SectorId::Technology);
        assert_eq!(tech_etf, Some("XLK".to_string()));
        
        let finance_etf = mapper.get_sector_etf(&SectorId::FinancialServices);
        assert_eq!(finance_etf, Some("XLF".to_string()));
    }
}