# Market Data Implementation Plan

## Priority 1: Extend Historical Backfill (Immediate)

### 1.1 Modify `historical_backfill.py`

```python
# Current: Only loads 1 week
# Target: Load 5+ years using existing providers

class HistoricalBackfillCoordinator:
    DEFAULT_BACKFILL_YEARS = 5  # Increase from current 1 week
    
    # Update priority thresholds
    PRIORITY_THRESHOLDS = {
        'CRITICAL': timedelta(days=90),    # Last 3 months for live trading
        'HIGH': timedelta(days=365),       # Last year for pattern recognition  
        'MEDIUM': timedelta(days=1825),    # 5 years for ML training
        'LOW': timedelta(days=7300)        # 20 years for research
    }
```

### 1.2 Provider-Specific Optimizations

#### Yahoo Finance (20+ years free)
```python
class YahooFinanceProvider(BaseProvider):
    async def get_extended_history(self, symbol: str, years: int = 20):
        """Fetch up to 20 years of daily data"""
        # Already supports this, just need to use it
        end = datetime.now()
        start = end - timedelta(days=years * 365)
        return await self.get_market_data([symbol], start, end, "1day")
```

#### Alpaca (5 years free)
```python
class AlpacaProvider(BaseProvider):
    async def get_full_history(self, symbol: str):
        """Utilize full 5 years of minute data available on free tier"""
        # Implement chunking to respect rate limits
        # 200 requests/minute on basic plan
        chunk_size = timedelta(days=30)  # Month chunks
        # ... implementation
```

## Priority 2: Add Crypto Data Sources

### 2.1 Implement Binance Provider

```python
# data_ingestion/providers/binance.py
import ccxt.async_support as ccxt

class BinanceProvider(BaseProvider):
    """Binance data provider for cryptocurrency markets."""
    
    def __init__(self):
        super().__init__("binance")
        self.exchange = ccxt.binance({
            'enableRateLimit': True,
            'rateLimit': 50,  # 1200/min = 50ms between requests
        })
    
    async def get_market_data(self, symbols, start_time, end_time, interval="1h"):
        """Fetch historical crypto data from Binance"""
        # Convert symbols to Binance format (BTC/USDT)
        # Implement pagination for large date ranges
        # Handle Binance-specific intervals
```

### 2.2 Implement CoinGecko Provider

```python
# data_ingestion/providers/coingecko.py
class CoinGeckoProvider(BaseProvider):
    """CoinGecko for historical crypto prices and market data."""
    
    BASE_URL = "https://api.coingecko.com/api/v3"
    
    async def get_market_data(self, symbols, start_time, end_time):
        """Fetch historical data with market cap and volume"""
        # Map symbols to CoinGecko IDs
        # Respect rate limits (10-50 calls/min)
        # Include market cap data
```

## Priority 3: Data Quality & Validation

### 3.1 Cross-Provider Validation

```python
# utils/data_validation.py
class DataValidator:
    @staticmethod
    async def cross_validate_prices(symbol: str, date: datetime, providers: List[BaseProvider]):
        """Compare prices across multiple providers"""
        results = {}
        for provider in providers:
            try:
                data = await provider.get_market_data([symbol], date, date + timedelta(days=1))
                results[provider.name] = data
            except:
                continue
        
        # Calculate variance between providers
        # Flag discrepancies > 0.5%
        # Return consensus price
```

### 3.2 Corporate Actions Handler

```python
# utils/corporate_actions.py
class CorporateActionsHandler:
    async def detect_splits(self, symbol: str, price_data: pd.DataFrame):
        """Detect potential stock splits from price jumps"""
        # Check for 50%+ price drops with volume spike
        # Cross-reference with Yahoo Finance events
        # Adjust historical prices
    
    async def adjust_dividends(self, symbol: str, price_data: pd.DataFrame):
        """Adjust prices for dividend payments"""
        # Fetch dividend history
        # Apply adjustments to historical data
```

## Priority 4: Rate Limit Optimization

