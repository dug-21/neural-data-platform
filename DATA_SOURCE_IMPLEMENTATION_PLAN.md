# Data Source Implementation Plan

## Quick Start Implementation Guide

### Phase 1: Core Market Data Providers (Week 1)

#### 1.1 Yahoo Finance Provider
```rust
// src/providers/yahoo_finance.rs
use crate::integration::MarketDataProvider;

pub struct YahooFinanceProvider {
    base_url: String,
    rate_limiter: RateLimiter,
    cache: Arc<RedisCache>,
}

impl YahooFinanceProvider {
    // Key endpoints:
    // - /v8/finance/chart/{symbol} - Historical and real-time
    // - /v7/finance/quote - Real-time quotes
    // - /v8/finance/spark - Lightweight price data
    
    pub async fn fetch_intraday(&self, symbol: &str, interval: &str) -> Result<Vec<Candle>> {
        // Intervals: 1m, 2m, 5m, 15m, 30m, 60m, 90m, 1d
        // Range: 1d, 5d, 1mo, 3mo, 6mo, 1y, 2y, 5y, 10y, ytd, max
    }
}
```

#### 1.2 Binance WebSocket Provider
```rust
// src/providers/binance_ws.rs
pub struct BinanceWebSocketProvider {
    streams: Vec<String>,
    ws_client: WebSocketClient,
    message_handler: Arc<dyn MessageHandler>,
}

impl BinanceWebSocketProvider {
    // Streams available:
    // - {symbol}@trade - Real-time trades
    // - {symbol}@kline_{interval} - Real-time candles
    // - {symbol}@depth{levels} - Order book
    // - {symbol}@bookTicker - Best bid/ask
    
    pub async fn subscribe_klines(&self, symbols: Vec<&str>, interval: &str) {
        // wss://stream.binance.com:9443/stream
    }
}
```

### Phase 2: Economic & Alternative Data (Week 2)

#### 2.1 FRED Economic Data Provider
```rust
// src/providers/fred.rs
pub struct FREDProvider {
    api_key: String,
    series_cache: HashMap<String, SeriesMetadata>,
}

impl FREDProvider {
    // Key series for day trading:
    // - DGS10: 10-Year Treasury Rate
    // - DFF: Federal Funds Rate  
    // - DEXUSEU: USD/EUR Exchange Rate
    // - VIXCLS: VIX Close
    // - UNRATE: Unemployment Rate
    
    pub async fn get_series(&self, series_id: &str) -> Result<TimeSeries> {
        // https://api.stlouisfed.org/fred/series/observations
    }
}
```

#### 2.2 Reddit Sentiment Provider
```rust
// src/providers/reddit_sentiment.rs
pub struct RedditSentimentProvider {
    praw_client: PrawClient,
    sentiment_analyzer: SentimentModel,
    tracked_subreddits: Vec<String>,
}

impl RedditSentimentProvider {
    // Subreddits to monitor:
    // - wallstreetbets
    // - stocks  
    // - options
    // - cryptocurrency
    // - daytrading
    
    pub async fn get_ticker_sentiment(&self, ticker: &str) -> SentimentScore {
        // Track: mentions, sentiment, unusual activity
    }
}
```

### Phase 3: Integration Layer (Week 3)

#### 3.1 Unified Data Aggregator
```rust
// src/data/aggregator.rs
pub struct DataAggregator {
    providers: HashMap<String, Box<dyn MarketDataProvider>>,
    normalizer: DataNormalizer,
    quality_monitor: QualityMonitor,
}

impl DataAggregator {
    pub async fn get_unified_quote(&self, symbol: &str) -> Result<UnifiedQuote> {
        // 1. Check Redis cache
        // 2. Query multiple providers in parallel
        // 3. Normalize and validate data
        // 4. Apply quality scoring
        // 5. Return best available data
    }
}
```

#### 3.2 Smart Caching Strategy
```rust
// src/data/smart_cache.rs
pub struct SmartCache {
    redis: RedisCache,
    ttl_strategy: TTLStrategy,
}

impl SmartCache {
    // TTL Strategy:
    // - Real-time quotes: 1 second
    // - 1m candles: 5 seconds  
    // - Economic data: 1 hour
    // - News/sentiment: 5 minutes
    // - Company fundamentals: 24 hours
}
```

### Configuration Schema

```yaml
# config/data_sources.yaml
data_sources:
  yahoo_finance:
    enabled: true
    priority: 1
    rate_limit:
      requests_per_minute: 2000
      burst: 100
    symbols:
      stocks: ["SPY", "QQQ", "AAPL", "MSFT", "TSLA"]
      forex: ["EURUSD=X", "GBPUSD=X", "USDJPY=X"]
    
  binance:
    enabled: true
    priority: 1
    websocket_url: "wss://stream.binance.com:9443"
    symbols: ["BTCUSDT", "ETHUSDT", "SOLUSDT"]
    streams: ["kline_1m", "trade", "bookTicker"]
    
  fred:
    enabled: true
    api_key: "${FRED_API_KEY}"
    series:
      - series_id: "DGS10"
        name: "10 Year Treasury"
        frequency: "daily"
      - series_id: "VIXCLS"
        name: "VIX Close"
        frequency: "daily"
        
  reddit:
    enabled: true
    subreddits: ["wallstreetbets", "stocks", "daytrading"]
    sentiment_threshold: 0.7
    mention_threshold: 10
    
  alpha_vantage:
    enabled: true
    api_key: "${ALPHA_VANTAGE_KEY}"
    priority: 2  # Fallback source
    rate_limit:
      requests_per_minute: 5
      daily_limit: 500
```

