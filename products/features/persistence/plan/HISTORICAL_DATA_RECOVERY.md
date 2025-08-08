# Historical Data Recovery Strategy

## Executive Summary
This document outlines the strategy for loading historical market data from TimescaleDB on startup, enabling neural-trader to begin making informed trading decisions within 30 seconds of restart, rather than waiting 2-3 hours for fresh data accumulation.

## Current Problem

### Data Availability Gap
- TimescaleDB contains complete historical data (properly persisted)
- Neural-trader only subscribes to real-time Redis channels on startup
- Event bus starts empty, requiring 10+ new data points
- Results in 2-3 hour "blind period" after each restart

### Impact
- Cannot perform maintenance without trading downtime
- No quick recovery from crashes
- Unable to scale horizontally (new instances start cold)

## Proposed Solution

### 1. Multi-Phase Recovery Strategy

#### Phase 1: Critical Data (0-5 seconds)
```rust
// Load last 4 hours of data for active symbols
async fn load_critical_data(storage: &TimescaleDBStorage) -> Result<Vec<TimeSeriesData>> {
    let query = r#"
        SELECT symbol, timestamp, price, volume, 
               bid, ask, high, low, indicators
        FROM market_data
        WHERE timestamp > NOW() - INTERVAL '4 hours'
          AND symbol IN (SELECT DISTINCT symbol FROM trading_positions)
        ORDER BY timestamp DESC
        LIMIT 1000 PER symbol
    "#;
    
    storage.query_time_series(query).await
}
```

#### Phase 2: Context Data (5-15 seconds)
```rust
// Load 24 hours for broader market context
async fn load_context_data(storage: &TimescaleDBStorage) -> Result<Vec<TimeSeriesData>> {
    let query = r#"
        WITH ranked_data AS (
            SELECT symbol, timestamp, price, volume,
                   ROW_NUMBER() OVER (
                       PARTITION BY symbol, time_bucket('5 minutes', timestamp)
                       ORDER BY timestamp DESC
                   ) as rn
            FROM market_data
            WHERE timestamp > NOW() - INTERVAL '24 hours'
        )
        SELECT * FROM ranked_data WHERE rn = 1
    "#;
    
    storage.query_aggregated(query).await
}
```

#### Phase 3: Deep History (15-30 seconds)
```rust
// Load 7 days of aggregated data for pattern recognition
async fn load_deep_history(storage: &TimescaleDBStorage) -> Result<Vec<TimeSeriesData>> {
    let query = r#"
        SELECT symbol,
               time_bucket('1 hour', timestamp) as hour,
               AVG(price) as avg_price,
               SUM(volume) as total_volume,
               MAX(high) as period_high,
               MIN(low) as period_low
        FROM market_data
        WHERE timestamp > NOW() - INTERVAL '7 days'
        GROUP BY symbol, hour
        ORDER BY hour DESC
    "#;
    
    storage.query_hourly_aggregates(query).await
}
```

### 2. Event Bus Population

#### Event Synthesis from Historical Data
```rust
impl EventBus {
    pub async fn populate_from_historical(
        &self,
        historical_data: Vec<TimeSeriesData>
    ) -> Result<()> {
        // Group by symbol for efficient processing
        let mut by_symbol: HashMap<String, Vec<TimeSeriesData>> = HashMap::new();
        for data in historical_data {
            by_symbol.entry(data.symbol.clone())
                .or_default()
                .push(data);
        }
        
        // Convert to market events and populate
        for (symbol, data_points) in by_symbol {
            for point in data_points {
                let event = self.synthesize_market_event(point).await?;
                self.publish_market_event(event).await?;
            }
        }
        
        info!("Populated event bus with {} historical events", 
              self.get_event_count().await);
        Ok(())
    }
    
    async fn synthesize_market_event(&self, data: TimeSeriesData) -> Result<MarketEvent> {
        Ok(MarketEvent {
            symbol: data.symbol,
            timestamp: data.timestamp,
            price: data.values.get(0).copied().unwrap_or(0.0) as f64,
            volume: data.volume.unwrap_or(0),
            bid: data.bid,
            ask: data.ask,
            source: "historical".to_string(),
        })
    }
}
```

### 3. Data Window Selection

