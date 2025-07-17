# Alpaca Market Data API Analysis for Real-Time Ingestion

## Executive Summary

This document provides a comprehensive analysis of Alpaca's Market Data API with a focus on real-time streaming capabilities for the Neural Trader platform. Based on the API documentation and current implementation analysis, we identify the path to migrate from polling-based to WebSocket-based real-time data ingestion.

**Note on Minimal Integration Approach**: While this document covers the full capabilities of Alpaca's WebSocket API, the actual implementation will follow a minimal approach by simply adding WebSocket support to the existing AlpacaProvider class. This requires modifying only a single file (`data_ingestion/providers/alpaca.py`) and can be completed in 3 weeks with full backward compatibility.

## 1. Real-Time Streaming Architecture

### 1.1 WebSocket Endpoints

Alpaca provides WebSocket streaming for multiple asset classes:

- **Stocks**: `wss://stream.data.alpaca.markets/v2/{feed}`
  - IEX feed: `wss://stream.data.alpaca.markets/v2/iex`
  - SIP feed: `wss://stream.data.alpaca.markets/v2/sip`
- **Options**: `wss://stream.data.alpaca.markets/v1beta1/options`
- **Crypto**: `wss://stream.data.alpaca.markets/v1beta3/crypto/{exchange}`
- **News**: `wss://stream.data.alpaca.markets/v1beta1/news`

### 1.2 Authentication

WebSocket authentication follows a handshake protocol:

```json
{
  "action": "auth",
  "key": "YOUR_API_KEY",
  "secret": "YOUR_API_SECRET"
}
```

Response:
```json
{
  "T": "success",
  "msg": "authenticated"
}
```

## 2. Available Data Types

### 2.1 Real-Time Stock Data

| Data Type | Channel | Description | Fields |
|-----------|---------|-------------|--------|
| Trades | `trades` | Individual trade executions | `symbol`, `price`, `size`, `timestamp`, `conditions`, `exchange` |
| Quotes | `quotes` | Best bid/ask updates | `symbol`, `bid_price`, `bid_size`, `ask_price`, `ask_size`, `timestamp` |
| Bars | `bars` | Aggregated minute bars | `symbol`, `open`, `high`, `low`, `close`, `volume`, `timestamp` |
| Trade Updates | `updatedBars` | Bar corrections | Same as bars with correction flag |
| Daily Bars | `dailyBars` | End-of-day summaries | Complete OHLCV with extended hours data |
| Statuses | `statuses` | Trading halts/resumes | `symbol`, `status_code`, `status_message`, `timestamp` |
| LULDs | `lulds` | Limit Up/Limit Down | `symbol`, `limit_up_price`, `limit_down_price`, `timestamp` |

### 2.2 Advanced Data Types (SIP Feed Only)

- **Trade Corrections**: Post-trade adjustments
- **Trading Statuses**: Real-time halt and resume events
- **Auction Data**: Opening/closing auction imbalances
- **Order Imbalances**: Real-time order flow imbalances

## 3. Free vs Paid Tier Comparison

### 3.1 Free Tier (Basic - IEX Only)

**Capabilities:**
- Real-time trades and quotes from IEX exchange only
- 30 concurrent WebSocket symbol subscriptions
- 15-minute delayed historical data
- 200 API calls per minute
- Single WebSocket connection

**Limitations:**
- IEX represents only ~2-3% of total market volume
- Missing trades from major exchanges (NYSE, NASDAQ)
- No access to NBBO (National Best Bid/Offer)
- Limited historical data access

### 3.2 Paid Tier (Algo Trader Plus - $99/month)

**Enhanced Features:**
- Full market coverage (all US exchanges via SIP feed)
- Unlimited symbol subscriptions
- Real-time NBBO quotes
- No historical data restrictions
- 10,000 API calls per minute
- Multiple concurrent connections
- Access to advanced data types

## 4. Data Quality and Latency

### 4.1 Timestamp Precision
- Nanosecond precision timestamps (Unix epoch in nanoseconds)
- Exchange timestamps (when trade occurred)
- SIP timestamps (when consolidated)

