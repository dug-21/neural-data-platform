# Sector Mapping and Aggregation Strategy

## Problem Statement

The neural system uses **sector-based models** with symbol-specific enhancements, but we receive data for **hundreds of individual symbols**. We need:

1. **Sector Classification**: Map individual symbols to sectors
2. **Sector Aggregation**: Create sector-level features from individual symbols
3. **ETF Representation**: Use sector ETFs as sector proxies
4. **Real-time Processing**: Handle high-frequency individual symbol data efficiently

## Solution Architecture

### 1. Symbol-to-Sector Mapping System

```rust
// src/data/sector_mapper.rs
pub struct SectorMapper {
    /// Static symbol-to-sector mappings
    symbol_sectors: Arc<DashMap<String, SectorInfo>>,
    /// Sector ETF representatives  
    sector_etfs: Arc<DashMap<SectorId, String>>, // Sector -> ETF symbol
    /// Dynamic sector updates (M&A, sector changes)
    sector_updates: Arc<RwLock<Vec<SectorUpdate>>>,
    /// Configuration
    config: SectorConfig,
}

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
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
    // Custom sectors
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct SectorInfo {
    pub sector_id: SectorId,
    pub sub_sector: Option<String>,        // e.g., "Software", "Semiconductors"
    pub market_cap_tier: MarketCapTier,    // Large, Mid, Small cap
    pub weight_in_sector: f64,             // 0.0-1.0, symbol's importance in sector
    pub correlation_group: Option<String>, // For fine-grained grouping
}

#[derive(Debug, Clone)]
pub enum MarketCapTier {
    LargeCap,    // > $10B
    MidCap,      // $2B - $10B  
    SmallCap,    // < $2B
}

impl SectorMapper {
    /// Load sector mappings from configuration
    pub fn load_from_config(config_path: &Path) -> Result<Self> {
        let mappings = Self::load_static_mappings(config_path)?;
        let etf_mappings = Self::load_etf_mappings(config_path)?;
        
        Ok(Self {
            symbol_sectors: Arc::new(DashMap::from_iter(mappings)),
            sector_etfs: Arc::new(DashMap::from_iter(etf_mappings)),
            sector_updates: Arc::new(RwLock::new(Vec::new())),
            config: SectorConfig::load(config_path)?,
        })
    }
    
    /// Get sector for a symbol
    pub fn get_sector(&self, symbol: &str) -> Option<SectorInfo> {
        self.symbol_sectors.get(symbol).map(|entry| entry.clone())
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
}
```

### 2. Configuration-Based Sector Definitions

```toml
# config/sector_mappings.toml
[sectors.technology]
etf_representative = "XLK"  # Technology Select Sector SPDR Fund
symbols = [
    { symbol = "AAPL", weight = 0.22, sub_sector = "Consumer Electronics", market_cap = "LargeCap" },
    { symbol = "MSFT", weight = 0.21, sub_sector = "Software", market_cap = "LargeCap" },
    { symbol = "GOOGL", weight = 0.10, sub_sector = "Internet Services", market_cap = "LargeCap" },
    { symbol = "META", weight = 0.08, sub_sector = "Social Media", market_cap = "LargeCap" },
    { symbol = "NVDA", weight = 0.07, sub_sector = "Semiconductors", market_cap = "LargeCap" },
    # ... hundreds more
]

[sectors.financial_services]
etf_representative = "XLF"  # Financial Select Sector SPDR Fund
symbols = [
    { symbol = "JPM", weight = 0.13, sub_sector = "Banking", market_cap = "LargeCap" },
    { symbol = "BAC", weight = 0.09, sub_sector = "Banking", market_cap = "LargeCap" },
    { symbol = "WFC", weight = 0.07, sub_sector = "Banking", market_cap = "LargeCap" },
    # ... more financial symbols
]

[sectors.healthcare]
etf_representative = "XLV"  # Health Care Select Sector SPDR Fund
symbols = [
    { symbol = "JNJ", weight = 0.12, sub_sector = "Pharmaceuticals", market_cap = "LargeCap" },
    { symbol = "PFE", weight = 0.09, sub_sector = "Pharmaceuticals", market_cap = "LargeCap" },
    # ... more healthcare symbols
]

# Custom correlation groups within sectors
[correlation_groups]
"faang" = ["AAPL", "AMZN", "NFLX", "GOOGL", "META"]
"big_banks" = ["JPM", "BAC", "WFC", "C"]
"chip_stocks" = ["NVDA", "AMD", "INTC", "TSM"]
```

### 3. Real-Time Sector Aggregation Engine

