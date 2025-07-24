#!/usr/bin/env python3
"""
Mock API Server for Neural Trader testing.
Provides mock responses for external data providers like Alpaca, Finnhub, etc.
"""
import asyncio
import json
import random
import time
from datetime import datetime, timedelta
from typing import Dict, List, Optional

import uvicorn
from fastapi import FastAPI, HTTPException, Query, Request
from fastapi.responses import JSONResponse
from faker import Faker

fake = Faker()
fake.seed_instance(42)

app = FastAPI(
    title="Neural Trader Mock API Server",
    description="Mock external API endpoints for testing",
    version="1.0.0"
)

# Global state for maintaining realistic data consistency
SYMBOL_PRICES = {
    "AAPL": 150.25,
    "MSFT": 280.50,
    "GOOGL": 2150.75,
    "AMZN": 135.80,
    "NVDA": 420.90,
    "TSLA": 250.30,
    "META": 185.40,
    "NFLX": 400.60
}

LAST_UPDATE = {}


class MockResponseGenerator:
    """Generates realistic mock responses for different providers."""
    
    @staticmethod
    def update_price(symbol: str, volatility: float = 0.01) -> float:
        """Update symbol price with random walk."""
        if symbol not in SYMBOL_PRICES:
            SYMBOL_PRICES[symbol] = random.uniform(50, 500)
        
        # Random walk with mean reversion
        current_price = SYMBOL_PRICES[symbol]
        change_percent = random.uniform(-volatility, volatility)
        new_price = current_price * (1 + change_percent)
        
        # Ensure price stays positive and reasonable
        new_price = max(new_price, current_price * 0.5)
        new_price = min(new_price, current_price * 2.0)
        
        SYMBOL_PRICES[symbol] = new_price
        LAST_UPDATE[symbol] = datetime.now()
        
        return new_price
    
    @staticmethod
    def generate_ohlc(symbol: str, base_price: float) -> Dict:
        """Generate OHLC data around base price."""
        # Small variations around base price
        variation = base_price * 0.005  # 0.5% variation
        
        open_price = base_price + random.uniform(-variation, variation)
        close_price = base_price + random.uniform(-variation, variation)
        high_price = max(open_price, close_price) + random.uniform(0, variation)
        low_price = min(open_price, close_price) - random.uniform(0, variation)
        
        return {
            "open": round(open_price, 4),
            "high": round(high_price, 4),
            "low": round(low_price, 4),
            "close": round(close_price, 4)
        }


# Health check endpoint
@app.get("/health")
async def health_check():
    return {"status": "healthy", "timestamp": datetime.now().isoformat()}


# Metrics endpoint for Prometheus
@app.get("/metrics")
async def metrics():
    """Basic Prometheus metrics."""
    metrics_text = f"""
# HELP mock_api_requests_total Total number of API requests
# TYPE mock_api_requests_total counter
mock_api_requests_total 1000

# HELP mock_api_response_time_seconds Response time in seconds
# TYPE mock_api_response_time_seconds histogram
mock_api_response_time_seconds_bucket{{le="0.1"}} 800
mock_api_response_time_seconds_bucket{{le="0.5"}} 950
mock_api_response_time_seconds_bucket{{le="1.0"}} 990
mock_api_response_time_seconds_bucket{{le="+Inf"}} 1000
mock_api_response_time_seconds_count 1000
mock_api_response_time_seconds_sum 250.5

# HELP mock_api_active_symbols Number of active symbols
# TYPE mock_api_active_symbols gauge
mock_api_active_symbols {len(SYMBOL_PRICES)}
"""
    return metrics_text.strip()


# =============================================================================
# Alpaca Mock Endpoints
# =============================================================================

@app.get("/alpaca/v2/stocks/{symbol}/quotes/latest")
async def alpaca_latest_quote(symbol: str):
    """Mock Alpaca latest quote endpoint."""
    price = MockResponseGenerator.update_price(symbol)
    
    return {
        "quote": {
            "timeframe": "1Min",
            "symbol": symbol,
            "timestamp": datetime.now().isoformat(),
            "bid": price - 0.01,
            "ask": price + 0.01,
            "bid_size": random.randint(100, 1000),
            "ask_size": random.randint(100, 1000)
        }
    }


@app.get("/alpaca/v2/stocks/{symbol}/bars")
async def alpaca_bars(
    symbol: str,
    timeframe: str = "1Min",
    start: Optional[str] = None,
    end: Optional[str] = None,
    limit: int = 100
):
    """Mock Alpaca bars endpoint."""
    base_price = MockResponseGenerator.update_price(symbol)
    bars = []
    
    # Generate historical bars
    current_time = datetime.now()
    if start:
        try:
            current_time = datetime.fromisoformat(start.replace('Z', ''))
        except:
            current_time = datetime.now() - timedelta(hours=1)
    
    for i in range(min(limit, 100)):
        bar_time = current_time + timedelta(minutes=i)
        ohlc = MockResponseGenerator.generate_ohlc(symbol, base_price)
        
        bars.append({
            "t": bar_time.isoformat(),
            "o": ohlc["open"],
            "h": ohlc["high"],
            "l": ohlc["low"],
            "c": ohlc["close"],
            "v": random.randint(1000, 100000),
            "n": random.randint(10, 500),
            "vw": (ohlc["high"] + ohlc["low"]) / 2
        })
    
    return {
        "bars": bars,
        "symbol": symbol,
        "next_page_token": None
    }


