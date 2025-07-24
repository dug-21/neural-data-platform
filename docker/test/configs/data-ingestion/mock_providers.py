"""
Mock data providers for testing data ingestion.
These providers generate realistic but predictable test data.
"""
import asyncio
import json
import random
import time
from datetime import datetime, timedelta
from decimal import Decimal
from typing import Dict, List, Optional

import pandas as pd
from faker import Faker

fake = Faker()
fake.seed_instance(12345)  # Ensure reproducible test data


class MockAlpacaProvider:
    """Mock Alpaca provider for testing."""
    
    def __init__(self):
        self.api_key = "test_alpaca_key"
        self.secret = "test_alpaca_secret"
        self.base_url = "http://mock-api-server:8000/alpaca"
        self._symbols = ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA"]
        self._last_prices = {
            "AAPL": 150.00,
            "MSFT": 280.00,
            "GOOGL": 2150.00,
            "AMZN": 135.00,
            "NVDA": 420.00
        }
    
    async def get_market_data(self, symbol: str, timeframe: str = "1Min") -> Dict:
        """Generate mock market data for a symbol."""
        current_price = self._last_prices.get(symbol, 100.00)
        
        # Simulate price movement
        change_percent = random.uniform(-0.02, 0.02)  # ±2% change
        new_price = current_price * (1 + change_percent)
        self._last_prices[symbol] = new_price
        
        volume = random.randint(100000, 2000000)
        
        return {
            "symbol": symbol,
            "timestamp": datetime.now().isoformat(),
            "open": float(current_price),
            "high": float(max(current_price, new_price) * 1.005),
            "low": float(min(current_price, new_price) * 0.995),
            "close": float(new_price),
            "volume": volume,
            "provider": "alpaca_mock"
        }
    
    async def get_historical_data(self, symbol: str, start_date: str, end_date: str) -> List[Dict]:
        """Generate mock historical data."""
        data = []
        start = datetime.fromisoformat(start_date.replace('Z', ''))
        end = datetime.fromisoformat(end_date.replace('Z', ''))
        
        current_time = start
        base_price = self._last_prices.get(symbol, 100.00)
        
        while current_time <= end:
            # Random walk price generation
            change = random.uniform(-0.01, 0.01)
            base_price *= (1 + change)
            
            data.append({
                "symbol": symbol,
                "timestamp": current_time.isoformat(),
                "open": float(base_price * 0.999),
                "high": float(base_price * 1.002),
                "low": float(base_price * 0.998),
                "close": float(base_price),
                "volume": random.randint(50000, 1000000),
                "provider": "alpaca_mock"
            })
            
            current_time += timedelta(minutes=1)
        
        return data


class MockFinnhubProvider:
    """Mock Finnhub provider for testing."""
    
    def __init__(self):
        self.api_key = "test_finnhub_key"
        self.base_url = "http://mock-api-server:8000/finnhub"
    
    async def get_quote(self, symbol: str) -> Dict:
        """Generate mock quote data."""
        base_price = random.uniform(50, 500)
        
        return {
            "c": base_price,  # current price
            "h": base_price * 1.02,  # high
            "l": base_price * 0.98,  # low
            "o": base_price * 0.999,  # open
            "pc": base_price * 1.001,  # previous close
            "t": int(time.time()),  # timestamp
            "symbol": symbol,
            "provider": "finnhub_mock"
        }
    
    async def get_news_sentiment(self, symbol: str) -> Dict:
        """Generate mock sentiment data."""
        return {
            "symbol": symbol,
            "sentiment": random.uniform(-1, 1),
            "confidence": random.uniform(0.5, 1.0),
            "news_count": random.randint(5, 50),
            "timestamp": datetime.now().isoformat(),
            "provider": "finnhub_mock"
        }


class MockAlphaVantageProvider:
    """Mock Alpha Vantage provider for testing."""
    
    def __init__(self):
        self.api_key = "test_alpha_vantage_key"
        self.base_url = "http://mock-api-server:8000/alphavantage"
    
    async def get_intraday_data(self, symbol: str, interval: str = "1min") -> Dict:
        """Generate mock intraday data."""
        data = {}
        current_time = datetime.now()
        base_price = random.uniform(100, 300)
        
        # Generate last 100 data points
        for i in range(100):
            timestamp = current_time - timedelta(minutes=i)
            price = base_price * (1 + random.uniform(-0.02, 0.02))
            
            data[timestamp.strftime("%Y-%m-%d %H:%M:%S")] = {
                "1. open": f"{price:.4f}",
                "2. high": f"{price * 1.001:.4f}",
                "3. low": f"{price * 0.999:.4f}",
                "4. close": f"{price:.4f}",
                "5. volume": str(random.randint(1000, 100000))
            }
        
        return {
            f"Time Series ({interval})": data,
            "Meta Data": {
                "1. Information": f"Intraday ({interval}) open, high, low, close prices and volume",
                "2. Symbol": symbol,
                "3. Last Refreshed": current_time.strftime("%Y-%m-%d %H:%M:%S"),
                "4. Interval": interval,
                "5. Output Size": "Compact",
                "6. Time Zone": "US/Eastern"
            }
        }