#### Optimal Window Sizes
```rust
struct RecoveryWindows {
    critical: Duration,    // 4 hours - minimum for decisions
    context: Duration,     // 24 hours - daily patterns
    deep: Duration,        // 7 days - weekly patterns
    archive: Duration,     // 30 days - monthly trends
}

impl Default for RecoveryWindows {
    fn default() -> Self {
        Self {
            critical: Duration::hours(4),
            context: Duration::hours(24),
            deep: Duration::days(7),
            archive: Duration::days(30),
        }
    }
}
```

#### Dynamic Window Adjustment
```rust
// Adjust based on market conditions
fn adjust_windows_for_market(base: RecoveryWindows) -> RecoveryWindows {
    let now = Utc::now();
    let market_hours = MarketHours::new();
    
    if market_hours.is_weekend(now) {
        // Extend windows on weekends
        RecoveryWindows {
            critical: base.critical + Duration::hours(48),
            context: base.context + Duration::hours(48),
            ..base
        }
    } else if market_hours.is_holiday(now) {
        // Adjust for holidays
        RecoveryWindows {
            critical: base.critical + Duration::hours(24),
            ..base
        }
    } else {
        base
    }
}
```

### 4. Memory Management

#### Streaming Large Datasets
```rust
struct HistoricalDataLoader {
    storage: Arc<TimescaleDBStorage>,
    batch_size: usize,
    max_memory_mb: usize,
}

impl HistoricalDataLoader {
    async fn stream_historical_data(&self) -> Result<DataStream> {
        let mut stream = self.storage.create_stream(
            self.batch_size,
            self.max_memory_mb
        ).await?;
        
        Ok(DataStream {
            inner: stream,
            processed: 0,
            total: stream.estimated_count().await?,
        })
    }
}

// Process in chunks to avoid memory overflow
async fn process_historical_stream(stream: DataStream) -> Result<()> {
    let mut buffer = Vec::with_capacity(1000);
    
    while let Some(batch) = stream.next_batch().await? {
        buffer.extend(batch);
        
        if buffer.len() >= 1000 {
            process_buffer(&buffer).await?;
            buffer.clear();
        }
    }
    
    // Process remaining
    if !buffer.is_empty() {
        process_buffer(&buffer).await?;
    }
    
    Ok(())
}
```

#### Memory-Aware Caching
```rust
struct AdaptiveCache {
    max_memory_mb: usize,
    current_usage_mb: AtomicUsize,
    lru_cache: Arc<RwLock<LruCache<String, TimeSeriesData>>>,
}

impl AdaptiveCache {
    async fn add_with_eviction(&self, key: String, data: TimeSeriesData) -> Result<()> {
        let data_size = std::mem::size_of_val(&data) / 1_048_576; // Convert to MB
        
        // Evict if necessary
        while self.current_usage_mb.load(Ordering::Relaxed) + data_size > self.max_memory_mb {
            let mut cache = self.lru_cache.write().await;
            if let Some((evicted_key, evicted_data)) = cache.pop_lru() {
                let evicted_size = std::mem::size_of_val(&evicted_data) / 1_048_576;
                self.current_usage_mb.fetch_sub(evicted_size, Ordering::Relaxed);
                debug!("Evicted {} to make room", evicted_key);
            } else {
                break;
            }
        }
        
        // Add new data
        let mut cache = self.lru_cache.write().await;
        cache.put(key, data);
        self.current_usage_mb.fetch_add(data_size, Ordering::Relaxed);
        
        Ok(())
    }
}
```

### 5. Query Optimization

#### Use EXISTING TimescaleDB Continuous Aggregates
```rust
// TimescaleDB already has these aggregates - just query them!
// - market_data_1h: Hourly rollups (already exists)
// - mv_5min_ohlcv: 5-minute rollups (already exists)

async fn query_existing_aggregates(storage: &TimescaleDBStorage) -> Result<Vec<TimeSeriesData>> {
    // Use the EXISTING continuous aggregate - NO SCHEMA CHANGES
    let query = r#"
        SELECT bucket, symbol, open, high, low, close, volume
        FROM market_data_1h  -- This already exists!
        WHERE bucket > NOW() - INTERVAL '24 hours'
        ORDER BY bucket DESC
    "#;
    
    storage.query(query).await
}
```

#### Parallel Query Execution
```rust
async fn parallel_historical_load(
    storage: &TimescaleDBStorage,
    symbols: Vec<String>
) -> Result<Vec<TimeSeriesData>> {
    let futures: Vec<_> = symbols
        .into_iter()
        .map(|symbol| {
            let storage = storage.clone();
            async move {
                storage.get_symbol_history(&symbol, Duration::hours(24)).await
            }
        })
        .collect();
    
    let results = futures::future::join_all(futures).await;
    
    let mut all_data = Vec::new();
    for result in results {
        all_data.extend(result?);
    }
    
    Ok(all_data)
}
```

