# Real-Time Data Sources Directory

A comprehensive catalog of free and low-cost real-time data feeds suitable for neural network training and autonomous trading systems. All sources listed provide consistent near real-time updates rather than static datasets.

## 🚀 NEW: Creative Multi-Source Applications

Discover innovative ways to combine multiple data sources for unique insights:
- [Multi-Source Data Fusion Applications](creative-applications/multi-source-fusion.md) - 10 creative applications combining 3-7 data sources each
- [Implementation Guide](creative-applications/implementation-guide.md) - Practical code examples and deployment strategies

## 📋 Quick Navigation

- [Financial Markets](#financial-markets)
- [Cryptocurrency](#cryptocurrency)
- [Weather & Environmental](#weather--environmental)
- [Transportation](#transportation)
- [Government & Public Data](#government--public-data)
- [Social Media & News](#social-media--news)
- [IoT & Sensors](#iot--sensors)
- [Space & Satellite](#space--satellite)
- [Energy & Utilities](#energy--utilities)
- [Sports & Gaming](#sports--gaming)

---

## Financial Markets

### 🏆 Top Recommendations

#### **Finnhub** ⭐⭐⭐⭐⭐
- **URL**: https://finnhub.io/
- **Update Frequency**: Real-time (WebSocket) / 1 second (REST)
- **Free Tier**: 60 calls/minute, unlimited WebSocket connections
- **Data Types**: Stocks, forex, crypto, economic indicators, earnings, IPOs
- **Authentication**: API key required (free registration)
- **Best For**: Most generous free tier with institutional-grade data

#### **Alpha Vantage** ⭐⭐⭐⭐
- **URL**: https://www.alphavantage.co/
- **Update Frequency**: Real-time to 1 minute intervals
- **Free Tier**: 5 API calls/minute, 500 calls/day
- **Data Types**: Stocks, forex, crypto, technical indicators (50+)
- **Authentication**: API key required
- **Best For**: Technical analysis and historical data

#### **Polygon.io** ⭐⭐⭐⭐
- **URL**: https://polygon.io/
- **Update Frequency**: Real-time (<20ms latency)
- **Free Tier**: 5 API calls/minute, end-of-day data only
- **Data Types**: Stocks, options, forex, crypto
- **Protocols**: REST + WebSocket
- **Best For**: Low-latency requirements (paid tiers)

### Other Financial Sources

- **Yahoo Finance** (unofficial APIs, no rate limits)
- **IEX Cloud** (50k messages/month free)
- **Twelve Data** (800 API calls/day free)
- **EODHD** (20 API calls/day free, "DEMO" key available)
- **Marketstack** (100 requests/month free)

---

## Cryptocurrency

### 🏆 Top Recommendations

#### **Binance WebSocket Streams** ⭐⭐⭐⭐⭐
- **URL**: wss://stream.binance.com:9443
- **Update Frequency**: Real-time (sub-second)
- **Free Tier**: Unlimited public data streams
- **Data Types**: Trades, order book, klines, tickers
- **Authentication**: None for public data
- **Example**: `wss://stream.binance.com:9443/ws/btcusdt@trade`

#### **Coinbase WebSocket Feed** ⭐⭐⭐⭐⭐
- **URL**: wss://ws-feed.exchange.coinbase.com
- **Update Frequency**: Real-time
- **Free Tier**: Unlimited public data
- **Data Types**: Matches, order book, ticker, heartbeat
- **Authentication**: None for public data
- **Best For**: USD pairs and institutional reliability

#### **CoinCap** ⭐⭐⭐⭐
- **URL**: https://coincap.io/
- **Update Frequency**: Real-time WebSocket
- **Free Tier**: Unlimited
- **Data Types**: Prices for 1000+ cryptocurrencies
- **WebSocket**: `wss://ws.coincap.io/prices?assets=bitcoin,ethereum`

### Other Crypto Sources

- **Kraken WebSocket** (public data free)
- **Bitfinex WebSocket** (public data free)
- **CoinGecko** (30 calls/min free)
- **CoinMarketCap** (333 calls/day free)
- **Bitstamp WebSocket** (public data free)

---

## Weather & Environmental

### 🏆 Top Recommendations

#### **NOAA/National Weather Service** ⭐⭐⭐⭐⭐
- **URL**: https://api.weather.gov/
- **Update Frequency**: 5-60 minutes depending on product
- **Free Tier**: Unlimited (reasonable use)
- **Data Types**: Forecasts, observations, alerts, radar
- **Authentication**: None (User-Agent header required)
- **Best For**: US weather data, completely free

#### **OpenWeatherMap** ⭐⭐⭐⭐
- **URL**: https://openweathermap.org/api
- **Update Frequency**: 10 minutes
- **Free Tier**: 1000 calls/day
- **Data Types**: Current, forecast, historical, air pollution
- **Authentication**: API key required
- **Best For**: Global coverage, easy to use

#### **PurpleAir** ⭐⭐⭐⭐
- **URL**: https://www2.purpleair.com/
- **Update Frequency**: 2 minutes
- **Free Tier**: 1 million points/day
- **Data Types**: Air quality from 10000+ sensors
- **Authentication**: API key required
- **Best For**: Real-time air quality data

### Other Environmental Sources

- **WeatherAPI** (1M calls/month free)
- **EPA AirNow** (air quality, free)
- **USGS Earthquake** (real-time seismic, free)
- **UK Environment Agency** (flood data, free)
- **NDBC Buoys** (ocean conditions, free)

---

## Transportation

### 🏆 Top Recommendations

#### **OpenSky Network** ⭐⭐⭐⭐⭐
- **URL**: https://opensky-network.org/
- **Update Frequency**: 5-10 seconds
- **Free Tier**: Anonymous access allowed
- **Data Types**: Live aircraft positions (ADS-B)
- **Authentication**: Optional (better with account)
- **Best For**: Global flight tracking

#### **GTFS Realtime Feeds** ⭐⭐⭐⭐
- **Multiple Cities**: See https://gtfs.org/realtime/
- **Update Frequency**: 10-30 seconds
- **Free Tier**: Completely free
- **Data Types**: Vehicle positions, trip updates, alerts
- **Protocol**: Protocol Buffers
- **Examples**: NYC MTA, SF Muni, London TfL

#### **Marine Traffic (AIS)** ⭐⭐⭐
- **Kystverket (Norway)**: Free AIS stream
- **Update Frequency**: Real-time
- **Data Types**: Ship positions, speed, course
- **Authentication**: Registration required
- **Protocol**: MQTT/WebSocket

### Other Transportation Sources

- **511.org** (Bay Area traffic)
- **TomTom Traffic** (limited free tier)
- **HERE Traffic** (freemium)
- **Citybikes API** (bike share data)
- **FlightAware** (limited free tier)

---

## Government & Public Data

### 🏆 Top Sources

#### **Data.gov Real-Time Feeds**
- **URL**: https://catalog.data.gov/dataset?res_format=api
- **Categories**: Multiple government departments
- **Update Frequency**: Varies by dataset
- **Authentication**: Usually none
- **Notable Feeds**:
  - FDA Drug Recalls (hourly)
  - USDA Crop Reports (daily)
  - Treasury Rates (daily)
  - Census Economic Indicators

#### **European Data Portal**
- **URL**: https://data.europa.eu/
- **Real-Time Sources**: Traffic, air quality, energy
- **Free Tier**: Completely free
- **Best For**: EU-wide standardized data

#### **NASA APIs**
- **URL**: https://api.nasa.gov/
- **Data Types**: ISS location, solar flare, asteroids
- **Update Frequency**: Minutes to hours
- **Authentication**: API key (free)

---

## Social Media & News

### 🏆 Top Recommendations

#### **Reddit API** ⭐⭐⭐⭐
- **URL**: https://www.reddit.com/dev/api/
- **Update Frequency**: Real-time streaming
- **Free Tier**: 60 requests/minute
- **Data Types**: Posts, comments, votes
- **Authentication**: OAuth2
- **Best For**: Sentiment analysis, trending topics

#### **NewsAPI** ⭐⭐⭐⭐
- **URL**: https://newsapi.org/
- **Update Frequency**: 15 minutes
- **Free Tier**: 100 requests/day
- **Data Types**: Headlines from 80k+ sources
- **Authentication**: API key required

#### **MediaStack** ⭐⭐⭐
- **URL**: https://mediastack.com/
- **Update Frequency**: Near real-time
- **Free Tier**: 500 requests/month
- **Data Types**: News from 7500+ sources

### Other Social/News Sources

- **Mastodon Streaming API** (free, federated)
- **Discord Gateway** (bot required)
- **RSS Feeds** (varies by source)
- **Hacker News API** (free, Firebase)
- **NewsData.io** (200 requests/day free)

---

## IoT & Sensors

### Community Networks

#### **The Things Network**
- **URL**: https://www.thethingsnetwork.org/
- **Protocol**: LoRaWAN, MQTT
- **Data Types**: Community IoT sensors
- **Free Tier**: Completely free
- **Use Cases**: Environmental monitoring, asset tracking

#### **Particle.io**
- **URL**: https://www.particle.io/
- **Update Frequency**: Real-time events
- **Free Tier**: 100k events/month
- **Protocols**: REST, SSE, Webhooks

#### **Adafruit IO**
- **URL**: https://io.adafruit.com/
- **Free Tier**: 30 data points/minute
- **Protocols**: MQTT, REST, WebSocket
- **Best For**: Maker projects

---

## Space & Satellite

### 🏆 Top Sources

#### **N2YO Satellite Tracking**
- **URL**: https://www.n2yo.com/api/
- **Update Frequency**: Real-time positions
- **Free Tier**: 1000 requests/day
- **Data Types**: Satellite positions, passes
- **Authentication**: API key required

#### **ISS Location Now**
- **URL**: http://api.open-notify.org/
- **Update Frequency**: Real-time
- **Free Tier**: Unlimited
- **Data Types**: ISS position, crew info
- **Authentication**: None

#### **Space-Track.org**
- **URL**: https://www.space-track.org/
- **Data Types**: TLE data, satellite catalog
- **Authentication**: Free registration
- **Best For**: Comprehensive orbital data

---

## Energy & Utilities

### Grid Data

#### **US Energy Information Admin**
- **URL**: https://www.eia.gov/opendata/
- **Update Frequency**: Hourly to daily
- **Data Types**: Electricity, petroleum, natural gas
- **Free Tier**: Completely free
- **Authentication**: API key required

#### **European Network (ENTSO-E)**
- **URL**: https://transparency.entsoe.eu/
- **Update Frequency**: 15 minutes
- **Data Types**: Grid load, generation, prices
- **Authentication**: Free registration

#### **GridStatus.io**
- **URL**: https://www.gridstatus.io/
- **Coverage**: Multiple US ISOs
- **Update Frequency**: 5 minutes
- **Free Tier**: Limited historical data

---

## Sports & Gaming

### 🏆 Top Sources

#### **The Odds API**
- **URL**: https://the-odds-api.com/
- **Update Frequency**: Real-time odds
- **Free Tier**: 500 requests/month
- **Sports**: 20+ sports covered
- **Authentication**: API key required

#### **API-Football**
- **URL**: https://www.api-football.com/
- **Update Frequency**: Live scores
- **Free Tier**: 100 requests/day
- **Coverage**: 1000+ competitions

#### **Steam Web API**
- **URL**: https://steamcommunity.com/dev
- **Data Types**: Player stats, game data
- **Update Frequency**: Real-time
- **Free Tier**: Rate limited but free

---

## 🔧 Implementation Tips

### WebSocket Connection Example
```javascript
// Binance WebSocket Example
const ws = new WebSocket('wss://stream.binance.com:9443/ws/btcusdt@trade');
ws.on('message', (data) => {
  const trade = JSON.parse(data);
  console.log(`Price: ${trade.p}, Quantity: ${trade.q}`);
});
```

### Rate Limit Best Practices
1. Implement exponential backoff
2. Cache responses when possible
3. Use WebSocket for high-frequency data
4. Batch requests where supported
5. Monitor rate limit headers

### Data Quality Considerations
- Verify data freshness with timestamps
- Implement fallback data sources
- Handle connection drops gracefully
- Validate data against schemas
- Monitor for anomalies

---

## 📝 Contributing

Found a new real-time data source? Please contribute by adding it to the appropriate category with:
- Name and URL
- Update frequency
- Free tier details
- Authentication requirements
- Best use cases

Last Updated: January 2024