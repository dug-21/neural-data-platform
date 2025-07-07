# Stock Market Real-Time Data Sources

## Finnhub Stock API ⭐⭐⭐⭐⭐
**Best Free Tier Available**

### Overview
- **Website**: https://finnhub.io/
- **Documentation**: https://finnhub.io/docs/api
- **Update Frequency**: Real-time (WebSocket), 1 second (REST)
- **Coverage**: US stocks, international exchanges, forex, crypto

### Free Tier Details
- 60 API calls per minute
- Unlimited WebSocket connections
- Real-time US stock trades
- 15-minute delayed international stocks
- Basic company fundamentals
- Market news and sentiment

### WebSocket Example
```javascript
const socket = new WebSocket('wss://ws.finnhub.io?token=YOUR_API_KEY');

socket.addEventListener('open', function (event) {
    socket.send(JSON.stringify({'type':'subscribe', 'symbol': 'AAPL'}))
    socket.send(JSON.stringify({'type':'subscribe', 'symbol': 'MSFT'}))
});

socket.addEventListener('message', function (event) {
    const data = JSON.parse(event.data);
    console.log('Trade data:', data);
});
```

### REST API Example
```bash
# Real-time quote
curl https://finnhub.io/api/v1/quote?symbol=AAPL&token=YOUR_API_KEY

# Company profile
curl https://finnhub.io/api/v1/stock/profile2?symbol=AAPL&token=YOUR_API_KEY
```

### Rate Limits
- REST API: 60 calls/minute
- WebSocket: No limits on connections
- Symbols per connection: 50

---

## Alpha Vantage ⭐⭐⭐⭐

### Overview
- **Website**: https://www.alphavantage.co/
- **Documentation**: https://www.alphavantage.co/documentation/
- **Update Frequency**: 1 minute (intraday), real-time (forex)
- **Coverage**: US stocks, forex, crypto, commodities

### Free Tier Details
- 5 API requests per minute
- 500 requests per day
- Full historical data access
- 50+ technical indicators
- Fundamental data
- Economic indicators

### API Examples
```python
import requests

# Intraday data
url = 'https://www.alphavantage.co/query'
params = {
    'function': 'TIME_SERIES_INTRADAY',
    'symbol': 'MSFT',
    'interval': '1min',
    'apikey': 'YOUR_API_KEY'
}
response = requests.get(url, params=params)
data = response.json()

# Real-time forex
params = {
    'function': 'CURRENCY_EXCHANGE_RATE',
    'from_currency': 'USD',
    'to_currency': 'EUR',
    'apikey': 'YOUR_API_KEY'
}
```

### Technical Indicators
- SMA, EMA, WMA, DEMA, TEMA
- RSI, MACD, STOCH, CCI, AROON
- BBANDS, AD, OBV
- And 40+ more indicators

---

## Polygon.io ⭐⭐⭐⭐

### Overview
- **Website**: https://polygon.io/
- **Documentation**: https://polygon.io/docs
- **Update Frequency**: Real-time (<20ms latency)
- **Coverage**: Stocks, options, forex, crypto

### Free Tier Details
- 5 API calls per minute
- End-of-day data only
- 2 years historical data
- All US exchanges
- Basic aggregates

### WebSocket (Paid Tiers Only)
```javascript
// WebSocket available in paid tiers
const socket = new WebSocket('wss://socket.polygon.io/stocks');
socket.send(JSON.stringify({
    "action": "auth",
    "params": "YOUR_API_KEY"
}));
```

### REST API Example
```bash
# Previous day's aggregate
curl "https://api.polygon.io/v2/aggs/ticker/AAPL/prev?apiKey=YOUR_API_KEY"

# Ticker details
curl "https://api.polygon.io/v3/reference/tickers/AAPL?apiKey=YOUR_API_KEY"
```

---

## Yahoo Finance (Unofficial)

### Overview
- **Note**: No official API, community-maintained
- **Update Frequency**: Real-time during market hours
- **Coverage**: Global markets
- **Risk**: Can break without notice

### Popular Libraries
```python
# yfinance (Python)
import yfinance as yf

ticker = yf.Ticker("AAPL")
# Real-time price
info = ticker.info
current_price = info['currentPrice']

# Historical data
hist = ticker.history(period="1d", interval="1m")
```

```javascript
// yahoo-finance2 (Node.js)
import yahooFinance from 'yahoo-finance2';

const quote = await yahooFinance.quote('AAPL');
const historical = await yahooFinance.historical('AAPL', {
    period1: '2024-01-01',
    interval: '1m'
});
```

---

## IEX Cloud ⭐⭐⭐

### Overview
- **Website**: https://iexcloud.io/
- **Documentation**: https://iexcloud.io/docs/
- **Update Frequency**: Real-time (paid), 15-min delay (free)
- **Coverage**: US markets primarily

### Free Tier Details
- 50,000 messages per month
- Core data only
- 15-minute delayed quotes
- 5 years historical data
- Limited endpoints

