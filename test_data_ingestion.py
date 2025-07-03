#!/usr/bin/env python3
"""
Simple test script to verify data ingestion APIs work correctly.
Tests each data provider to see if they can connect and pull sample data.
"""

import os
import sys
import asyncio
import aiohttp
import yfinance as yf
from datetime import datetime, timedelta
import json


def test_yahoo_finance():
    """Test Yahoo Finance data provider"""
    print("🔍 Testing Yahoo Finance...")
    try:
        # Get Apple stock data for the last 5 days
        ticker = yf.Ticker("AAPL")
        data = ticker.history(period="5d")
        
        if not data.empty:
            latest_price = data['Close'].iloc[-1]
            print(f"✅ Yahoo Finance: Successfully fetched AAPL data")
            print(f"   Latest Close Price: ${latest_price:.2f}")
            print(f"   Data Points: {len(data)} days")
            return True
        else:
            print("❌ Yahoo Finance: No data returned")
            return False
            
    except Exception as e:
        print(f"❌ Yahoo Finance Error: {e}")
        return False


async def test_alpha_vantage():
    """Test Alpha Vantage API (if API key available)"""
    print("\n🔍 Testing Alpha Vantage...")
    
    api_key = os.getenv('ALPHA_VANTAGE_API_KEY')
    if not api_key:
        print("⚠️  Alpha Vantage: No API key found (ALPHA_VANTAGE_API_KEY)")
        return False
        
    try:
        url = f"https://www.alphavantage.co/query"
        params = {
            'function': 'GLOBAL_QUOTE',
            'symbol': 'AAPL',
            'apikey': api_key
        }
        
        async with aiohttp.ClientSession() as session:
            async with session.get(url, params=params) as response:
                if response.status == 200:
                    data = await response.json()
                    
                    if 'Global Quote' in data:
                        quote = data['Global Quote']
                        price = quote.get('05. price', 'N/A')
                        print(f"✅ Alpha Vantage: Successfully fetched AAPL quote")
                        print(f"   Current Price: ${price}")
                        return True
                    else:
                        print(f"❌ Alpha Vantage: Unexpected response format: {data}")
                        return False
                else:
                    print(f"❌ Alpha Vantage: HTTP {response.status}")
                    return False
                    
    except Exception as e:
        print(f"❌ Alpha Vantage Error: {e}")
        return False


async def test_polygon():
    """Test Polygon.io API (if API key available)"""
    print("\n🔍 Testing Polygon.io...")
    
    api_key = os.getenv('POLYGON_API_KEY')
    if not api_key:
        print("⚠️  Polygon.io: No API key found (POLYGON_API_KEY)")
        return False
        
    try:
        # Get yesterday's date for the API call
        yesterday = (datetime.now() - timedelta(days=1)).strftime('%Y-%m-%d')
        
        url = f"https://api.polygon.io/v1/open-close/AAPL/{yesterday}"
        params = {'apikey': api_key}
        
        async with aiohttp.ClientSession() as session:
            async with session.get(url, params=params) as response:
                if response.status == 200:
                    data = await response.json()
                    
                    if data.get('status') == 'OK':
                        open_price = data.get('open', 'N/A')
                        close_price = data.get('close', 'N/A')
                        print(f"✅ Polygon.io: Successfully fetched AAPL data for {yesterday}")
                        print(f"   Open: ${open_price}, Close: ${close_price}")
                        return True
                    else:
                        print(f"❌ Polygon.io: {data.get('message', 'Unknown error')}")
                        return False
                else:
                    print(f"❌ Polygon.io: HTTP {response.status}")
                    return False
                    
    except Exception as e:
        print(f"❌ Polygon.io Error: {e}")
        return False


async def test_finnhub():
    """Test Finnhub API (if API key available)"""
    print("\n🔍 Testing Finnhub...")
    
    api_key = os.getenv('FINNHUB_API_KEY')
    if not api_key:
        print("⚠️  Finnhub: No API key found (FINNHUB_API_KEY)")
        return False
        
    try:
        url = "https://finnhub.io/api/v1/quote"
        params = {
            'symbol': 'AAPL',
            'token': api_key
        }
        
        async with aiohttp.ClientSession() as session:
            async with session.get(url, params=params) as response:
                if response.status == 200:
                    data = await response.json()
                    
                    if 'c' in data:  # 'c' is current price
                        current_price = data['c']
                        high = data.get('h', 'N/A')
                        low = data.get('l', 'N/A')
                        print(f"✅ Finnhub: Successfully fetched AAPL quote")
                        print(f"   Current: ${current_price}, High: ${high}, Low: ${low}")
                        return True
                    else:
                        print(f"❌ Finnhub: Unexpected response format: {data}")
                        return False
                else:
                    print(f"❌ Finnhub: HTTP {response.status}")
                    return False
                    
    except Exception as e:
        print(f"❌ Finnhub Error: {e}")
        return False


