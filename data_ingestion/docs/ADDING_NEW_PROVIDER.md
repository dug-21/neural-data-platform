# 🚀 Quick Guide: Adding a New Data Provider

## Overview
Adding a new data provider to the Neural Trader data ingestion platform takes just 3 simple steps!

## Step 1: Create Your Provider Class

Create a new file in `data_ingestion/providers/`:

```python
# data_ingestion/providers/my_provider.py
from typing import List, AsyncIterator
from datetime import datetime
import aiohttp

from .base import BaseProvider, MarketData

class MyProvider(BaseProvider):
    """Description of your provider."""
    
    def __init__(self):
        super().__init__("my_provider")
        self.api_key = self.settings.my_provider_api_key  # Auto-loaded from env
        self.base_url = "https://api.myprovider.com/v1"
        self.session = None
    
    async def connect(self):
        """Initialize connection (HTTP session, WebSocket, etc)."""
        self.session = aiohttp.ClientSession(
            headers={"Authorization": f"Bearer {self.api_key}"}
        )
        self._connected = True
        self.logger.info("Connected to MyProvider")
    
    async def disconnect(self):
        """Clean up connections."""
        if self.session:
            await self.session.close()
        self._connected = False
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data."""
        await self._rate_limit()  # Automatic rate limiting
        
        for symbol in self._validate_symbols(symbols):  # Auto validation
            # Your API call here
            url = f"{self.base_url}/candles/{symbol}"
            params = {
                "from": int(start_time.timestamp()),
                "to": int(end_time.timestamp()),
                "resolution": self._map_interval(interval)
            }
            
            async with self.session.get(url, params=params) as response:
                data = await response.json()
                
                # Convert to standard MarketData format
                for candle in data['candles']:
                    yield MarketData(
                        time=datetime.fromtimestamp(candle['t']),
                        symbol=symbol,
                        open=float(candle['o']),
                        high=float(candle['h']),
                        low=float(candle['l']),
                        close=float(candle['c']),
                        volume=int(candle['v']),
                        provider=self.name,
                        metadata={"source": "historical"}
                    )
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data."""
        # Example WebSocket implementation
        import websockets
        
        ws_url = "wss://stream.myprovider.com/quotes"
        async with websockets.connect(ws_url) as websocket:
            # Subscribe to symbols
            await websocket.send(json.dumps({
                "action": "subscribe",
                "symbols": symbols
            }))
            
            while True:
                data = await websocket.recv()
                quote = json.loads(data)
                
                yield MarketData(
                    time=datetime.now(),
                    symbol=quote['symbol'],
                    open=quote['open'],
                    high=quote['high'],
                    low=quote['low'],
                    close=quote['price'],
                    volume=quote['volume'],
                    provider=self.name
                )
    
    def _map_interval(self, interval: str) -> str:
        """Map our standard intervals to provider-specific format."""
        mapping = {
            "1min": "1",
            "5min": "5",
            "15min": "15",
            "30min": "30",
            "1hour": "60",
            "1day": "D",
            "1week": "W"
        }
        return mapping.get(interval, "1")
```

## Step 2: Register Your Provider

Add your provider to the registry in `data_ingestion/providers/__init__.py`:

```python
from .my_provider import MyProvider  # Add import

PROVIDERS = {
    "iex_cloud": IEXCloudProvider,
    "alpha_vantage": AlphaVantageProvider,
    "polygon": PolygonProvider,
    "yahoo_finance": YahooFinanceProvider,
    "finnhub": FinnhubProvider,
    "my_provider": MyProvider,  # Add this line
}
```

## Step 3: Add Configuration

Add your API key to `.env`:

```bash
# Existing keys...
MY_PROVIDER_API_KEY=your_api_key_here
```

## 🎉 That's It! Your Provider is Ready

Use it immediately:

```bash
# Start real-time streaming
python -m data_ingestion.main start \
  --providers my_provider \
  --symbols AAPL MSFT GOOGL

# Backfill historical data
python -m data_ingestion.main backfill \
  --providers my_provider \
  --symbols AAPL \
  --start-date 2024-01-01 \
  --end-date 2024-07-03
```

## 🎁 What You Get For Free

Your provider automatically inherits:

### 1. **Rate Limiting**
```python
await self._rate_limit()  # Respects MAX_REQUESTS_PER_MINUTE
```

### 2. **Symbol Validation**
```python
symbols = self._validate_symbols(symbols)  # Cleans and validates
```

### 3. **Retry Logic**
```python
@with_retry(max_attempts=3, backoff_factor=2)
async def your_method(self):
    # Automatic exponential backoff retry
```

### 4. **Metrics Collection**
```python
# Automatic Prometheus metrics:
- data_points_processed_total{provider="my_provider"}
- api_requests_total{provider="my_provider"}
- stream_health{provider="my_provider"}
```