# =============================================================================
# Finnhub Mock Endpoints  
# =============================================================================

@app.get("/finnhub/api/v1/quote")
async def finnhub_quote(symbol: str):
    """Mock Finnhub quote endpoint."""
    price = MockResponseGenerator.update_price(symbol)
    
    return {
        "c": price,  # current price
        "h": price * 1.02,  # high
        "l": price * 0.98,  # low
        "o": price * 0.999,  # open
        "pc": price * 1.001,  # previous close
        "t": int(time.time())  # timestamp
    }


@app.get("/finnhub/api/v1/news-sentiment")
async def finnhub_news_sentiment(symbol: str):
    """Mock Finnhub news sentiment endpoint."""
    return {
        "buzz": {
            "articlesInLastWeek": random.randint(10, 100),
            "buzz": random.uniform(0.5, 2.0),
            "weeklyAverage": random.uniform(0.5, 1.5)
        },
        "companyNewsScore": random.uniform(-1, 1),
        "sectorAverageBullishPercent": random.uniform(0.4, 0.8),
        "sectorAverageNewsScore": random.uniform(-0.5, 0.5),
        "sentiment": {
            "bearishPercent": random.uniform(0.1, 0.4),
            "bullishPercent": random.uniform(0.4, 0.7)
        },
        "symbol": symbol
    }


# =============================================================================
# Alpha Vantage Mock Endpoints
# =============================================================================

@app.get("/alphavantage/query")
async def alpha_vantage_query(
    function: str,
    symbol: str,
    interval: Optional[str] = "1min",
    apikey: Optional[str] = None
):
    """Mock Alpha Vantage query endpoint."""
    if function == "TIME_SERIES_INTRADAY":
        base_price = MockResponseGenerator.update_price(symbol)
        time_series = {}
        
        # Generate last 100 data points
        current_time = datetime.now()
        for i in range(100):
            timestamp = current_time - timedelta(minutes=i)
            ohlc = MockResponseGenerator.generate_ohlc(symbol, base_price)
            
            time_series[timestamp.strftime("%Y-%m-%d %H:%M:%S")] = {
                "1. open": f"{ohlc['open']:.4f}",
                "2. high": f"{ohlc['high']:.4f}",
                "3. low": f"{ohlc['low']:.4f}",
                "4. close": f"{ohlc['close']:.4f}",
                "5. volume": str(random.randint(1000, 100000))
            }
        
        return {
            f"Time Series ({interval})": time_series,
            "Meta Data": {
                "1. Information": f"Intraday ({interval}) open, high, low, close prices and volume",
                "2. Symbol": symbol,
                "3. Last Refreshed": current_time.strftime("%Y-%m-%d %H:%M:%S"),
                "4. Interval": interval,
                "5. Output Size": "Compact",
                "6. Time Zone": "US/Eastern"
            }
        }
    
    elif function == "GLOBAL_QUOTE":
        price = MockResponseGenerator.update_price(symbol)
        
        return {
            "Global Quote": {
                "01. symbol": symbol,
                "02. open": f"{price * 0.999:.4f}",
                "03. high": f"{price * 1.002:.4f}",
                "04. low": f"{price * 0.998:.4f}",
                "05. price": f"{price:.4f}",
                "06. volume": str(random.randint(1000000, 10000000)),
                "07. latest trading day": datetime.now().strftime("%Y-%m-%d"),
                "08. previous close": f"{price * 1.001:.4f}",
                "09. change": f"{price * random.uniform(-0.01, 0.01):.4f}",
                "10. change percent": f"{random.uniform(-1, 1):.2f}%"
            }
        }
    
    return {"Error Message": "Invalid API call"}


# =============================================================================
# Polygon Mock Endpoints
# =============================================================================

