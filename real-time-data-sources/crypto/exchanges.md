# Cryptocurrency Exchange Real-Time Data Sources

## Binance WebSocket Streams ⭐⭐⭐⭐⭐
**Best Free Real-Time Crypto Data**

### Overview
- **WebSocket Base**: `wss://stream.binance.com:9443`
- **Documentation**: https://binance-docs.github.io/apidocs/spot/en/#websocket-market-streams
- **Update Frequency**: Real-time (sub-second)
- **Coverage**: 1500+ trading pairs

### Free Access Details
- **No API key required** for public market data
- Unlimited connections (reasonable use)
- All market data streams available
- No message rate limits
- 24-hour rolling window

### Stream Types

#### Individual Symbol Streams
```javascript
// Trade Stream
const tradeWs = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@trade');

// Kline/Candlestick Stream
const klineWs = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@kline_1m');

// Individual Symbol Ticker Stream
const tickerWs = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@ticker');

// Partial Book Depth Stream
const depthWs = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@depth20@100ms');
```

#### Combined Streams
```javascript
// Multiple streams in one connection
const combinedWs = new WebSocket('wss://stream.binance.com:9443/stream?streams=btcusdt@trade/ethusdt@trade/bnbusdt@trade');

combinedWs.onmessage = (event) => {
    const packet = JSON.parse(event.data);
    const stream = packet.stream;  // e.g., "btcusdt@trade"
    const data = packet.data;      // actual trade data
};
```

#### All Market Tickers Stream
```javascript
// All symbols ticker updates (~1 second)
const allTickersWs = new WebSocket('wss://stream.binance.com:9443/ws/!ticker@arr');

allTickersWs.onmessage = (event) => {
    const tickers = JSON.parse(event.data);
    tickers.forEach(ticker => {
        console.log(`${ticker.s}: ${ticker.c}`);  // Symbol: Current price
    });
};
```

### Complete Implementation Example
```javascript
class BinanceWebSocket {
    constructor(symbols = ['BTCUSDT', 'ETHUSDT']) {
        this.symbols = symbols;
        this.streams = {
            trades: {},
            orderBook: {},
            klines: {}
        };
        this.reconnectDelay = 5000;
        this.connect();
    }

    connect() {
        // Create combined stream URL
        const streams = [];
        this.symbols.forEach(symbol => {
            const s = symbol.toLowerCase();
            streams.push(`${s}@trade`);
            streams.push(`${s}@depth20@100ms`);
            streams.push(`${s}@kline_1m`);
        });
        
        const url = `wss://stream.binance.com:9443/stream?streams=${streams.join('/')}`;
        this.ws = new WebSocket(url);
        
        this.ws.onopen = () => {
            console.log('Binance WebSocket connected');
            this.reconnectDelay = 5000;
        };
        
        this.ws.onmessage = (event) => {
            const packet = JSON.parse(event.data);
            this.handleMessage(packet);
        };
        
        this.ws.onerror = (error) => {
            console.error('WebSocket error:', error);
        };
        
        this.ws.onclose = () => {
            console.log('WebSocket disconnected, reconnecting...');
            setTimeout(() => this.connect(), this.reconnectDelay);
            this.reconnectDelay = Math.min(this.reconnectDelay * 2, 30000);
        };
    }
    
    handleMessage(packet) {
        const { stream, data } = packet;
        const [symbol, streamType] = stream.split('@');
        
        if (streamType === 'trade') {
            this.handleTrade(symbol.toUpperCase(), data);
        } else if (streamType.startsWith('depth')) {
            this.handleOrderBook(symbol.toUpperCase(), data);
        } else if (streamType.startsWith('kline')) {
            this.handleKline(symbol.toUpperCase(), data);
        }
    }
    
    handleTrade(symbol, trade) {
        // Process trade data
        console.log(`Trade ${symbol}: ${trade.p} @ ${trade.q}`);
    }
    
    handleOrderBook(symbol, depth) {
        // Process order book update
        this.streams.orderBook[symbol] = {
            bids: depth.bids,
            asks: depth.asks,
            lastUpdateId: depth.lastUpdateId
        };
    }
    
    handleKline(symbol, data) {
        // Process candlestick data
        const kline = data.k;
        console.log(`Kline ${symbol}: O:${kline.o} H:${kline.h} L:${kline.l} C:${kline.c}`);
    }
}

// Usage
const binance = new BinanceWebSocket(['BTCUSDT', 'ETHUSDT', 'BNBUSDT']);
```

---

## Coinbase WebSocket Feed ⭐⭐⭐⭐⭐
**Most Reliable for USD Pairs**

### Overview
- **WebSocket URL**: `wss://ws-feed.exchange.coinbase.com`
- **Documentation**: https://docs.cloud.coinbase.com/exchange/docs/websocket-overview
- **Update Frequency**: Real-time
- **Coverage**: Major crypto/USD pairs

### Free Access Details
- No authentication for public channels
- All market data free
- Professional-grade reliability
- Used by many trading firms