### 5. **Structured Logging**
```python
self.logger.info("Connected", extra={"symbols": symbols})
# Outputs: {"time": "...", "level": "INFO", "message": "Connected", "symbols": [...]}
```

### 6. **Error Handling**
Built-in exception handling and graceful degradation

### 7. **Connection Management**
```python
async with MyProvider() as provider:
    # Auto connect/disconnect
```

## 📚 Optional Features

### Add Tick Data Support
```python
async def get_tick_data(self, symbols, start_time, end_time):
    """Optional: Implement for tick-level data."""
    for tick in self._fetch_ticks(symbols, start_time, end_time):
        yield TickData(
            time=tick['time'],
            symbol=tick['symbol'],
            price=tick['price'],
            size=tick['size'],
            exchange=tick['exchange'],
            provider=self.name
        )
```

### Add Order Book Support
```python
async def get_order_book(self, symbols):
    """Optional: Implement for Level 2 data."""
    book = await self._fetch_order_book(symbols)
    yield OrderBookData(
        time=datetime.now(),
        symbol=book['symbol'],
        bid_price=book['bid'],
        bid_size=book['bid_size'],
        ask_price=book['ask'],
        ask_size=book['ask_size'],
        mid_price=(book['bid'] + book['ask']) / 2,
        spread=book['ask'] - book['bid'],
        provider=self.name
    )
```

## 🧪 Testing Your Provider

Create a test file:

```python
# tests/providers/test_my_provider.py
import pytest
from data_ingestion.providers.my_provider import MyProvider

@pytest.mark.asyncio
async def test_my_provider_connection():
    provider = MyProvider()
    await provider.connect()
    assert provider._connected
    await provider.disconnect()
    assert not provider._connected

@pytest.mark.asyncio
async def test_my_provider_data():
    async with MyProvider() as provider:
        data_count = 0
        async for data in provider.get_market_data(
            ["AAPL"], 
            datetime(2024, 1, 1), 
            datetime(2024, 1, 2)
        ):
            assert data.symbol == "AAPL"
            assert data.provider == "my_provider"
            data_count += 1
        assert data_count > 0
```

## 🌟 Real Examples

### Cryptocurrency Provider (No API Key)
```python
class BinanceProvider(BaseProvider):
    def __init__(self):
        super().__init__("binance")
        self.base_url = "https://api.binance.com/api/v3"
        # No API key needed for public data!
```

### News Sentiment Provider
```python
class NewsAPIProvider(BaseProvider):
    async def get_news_sentiment(self, symbols):
        """Custom method for news data."""
        # Implementation here
```

### Social Sentiment Provider
```python
class RedditProvider(BaseProvider):
    async def get_social_sentiment(self, symbols):
        """Track r/wallstreetbets mentions."""
        # Implementation here
```

## 🚀 Advanced Features

### Custom Aggregation
```python
class MyProvider(BaseProvider):
    def aggregate_order_flow(self, data):
        """Provider-specific aggregation logic."""
        return custom_aggregation(data)
```

### Provider-Specific Caching
```python
class MyProvider(BaseProvider):
    def __init__(self):
        super().__init__("my_provider")
        self.cache = {}  # Provider-specific cache
```

### Dynamic Symbol Mapping
```python
def _map_symbol(self, symbol: str) -> str:
    """Map standard symbols to provider format."""
    # AAPL → AAPL.US or AAPL:NASDAQ
    return f"{symbol}.US"
```

## 📊 Provider Comparison

| Feature | Yahoo | Polygon | Alpha Vantage | Finnhub | Your Provider |
|---------|-------|---------|---------------|---------|---------------|
| Free Tier | ✅ | ❌ | ✅ | ✅ | ? |
| Real-time | ❌ | ✅ | ❌ | ✅ | ? |
| Historical | ✅ | ✅ | ✅ | ✅ | ? |
| WebSocket | ❌ | ✅ | ❌ | ✅ | ? |
| Rate Limit | None | 5/sec | 5/min | 60/min | ? |

## 🎯 Best Practices

1. **Use Async Throughout** - All methods should be async
2. **Yield Data** - Use AsyncIterator for memory efficiency
3. **Handle Errors Gracefully** - Don't crash on bad data
4. **Respect Rate Limits** - Always call `_rate_limit()`
5. **Normalize Data** - Convert to standard MarketData format
6. **Add Metadata** - Use the metadata field for provider-specific info
7. **Log Important Events** - Connection, errors, warnings
8. **Test Thoroughly** - Unit tests and integration tests

## 🤝 Contributing

1. Fork the repository
2. Create your provider following this guide
3. Add comprehensive tests
4. Update the provider comparison table
5. Submit a pull request

Happy coding! 🚀