### Database Schema Updates

```sql
-- TimescaleDB schemas for new data types

-- Market depth approximation
CREATE TABLE market_depth (
    symbol TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    bid_volume NUMERIC,
    ask_volume NUMERIC,
    bid_ask_imbalance NUMERIC,
    spread NUMERIC,
    PRIMARY KEY (symbol, timestamp)
);
SELECT create_hypertable('market_depth', 'timestamp');

-- Sentiment scores
CREATE TABLE sentiment_data (
    symbol TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,  -- reddit, twitter, news
    sentiment_score NUMERIC,
    mention_count INTEGER,
    unusual_activity BOOLEAN,
    PRIMARY KEY (symbol, timestamp, source)
);
SELECT create_hypertable('sentiment_data', 'timestamp');

-- Economic indicators
CREATE TABLE economic_indicators (
    series_id TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    value NUMERIC,
    previous_value NUMERIC,
    change_percent NUMERIC,
    PRIMARY KEY (series_id, timestamp)
);
SELECT create_hypertable('economic_indicators', 'timestamp');
```

### Rate Limit Management

```rust
// src/utils/rate_limiter.rs
pub struct AdaptiveRateLimiter {
    limits: HashMap<String, RateLimit>,
    backoff_strategy: ExponentialBackoff,
}

impl AdaptiveRateLimiter {
    pub async fn check_and_wait(&self, source: &str) -> Result<()> {
        // Smart rate limiting:
        // 1. Track usage per source
        // 2. Implement token bucket algorithm
        // 3. Automatic backoff on 429 errors
        // 4. Prioritize real-time data requests
    }
}

// Usage pattern
let limiter = AdaptiveRateLimiter::new();
limiter.check_and_wait("yahoo_finance").await?;
let data = yahoo_provider.fetch_quote(symbol).await?;
```

### Error Handling & Failover

```rust
// src/data/failover.rs
pub struct DataSourceFailover {
    primary: Box<dyn MarketDataProvider>,
    fallbacks: Vec<Box<dyn MarketDataProvider>>,
    health_checker: HealthChecker,
}

impl DataSourceFailover {
    pub async fn get_data_with_failover(&self, symbol: &str) -> Result<MarketData> {
        // Try primary source
        match self.primary.get_real_time_data(symbol).await {
            Ok(data) => Ok(data),
            Err(e) => {
                log::warn!("Primary source failed: {}", e);
                // Try fallbacks in order
                for fallback in &self.fallbacks {
                    if let Ok(data) = fallback.get_real_time_data(symbol).await {
                        return Ok(data);
                    }
                }
                Err(anyhow!("All data sources failed"))
            }
        }
    }
}
```

### Testing Strategy

```rust
// tests/integration/data_providers_test.rs
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_yahoo_finance_integration() {
        // Test with known symbols
        // Verify rate limiting
        // Check data normalization
    }
    
    #[tokio::test]
    async fn test_failover_mechanism() {
        // Simulate primary source failure
        // Verify automatic failover
        // Check data consistency
    }
    
    #[tokio::test]
    async fn test_cache_effectiveness() {
        // Measure cache hit rates
        // Verify TTL strategies
        // Test cache invalidation
    }
}
```

### Monitoring & Alerts

```yaml
# monitoring/data_quality.yaml
quality_checks:
  - name: data_freshness
    threshold: 5s
    alert: "Data older than 5 seconds for {symbol}"
    
  - name: source_availability  
    threshold: 0.95
    alert: "Source {source} availability below 95%"
    
  - name: data_consistency
    threshold: 0.02  
    alert: "Price discrepancy > 2% between sources"
    
  - name: rate_limit_usage
    threshold: 0.8
    alert: "Rate limit usage above 80% for {source}"
```

## Implementation Timeline

### Week 1: Foundation
- [ ] Implement Yahoo Finance provider
- [ ] Set up Binance WebSocket connection
- [ ] Create unified data models
- [ ] Basic Redis caching

### Week 2: Expansion  
- [ ] Add FRED economic data
- [ ] Implement Reddit sentiment
- [ ] Create data normalization layer
- [ ] Add TimescaleDB schemas

### Week 3: Integration
- [ ] Build data aggregator
- [ ] Implement failover logic
- [ ] Add rate limit management
- [ ] Create quality monitoring

### Week 4: Optimization
- [ ] Performance tuning
- [ ] Advanced caching strategies
- [ ] Comprehensive testing
- [ ] Production deployment

## Success Metrics

1. **Data Latency**: < 100ms for cached, < 500ms for fresh
2. **Availability**: > 99.5% uptime across all sources
3. **Coverage**: 95% of requested symbols available
4. **Quality Score**: > 8/10 for integrated data
5. **Cost**: $0 for all data sources

This implementation provides professional-grade data capabilities while staying completely within free tiers.