async def test_iex_cloud():
    """Test IEX Cloud API (if API key available)"""
    print("\n🔍 Testing IEX Cloud...")
    
    api_key = os.getenv('IEX_CLOUD_API_KEY')
    if not api_key:
        print("⚠️  IEX Cloud: No API key found (IEX_CLOUD_API_KEY)")
        return False
        
    try:
        url = f"https://cloud.iexapis.com/stable/stock/AAPL/quote"
        params = {'token': api_key}
        
        async with aiohttp.ClientSession() as session:
            async with session.get(url, params=params) as response:
                if response.status == 200:
                    data = await response.json()
                    
                    if 'latestPrice' in data:
                        latest_price = data['latestPrice']
                        company_name = data.get('companyName', 'N/A')
                        print(f"✅ IEX Cloud: Successfully fetched {company_name} quote")
                        print(f"   Latest Price: ${latest_price}")
                        return True
                    else:
                        print(f"❌ IEX Cloud: Unexpected response format: {data}")
                        return False
                else:
                    print(f"❌ IEX Cloud: HTTP {response.status}")
                    return False
                    
    except Exception as e:
        print(f"❌ IEX Cloud Error: {e}")
        return False


async def test_database_connections():
    """Test database connections (PostgreSQL and Redis)"""
    print("\n🔍 Testing Database Connections...")
    
    # Test PostgreSQL connection
    try:
        import asyncpg
        
        db_url = "postgresql://postgres:dev_password@localhost:5432/neural_trader"
        conn = await asyncpg.connect(db_url)
        
        # Simple query to test connection
        result = await conn.fetchval("SELECT version()")
        await conn.close()
        
        print(f"✅ PostgreSQL: Connected successfully")
        print(f"   Version: {result.split(',')[0]}")
        
    except Exception as e:
        print(f"❌ PostgreSQL Error: {e}")
    
    # Test Redis connection
    try:
        import redis.asyncio as redis
        
        redis_client = redis.from_url("redis://localhost:6379")
        await redis_client.ping()
        
        # Test set/get
        await redis_client.set("test_key", "test_value", ex=10)
        value = await redis_client.get("test_key")
        await redis_client.delete("test_key")
        await redis_client.close()
        
        print(f"✅ Redis: Connected successfully")
        print(f"   Test operation: {value.decode() if value else 'Failed'}")
        
    except Exception as e:
        print(f"❌ Redis Error: {e}")


async def main():
    """Run all data ingestion tests"""
    print("🚀 Neural Trader - Data Ingestion API Test")
    print("=" * 50)
    
    results = []
    
    # Test free APIs first (no API key required)
    print("\n📊 Testing Free Data Sources:")
    results.append(test_yahoo_finance())
    
    # Test paid APIs (require API keys)
    print("\n💳 Testing Paid Data Sources:")
    results.extend(await asyncio.gather(
        test_alpha_vantage(),
        test_polygon(),
        test_finnhub(),
        test_iex_cloud(),
        return_exceptions=True
    ))
    
    # Test database connections
    await test_database_connections()
    
    # Summary
    print("\n" + "=" * 50)
    print("📋 Test Summary:")
    
    working_apis = sum(1 for r in results if r is True)
    total_apis = len([r for r in results if r is not None])
    
    print(f"✅ Working APIs: {working_apis}/{total_apis}")
    
    if working_apis == 0:
        print("⚠️  No data sources are working. Check API keys and network connectivity.")
    elif working_apis < total_apis:
        print("⚠️  Some data sources failed. Check API keys for paid services.")
    else:
        print("🎉 All tested data sources are working!")
    
    print("\n💡 API Key Setup:")
    print("   To test paid APIs, set these environment variables:")
    print("   - ALPHA_VANTAGE_API_KEY")
    print("   - POLYGON_API_KEY") 
    print("   - FINNHUB_API_KEY")
    print("   - IEX_CLOUD_API_KEY")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n⏹️  Test interrupted by user")
    except Exception as e:
        print(f"\n💥 Unexpected error: {e}")
        sys.exit(1)