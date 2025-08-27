# Fast vs Slow Data Patterns - EventBus Integration

## Executive Summary
This document defines the clear separation between fast (real-time) and slow (historical) data in the Neural-Trader system, showing how ML-Ops combines both to create enriched features.

## 1. Data Classification

### 1.1 Fast Data (< 1 second latency)
**Storage**: Redis  
**Access Pattern**: In-memory, pub/sub  
**Retention**: 1-24 hours  

```yaml
fast_data:
  market_prices:
    latency: < 10ms
    update_frequency: 100-1000/sec per symbol
    examples:
      - bid/ask quotes
      - last trade price
      - current volume
      
  order_book:
    latency: < 5ms
    update_frequency: 10-100/sec per symbol
    examples:
      - level 2 depth
      - order imbalances
      - micro-structure signals
      
  short_term_indicators:
    latency: < 50ms
    update_frequency: 1-10/sec
    examples:
      - 1-min moving average
      - current volatility
      - momentum indicators
```

### 1.2 Slow Data (> 1 second latency)
**Storage**: TimescaleDB  
**Access Pattern**: SQL queries, indexed lookups  
**Retention**: Days to years  

```yaml
slow_data:
  historical_ohlcv:
    latency: 100-500ms
    update_frequency: 1/min to 1/day
    retention: 5+ years
    examples:
      - daily bars
      - hourly candles
      - weekly/monthly aggregates
      
  economic_indicators:
    latency: 500-1000ms
    update_frequency: 1/day to 1/month
    retention: 10+ years
    examples:
      - GDP growth
      - interest rates
      - unemployment data
      - inflation metrics
      
  reference_data:
    latency: 100-200ms
    update_frequency: 1/day to 1/quarter
    retention: permanent
    examples:
      - company fundamentals
      - sector classifications
      - market calendars
      - dividend schedules
```

## 2. Data Flow Architecture

```
┌─────────────────────────────────────────────────────┐
│              External Data Sources                   │
│  (Market Data Feeds, Economic APIs, News Feeds)     │
└─────────────────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────┐
│            Data Ingestion Service (Python)          │
│                                                      │
│  ┌─────────────────┐      ┌────────────────────┐   │
│  │   Fast Path     │      │    Slow Path       │   │
│  │  (Real-time)    │      │   (Historical)     │   │
│  └────────┬────────┘      └─────────┬──────────┘   │
└───────────┼──────────────────────────┼──────────────┘
            │                          │
            ▼                          ▼
    ┌──────────────┐          ┌──────────────┐
    │    Redis     │          │  TimescaleDB  │
    │  (Fast Data) │          │  (Slow Data)  │
    └──────┬───────┘          └───────┬───────┘
           │                          │
           └──────────┬───────────────┘
                      ▼
┌─────────────────────────────────────────────────────┐
│              Neural ML-Ops Service                  │
│                                                      │
│  ┌──────────────────────────────────────────────┐  │
│  │         Feature Engineering Pipeline         │  │
│  │                                              │  │
│  │  1. Query fast data from Redis               │  │
│  │  2. Query historical context from TimescaleDB│  │
│  │  3. Combine and compute ML features         │  │
│  │  4. Publish enriched features to EventBus    │  │
│  └──────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
                      │
                      ▼
            ┌──────────────┐
            │   EventBus   │
            │ (ML Features)│
            └──────┬───────┘
                   │
                   ▼
┌─────────────────────────────────────────────────────┐
│            Neural Trading Service                   │
│         (Consumes ML Features Only)                 │
└─────────────────────────────────────────────────────┘
```

## 3. ML-Ops Data Combination Strategy

### 3.1 Feature Engineering Pipeline
```python
class MLOpsFeatureEngine:
    def __init__(self):
        self.redis_client = Redis()  # Fast data
        self.timescale = TimescaleDB()  # Slow data
        self.eventbus = EventBus()  # Output
        
    async def compute_features(self, symbol: str) -> MLFeatures:
        # Parallel data fetching
        fast_data_future = self.get_fast_data(symbol)
        slow_data_future = self.get_slow_data(symbol)
        
        # Wait for both
        fast_data = await fast_data_future  # < 10ms
        slow_data = await slow_data_future  # < 500ms
        
        # Combine and compute
        features = self.combine_features(fast_data, slow_data)
        
        # Publish to EventBus
        await self.eventbus.publish(f"stream:ml:features:{symbol}", features)
        
        return features
```

### 3.2 Fast Data Queries (Redis)
```python
async def get_fast_data(self, symbol: str) -> FastData:
    """Get real-time market data from Redis"""
    
    # Current price (< 5ms)
    current_price = await self.redis_client.get(f"price:{symbol}")
    
    # Recent ticks (< 10ms)
    recent_ticks = await self.redis_client.lrange(
        f"ticks:{symbol}", 0, 100
    )
    
    # Order book snapshot (< 5ms)
    orderbook = await self.redis_client.hgetall(f"orderbook:{symbol}")
    
    return FastData(
        price=current_price,
        ticks=recent_ticks,
        orderbook=orderbook,
        timestamp=time.time()
    )
```