```rust
// src/data/sector_aggregator.rs
pub struct SectorAggregator {
    sector_mapper: Arc<SectorMapper>,
    /// Real-time sector metrics calculated from individual symbols
    sector_metrics: Arc<DashMap<SectorId, SectorMetrics>>,
    /// Individual symbol data buffer
    symbol_buffer: Arc<DashMap<String, SymbolData>>,
    /// Aggregation configuration
    config: AggregationConfig,
}

#[derive(Debug, Clone)]
pub struct SectorMetrics {
    pub sector_id: SectorId,
    pub timestamp: DateTime<Utc>,
    
    // Price-based metrics (weighted by market cap)
    pub weighted_price_change: f64,        // Sector price movement
    pub weighted_volume: f64,              // Sector volume
    pub volatility: f64,                   // Sector volatility
    
    // Breadth metrics
    pub advancing_stocks: u32,             // Number of stocks up
    pub declining_stocks: u32,             // Number of stocks down
    pub advance_decline_ratio: f64,        // A/D ratio
    
    // Momentum metrics
    pub momentum_score: f64,               // Sector momentum
    pub relative_strength: f64,            // Relative to market
    pub money_flow_index: f64,             // Sector money flow
    
    // Correlation metrics
    pub internal_correlation: f64,         // How correlated sector stocks are
    pub market_correlation: f64,           // Correlation with broad market
    
    // ETF data (if available)
    pub etf_data: Option<ETFData>,
}

#[derive(Debug, Clone)]
pub struct ETFData {
    pub price: f64,
    pub volume: f64,
    pub price_change: f64,
    pub relative_volume: f64,
}

impl SectorAggregator {
    /// Process incoming symbol data and update sector metrics
    pub async fn process_symbol_update(
        &self,
        symbol: &str,
        price_data: &PriceData,
        timestamp: DateTime<Utc>
    ) -> Result<Vec<SectorId>> {
        // Update symbol buffer
        self.symbol_buffer.insert(symbol.to_string(), SymbolData {
            price: price_data.price,
            volume: price_data.volume,
            price_change: price_data.price_change_pct,
            timestamp,
        });
        
        // Get sector for this symbol
        let sector_info = self.sector_mapper
            .get_sector(symbol)
            .ok_or_else(|| anyhow!("Unknown sector for symbol: {}", symbol))?;
        
        // Update sector metrics
        self.update_sector_metrics(&sector_info.sector_id).await?;
        
        // Also update correlation groups if applicable
        let mut updated_sectors = vec![sector_info.sector_id.clone()];
        if let Some(corr_groups) = self.get_correlation_groups(symbol) {
            for group in corr_groups {
                if let Some(group_sector) = self.get_group_sector(&group) {
                    self.update_sector_metrics(&group_sector).await?;
                    updated_sectors.push(group_sector);
                }
            }
        }
        
        Ok(updated_sectors)
    }
    
    /// Calculate sector metrics from constituent symbols
    async fn update_sector_metrics(&self, sector_id: &SectorId) -> Result<()> {
        let symbols = self.sector_mapper.get_symbols_in_sector(sector_id);
        let mut sector_data = Vec::new();
        let mut total_weight = 0.0;
        
        // Collect data for all symbols in sector
        for symbol in &symbols {
            if let Some(symbol_data) = self.symbol_buffer.get(symbol) {
                if let Some(sector_info) = self.sector_mapper.get_sector(symbol) {
                    sector_data.push((symbol_data.clone(), sector_info.weight_in_sector));
                    total_weight += sector_info.weight_in_sector;
                }
            }
        }
        
        if sector_data.is_empty() {
            return Ok(()); // No data available yet
        }
        
        // Calculate weighted metrics
        let weighted_price_change = sector_data.iter()
            .map(|(data, weight)| data.price_change * weight)
            .sum::<f64>() / total_weight;
        
        let weighted_volume = sector_data.iter()
            .map(|(data, weight)| data.volume * weight)
            .sum::<f64>();
        
        // Calculate breadth metrics
        let advancing = sector_data.iter()
            .filter(|(data, _)| data.price_change > 0.0)
            .count() as u32;
        let declining = sector_data.iter()
            .filter(|(data, _)| data.price_change < 0.0)
            .count() as u32;
        
        // Get ETF data if available
        let etf_data = if let Some(etf_symbol) = self.sector_mapper.get_sector_etf(sector_id) {
            self.symbol_buffer.get(&etf_symbol)
                .map(|data| ETFData {
                    price: data.price,
                    volume: data.volume,
                    price_change: data.price_change,
                    relative_volume: data.volume / data.avg_volume.unwrap_or(data.volume),
                })
        } else {
            None
        };
        
        // Update sector metrics
        let metrics = SectorMetrics {
            sector_id: sector_id.clone(),
            timestamp: Utc::now(),
            weighted_price_change,
            weighted_volume,
            volatility: self.calculate_sector_volatility(&sector_data),
            advancing_stocks: advancing,
            declining_stocks: declining,
            advance_decline_ratio: if declining > 0 { advancing as f64 / declining as f64 } else { f64::INFINITY },
            momentum_score: self.calculate_momentum_score(&sector_data),
            relative_strength: self.calculate_relative_strength(sector_id, &sector_data).await?,
            money_flow_index: self.calculate_money_flow(&sector_data),
            internal_correlation: self.calculate_internal_correlation(&sector_data).await?,
            market_correlation: self.calculate_market_correlation(sector_id).await?,
            etf_data,
        };
        
        self.sector_metrics.insert(sector_id.clone(), metrics);
        Ok(())
    }
}
```