### Channel Types
```javascript
const ws = new WebSocket('wss://ws-feed.exchange.coinbase.com');

ws.on('open', () => {
    // Subscribe to multiple channels
    ws.send(JSON.stringify({
        type: 'subscribe',
        product_ids: ['BTC-USD', 'ETH-USD', 'SOL-USD'],
        channels: [
            'ticker',      // Real-time price updates
            'matches',     // Real-time trades
            'level2',      // Order book
            'heartbeat'    // Connection health
        ]
    }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    
    switch(msg.type) {
        case 'ticker':
            console.log(`${msg.product_id}: $${msg.price}`);
            break;
        case 'match':
            console.log(`Trade: ${msg.size} @ $${msg.price}`);
            break;
        case 'l2update':
            // Order book update
            break;
    }
});
```

### Advanced Features
```javascript
// Authenticated feed for account updates (requires API key)
const crypto = require('crypto');

function signMessage(timestamp, method, path, body = '') {
    const message = timestamp + method + path + body;
    return crypto.createHmac('sha256', API_SECRET)
        .update(message)
        .digest('base64');
}

// Subscribe with authentication
const timestamp = Date.now() / 1000;
const signature = signMessage(timestamp, 'GET', '/users/self/verify');

ws.send(JSON.stringify({
    type: 'subscribe',
    product_ids: ['BTC-USD'],
    channels: ['user'],
    signature,
    key: API_KEY,
    passphrase: API_PASSPHRASE,
    timestamp
}));
```

---

## CoinCap Real-Time API ⭐⭐⭐⭐
**Aggregated Market Data**

### Overview
- **WebSocket URL**: `wss://ws.coincap.io`
- **REST API**: https://api.coincap.io/v2/
- **Documentation**: https://docs.coincap.io/
- **Coverage**: 1000+ cryptocurrencies

### WebSocket Streams
```javascript
// Price updates for specific assets
const pricesWs = new WebSocket('wss://ws.coincap.io/prices?assets=bitcoin,ethereum,cardano');

pricesWs.onmessage = (event) => {
    const prices = JSON.parse(event.data);
    // {"bitcoin":"45234.5932104328",...}
    Object.entries(prices).forEach(([asset, price]) => {
        console.log(`${asset}: $${parseFloat(price).toFixed(2)}`);
    });
};

// All trades across exchanges
const tradesWs = new WebSocket('wss://ws.coincap.io/trades/binance');

tradesWs.onmessage = (event) => {
    const trade = JSON.parse(event.data);
    console.log(`${trade.base}/${trade.quote}: ${trade.price} (${trade.exchange})`);
};
```

### REST API Example
```javascript
// Get real-time rates
const response = await fetch('https://api.coincap.io/v2/rates');
const data = await response.json();

// Get specific asset
const btc = await fetch('https://api.coincap.io/v2/assets/bitcoin');
const btcData = await btc.json();
```

---

## Kraken WebSocket API ⭐⭐⭐⭐

### Overview
- **WebSocket URL**: `wss://ws.kraken.com`
- **Documentation**: https://docs.kraken.com/websockets/
- **Update Frequency**: Real-time
- **Coverage**: 200+ pairs

### Public Channels
```javascript
const ws = new WebSocket('wss://ws.kraken.com');

ws.on('open', () => {
    // Subscribe to ticker
    ws.send(JSON.stringify({
        event: 'subscribe',
        pair: ['XBT/USD', 'ETH/USD'],
        subscription: { name: 'ticker' }
    }));
    
    // Subscribe to trades
    ws.send(JSON.stringify({
        event: 'subscribe',
        pair: ['XBT/USD'],
        subscription: { name: 'trade' }
    }));
    
    // Subscribe to order book
    ws.send(JSON.stringify({
        event: 'subscribe',
        pair: ['XBT/USD'],
        subscription: { name: 'book', depth: 10 }
    }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    if (Array.isArray(msg)) {
        const [channelID, payload, channelName, pair] = msg;
        console.log(`${channelName} update for ${pair}:`, payload);
    }
});
```

---

## Bitfinex WebSocket API ⭐⭐⭐

### Overview
- **WebSocket URL**: `wss://api-pub.bitfinex.com/ws/2`
- **Documentation**: https://docs.bitfinex.com/docs/ws-general
- **Update Frequency**: Real-time
- **Coverage**: 300+ pairs

### Connection Example
```javascript
const ws = new WebSocket('wss://api-pub.bitfinex.com/ws/2');

ws.on('open', () => {
    // Subscribe to ticker
    ws.send(JSON.stringify({
        event: 'subscribe',
        channel: 'ticker',
        symbol: 'tBTCUSD'
    }));
    
    // Subscribe to trades
    ws.send(JSON.stringify({
        event: 'subscribe',
        channel: 'trades',
        symbol: 'tBTCUSD'
    }));
});

ws.on('message', (data) => {
    const msg = JSON.parse(data);
    if (msg.event) return;  // Skip event messages
    
    const [channelId, payload] = msg;
    // Process based on channel ID
});
```

---

## Free Cryptocurrency Data Aggregators