### 4.1 Intelligent Rate Limiter

```python
# utils/rate_limiter.py
class IntelligentRateLimiter:
    def __init__(self):
        self.provider_limits = {
            'yahoo': {'calls': float('inf'), 'period': 60},
            'alpaca': {'calls': 200, 'period': 60},
            'polygon': {'calls': 5, 'period': 60},  # Free tier
            'alpha_vantage': {'calls': 5, 'period': 60},
            'binance': {'calls': 1200, 'period': 60},
            'coingecko': {'calls': 50, 'period': 60}
        }
        self.call_history = defaultdict(list)
    
    async def acquire(self, provider: str):
        """Smart rate limiting with provider rotation"""
        # Check current usage
        # If at limit, try alternate provider
        # Implement exponential backoff
        # Track success rates
```

### 4.2 Provider Rotation Strategy

```python
# utils/provider_router.py
class ProviderRouter:
    def __init__(self):
        self.provider_health = {}  # Track success rates
        self.provider_costs = {}   # Track API usage
    
    async def get_best_provider(self, data_type: str, timeframe: str):
        """Select optimal provider based on request type"""
        if data_type == 'crypto':
            return self._select_crypto_provider()
        elif timeframe == 'intraday':
            return self._select_intraday_provider()
        else:
            return self._select_daily_provider()
```

## Priority 5: Storage Optimization

### 5.1 TimescaleDB Partitioning

```sql
-- Partition by year for efficient queries
CREATE TABLE market_data_2024 PARTITION OF market_data
FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');

-- Create hypertable with compression
SELECT create_hypertable('market_data', 'time', 
    chunk_time_interval => INTERVAL '1 month');

-- Enable compression after 1 week
ALTER TABLE market_data SET (
    timescaledb.compress,
    timescaledb.compress_after = '1 week'
);
```

### 5.2 Materialized Views for Common Queries

```sql
-- 5-minute aggregates
CREATE MATERIALIZED VIEW market_data_5min AS
SELECT 
    time_bucket('5 minutes', time) AS bucket,
    symbol,
    first(open, time) AS open,
    max(high) AS high,
    min(low) AS low,
    last(close, time) AS close,
    sum(volume) AS volume
FROM market_data
GROUP BY bucket, symbol;

-- Daily aggregates with technical indicators
CREATE MATERIALIZED VIEW market_data_daily_ta AS
SELECT 
    date_trunc('day', time) AS day,
    symbol,
    open, high, low, close, volume,
    avg(close) OVER (PARTITION BY symbol ORDER BY time ROWS 20 PRECEDING) AS sma_20,
    avg(close) OVER (PARTITION BY symbol ORDER BY time ROWS 50 PRECEDING) AS sma_50
FROM market_data;
```

## Implementation Timeline

### Week 1: Core Infrastructure
- Day 1-2: Extend historical_backfill.py to 5 years
- Day 3-4: Implement Binance provider
- Day 5: Implement CoinGecko provider

### Week 2: Data Quality
- Day 1-2: Cross-provider validation
- Day 3-4: Corporate actions handler
- Day 5: Rate limit optimization

### Week 3: Storage & Performance
- Day 1-2: TimescaleDB partitioning
- Day 3-4: Materialized views
- Day 5: Performance testing

### Week 4: Testing & Deployment
- Day 1-2: Integration testing
- Day 3-4: Backfill execution
- Day 5: Documentation & monitoring

## Success Metrics

1. **Data Coverage**: 5+ years for all major symbols
2. **Data Quality**: <0.1% price discrepancies
3. **Performance**: <100ms query time for any date range
4. **Cost**: $0 additional monthly cost (free tiers only)
5. **Reliability**: 99.9% uptime with automatic failover

## Risk Mitigation

1. **API Changes**: Implement version detection and adapters
2. **Rate Limits**: Conservative limits with exponential backoff
3. **Data Loss**: Regular backups to S3/cloud storage
4. **Provider Outages**: Multiple fallback providers
5. **Cost Overruns**: Strict monitoring of API usage