# Data Provider API Key Setup Guide

## Overview

This guide walks you through obtaining API keys for each data provider used by the neural-trader platform. We recommend setting these up in the order listed below based on reliability and usefulness.

## 1. FRED (Federal Reserve Economic Data) - RECOMMENDED FIRST

**Why First?** Most reliable, no rate limits, government-backed data

1. Visit: https://fred.stlouisfed.org/docs/api/api_key.html
2. Click "Request API Key"
3. Create account (or login)
4. API key is generated immediately
5. Add to your `.env` file:
   ```
   FRED_API_KEY=your_fred_api_key_here
   ```

**Features:**
- 800,000+ economic time series
- No documented rate limits
- Professional quality data
- Free forever

## 2. Alpaca Markets - RECOMMENDED SECOND

**Why Second?** Real-time market data, WebSocket streaming, professional-grade API

1. Visit: https://alpaca.markets/
2. Click "Sign Up" (top right)
3. Complete registration (choose "Individual" account)
4. After email verification, log in to dashboard
5. Go to "Your API Keys" section
6. Generate new API key pair (Paper Trading first recommended)
7. Add to your `.env` file:
   ```
   ALPACA_API_KEY=your_api_key_here
   ALPACA_API_SECRET=your_api_secret_here
   ALPACA_SUBSCRIPTION_LEVEL=basic  # or unlimited
   ```

**Free Tier (Basic):**
- 30 WebSocket symbol subscriptions
- 200 historical API calls/minute
- Data limited to last 15 minutes
- IEX data feed only

**Paid Tier (Algo Trader Plus - $99/month):**
- Unlimited WebSocket subscriptions
- 10,000 historical API calls/minute
- Full historical data access
- SIP data feed (consolidated from all exchanges)

**Key Features:**
- Real-time stock and crypto data
- WebSocket streaming
- Historical bars and trades
- Order book snapshots
- Both REST and WebSocket APIs

## 3. NASDAQ Data Link (formerly Quandl) - RECOMMENDED THIRD

**Why Second?** Professional data quality, extensive historical data, generous free tier

1. Visit: https://data.nasdaq.com/
2. Click "Sign Up" (top right)
3. Complete registration
4. Go to Account Settings → API Key
5. Add to your `.env` file:
   ```
   QUANDL_API_KEY=your_quandl_api_key_here
   ```

**Free Tier Limits:**
- 50,000 requests per day
- 300 requests per 10 seconds
- 2,000 requests per 10 minutes
- Access to many free datasets including:
  - WIKI: 3000+ US stocks EOD prices
  - FRED: Federal Reserve economic data
  - ECB: European Central Bank data
  - CHRIS: Continuous futures data

**Key Features:**
- Historical stock prices (EOD)
- Economic indicators
- Futures and commodities
- Foreign exchange rates
- Easy-to-use API with multiple data formats

## 4. Reddit API - RECOMMENDED FOURTH

**Why Third?** Good for sentiment analysis, reasonable rate limits

1. Visit: https://www.reddit.com/prefs/apps
2. Click "Create App" or "Create Another App"
3. Fill in:
   - Name: neural-trader-bot (or your choice)
   - Type: Select "script" for personal use
   - Description: Personal trading data collection
   - About URL: (leave blank)
   - Redirect URI: http://localhost:8080 (required but not used)
4. Click "Create app"
5. Note your credentials:
   - Client ID: Under "personal use script" (short string)
   - Client Secret: The longer string
6. Add to your `.env` file:
   ```
   REDDIT_CLIENT_ID=your_client_id_here
   REDDIT_CLIENT_SECRET=your_client_secret_here
   REDDIT_USER_AGENT=neural-trader/1.0 by YourRedditUsername
   ```

**Rate Limits:**
- 60 requests per minute (authenticated)
- Must include User-Agent header

## 5. Yahoo Finance - USE WITH CAUTION

**Note:** Yahoo Finance doesn't provide official API keys. The yfinance library uses web scraping.

**No API key needed**, but be aware:
- Rate limiting is aggressive (few hundred requests/day)
- IP bans are possible with excessive use
- Best for testing/development only

**Configuration:**
```
# No API key needed, but add rate limiting config
YAHOO_MAX_REQUESTS_PER_DAY=200
YAHOO_REQUEST_DELAY_SECONDS=5
```

## 6. NewsAPI - VERY LIMITED FREE TIER

**Why Last?** Only 100 requests/day on free tier

1. Visit: https://newsapi.org/register
2. Complete registration
3. API key sent to email
4. Add to your `.env` file:
   ```
   NEWSAPI_KEY=your_newsapi_key_here
   ```

**Free Tier Limitations:**
- Only 100 requests per day
- 24-hour delay on articles
- Development use only (no production)

**Alternative (Better Free Tier):**
Consider Finnhub instead: https://finnhub.io/
- 60 API calls/minute
- Real-time news
- No daily limit

## Environment Setup

Create or update your `.env` file with all API keys:

```bash
# Economic Data
FRED_API_KEY=your_fred_api_key_here

# Market Data
ALPACA_API_KEY=your_alpaca_api_key_here
ALPACA_API_SECRET=your_alpaca_api_secret_here
ALPACA_SUBSCRIPTION_LEVEL=basic  # basic or unlimited
QUANDL_API_KEY=your_quandl_api_key_here

# Social Sentiment
REDDIT_CLIENT_ID=your_reddit_client_id
REDDIT_CLIENT_SECRET=your_reddit_client_secret
REDDIT_USER_AGENT=neural-trader/1.0 by YourUsername

# News (if using)
NEWSAPI_KEY=your_newsapi_key_here

# Rate Limiting Configuration
MAX_REQUESTS_PER_MINUTE=60
YAHOO_MAX_REQUESTS_PER_DAY=200
YAHOO_REQUEST_DELAY_SECONDS=5
```

## Testing Your API Keys

After setting up your `.env` file, run:

```bash
python data_ingestion/test_connections.py
```

This will verify each API key without consuming your quotas.

## Security Notes

1. **NEVER** commit your `.env` file to git
2. Add `.env` to your `.gitignore` file
3. Use environment variables in production
4. Rotate API keys periodically
5. Monitor usage to stay within limits

## Troubleshooting

### FRED API Issues
- Ensure API key is activated (check email)
- No special characters in the key

### Reddit API Issues
- User-Agent must be descriptive
- Client ID vs Secret confusion (ID is shorter)
- Use "script" type for personal use

### NASDAQ/Quandl Issues
- Some datasets require subscription
- Check if using correct endpoint

### Yahoo Finance Issues
- If blocked, wait 24 hours
- Use VPN as last resort
- Consider switching to official data provider

## Next Steps

Once API keys are configured:
1. Run connection tests
2. Start with FRED provider implementation
3. Add providers incrementally
4. Monitor rate limits closely