### 6. Implementation Integration

#### Startup Sequence
```rust
// main.rs modifications
async fn initialize_with_historical(
    storage: Arc<TimescaleDBStorage>,
    event_bus: Arc<EventBus>,
) -> Result<()> {
    let start = Instant::now();
    
    // Phase 1: Critical data
    info!("Loading critical historical data...");
    let critical = load_critical_data(&storage).await?;
    event_bus.populate_from_historical(critical).await?;
    info!("Critical data loaded in {:?}", start.elapsed());
    
    // Phase 2: Context data (async)
    let storage_clone = storage.clone();
    let event_bus_clone = event_bus.clone();
    tokio::spawn(async move {
        if let Ok(context) = load_context_data(&storage_clone).await {
            let _ = event_bus_clone.populate_from_historical(context).await;
            info!("Context data loaded");
        }
    });
    
    // Phase 3: Deep history (background)
    let storage_clone = storage.clone();
    let event_bus_clone = event_bus.clone();
    tokio::spawn(async move {
        if let Ok(deep) = load_deep_history(&storage_clone).await {
            let _ = event_bus_clone.populate_from_historical(deep).await;
            info!("Deep history loaded");
        }
    });
    
    info!("Historical data recovery initiated in {:?}", start.elapsed());
    Ok(())
}
```

### 7. Performance Metrics

#### Recovery Benchmarks
```rust
#[derive(Debug, Serialize)]
struct RecoveryMetrics {
    critical_load_ms: u64,
    context_load_ms: u64,
    deep_load_ms: u64,
    total_events_loaded: usize,
    memory_used_mb: usize,
    time_to_first_decision_ms: u64,
}

impl RecoveryMetrics {
    async fn measure(storage: &TimescaleDBStorage) -> Self {
        let start = Instant::now();
        
        // Measure each phase
        let critical_start = Instant::now();
        let critical = load_critical_data(storage).await.unwrap();
        let critical_load_ms = critical_start.elapsed().as_millis() as u64;
        
        // ... measure other phases
        
        Self {
            critical_load_ms,
            context_load_ms: 0, // Measured separately
            deep_load_ms: 0,     // Measured separately
            total_events_loaded: critical.len(),
            memory_used_mb: get_process_memory_mb(),
            time_to_first_decision_ms: start.elapsed().as_millis() as u64,
        }
    }
}
```

### 8. Configuration

#### Recovery Settings
```yaml
# config/recovery.yaml
historical_recovery:
  enabled: true
  
  windows:
    critical_hours: 4
    context_hours: 24
    deep_days: 7
    archive_days: 30
    
  performance:
    max_memory_mb: 2048
    batch_size: 1000
    parallel_queries: 4
    
  query_optimization:
    use_continuous_aggregates: true
    use_compression: true
    
  startup_behavior:
    block_until_critical: true  # Wait for critical data
    async_context: true         # Load context async
    background_deep: true       # Load deep history in background
```

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_historical_data_recovery() {
        let storage = create_test_storage().await;
        insert_test_data(&storage, 1000).await;
        
        let recovered = load_critical_data(&storage).await.unwrap();
        assert!(recovered.len() >= 100);
        assert!(recovered[0].timestamp > Utc::now() - Duration::hours(4));
    }
    
    #[tokio::test]
    async fn test_memory_limits() {
        let loader = HistoricalDataLoader {
            storage: create_test_storage().await,
            batch_size: 100,
            max_memory_mb: 10,
        };
        
        let stream = loader.stream_historical_data().await.unwrap();
        let memory_before = get_process_memory_mb();
        
        process_historical_stream(stream).await.unwrap();
        
        let memory_after = get_process_memory_mb();
        assert!(memory_after - memory_before < 50); // Max 50MB increase
    }
}
```

## Success Metrics

1. **Startup Time**: < 30 seconds to first trading decision
2. **Data Completeness**: 100% of last 4 hours loaded
3. **Memory Usage**: < 500MB for historical data
4. **Query Performance**: < 5 seconds for critical data
5. **Decision Quality**: Same as continuous operation

## Risk Mitigation

1. **Database Overload**: Use read replicas for historical queries
2. **Memory Overflow**: Streaming and pagination
3. **Data Gaps**: Fallback to reduced windows
4. **Query Timeouts**: Prepared statements and connection pooling