@app.get("/polygon/v2/aggs/ticker/{symbol}/range/{multiplier}/{timespan}/{from_date}/{to_date}")
async def polygon_aggregates(
    symbol: str,
    multiplier: int,
    timespan: str,
    from_date: str,
    to_date: str
):
    """Mock Polygon aggregates endpoint."""
    base_price = MockResponseGenerator.update_price(symbol)
    results = []
    
    # Generate 50 data points
    for i in range(50):
        timestamp = int(time.time() * 1000) - (i * 60000)  # milliseconds
        ohlc = MockResponseGenerator.generate_ohlc(symbol, base_price)
        
        results.append({
            "c": ohlc["close"],  # close
            "h": ohlc["high"],   # high
            "l": ohlc["low"],    # low
            "o": ohlc["open"],   # open
            "t": timestamp,      # timestamp
            "v": random.randint(1000, 50000),  # volume
            "vw": (ohlc["high"] + ohlc["low"]) / 2,  # volume weighted average
            "n": random.randint(10, 100)  # number of transactions
        })
    
    return {
        "ticker": symbol,
        "status": "OK",
        "request_id": fake.uuid4(),
        "count": len(results),
        "results": results
    }


# =============================================================================
# News API Mock Endpoints
# =============================================================================

@app.get("/newsapi/v2/everything")
async def news_api_everything(
    q: Optional[str] = None,
    sources: Optional[str] = None,
    language: str = "en",
    sortBy: str = "publishedAt",
    pageSize: int = 20
):
    """Mock News API everything endpoint."""
    articles = []
    
    symbols = q.split(' OR ') if q else ["AAPL", "MSFT", "GOOGL"]
    
    for symbol in symbols[:3]:  # Limit to 3 symbols
        for _ in range(random.randint(2, 4)):
            sentiment_score = random.uniform(-1, 1)
            
            # Generate sentiment-appropriate headlines
            if sentiment_score > 0.3:
                title = f"{symbol} shows strong performance in latest earnings report"
                description = f"Positive developments for {symbol} as company exceeds expectations"
            elif sentiment_score < -0.3:
                title = f"Analysts express concerns over {symbol} future prospects"
                description = f"Recent market conditions raise questions about {symbol} strategy"
            else:
                title = f"{symbol} maintains steady market position amid volatility"
                description = f"Market analysts provide mixed outlook for {symbol} stock"
            
            articles.append({
                "source": {
                    "id": fake.slug(),
                    "name": fake.company()
                },
                "author": fake.name(),
                "title": title,
                "description": description,
                "url": fake.url(),
                "urlToImage": fake.image_url(),
                "publishedAt": fake.date_time_between(
                    start_date="-3d", end_date="now"
                ).isoformat(),
                "content": fake.text(max_nb_chars=200),
                "sentiment_score": round(sentiment_score, 2),
                "confidence": round(random.uniform(0.6, 0.95), 2)
            })
    
    return {
        "status": "ok",
        "totalResults": len(articles),
        "articles": articles
    }


# =============================================================================
# Reddit Mock Endpoints (simplified)
# =============================================================================

@app.get("/reddit/api/v1/search")
async def reddit_search(
    q: str,
    limit: int = 25,
    sort: str = "relevance",
    t: str = "day"
):
    """Mock Reddit search endpoint."""
    posts = []
    
    for _ in range(random.randint(5, limit)):
        sentiment_score = random.uniform(-1, 1)
        
        posts.append({
            "id": fake.uuid4(),
            "title": fake.sentence(nb_words=8),
            "selftext": fake.text(max_nb_chars=300),
            "score": random.randint(-50, 200),
            "num_comments": random.randint(0, 100),
            "created_utc": time.time() - random.randint(0, 86400),
            "subreddit": "investing",
            "author": fake.user_name(),
            "url": fake.url(),
            "sentiment_score": round(sentiment_score, 2),
            "confidence": round(random.uniform(0.5, 0.9), 2)
        })
    
    return {
        "kind": "Listing",
        "data": {
            "children": [{"kind": "t3", "data": post} for post in posts],
            "after": fake.uuid4(),
            "before": None
        }
    }


# =============================================================================
# Error simulation and testing utilities
# =============================================================================

@app.get("/test/simulate-error")
async def simulate_error(
    error_type: str = "500",
    delay: float = 0
):
    """Simulate different types of errors for testing."""
    if delay > 0:
        await asyncio.sleep(delay)
    
    if error_type == "500":
        raise HTTPException(status_code=500, detail="Internal server error")
    elif error_type == "429":
        raise HTTPException(status_code=429, detail="Rate limit exceeded")
    elif error_type == "404":
        raise HTTPException(status_code=404, detail="Not found")
    elif error_type == "timeout":
        await asyncio.sleep(30)  # Simulate timeout
    
    return {"error": "Unknown error type"}


@app.get("/test/status")
async def test_status():
    """Get status of mock server for testing."""
    return {
        "status": "running",
        "active_symbols": len(SYMBOL_PRICES),
        "symbol_prices": SYMBOL_PRICES,
        "last_updates": {k: v.isoformat() for k, v in LAST_UPDATE.items()},
        "uptime_seconds": time.time() - 1640995200  # Mock uptime
    }


if __name__ == "__main__":
    uvicorn.run(
        "mock_api_server:app",
        host="0.0.0.0",
        port=8000,
        log_level="info",
        reload=False
    )