### API Example
```python
import requests

base_url = 'https://cloud.iexapis.com/stable'
token = 'YOUR_TOKEN'

# Quote
url = f'{base_url}/stock/AAPL/quote?token={token}'
response = requests.get(url)

# Batch request
symbols = 'AAPL,MSFT,GOOGL'
url = f'{base_url}/stock/market/batch?symbols={symbols}&types=quote&token={token}'
```

---

## Twelve Data ⭐⭐⭐

### Overview
- **Website**: https://twelvedata.com/
- **Documentation**: https://twelvedata.com/docs
- **Update Frequency**: Real-time (WebSocket)
- **Coverage**: Global - 20+ exchanges

### Free Tier Details
- 800 API calls/day
- 8 requests/minute
- Real-time data
- 2 years historical
- Basic technical indicators

### WebSocket Example
```javascript
const ws = new WebSocket('wss://ws.twelvedata.com/v1/quotes/price');

ws.onopen = () => {
    ws.send(JSON.stringify({
        "action": "subscribe",
        "params": {
            "symbols": ["AAPL", "MSFT"]
        }
    }));
};
```

---

## EODHD

### Overview
- **Website**: https://eodhd.com/
- **Documentation**: https://eodhd.com/financial-apis/
- **Update Frequency**: 15-20 min delay (free)
- **Coverage**: 70+ exchanges worldwide

### Free Tier Details
- 20 API calls/day
- Historical data only
- End-of-day prices
- Fundamental data
- "DEMO" API key for testing

### Demo Usage
```bash
# Test with DEMO key (limited symbols)
curl "https://eodhd.com/api/real-time/AAPL.US?api_token=demo&fmt=json"

# Available demo symbols: AAPL.US, TSLA.US, VTI.US, AMZN.US
```

---

## Best Practices

### 1. Rate Limit Management
```python
import time
from functools import wraps

def rate_limit(calls_per_minute):
    min_interval = 60.0 / calls_per_minute
    last_called = [0.0]
    
    def decorator(func):
        @wraps(func)
        def wrapper(*args, **kwargs):
            elapsed = time.time() - last_called[0]
            left_to_wait = min_interval - elapsed
            if left_to_wait > 0:
                time.sleep(left_to_wait)
            ret = func(*args, **kwargs)
            last_called[0] = time.time()
            return ret
        return wrapper
    return decorator

@rate_limit(60)  # 60 calls per minute
def call_finnhub_api():
    # Your API call here
    pass
```

### 2. WebSocket Reconnection
```javascript
class ResilientWebSocket {
    constructor(url) {
        this.url = url;
        this.reconnectInterval = 5000;
        this.shouldReconnect = true;
        this.connect();
    }
    
    connect() {
        this.ws = new WebSocket(this.url);
        
        this.ws.onopen = () => {
            console.log('Connected');
            this.reconnectInterval = 5000;
        };
        
        this.ws.onclose = () => {
            if (this.shouldReconnect) {
                setTimeout(() => this.connect(), this.reconnectInterval);
                this.reconnectInterval = Math.min(30000, this.reconnectInterval * 2);
            }
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
    }
}
```

### 3. Data Caching Strategy
```python
import redis
import json
from datetime import datetime, timedelta

class MarketDataCache:
    def __init__(self):
        self.redis = redis.Redis()
        self.ttl = 60  # 1 minute cache
    
    def get_quote(self, symbol):
        cached = self.redis.get(f"quote:{symbol}")
        if cached:
            return json.loads(cached)
        
        # Fetch from API
        data = fetch_from_api(symbol)
        
        # Cache with TTL
        self.redis.setex(
            f"quote:{symbol}",
            self.ttl,
            json.dumps(data)
        )
        return data
```

### 4. Multi-Source Fallback
```python
class MarketDataAggregator:
    def __init__(self):
        self.sources = [
            FinnhubClient(),
            AlphaVantageClient(),
            YahooFinanceClient()
        ]
    
    async def get_quote(self, symbol):
        for source in self.sources:
            try:
                return await source.get_quote(symbol)
            except Exception as e:
                print(f"Failed with {source.__class__.__name__}: {e}")
                continue
        raise Exception("All sources failed")
```

---

## Comparison Matrix

| Provider | Real-time | Free Limits | WebSocket | Global Coverage | Best For |
|----------|-----------|-------------|-----------|-----------------|----------|
| Finnhub | ✓ | 60/min | ✓ | ✓ | Best overall free tier |
| Alpha Vantage | 1-min | 500/day | ✗ | Limited | Technical analysis |
| Polygon.io | ✓ (paid) | 5/min | ✓ (paid) | US only | Low latency needs |
| Yahoo Finance | ✓ | None* | ✗ | ✓ | Quick prototypes |
| IEX Cloud | 15-min | 50k/month | ✓ | US only | Reliable data |
| Twelve Data | ✓ | 800/day | ✓ | ✓ | Global coverage |

*Unofficial API - use at your own risk