### CoinGecko API ⭐⭐⭐
- **REST API**: https://api.coingecko.com/api/v3/
- **Free Tier**: 30 calls/minute
- **Coverage**: 13,000+ cryptocurrencies
- **Note**: No official WebSocket (REST only)

```javascript
// Simple price endpoint
const url = 'https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum&vs_currencies=usd&include_24hr_change=true';
const response = await fetch(url);
const prices = await response.json();
```

### CoinMarketCap ⭐⭐⭐
- **REST API**: Requires API key
- **Free Tier**: 333 calls/day
- **Coverage**: 20,000+ cryptocurrencies
- **Note**: More restricted than others

---

## Multi-Exchange WebSocket Manager

```javascript
class CryptoWebSocketManager {
    constructor() {
        this.exchanges = {
            binance: {
                url: 'wss://stream.binance.com:9443/ws',
                subscribed: new Set()
            },
            coinbase: {
                url: 'wss://ws-feed.exchange.coinbase.com',
                subscribed: new Set()
            },
            kraken: {
                url: 'wss://ws.kraken.com',
                subscribed: new Set()
            }
        };
        
        this.handlers = {
            trade: [],
            orderbook: [],
            ticker: []
        };
    }
    
    connectExchange(exchange) {
        const config = this.exchanges[exchange];
        const ws = new WebSocket(config.url);
        
        ws.on('open', () => {
            console.log(`Connected to ${exchange}`);
            this.resubscribe(exchange, ws);
        });
        
        ws.on('message', (data) => {
            this.routeMessage(exchange, JSON.parse(data));
        });
        
        ws.on('close', () => {
            setTimeout(() => this.connectExchange(exchange), 5000);
        });
        
        config.ws = ws;
    }
    
    subscribe(exchange, symbol, channels) {
        const config = this.exchanges[exchange];
        
        if (exchange === 'binance') {
            channels.forEach(channel => {
                const stream = `${symbol.toLowerCase()}@${channel}`;
                config.ws.send(JSON.stringify({
                    method: 'SUBSCRIBE',
                    params: [stream],
                    id: Date.now()
                }));
                config.subscribed.add(stream);
            });
        } else if (exchange === 'coinbase') {
            config.ws.send(JSON.stringify({
                type: 'subscribe',
                product_ids: [symbol],
                channels: channels
            }));
        }
        // Add other exchanges...
    }
    
    onTrade(callback) {
        this.handlers.trade.push(callback);
    }
    
    routeMessage(exchange, data) {
        // Route to appropriate handlers based on exchange and message type
        // Implementation depends on exchange message format
    }
}

// Usage
const manager = new CryptoWebSocketManager();
manager.connectExchange('binance');
manager.connectExchange('coinbase');

manager.onTrade((trade) => {
    console.log(`${trade.exchange} ${trade.symbol}: ${trade.price} @ ${trade.amount}`);
});

manager.subscribe('binance', 'BTCUSDT', ['trade', 'depth20']);
manager.subscribe('coinbase', 'BTC-USD', ['matches', 'ticker']);
```

---

## Best Practices

### 1. Connection Management
- Implement automatic reconnection
- Use exponential backoff
- Monitor heartbeat/ping messages
- Handle connection limits

### 2. Message Parsing
- Validate message format
- Handle malformed data gracefully
- Use message queues for processing
- Implement rate limiting

### 3. Data Normalization
```javascript
class ExchangeNormalizer {
    normalizeTrade(exchange, data) {
        switch(exchange) {
            case 'binance':
                return {
                    exchange: 'binance',
                    symbol: data.s,
                    price: parseFloat(data.p),
                    amount: parseFloat(data.q),
                    timestamp: data.T,
                    side: data.m ? 'sell' : 'buy'
                };
            case 'coinbase':
                return {
                    exchange: 'coinbase',
                    symbol: data.product_id.replace('-', ''),
                    price: parseFloat(data.price),
                    amount: parseFloat(data.size),
                    timestamp: new Date(data.time).getTime(),
                    side: data.side
                };
            // Add other exchanges
        }
    }
}
```

### 4. Error Handling
```javascript
ws.on('error', (error) => {
    console.error(`WebSocket error: ${error.message}`);
    // Don't close connection on error
});

ws.on('unexpected-response', (request, response) => {
    console.error(`Unexpected response: ${response.statusCode}`);
});
```

---

## Comparison Matrix

| Exchange | WebSocket | Auth Required | Rate Limits | Best For |
|----------|-----------|---------------|-------------|----------|
| Binance | ✓ | No (public) | None* | Most pairs, best free access |
| Coinbase | ✓ | No (public) | None | USD pairs, reliability |
| CoinCap | ✓ | No | None | Aggregated data |
| Kraken | ✓ | No (public) | Reasonable | European markets |
| Bitfinex | ✓ | No (public) | Reasonable | Advanced trading data |

*Reasonable use expected

## Legal Considerations
- Read exchange terms of service
- Respect rate limits even if not enforced
- Don't redistribute real-time data
- Consider data licensing for commercial use