class MockPolygonProvider:
    """Mock Polygon provider for testing."""
    
    def __init__(self):
        self.api_key = "test_polygon_key"
        self.base_url = "http://mock-api-server:8000/polygon"
    
    async def get_aggregates(self, symbol: str, timespan: str = "minute", 
                           multiplier: int = 1, from_date: str = None, 
                           to_date: str = None) -> Dict:
        """Generate mock aggregate data."""
        results = []
        base_price = random.uniform(50, 400)
        
        # Generate 50 data points
        for i in range(50):
            timestamp = int(time.time() * 1000) - (i * 60000)  # milliseconds
            price = base_price * (1 + random.uniform(-0.015, 0.015))
            
            results.append({
                "c": price,  # close
                "h": price * 1.005,  # high
                "l": price * 0.995,  # low
                "o": price * 1.001,  # open
                "t": timestamp,  # timestamp
                "v": random.randint(1000, 50000),  # volume
                "vw": price * random.uniform(0.999, 1.001),  # volume weighted average
                "n": random.randint(10, 100)  # number of transactions
            })
        
        return {
            "ticker": symbol,
            "status": "OK",
            "request_id": fake.uuid4(),
            "count": len(results),
            "results": results
        }


class MockNewsProvider:
    """Mock news provider for testing sentiment data."""
    
    def __init__(self):
        self.api_key = "test_news_key"
        self.base_url = "http://mock-api-server:8000/news"
    
    async def get_market_news(self, symbols: List[str] = None) -> Dict:
        """Generate mock news articles."""
        articles = []
        
        for symbol in (symbols or ["AAPL", "MSFT", "GOOGL"]):
            # Generate 3-5 articles per symbol
            for _ in range(random.randint(3, 5)):
                sentiment_score = random.uniform(-1, 1)
                
                # Generate sentiment-appropriate headlines
                if sentiment_score > 0.3:
                    headline = f"{symbol} shows strong performance in latest earnings"
                elif sentiment_score < -0.3:
                    headline = f"Concerns grow over {symbol} market position"
                else:
                    headline = f"{symbol} maintains steady market presence"
                
                articles.append({
                    "title": headline,
                    "description": fake.text(max_nb_chars=200),
                    "url": fake.url(),
                    "source": fake.company(),
                    "publishedAt": fake.date_time_between(
                        start_date="-7d", end_date="now"
                    ).isoformat(),
                    "sentiment_score": sentiment_score,
                    "confidence": random.uniform(0.6, 0.95),
                    "symbol": symbol,
                    "provider": "news_mock"
                })
        
        return {
            "status": "ok",
            "totalResults": len(articles),
            "articles": articles
        }


class TestDataGenerator:
    """Utility class for generating comprehensive test datasets."""
    
    def __init__(self):
        self.providers = {
            "alpaca": MockAlpacaProvider(),
            "finnhub": MockFinnhubProvider(),
            "alphavantage": MockAlphaVantageProvider(),
            "polygon": MockPolygonProvider(),
            "news": MockNewsProvider()
        }
    
    async def generate_market_data_batch(self, symbols: List[str], 
                                       duration_minutes: int = 60) -> List[Dict]:
        """Generate a batch of market data for testing."""
        all_data = []
        
        for symbol in symbols:
            for provider_name, provider in self.providers.items():
                if hasattr(provider, 'get_market_data'):
                    try:
                        data = await provider.get_market_data(symbol)
                        data['test_id'] = f"{symbol}_{provider_name}_{int(time.time())}"
                        all_data.append(data)
                    except Exception as e:
                        print(f"Error generating data for {symbol} from {provider_name}: {e}")
                        continue
        
        return all_data
    
    def save_test_fixtures(self, data: List[Dict], filename: str):
        """Save test data as fixtures."""
        import os
        fixtures_dir = "/test-fixtures/generated"
        os.makedirs(fixtures_dir, exist_ok=True)
        
        filepath = os.path.join(fixtures_dir, filename)
        with open(filepath, 'w') as f:
            json.dump(data, f, indent=2, default=str)
        
        print(f"Test fixtures saved to {filepath}")
    
    async def generate_comprehensive_test_data(self) -> Dict:
        """Generate comprehensive test data for all testing scenarios."""
        symbols = ["AAPL", "MSFT", "GOOGL", "AMZN", "NVDA"]
        
        # Generate different types of test data
        market_data = await self.generate_market_data_batch(symbols)
        news_data = await self.providers["news"].get_market_news(symbols)
        
        test_dataset = {
            "metadata": {
                "generated_at": datetime.now().isoformat(),
                "symbols": symbols,
                "total_records": len(market_data) + len(news_data.get("articles", [])),
                "test_version": "1.0"
            },
            "market_data": market_data,
            "news_data": news_data,
            "sentiment_data": [
                {
                    "symbol": symbol,
                    "sentiment_score": random.uniform(-1, 1),
                    "confidence": random.uniform(0.5, 1.0),
                    "timestamp": datetime.now().isoformat(),
                    "source": "mock_sentiment",
                    "provider": "test"
                }
                for symbol in symbols
            ]
        }
        
        return test_dataset


# Export mock providers for use in tests
__all__ = [
    'MockAlpacaProvider',
    'MockFinnhubProvider', 
    'MockAlphaVantageProvider',
    'MockPolygonProvider',
    'MockNewsProvider',
    'TestDataGenerator'
]