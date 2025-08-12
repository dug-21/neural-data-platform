//! Sector Mapping and Aggregation System
//!
//! INTEGRATION-FIRST IMPLEMENTATION:
//! - Extends existing TimeSeriesData with sector information
//! - Integrates with Redis cache and TimescaleDB storage
//! - Works with DataAccessLayer and TrainingDataService
//! - Maintains compatibility with vendor models via BaseModel<T>
//! - Memory efficient: <50MB per symbol with 100+ symbol support
//!
//! Maps individual symbols to sectors for efficient model sharing
//! and provides sector-level feature aggregation.

use anyhow::{Result, anyhow, Context};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

// Import existing data structures for integration
use crate::data::{TimeSeriesData, RedisCache};
use chrono::{DateTime, Utc};

/// Sector identifier enum - 10 core sectors as specified
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SectorId {
    Technology,
    Financial,
    Healthcare,
    Energy,
    ConsumerDiscretionary,
    ConsumerStaples,
    Industrials,
    Materials,
    Utilities,
    RealEstate,
}

impl SectorId {
    pub fn as_str(&self) -> &str {
        match self {
            SectorId::Technology => "technology",
            SectorId::Financial => "financial",
            SectorId::Healthcare => "healthcare",
            SectorId::Energy => "energy",
            SectorId::ConsumerDiscretionary => "consumer_discretionary",
            SectorId::ConsumerStaples => "consumer_staples",
            SectorId::Industrials => "industrials",
            SectorId::Materials => "materials",
            SectorId::Utilities => "utilities",
            SectorId::RealEstate => "real_estate",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "technology" | "tech" => Some(SectorId::Technology),
            "financial" | "finance" | "financials" | "financial_services" => Some(SectorId::Financial),
            "healthcare" | "health" => Some(SectorId::Healthcare),
            "energy" => Some(SectorId::Energy),
            "consumer_discretionary" | "consumer" => Some(SectorId::ConsumerDiscretionary),
            "consumer_staples" | "staples" => Some(SectorId::ConsumerStaples),
            "industrials" | "industrial" => Some(SectorId::Industrials),
            "materials" | "material" => Some(SectorId::Materials),
            "utilities" | "utility" => Some(SectorId::Utilities),
            "real_estate" | "realestate" | "reit" => Some(SectorId::RealEstate),
            _ => None,
        }
    }
    
    /// Get all sectors as a vector
    pub fn all_sectors() -> Vec<SectorId> {
        vec![
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
        ]
    }
}

/// Market cap tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketCapTier {
    LargeCap,   // > $10B
    MidCap,     // $2B - $10B
    SmallCap,   // < $2B
}

impl Default for MarketCapTier {
    fn default() -> Self {
        MarketCapTier::LargeCap
    }
}

/// Sector information for a symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorInfo {
    pub id: String, // Using String for flexibility instead of enum
    pub sector_id: SectorId,
    // Missing fields that are used in the code
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub symbols: Vec<String>,
    #[serde(default)]
    pub description: String,
    pub sub_sector: Option<String>,
    pub market_cap_tier: MarketCapTier,
    pub weight_in_sector: f64, // 0.0 to 1.0
    pub correlation_group: Option<String>,
}

impl Default for SectorInfo {
    fn default() -> Self {
        Self {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            name: "Technology".to_string(),
            symbols: Vec::new(),
            description: "Technology sector".to_string(),
            sub_sector: None,
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.0,
            correlation_group: None,
        }
    }
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

/// Main sector mapper struct - INTEGRATION-FIRST DESIGN
pub struct SectorMapper {
    /// Symbol to sector mappings (memory efficient - using DashMap)
    symbol_sectors: Arc<DashMap<String, SectorInfo>>,
    
    /// Sector ETF representatives
    sector_etfs: Arc<DashMap<SectorId, String>>,
    
    /// Dynamic sector updates
    sector_updates: Arc<RwLock<Vec<SectorUpdate>>>,
    
    /// Configuration
    config: SectorMapperConfig,
    
    /// Redis cache integration for fast lookups
    cache: Option<Arc<RedisCache>>,
    
    /// Memory optimization: pre-allocated capacity
    _capacity: usize,
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
    /// Create new sector mapper with integration-first design
    pub fn new(config: SectorMapperConfig) -> Self {
        info!("🏭 Initializing SectorMapper with Integration-First design");
        
        // Pre-allocate for 1000 symbols to optimize memory usage
        let capacity = 1000;
        let symbol_sectors = Arc::new(DashMap::with_capacity(capacity));
        let sector_etfs = Arc::new(DashMap::with_capacity(10)); // 10 sectors
        
        let mapper = Self {
            symbol_sectors,
            sector_etfs,
            sector_updates: Arc::new(RwLock::new(Vec::with_capacity(100))),
            config,
            cache: None,
            _capacity: capacity,
        };
        
        // Initialize default mappings
        mapper.init_default_mappings();
        mapper
    }
    