### 3.3 Slow Data Queries (TimescaleDB)
```sql
-- Get historical context for ML features
async def get_slow_data(self, symbol: str) -> SlowData:
    """Get historical data from TimescaleDB"""
    
    # Historical volatility (20-day)
    volatility = await self.timescale.query("""
        SELECT stddev(close) as volatility
        FROM ohlcv
        WHERE symbol = $1 
        AND time > NOW() - INTERVAL '20 days'
    """, symbol)
    
    # Support/Resistance levels
    levels = await self.timescale.query("""
        SELECT 
            percentile_cont(0.25) WITHIN GROUP (ORDER BY low) as support,
            percentile_cont(0.75) WITHIN GROUP (ORDER BY high) as resistance
        FROM ohlcv
        WHERE symbol = $1 
        AND time > NOW() - INTERVAL '60 days'
    """, symbol)
    
    # Economic context
    economics = await self.timescale.query("""
        SELECT indicator, value, timestamp
        FROM economic_indicators
        WHERE affects_symbol($1)
        AND timestamp > NOW() - INTERVAL '30 days'
        ORDER BY timestamp DESC
    """, symbol)
    
    # Historical patterns
    patterns = await self.timescale.query("""
        SELECT pattern_type, confidence, last_occurrence
        FROM detected_patterns
        WHERE symbol = $1
        AND confidence > 0.7
        ORDER BY last_occurrence DESC
        LIMIT 10
    """, symbol)
    
    return SlowData(
        volatility=volatility,
        support_resistance=levels,
        economic_context=economics,
        patterns=patterns
    )
```

### 3.4 Feature Combination
```python
def combine_features(self, fast: FastData, slow: SlowData) -> MLFeatures:
    """Combine fast and slow data into ML features"""
    
    return MLFeatures(
        # Fast features (real-time)
        current_price=fast.price,
        bid_ask_spread=(fast.orderbook['ask'] - fast.orderbook['bid']),
        order_imbalance=self.compute_imbalance(fast.orderbook),
        tick_momentum=self.compute_momentum(fast.ticks),
        
        # Slow features (historical)
        historical_volatility=slow.volatility,
        price_to_support_ratio=fast.price / slow.support_resistance['support'],
        price_to_resistance_ratio=fast.price / slow.support_resistance['resistance'],
        
        # Combined features (fast + slow)
        volatility_adjusted_return=(fast.price - slow.patterns[0]['price']) / slow.volatility,
        regime_context=self.detect_regime(fast, slow),
        pattern_similarity=self.pattern_match(fast.ticks, slow.patterns),
        economic_alignment=self.economic_score(fast.price, slow.economic_context),
        
        # Metadata
        timestamp=fast.timestamp,
        feature_version="2.0",
        confidence=self.compute_confidence(fast, slow)
    )
```

## 4. TimescaleDB Schema Design

### 4.1 Hypertables for Time-Series Data
```sql
-- Market data hypertable
CREATE TABLE ohlcv (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    open NUMERIC,
    high NUMERIC,
    low NUMERIC,
    close NUMERIC,
    volume BIGINT
);

SELECT create_hypertable('ohlcv', 'time');
CREATE INDEX ON ohlcv (symbol, time DESC);

-- Compression policy (after 7 days)
ALTER TABLE ohlcv SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'symbol'
);

SELECT add_compression_policy('ohlcv', INTERVAL '7 days');

-- Continuous aggregates for faster queries
CREATE MATERIALIZED VIEW ohlcv_hourly
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS hour,
    symbol,
    first(open, time) as open,
    max(high) as high,
    min(low) as low,
    last(close, time) as close,
    sum(volume) as volume
FROM ohlcv
GROUP BY hour, symbol;

-- Retention policy
SELECT add_retention_policy('ohlcv', INTERVAL '5 years');
```

### 4.2 Economic Indicators Table
```sql
CREATE TABLE economic_indicators (
    time TIMESTAMPTZ NOT NULL,
    indicator_type TEXT NOT NULL,
    country TEXT,
    value NUMERIC,
    previous_value NUMERIC,
    forecast_value NUMERIC,
    impact_level TEXT -- 'high', 'medium', 'low'
);

SELECT create_hypertable('economic_indicators', 'time');

-- Never expire economic data
-- No retention policy
```

### 4.3 ML Training Data
```sql
CREATE TABLE ml_training_data (
    time TIMESTAMPTZ NOT NULL,
    symbol TEXT NOT NULL,
    features JSONB,  -- Store feature vectors
    label NUMERIC,   -- Target variable
    model_version TEXT,
    is_validated BOOLEAN DEFAULT FALSE
);

SELECT create_hypertable('ml_training_data', 'time');

-- Partition by month for efficient training queries
SELECT add_dimension('ml_training_data', 'symbol', 10);
```

