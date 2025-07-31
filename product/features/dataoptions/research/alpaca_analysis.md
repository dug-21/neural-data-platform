# Alpaca Markets - Market Data Analysis

## Executive Summary

Alpaca Markets offers a competitive market data solution with a generous free tier and affordable paid options. Their data API is separate from their commission-free trading services, making them suitable for both data-only and integrated trading applications.

## Pricing Structure

### Basic Plan (Free)
- **Cost**: $0/month
- **API Rate Limit**: 200 calls/minute
- **WebSocket Symbols**: 30 concurrent subscriptions
- **Historical Data**: 7+ years available, but limited to latest 15 minutes via API
- **Data Source**: IEX exchange only for equities
- **Options Data**: Indicative pricing only
- **Use Case**: Development, testing, small-scale applications

### Algo Trader Plus Plan
- **Cost**: $99/month
- **API Rate Limit**: 10,000 calls/minute (50x increase)
- **WebSocket Symbols**: Unlimited subscriptions
- **Historical Data**: Full 7+ years access without restrictions
- **Data Sources**: All major US exchanges (CTA/UTP streams - 100% market volume)
- **Options Data**: Full OPRA feed
- **Crypto Data**: Included
- **Use Case**: Professional trading, high-frequency strategies, production systems

### Broker API Partner Plan
- **Cost**: Custom pricing
- **Features**: Tailored for commercial redistribution
- **API Limits**: Up to 10,000 calls/minute
- **Symbols**: Unlimited
- **Use Case**: Building broker platforms, commercial applications

## WebSocket Implementation

### Capabilities
- **Protocol**: WebSocket for real-time streaming
- **Latency**: Sub-20ms advertised
- **Data Types**: Trades, quotes, bars (minute/hour/daily aggregates)
- **Architecture**: Event-based streaming

### Limitations by Plan
- **Basic**: 30 concurrent symbol subscriptions
- **Algo Trader Plus**: Unlimited subscriptions
- **Connection Limits**: Not explicitly stated in documentation

## Data Quality and Sources

### Equity Data
- **Primary Sources**: Direct feeds from exchanges
- **Coverage**: 
  - Basic: IEX only (limited market coverage)
  - Paid: Full CTA/UTP consolidated tape (100% US market volume)
- **Data Types**: Real-time trades, quotes, and aggregated bars

### Options Data
- **Source**: OPRA (Options Price Reporting Authority)
- **Basic Plan**: Indicative pricing only
- **Paid Plan**: Full OPRA feed with all options exchanges

### Crypto Data
- **Availability**: Included in Algo Trader Plus
- **Coverage**: Major cryptocurrencies
- **Real-time**: Yes

## Real-time vs Delayed Data

### Real-time Access
- **Basic Plan**: Real-time for IEX exchange only
- **Paid Plans**: Real-time for all supported exchanges
- **No Delay Option**: Alpaca focuses on real-time data; no delayed feed mentioned

### Historical Data Access
- **Storage**: 7+ years of historical data
- **Basic Plan Restriction**: API access limited to latest 15 minutes
- **Paid Plans**: Full historical access without time restrictions

## API Features and Limits

### REST API
- **Languages**: Python, Node.js, Go, C# SDKs
- **Authentication**: API key/secret for trading API, HTTP Basic for broker API
- **Rate Limits**: 
  - Basic: 200/minute
  - Algo Trader Plus: 10,000/minute

### Data Types Available
- Real-time quotes
- Trades
- Aggregated bars (1min, 5min, 15min, 1hour, 1day)
- Historical data queries
- Corporate actions
- News data (mentioned but details not provided)

## Trading Account Integration

### Synergies
- **Unified API**: Same credentials for trading and data
- **Commission-Free Trading**: No per-trade costs
- **Paper Trading**: Available for testing strategies
- **Seamless Integration**: Data API designed to work with trading API

### Benefits for Autonomous Trading
- Low latency between data receipt and order placement
- No need to manage multiple vendor relationships
- Consistent data format across trading and analysis

## Pros and Cons for Autonomous Trading

### Pros
1. **Generous Free Tier**: 30 WebSocket symbols sufficient for focused strategies
2. **Affordable Paid Plan**: $99/month for unlimited data is competitive
3. **Low Latency**: Sub-20ms streaming suitable for real-time trading
4. **API-First Design**: Built for algorithmic trading
5. **No Per-Symbol Charges**: Unlimited symbols on paid plan
6. **Integrated Trading**: Seamless data-to-execution pipeline
7. **Modern Architecture**: WebSocket streaming, multiple SDKs
8. **Crypto Included**: No extra cost for cryptocurrency data

### Cons
1. **Free Tier Exchange Limitation**: IEX-only data misses significant market activity
2. **Historical Data Restriction**: 15-minute limit on free tier problematic for backtesting
3. **US Market Focus**: Limited to US equities and options
4. **No SIP Option**: No explicit SIP vs consolidated feed choice
5. **WebSocket Symbol Limit**: 30 symbols on free tier may be restrictive
6. **No Forex/Futures**: Limited to stocks, options, and crypto

## Recommendation for Neural Trader

### For Development Phase
The **Basic (Free) plan** is suitable for:
- Initial development and testing
- Proof of concept with limited symbols
- Learning the API architecture

### For Production Deployment
The **Algo Trader Plus ($99/month)** is recommended because:
- Unlimited symbols allow diversified strategies
- Full exchange coverage ensures accurate market representation
- 10,000 API calls/minute supports high-frequency operations
- Full historical data enables proper backtesting
- Cost-effective compared to traditional market data vendors

### Integration Strategy
1. Start with free tier for development
2. Upgrade to Algo Trader Plus for backtesting (need full historical data)
3. Maintain paid plan for production trading
4. Consider Broker API plan only if redistributing data to clients

## Conclusion

Alpaca Markets offers a compelling market data solution for autonomous trading systems. The $99/month Algo Trader Plus plan provides professional-grade data at a fraction of traditional vendor costs. The tight integration with their commission-free trading platform makes them particularly attractive for retail algorithmic traders and small funds. The main limitation is the US-only market coverage, which may require additional vendors for international strategies.