# Sector Aggregation Metrics Specification
**Phase 2, Week 5 - Market Metrics Analyst Research**

## Executive Summary

This specification defines the comprehensive sector aggregation metrics system for the SectorAggregator component, enabling real-time calculation of sector-level metrics from individual symbol data with <50ms update latency and integration with existing Redis streaming infrastructure.

## 1. Core Sector Metrics Requirements

### 1.1 Advance/Decline Ratio
**Definition**: Percentage of symbols moving up vs down within sector
**Formula**: `(symbols_up / total_symbols) * 100`
**Update Frequency**: Per tick (real-time)
**Storage**: Redis key `sector:{sector_id}:ad_ratio`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvanceDeclineMetrics {
    pub sector_id: SectorId,
    pub advancing_count: u32,
    pub declining_count: u32,
    pub unchanged_count: u32,
    pub advance_decline_ratio: f64,  // 0.0 to 100.0
    pub net_advances: i32,           // advancing - declining
    pub breadth_momentum: f64,       // 5-period momentum of A/D ratio
    pub timestamp: DateTime<Utc>,
}
```

### 1.2 Sector Momentum Score
**Definition**: Rate of change in sector price movement
**Formula**: `(current_sector_price - price_n_periods_ago) / price_n_periods_ago * 100`
**Timeframes**: 1min, 5min, 15min, 1hour, 1day
**Storage**: Redis key `sector:{sector_id}:momentum:{timeframe}`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorMomentumMetrics {
    pub sector_id: SectorId,
    pub momentum_1m: f64,     // 1-minute momentum
    pub momentum_5m: f64,     // 5-minute momentum
    pub momentum_15m: f64,    // 15-minute momentum
    pub momentum_1h: f64,     // 1-hour momentum
    pub momentum_1d: f64,     // 1-day momentum
    pub acceleration: f64,     // Second derivative of momentum
    pub velocity_index: f64,   // Momentum-weighted by volume
    pub momentum_rank: u8,     // Rank vs other sectors (1-10)
    pub timestamp: DateTime<Utc>,
}
```