### 4.2 Expected Latencies
- **IEX Feed**: ~5-10ms from exchange
- **SIP Feed**: ~20-50ms consolidated
- **Network overhead**: +10-30ms depending on location

## 5. WebSocket Protocol Details

### 5.1 Connection Flow

1. **Connect**: Establish WebSocket connection
2. **Authenticate**: Send auth message
3. **Subscribe**: Subscribe to channels and symbols
4. **Receive**: Process streaming messages
5. **Heartbeat**: Maintain connection with ping/pong

### 5.2 Subscription Format

```json
{
  "action": "subscribe",
  "trades": ["AAPL", "TSLA"],
  "quotes": ["AAPL", "TSLA"],
  "bars": ["*"]  // Subscribe to all symbols for bars
}
```

### 5.3 Message Format

Trade message example:
```json
{
  "T": "t",
  "S": "AAPL",
  "p": 150.25,
  "s": 100,
  "t": "2024-01-14T09:30:00.123456789Z",
  "c": ["@", "F"],
  "i": 12345,
  "x": "V"
}
```

## 6. Rate Limits and Quotas

### 6.1 Connection Limits
- **Free**: 1 WebSocket connection
- **Paid**: Multiple connections allowed

### 6.2 Message Rate Limits
- No explicit message rate limits
- Bandwidth-based fair usage policy
- Recommended: Implement client-side buffering

### 6.3 Subscription Limits
- **Free**: 30 symbols max
- **Paid**: Unlimited (recommended < 1000 per connection)

## 7. Integration Recommendations

### 7.1 For Neural Trader Development

**Phase 1 - Free Tier Development:**
1. Implement WebSocket connection manager
2. Test with 30 liquid symbols on IEX
3. Focus on connection reliability and data processing
4. Implement fallback to REST polling

**Phase 2 - Production Deployment:**
1. Upgrade to Algo Trader Plus for full market coverage
2. Implement multi-connection load balancing
3. Add SIP-specific data types
4. Scale to full symbol universe

### 7.2 Symbol Selection for Free Tier

Recommended symbols for IEX liquidity:
- **High Volume**: AAPL, MSFT, AMZN, GOOGL, META
- **ETFs**: SPY, QQQ, IWM, DIA, VTI
- **Volatile**: TSLA, NVDA, AMD, NFLX, SHOP
- **Sectors**: XLF, XLK, XLE, XLV, XLI

### 7.3 Architecture Considerations

1. **Connection Pooling**: Manage multiple WebSocket connections
2. **Symbol Routing**: Distribute symbols across connections
3. **Data Deduplication**: Handle duplicate messages
4. **Failover**: Automatic reconnection and resubscription
5. **Backpressure**: Handle high-volume data bursts

## 8. Cost-Benefit Analysis

### 8.1 Free Tier ROI
- **Pros**: Zero cost, sufficient for development and testing
- **Cons**: Limited market visibility, not suitable for production trading

### 8.2 Paid Tier ROI
- **Cost**: $99/month ($1,188/year)
- **Benefits**: 
  - Full market data coverage
  - Production-ready data quality
  - Unlimited scaling potential
  - Professional trading capabilities

### 8.3 Recommendation
Start with free tier for development and testing. The limitations are acceptable for building the WebSocket infrastructure. Upgrade to paid tier before production deployment for accurate market representation.

## 9. Implementation Priority

### High Priority
1. WebSocket connection manager
2. Authentication and subscription handling  
3. Message parsing and normalization
4. Error handling and reconnection logic

### Medium Priority
1. Multi-connection load balancing
2. Symbol subscription management
3. Data quality monitoring
4. Performance optimization

### Low Priority
1. Advanced data types (LULD, imbalances)
2. Options and crypto streaming
3. News feed integration
4. Historical data backfill

## 10. Conclusion

Alpaca's WebSocket API provides a robust foundation for real-time market data streaming. The free tier is sufficient for development, while the paid tier offers production-grade data quality at a competitive price point. The migration from polling to streaming will significantly reduce latency and improve the Neural Trader's ability to react to market events in real-time.