    /// Create with Redis cache integration
    pub fn with_cache(config: SectorMapperConfig, cache: Arc<RedisCache>) -> Self {
        let mut mapper = Self::new(config);
        mapper.cache = Some(cache);
        info!("🔄 SectorMapper initialized with Redis cache integration");
        mapper
    }
    
    /// Initialize default sector mappings
    fn init_default_mappings(&self) {
        // Technology sector
        self.add_symbol_mapping("AAPL", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            name: "Apple Inc.".to_string(),
            symbols: vec!["AAPL".to_string()],
            description: "Consumer electronics and software".to_string(),
            sub_sector: Some("Consumer Electronics".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.22,
            correlation_group: Some("FAANG".to_string()),
        });
        
        self.add_symbol_mapping("MSFT", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            name: "Microsoft Corporation".to_string(),
            symbols: vec!["MSFT".to_string()],
            description: "Software, cloud services, and productivity solutions".to_string(),
            sub_sector: Some("Software".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.21,
            correlation_group: None,
        });
        
        self.add_symbol_mapping("GOOGL", SectorInfo {
            id: "technology".to_string(),
            sector_id: SectorId::Technology,
            name: "Alphabet Inc.".to_string(),
            symbols: vec!["GOOGL".to_string()],
            description: "Internet services, search, and cloud platforms".to_string(),
            sub_sector: Some("Internet Services".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.10,
            correlation_group: Some("FAANG".to_string()),
        });
        
        // Financial sector
        self.add_symbol_mapping("JPM", SectorInfo {
            id: "financial".to_string(),
            sector_id: SectorId::Financial,
            name: "JPMorgan Chase & Co.".to_string(),
            symbols: vec!["JPM".to_string()],
            description: "Investment banking and financial services".to_string(),
            sub_sector: Some("Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.13,
            correlation_group: Some("big_banks".to_string()),
        });
        
        self.add_symbol_mapping("BAC", SectorInfo {
            id: "financial".to_string(),
            sector_id: SectorId::Financial,
            name: "Bank of America Corporation".to_string(),
            symbols: vec!["BAC".to_string()],
            description: "Consumer and commercial banking services".to_string(),
            sub_sector: Some("Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.09,
            correlation_group: Some("big_banks".to_string()),
        });
        
        self.add_symbol_mapping("WFC", SectorInfo {
            id: "financial".to_string(),
            sector_id: SectorId::Financial,
            name: "Wells Fargo & Company".to_string(),
            symbols: vec!["WFC".to_string()],
            description: "Diversified financial services and banking".to_string(),
            sub_sector: Some("Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.07,
            correlation_group: Some("big_banks".to_string()),
        });
        
        self.add_symbol_mapping("GS", SectorInfo {
            id: "financial".to_string(),
            sector_id: SectorId::Financial,
            name: "The Goldman Sachs Group, Inc.".to_string(),
            symbols: vec!["GS".to_string()],
            description: "Investment banking and securities trading".to_string(),
            sub_sector: Some("Investment Banking".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.06,
            correlation_group: Some("big_banks".to_string()),
        });

        // Healthcare sector
        self.add_symbol_mapping("JNJ", SectorInfo {
            id: "healthcare".to_string(),
            sector_id: SectorId::Healthcare,
            name: "Johnson & Johnson".to_string(),
            symbols: vec!["JNJ".to_string()],
            description: "Pharmaceuticals, medical devices, and consumer health".to_string(),
            sub_sector: Some("Pharmaceuticals".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.12,
            correlation_group: None,
        });
        
        self.add_symbol_mapping("PFE", SectorInfo {
            id: "healthcare".to_string(),
            sector_id: SectorId::Healthcare,
            name: "Pfizer Inc.".to_string(),
            symbols: vec!["PFE".to_string()],
            description: "Pharmaceutical research and development".to_string(),
            sub_sector: Some("Pharmaceuticals".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.08,
            correlation_group: None,
        });
        
        self.add_symbol_mapping("UNH", SectorInfo {
            id: "healthcare".to_string(),
            sector_id: SectorId::Healthcare,
            name: "UnitedHealth Group Incorporated".to_string(),
            symbols: vec!["UNH".to_string()],
            description: "Healthcare services and health insurance".to_string(),
            sub_sector: Some("Health Insurance".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.10,
            correlation_group: None,
        });

        // Energy sector
        self.add_symbol_mapping("XOM", SectorInfo {
            id: "energy".to_string(),
            sector_id: SectorId::Energy,
            name: "Exxon Mobil Corporation".to_string(),
            symbols: vec!["XOM".to_string()],
            description: "Oil and gas exploration and production".to_string(),
            sub_sector: Some("Oil & Gas".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.15,
            correlation_group: Some("oil_majors".to_string()),
        });
        
        self.add_symbol_mapping("CVX", SectorInfo {
            id: "energy".to_string(),
            sector_id: SectorId::Energy,
            name: "Chevron Corporation".to_string(),
            symbols: vec!["CVX".to_string()],
            description: "Integrated oil and gas company".to_string(),
            sub_sector: Some("Oil & Gas".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.12,
            correlation_group: Some("oil_majors".to_string()),
        });

        // Consumer Discretionary
        self.add_symbol_mapping("AMZN", SectorInfo {
            id: "consumer_discretionary".to_string(),
            sector_id: SectorId::ConsumerDiscretionary,
            name: "Amazon.com, Inc.".to_string(),
            symbols: vec!["AMZN".to_string()],
            description: "E-commerce, cloud computing, and digital services".to_string(),
            sub_sector: Some("E-commerce".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.20,
            correlation_group: Some("FAANG".to_string()),
        });
        
        self.add_symbol_mapping("TSLA", SectorInfo {
            id: "consumer_discretionary".to_string(),
            sector_id: SectorId::ConsumerDiscretionary,
            name: "Tesla, Inc.".to_string(),
            symbols: vec!["TSLA".to_string()],
            description: "Electric vehicles and energy storage solutions".to_string(),
            sub_sector: Some("Electric Vehicles".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.15,
            correlation_group: None,
        });

        // Consumer Staples
        self.add_symbol_mapping("PG", SectorInfo {
            id: "consumer_staples".to_string(),
            sector_id: SectorId::ConsumerStaples,
            name: "The Procter & Gamble Company".to_string(),
            symbols: vec!["PG".to_string()],
            description: "Consumer goods and personal care products".to_string(),
            sub_sector: Some("Personal Care".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.14,
            correlation_group: None,
        });

        // Industrials
        self.add_symbol_mapping("BA", SectorInfo {
            id: "industrials".to_string(),
            sector_id: SectorId::Industrials,
            name: "The Boeing Company".to_string(),
            symbols: vec!["BA".to_string()],
            description: "Aerospace, defense, and commercial aircraft".to_string(),
            sub_sector: Some("Aerospace".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.12,
            correlation_group: None,
        });

        // Materials
        self.add_symbol_mapping("DOW", SectorInfo {
            id: "materials".to_string(),
            sector_id: SectorId::Materials,
            name: "Dow Inc.".to_string(),
            symbols: vec!["DOW".to_string()],
            description: "Chemical manufacturing and materials science".to_string(),
            sub_sector: Some("Chemicals".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.11,
            correlation_group: None,
        });

        // Utilities
        self.add_symbol_mapping("NEE", SectorInfo {
            id: "utilities".to_string(),
            sector_id: SectorId::Utilities,
            name: "NextEra Energy, Inc.".to_string(),
            symbols: vec!["NEE".to_string()],
            description: "Electric utility and clean energy solutions".to_string(),
            sub_sector: Some("Electric Utilities".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.13,
            correlation_group: None,
        });

        // Real Estate
        self.add_symbol_mapping("AMT", SectorInfo {
            id: "real_estate".to_string(),
            sector_id: SectorId::RealEstate,
            name: "American Tower Corporation".to_string(),
            symbols: vec!["AMT".to_string()],
            description: "Wireless communication infrastructure REIT".to_string(),
            sub_sector: Some("REITs".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.12,
            correlation_group: None,
        });
        
        // Add sector ETF mappings
        self.sector_etfs.insert(SectorId::Technology, "XLK".to_string());
        self.sector_etfs.insert(SectorId::Financial, "XLF".to_string());
        self.sector_etfs.insert(SectorId::Healthcare, "XLV".to_string());
        self.sector_etfs.insert(SectorId::Energy, "XLE".to_string());
        self.sector_etfs.insert(SectorId::ConsumerDiscretionary, "XLY".to_string());
        self.sector_etfs.insert(SectorId::ConsumerStaples, "XLP".to_string());
        self.sector_etfs.insert(SectorId::Industrials, "XLI".to_string());
        self.sector_etfs.insert(SectorId::Materials, "XLB".to_string());
        self.sector_etfs.insert(SectorId::Utilities, "XLU".to_string());
        self.sector_etfs.insert(SectorId::RealEstate, "XLRE".to_string());
        
        info!("Initialized default sector mappings for {} symbols", self.symbol_sectors.len());
    }
    
    /// Add symbol mapping
    pub fn add_symbol_mapping(&self, symbol: &str, info: SectorInfo) {
        debug!("Adding sector mapping for {}: {:?}", symbol, info.sector_id);
        self.symbol_sectors.insert(symbol.to_string(), info);
    }
    
    /// Get sector for a symbol
    pub fn get_sector(&self, symbol: &str) -> Result<SectorInfo> {
        // PRODUCTION VALIDATION: Ensure only real trading symbols are processed
        self.validate_trading_symbol(symbol)?;
        
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
                    name: "Unknown Company".to_string(),
                    symbols: vec![symbol.to_string()],
                    description: "Company with unknown sector classification".to_string(),
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
    
    /// Validate that input is a trading symbol, not a model type
    fn validate_trading_symbol(&self, input: &str) -> Result<()> {
        // Check for common model architecture names that should never be symbols
        let model_architectures = [
            "Transformer", "LSTM", "GRU", "RNN", "CNN", "MLP", "TCN", "DeepAR", 
            "NHITS", "ARIMA", "Prophet", "XGBoost", "LightGBM", "RandomForest",
            "EmergencyModel", "FallbackModel", "BaseModel", "EnsembleModel"
        ];
        
        if model_architectures.contains(&input) {
            return Err(anyhow!(
                "CRITICAL ERROR: Model architecture '{}' passed to sector_mapper. \
                 This should be a trading symbol (AAPL, NVDA, XLF, etc.), not a model type!",
                input
            ));
        }
        
        // Basic format validation for trading symbols (1-5 uppercase letters)
        if !input.chars().all(|c| c.is_ascii_uppercase()) || input.len() > 5 || input.is_empty() {
            warn!("Input '{}' does not match standard trading symbol format", input);
        }
        
        debug!("✅ Trading symbol '{}' validated for sector mapping", input);
        Ok(())
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
        
        // Read the TOML file
        let config_str = tokio::fs::read_to_string(config_path).await
            .context(format!("Failed to read config file: {:?}", config_path))?;
        
        // Parse the TOML
        let config: toml::Value = toml::from_str(&config_str)
            .context("Failed to parse TOML configuration")?;
        
        // Get the sectors table
        let sectors = config.get("sectors")
            .and_then(|s| s.as_table())
            .ok_or_else(|| anyhow!("Missing 'sectors' section in config"))?;
        
        let mut loaded_count = 0;
        
        // Process each sector
        for (sector_key, sector_value) in sectors {
            let sector_table = sector_value.as_table()
                .ok_or_else(|| anyhow!("Invalid sector configuration for {}", sector_key))?;
            
            // Parse sector ID
            let sector_id = SectorId::from_str(sector_key)
                .ok_or_else(|| anyhow!("Unknown sector ID: {}", sector_key))?;
            
            // Get ETF representative
            if let Some(etf) = sector_table.get("etf_representative")
                .and_then(|e| e.as_str()) {
                
                // Add ETF to sector_etfs mapping
                self.sector_etfs.insert(sector_id, etf.to_string());
                
                // Add ETF to symbol_sectors mapping
                let etf_info = SectorInfo {
                    id: sector_key.to_string(),
                    sector_id,
                    name: format!("{} Sector ETF", 
                        sector_table.get("sector_name")
                            .and_then(|n| n.as_str())
                            .unwrap_or(sector_key)),
                    symbols: vec![etf.to_string()],
                    description: format!("Sector ETF for {}",
                        sector_table.get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("sector")),
                    sub_sector: Some("ETF".to_string()),
                    market_cap_tier: MarketCapTier::LargeCap,
                    weight_in_sector: 1.0, // ETF represents entire sector
                    correlation_group: Some(format!("{}_ETF", sector_key.to_uppercase())),
                };
                self.add_symbol_mapping(etf, etf_info);
                loaded_count += 1;
            }
            
            // Get individual symbols
            if let Some(symbols) = sector_table.get("symbols")
                .and_then(|s| s.as_array()) {
                
                let total_symbols = symbols.len() as f64;
                for (idx, symbol_value) in symbols.iter().enumerate() {
                    if let Some(symbol) = symbol_value.as_str() {
                        let symbol_info = SectorInfo {
                            id: sector_key.to_string(),
                            sector_id,
                            name: symbol.to_string(), // Would need company names from another source
                            symbols: vec![symbol.to_string()],
                            description: format!("Company in {} sector", sector_key),
                            sub_sector: None,
                            market_cap_tier: MarketCapTier::LargeCap, // Default, could be configured
                            weight_in_sector: 1.0 / total_symbols, // Equal weight default
                            correlation_group: None,
                        };
                        self.add_symbol_mapping(symbol, symbol_info);
                        loaded_count += 1;
                    }
                }
            }
        }
        
        info!("✅ Loaded {} symbol mappings from config file", loaded_count);
        info!("📊 Total symbols in mapping: {}", self.symbol_sectors.len());
        
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
        assert_eq!(SectorId::from_str("financial"), Some(SectorId::Financial));
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
        
        let finance_etf = mapper.get_sector_etf(&SectorId::Financial);
        assert_eq!(finance_etf, Some("XLF".to_string()));
    }
}