### 1.3 Internal Correlation
**Definition**: How similarly symbols within sector move
**Formula**: Average pairwise correlation of symbol returns
**Window**: Rolling 20-period correlation
**Storage**: Redis key `sector:{sector_id}:correlation`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalCorrelationMetrics {
    pub sector_id: SectorId,
    pub average_correlation: f64,      // Mean pairwise correlation
    pub correlation_stability: f64,    // Std dev of correlations
    pub high_correlation_pairs: u32,   // Pairs with corr > 0.7
    pub correlation_regime: CorrelationRegime, // Low/Medium/High
    pub correlation_trend: f64,        // Change in correlation over time
    pub dominant_cluster_size: u32,    // Size of largest correlation cluster
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CorrelationRegime {
    Low,     // < 0.3
    Medium,  // 0.3 - 0.7
    High,    // > 0.7
    Extreme, // > 0.9
}
```

### 1.4 Relative Strength vs Market
**Definition**: Sector performance relative to SPY
**Formula**: `sector_return / spy_return`
**Rolling Windows**: 1d, 5d, 20d
**Storage**: Redis key `sector:{sector_id}:relative_strength`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelativeStrengthMetrics {
    pub sector_id: SectorId,
    pub rs_1d: f64,           // 1-day relative strength
    pub rs_5d: f64,           // 5-day relative strength
    pub rs_20d: f64,          // 20-day relative strength
    pub rs_rank: u8,          // Rank among sectors (1-10)
    pub outperformance_streak: i32, // Days of outperformance
    pub beta_to_market: f64,  // Sector beta vs SPY
    pub alpha_1m: f64,        // Monthly alpha vs market
    pub timestamp: DateTime<Utc>,
}
```

### 1.5 Volume Surge Detection
**Definition**: Unusual volume activity within sector
**Formula**: `current_volume / average_volume_20d`
**Threshold**: >2.0 for surge, >5.0 for extreme surge
**Storage**: Redis key `sector:{sector_id}:volume_surge`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeSurgeMetrics {
    pub sector_id: SectorId,
    pub volume_ratio: f64,         // Current vs 20-day average
    pub surge_level: SurgeLevel,   // None/Moderate/High/Extreme
    pub surge_duration: u32,       // Minutes of sustained surge
    pub participating_symbols: u32, // Symbols with volume surge
    pub volume_concentration: f64,  // % of volume in top 3 symbols
    pub money_flow_direction: f64,  // Net buying (+) vs selling (-)
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SurgeLevel {
    None,     // < 1.5x
    Moderate, // 1.5x - 2.5x
    High,     // 2.5x - 5.0x
    Extreme,  // > 5.0x
}
```

### 1.6 Breadth Indicators
**Definition**: Participation rate and distribution of moves
**Components**: McClellan Oscillator, Arms Index, High-Low Index
**Storage**: Redis key `sector:{sector_id}:breadth`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreadthIndicators {
    pub sector_id: SectorId,
    pub participation_rate: f64,     // % of symbols participating in move
    pub mcclellan_oscillator: f64,   // 19-39 EMA difference of A/D
    pub mcclellan_summation: f64,    // Cumulative McClellan
    pub arms_index: f64,             // (Declining/Advancing) / (Down Vol/Up Vol)
    pub high_low_index: f64,         // New Highs / (New Highs + New Lows)
    pub breadth_thrust: bool,        // Rapid expansion in breadth
    pub distribution_days: u32,      // Days of negative breadth
    pub timestamp: DateTime<Utc>,
}
```

## 2. Market Cap Weighting System

### 2.1 Dynamic Weight Calculation
**Approach**: Market cap weighted with monthly rebalancing
**Data Source**: Existing `SectorInfo.weight_in_sector` field
**Fallback**: Equal weighting if market cap unavailable

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketCapWeighting {
    pub sector_id: SectorId,
    pub total_market_cap: f64,       // Sector total market cap
    pub weights: HashMap<String, f64>, // Symbol -> weight mapping
    pub large_cap_weight: f64,       // % weight in large caps
    pub mid_cap_weight: f64,         // % weight in mid caps
    pub small_cap_weight: f64,       // % weight in small caps
    pub concentration_ratio: f64,     // Top 5 symbols weight %
    pub last_rebalance: DateTime<Utc>,
    pub next_rebalance: DateTime<Utc>,
}

impl MarketCapWeighting {
    /// Calculate sector-weighted price
    pub fn calculate_weighted_price(&self, symbol_prices: &HashMap<String, f64>) -> f64 {
        self.weights.iter()
            .map(|(symbol, weight)| {
                symbol_prices.get(symbol).unwrap_or(&0.0) * weight
            })
            .sum()
    }
    
    /// Update weights based on market cap changes
    pub async fn rebalance_weights(&mut self, market_caps: &HashMap<String, f64>) -> Result<()> {
        let total_cap: f64 = market_caps.values().sum();
        
        for (symbol, market_cap) in market_caps {
            let weight = market_cap / total_cap;
            self.weights.insert(symbol.clone(), weight);
        }
        
        self.last_rebalance = Utc::now();
        self.next_rebalance = Utc::now() + chrono::Duration::days(30);
        Ok(())
    }
}
```

### 2.2 ETF Component Weightings
**Source**: XLK, XLF, XLV, etc. published weightings
**Update Frequency**: Daily after market close
**Integration**: Map ETF weights to sector symbol weights

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ETFComponentWeights {
    pub etf_symbol: String,          // XLK, XLF, etc.
    pub sector_id: SectorId,
    pub components: HashMap<String, f64>, // Symbol -> ETF weight
    pub total_assets: f64,           // ETF total assets
    pub expense_ratio: f64,          // ETF expense ratio
    pub last_updated: DateTime<Utc>,
}
```

## 3. Real-time Calculation Requirements

### 3.1 Update Frequency Specifications
**Per Tick Updates** (<10ms target):
- Advance/Decline Ratio
- Volume Surge Detection
- Price-based momentum (1min)

**Per Second Updates** (<50ms target):
- Internal Correlation (simplified)
- Relative Strength vs Market
- Breadth Indicators

**Per Minute Updates** (<100ms target):
- Full correlation matrix recalculation
- Complex momentum calculations
- Market cap weight verification

### 3.2 Performance Targets
```rust
pub struct PerformanceTargets {
    pub max_update_latency_ms: u64,      // 50ms
    pub max_calculation_time_ms: u64,     // 20ms
    pub cache_hit_ratio_target: f64,      // 95%
    pub memory_usage_limit_mb: u64,       // 100MB per sector
    pub concurrent_sectors: u8,           // 10 sectors simultaneously
}
```

### 3.3 Aggregation Windows
```rust
#[derive(Debug, Clone)]
pub struct AggregationWindows {
    pub tick_window: Duration,       // Immediate (0ms)
    pub second_window: Duration,     // 1 second
    pub minute_windows: Vec<Duration>, // 1m, 5m, 15m
    pub hourly_windows: Vec<Duration>, // 1h, 4h
    pub daily_windows: Vec<Duration>,  // 1d, 5d, 20d
}
```

## 4. Integration with Existing Data Flow

### 4.1 Redis Channel Architecture
**Existing Channels** (preserved):
- `market_data:{symbol}` - Individual symbol updates
- `orderbook:{symbol}` - Order book snapshots
- `price:latest:{symbol}` - Latest price cache

**New Sector Channels**:
- `sector:{sector_id}:metrics` - Aggregated sector metrics
- `sector:{sector_id}:alerts` - Sector-level alerts
- `sector:all:summary` - Cross-sector summary

### 4.2 Data Flow Integration
```rust
pub struct DataFlowIntegration {
    // Input: Subscribe to existing symbol channels
    symbol_subscriber: Arc<RedisSubscriber>,
    
    // Processing: Real-time aggregation engine
    aggregation_engine: Arc<SectorAggregationEngine>,
    
    // Output: Publish to sector channels
    sector_publisher: Arc<RedisSectorPublisher>,
    
    // Cache: Store calculated metrics
    metrics_cache: Arc<SectorMetricsCache>,
}

impl DataFlowIntegration {
    /// Main processing loop for symbol data
    pub async fn process_symbol_update(
        &self,
        symbol: String,
        market_data: MarketData,
    ) -> Result<()> {
        // 1. Determine symbol's sector
        let sector_id = self.get_symbol_sector(&symbol)?;
        
        // 2. Update sector aggregations
        let sector_metrics = self.aggregation_engine
            .update_sector_metrics(sector_id, &symbol, &market_data)
            .await?;
        
        // 3. Check for alerts/thresholds
        let alerts = self.check_sector_alerts(&sector_metrics).await?;
        
        // 4. Publish updates
        self.sector_publisher
            .publish_sector_update(&sector_metrics)
            .await?;
        
        // 5. Cache results
        self.metrics_cache
            .store_metrics(&sector_metrics)
            .await?;
        
        Ok(())
    }
}
```

### 4.3 Existing Data Structure Compatibility
**TimeSeriesData Integration**:
```rust
impl From<&TimeSeriesData> for SectorDataPoint {
    fn from(ts_data: &TimeSeriesData) -> Self {
        Self {
            symbol: ts_data.symbol.clone(),
            timestamp: ts_data.timestamp,
            price: ts_data.close,
            volume: ts_data.volume,
            high: ts_data.high,
            low: ts_data.low,
            indicators: ts_data.indicators.clone(),
        }
    }
}
```

## 5. Implementation Architecture

### 5.1 Core SectorAggregator Structure
```rust
pub struct SectorAggregator {
    // Configuration
    config: SectorAggregatorConfig,
    
    // Sector mapping (existing integration)
    sector_mapper: Arc<SectorMapper>,
    
    // Metrics calculators
    advance_decline_calculator: Arc<AdvanceDeclineCalculator>,
    momentum_calculator: Arc<MomentumCalculator>,
    correlation_calculator: Arc<CorrelationCalculator>,
    relative_strength_calculator: Arc<RelativeStrengthCalculator>,
    volume_surge_detector: Arc<VolumeSurgeDetector>,
    breadth_calculator: Arc<BreadthCalculator>,
    
    // Data management
    symbol_data_cache: Arc<DashMap<String, CircularBuffer<MarketData>>>,
    sector_metrics_cache: Arc<DashMap<SectorId, SectorMetrics>>,
    
    // Redis integration
    redis_cache: Arc<RedisCache>,
    
    // Performance monitoring
    performance_tracker: Arc<SectorPerformanceTracker>,
}

#[derive(Debug, Clone)]
pub struct SectorAggregatorConfig {
    pub update_frequency_ms: u64,        // 50ms default
    pub correlation_window_periods: usize, // 20 periods
    pub volume_surge_threshold: f64,      // 2.0x average
    pub momentum_periods: Vec<usize>,     // [1, 5, 15, 60, 1440]
    pub cache_ttl_seconds: u64,          // 60 seconds
    pub max_symbols_per_sector: usize,   // 50 symbols
    pub enable_alerting: bool,           // true
}
```

### 5.2 Performance Optimization
```rust
impl SectorAggregator {
    /// High-performance update method with sub-50ms target
    pub async fn update_sector_metrics(
        &self,
        symbol: &str,
        market_data: &MarketData,
    ) -> Result<SectorMetrics> {
        let start_time = Instant::now();
        
        // 1. Fast sector lookup (O(1) with DashMap)
        let sector_id = self.sector_mapper.get_sector(symbol)?.sector_id;
        
        // 2. Update symbol data cache (circular buffer for efficiency)
        self.update_symbol_cache(symbol, market_data).await?;
        
        // 3. Parallel calculation of metrics
        let (advance_decline, momentum, correlation, rel_strength, volume_surge, breadth) = 
            tokio::try_join!(
                self.advance_decline_calculator.calculate(sector_id, market_data),
                self.momentum_calculator.calculate(sector_id, market_data),
                self.correlation_calculator.calculate_incremental(sector_id, symbol, market_data),
                self.relative_strength_calculator.calculate(sector_id, market_data),
                self.volume_surge_detector.detect(sector_id, market_data),
                self.breadth_calculator.calculate(sector_id, market_data)
            )?;
        
        // 4. Combine metrics
        let sector_metrics = SectorMetrics {
            sector_id,
            advance_decline,
            momentum,
            correlation,
            relative_strength,
            volume_surge,
            breadth,
            timestamp: Utc::now(),
            calculation_time_ms: start_time.elapsed().as_millis() as f64,
        };
        
        // 5. Cache results
        self.sector_metrics_cache.insert(sector_id, sector_metrics.clone());
        
        // 6. Track performance
        self.performance_tracker.record_update(
            sector_id,
            start_time.elapsed().as_millis() as u64
        ).await;
        
        Ok(sector_metrics)
    }
}
```

## 6. Alert and Threshold System

### 6.1 Sector Alert Conditions
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectorAlertConfig {
    pub momentum_threshold: f64,         // ±5% for alerts
    pub volume_surge_threshold: f64,     // 3x average volume
    pub correlation_spike_threshold: f64, // >0.85 correlation
    pub relative_strength_threshold: f64, // ±10% vs market
    pub breadth_thrust_threshold: f64,   // 90% participation
}

pub enum SectorAlert {
    MomentumBreakout { sector_id: SectorId, momentum: f64 },
    VolumeSurge { sector_id: SectorId, volume_ratio: f64 },
    CorrelationSpike { sector_id: SectorId, correlation: f64 },
    RelativeStrengthDivergence { sector_id: SectorId, rs_value: f64 },
    BreadthThrust { sector_id: SectorId, participation: f64 },
}
```

## 7. Testing and Validation

### 7.1 Performance Testing
- Load testing with 1000+ symbols across 10 sectors
- Latency benchmarks with target <50ms per update
- Memory usage monitoring with 100MB per sector limit
- Throughput testing with 10,000 updates/second

### 7.2 Accuracy Validation
- Compare calculated metrics with third-party data providers
- Backtesting against historical sector performance
- Cross-validation with existing sector ETFs (XLK, XLF, etc.)

## 8. Memory and Storage Usage

### 8.1 Memory Management
```rust
pub struct SectorMemoryManager {
    // Circular buffers for efficient memory usage
    symbol_buffers: Arc<DashMap<String, CircularBuffer<MarketData>>>,
    
    // Pre-allocated correlation matrices
    correlation_matrices: Arc<DashMap<SectorId, CorrelationMatrix>>,
    
    // Memory pools for frequent allocations
    metrics_pool: Arc<Pool<SectorMetrics>>,
}
```

### 8.2 Storage Strategy
- **Hot Data**: Redis cache (last 1 hour of metrics)
- **Warm Data**: TimescaleDB (last 30 days of hourly aggregates)
- **Cold Data**: S3/Archive (historical sector metrics)

## Conclusion

This specification provides a comprehensive framework for real-time sector aggregation metrics that integrates seamlessly with the existing neural-trader infrastructure while meeting strict performance requirements. The system is designed to scale to 100+ symbols across 10 sectors with sub-50ms update latency and efficient memory usage.

**Next Steps**:
1. Implement core SectorAggregator structure
2. Integrate with existing SectorMapper
3. Set up Redis channel architecture
4. Implement performance monitoring
5. Create alert system
6. Comprehensive testing and validation