### 4. Efficient Data Pipeline Architecture

```rust
// src/data/sector_data_pipeline.rs
pub struct SectorDataPipeline {
    /// Incoming symbol data stream
    symbol_receiver: mpsc::Receiver<SymbolUpdate>,
    /// Sector aggregator
    aggregator: Arc<SectorAggregator>,
    /// Sector data publishers for models
    sector_publishers: HashMap<SectorId, mpsc::Sender<SectorMetrics>>,
    /// ETF data integration
    etf_receiver: mpsc::Receiver<ETFUpdate>,
}

impl SectorDataPipeline {
    /// Main processing loop
    pub async fn run(&mut self) -> Result<()> {
        loop {
            tokio::select! {
                // Process individual symbol updates
                Some(symbol_update) = self.symbol_receiver.recv() => {
                    self.process_symbol_update(symbol_update).await?;
                }
                
                // Process ETF updates
                Some(etf_update) = self.etf_receiver.recv() => {
                    self.process_etf_update(etf_update).await?;
                }
                
                // Periodic sector metric calculations
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    self.calculate_periodic_metrics().await?;
                }
            }
        }
    }
    
    async fn process_symbol_update(&self, update: SymbolUpdate) -> Result<()> {
        // Update sector aggregations
        let updated_sectors = self.aggregator
            .process_symbol_update(&update.symbol, &update.price_data, update.timestamp)
            .await?;
        
        // Publish updated sector metrics to models
        for sector_id in updated_sectors {
            if let Some(metrics) = self.aggregator.sector_metrics.get(&sector_id) {
                if let Some(publisher) = self.sector_publishers.get(&sector_id) {
                    let _ = publisher.send(metrics.clone()).await;
                }
            }
        }
        
        Ok(())
    }
}
```

### 5. Model Integration with Sector Data

```rust
// src/neural/vendor_predictor.rs - Enhanced with sector data
impl VendorPredictor {
    /// Create TimeSeriesData with sector context
    pub fn create_sector_enhanced_data(
        &self,
        symbol: &str,
        symbol_data: &SymbolData,
        sector_data: &SectorMetrics
    ) -> Result<TimeSeriesData<f32>> {
        let mut ts_data = TimeSeriesData::new(vec![symbol_data.price]);
        
        // Add sector-based exogenous features
        let sector_features = vec![
            sector_data.weighted_price_change as f32,
            sector_data.volatility as f32,
            sector_data.advance_decline_ratio as f32,
            sector_data.momentum_score as f32,
            sector_data.relative_strength as f32,
            sector_data.internal_correlation as f32,
        ];
        
        // Add ETF data if available
        if let Some(etf_data) = &sector_data.etf_data {
            sector_features.extend(vec![
                etf_data.price_change as f32,
                etf_data.relative_volume as f32,
            ]);
        }
        
        ts_data = ts_data.with_exogenous(vec![sector_features])?;
        
        // Add static features (sector classification)
        if let Some(sector_info) = self.sector_mapper.get_sector(symbol) {
            let static_features = vec![
                sector_info.sector_id.as_numeric() as f32,
                sector_info.weight_in_sector as f32,
                sector_info.market_cap_tier.as_numeric() as f32,
            ];
            ts_data = ts_data.with_static_features(static_features);
        }
        
        Ok(ts_data)
    }
}
```

## Benefits of This Architecture

### 1. **Efficient Processing**
- Individual symbols update sector metrics in real-time
- ETF data provides sector-level validation
- Hierarchical aggregation (symbol → sector → market)

### 2. **Flexible Mapping**
- Configuration-driven sector assignments
- Support for custom correlation groups
- Dynamic sector updates (M&A, sector changes)

### 3. **Rich Sector Features**
- Price-weighted sector movements
- Breadth indicators (A/D ratios)
- Momentum and relative strength
- Internal correlation analysis

### 4. **Scalable Design**
- Handles hundreds of symbols efficiently
- Parallel processing of sector updates
- Memory-efficient data structures

This design allows your sector-based models to receive rich, real-time sector context while efficiently processing hundreds of individual symbol updates.