## 5. Query Patterns and Optimization

### 5.1 Fast Queries (< 100ms)
```sql
-- Recent price context
WITH recent_prices AS (
    SELECT time, close
    FROM ohlcv
    WHERE symbol = 'AAPL'
    AND time > NOW() - INTERVAL '1 hour'
    ORDER BY time DESC
    LIMIT 60
)
SELECT 
    AVG(close) as avg_price,
    STDDEV(close) as volatility
FROM recent_prices;
```

### 5.2 Medium Queries (100-500ms)
```sql
-- Historical pattern matching
SELECT 
    time,
    close,
    pattern_correlation(
        array_agg(close) OVER (ORDER BY time ROWS 20 PRECEDING),
        $1::numeric[]  -- Current pattern
    ) as similarity
FROM ohlcv
WHERE symbol = $2
AND time > NOW() - INTERVAL '6 months'
ORDER BY similarity DESC
LIMIT 10;
```

### 5.3 Slow Queries (> 500ms) - Cached
```sql
-- Complex correlation analysis (cached in materialized view)
CREATE MATERIALIZED VIEW symbol_correlations AS
SELECT 
    a.symbol as symbol1,
    b.symbol as symbol2,
    corr(a.close, b.close) as correlation
FROM ohlcv a
JOIN ohlcv b ON a.time = b.time
WHERE a.time > NOW() - INTERVAL '30 days'
GROUP BY a.symbol, b.symbol;

-- Refresh daily
CREATE EXTENSION IF NOT EXISTS pg_cron;
SELECT cron.schedule('refresh-correlations', '0 1 * * *', 
    'REFRESH MATERIALIZED VIEW CONCURRENTLY symbol_correlations');
```

## 6. Performance Characteristics

### 6.1 Latency Breakdown
```yaml
operation_latencies:
  fast_data_fetch:
    redis_get: 1-5ms
    redis_lrange: 5-10ms
    redis_hgetall: 2-5ms
    total: < 20ms
    
  slow_data_fetch:
    simple_query: 50-100ms
    aggregate_query: 100-300ms
    complex_join: 300-500ms
    total: < 500ms
    
  feature_computation:
    basic_features: 5-10ms
    ml_inference: 20-50ms
    total: < 60ms
    
  total_pipeline:
    p50: 80ms
    p95: 120ms
    p99: 200ms
```

### 6.2 Throughput Characteristics
```yaml
throughput:
  redis:
    reads: 100,000/sec
    writes: 50,000/sec
    
  timescaledb:
    simple_queries: 1,000/sec
    complex_queries: 100/sec
    writes: 10,000/sec
    
  ml_ops:
    features_per_second: 1,000
    symbols_processed: 50/sec
```

## 7. Caching Strategy

### 7.1 Redis Caching for Slow Data
```python
class SlowDataCache:
    def __init__(self):
        self.redis = Redis()
        self.ttl = {
            'volatility': 300,  # 5 minutes
            'support_resistance': 3600,  # 1 hour
            'economic': 86400,  # 1 day
            'patterns': 1800  # 30 minutes
        }
    
    async def get_cached_or_fetch(self, key: str, fetch_func):
        # Try cache first
        cached = await self.redis.get(f"cache:{key}")
        if cached:
            return json.loads(cached)
        
        # Fetch from TimescaleDB
        data = await fetch_func()
        
        # Cache with appropriate TTL
        ttl = self.ttl.get(key.split(':')[0], 300)
        await self.redis.setex(
            f"cache:{key}",
            ttl,
            json.dumps(data)
        )
        
        return data
```

## 8. Monitoring and Alerting

### 8.1 Key Metrics
```yaml
monitoring:
  fast_data:
    metric: redis_latency
    threshold: > 10ms
    action: alert
    
  slow_data:
    metric: timescale_query_time
    threshold: > 1000ms
    action: investigate_query
    
  data_freshness:
    metric: last_update_time
    threshold: > 60s
    action: check_ingestion
    
  feature_quality:
    metric: null_feature_rate
    threshold: > 1%
    action: validate_pipeline
```

## Summary

The fast vs slow data architecture provides:

1. **Clear Separation**: Fast data in Redis (< 20ms), slow data in TimescaleDB (< 500ms)
2. **Optimized Access**: Each data type stored and accessed optimally
3. **Rich Features**: ML-Ops combines both for comprehensive feature engineering
4. **Predictable Performance**: Known latencies for each data type
5. **Scalability**: Can scale fast and slow paths independently

This design ensures ML-Ops can create sophisticated features using both real-time market conditions and historical context, while maintaining the performance requirements for algorithmic trading.