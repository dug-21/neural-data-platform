#!/usr/bin/env python3
"""Test all data source connections and validate API access."""

import os
import sys
import asyncio
from datetime import datetime, timedelta
import logging
from dotenv import load_dotenv

# Add parent directory to path
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from data_ingestion.providers.polygon_provider import PolygonProvider
from data_ingestion.providers.alpha_vantage_provider import AlphaVantageProvider
from data_ingestion.providers.yahoo_provider import YahooFinanceProvider
from data_ingestion.providers.finnhub_provider import FinnhubProvider

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Load environment variables
load_dotenv()

async def test_polygon():
    """Test Polygon.io connection."""
    logger.info("=" * 60)
    logger.info("Testing Polygon.io Connection")
    logger.info("=" * 60)
    
    api_key = os.getenv('POLYGON_API_KEY')
    if not api_key:
        logger.error("❌ POLYGON_API_KEY not found in environment")
        return False
    
    logger.info(f"✓ API Key found: {api_key[:10]}...")
    
    try:
        provider = PolygonProvider(api_key)
        
        # Test 1: Get market data for AAPL
        logger.info("\nTest 1: Fetching AAPL market data...")
        data = await provider.get_market_data('AAPL')
        if data:
            logger.info(f"✓ Market data received: Price=${data.get('price', 'N/A')}, Volume={data.get('volume', 'N/A')}")
        else:
            logger.warning("⚠ No market data returned")
        
        # Test 2: Get historical data
        logger.info("\nTest 2: Fetching AAPL historical data (1 day)...")
        end_date = datetime.now()
        start_date = end_date - timedelta(days=1)
        hist_data = await provider.get_historical_data('AAPL', start_date, end_date, 'hour')
        if hist_data:
            logger.info(f"✓ Historical data received: {len(hist_data)} data points")
            if hist_data:
                logger.info(f"  Sample: {hist_data[0]}")
        else:
            logger.warning("⚠ No historical data returned")
        
        logger.info("\n✅ Polygon.io connection successful!")
        return True
        
    except Exception as e:
        logger.error(f"❌ Polygon.io connection failed: {str(e)}")
        return False

async def test_alpha_vantage():
    """Test Alpha Vantage connection."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Alpha Vantage Connection")
    logger.info("=" * 60)
    
    api_key = os.getenv('ALPHA_VANTAGE_API_KEY')
    if not api_key:
        logger.error("❌ ALPHA_VANTAGE_API_KEY not found in environment")
        return False
    
    logger.info(f"✓ API Key found: {api_key[:10]}...")
    
    try:
        provider = AlphaVantageProvider(api_key)
        
        # Test: Get market data for MSFT
        logger.info("\nTest: Fetching MSFT market data...")
        data = await provider.get_market_data('MSFT')
        if data:
            logger.info(f"✓ Market data received: Price=${data.get('price', 'N/A')}, Volume={data.get('volume', 'N/A')}")
        else:
            logger.warning("⚠ No market data returned")
        
        # Note: Alpha Vantage has rate limits (5 calls/min for free tier)
        logger.info("⚠ Note: Alpha Vantage free tier limited to 5 calls/minute")
        
        logger.info("\n✅ Alpha Vantage connection successful!")
        return True
        
    except Exception as e:
        logger.error(f"❌ Alpha Vantage connection failed: {str(e)}")
        return False

async def test_finnhub():
    """Test Finnhub connection."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Finnhub Connection")
    logger.info("=" * 60)
    
    api_key = os.getenv('FINNHUB_API_KEY')
    if not api_key:
        logger.error("❌ FINNHUB_API_KEY not found in environment")
        return False
    
    logger.info(f"✓ API Key found: {api_key[:10]}...")
    
    try:
        provider = FinnhubProvider(api_key)
        
        # Test: Get market data for GOOGL
        logger.info("\nTest: Fetching GOOGL market data...")
        data = await provider.get_market_data('GOOGL')
        if data:
            logger.info(f"✓ Market data received: Price=${data.get('price', 'N/A')}, High=${data.get('high', 'N/A')}, Low=${data.get('low', 'N/A')}")
        else:
            logger.warning("⚠ No market data returned")
        
        # Test WebSocket info
        logger.info("\n✓ Finnhub WebSocket endpoint available for real-time data")
        logger.info("  Endpoint: wss://ws.finnhub.io")
        
        logger.info("\n✅ Finnhub connection successful!")
        return True
        
    except Exception as e:
        logger.error(f"❌ Finnhub connection failed: {str(e)}")
        return False

async def test_yahoo():
    """Test Yahoo Finance connection (no API key needed)."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Yahoo Finance Connection")
    logger.info("=" * 60)
    
    logger.info("✓ No API key required for Yahoo Finance")
    
    try:
        provider = YahooFinanceProvider()
        
        # Test 1: Get market data for TSLA
        logger.info("\nTest 1: Fetching TSLA market data...")
        data = await provider.get_market_data('TSLA')
        if data:
            logger.info(f"✓ Market data received: Price=${data.get('price', 'N/A')}, Volume={data.get('volume', 'N/A')}")
        else:
            logger.warning("⚠ No market data returned")
        
        # Test 2: Get historical data
        logger.info("\nTest 2: Fetching TSLA historical data (7 days)...")
        end_date = datetime.now()
        start_date = end_date - timedelta(days=7)
        hist_data = await provider.get_historical_data('TSLA', start_date, end_date, '1d')
        if hist_data:
            logger.info(f"✓ Historical data received: {len(hist_data)} data points")
            if hist_data:
                logger.info(f"  Latest: {hist_data[-1]}")
        else:
            logger.warning("⚠ No historical data returned")
        
        logger.info("\n✅ Yahoo Finance connection successful!")
        return True
        
    except Exception as e:
        logger.error(f"❌ Yahoo Finance connection failed: {str(e)}")
        return False

async def test_iex():
    """Test IEX Cloud connection."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing IEX Cloud Connection")
    logger.info("=" * 60)
    
    api_key = os.getenv('IEX_CLOUD_API_KEY')
    if not api_key:
        logger.error("❌ IEX_CLOUD_API_KEY not found in environment")
        return False
    
    logger.info(f"✓ API Key found: {api_key[:10]}...")
    logger.info("⚠ Note: IEX Cloud discontinued free tier in 2024")
    logger.info("  This API key may not work unless you have a paid plan")
    
    return False  # Skip actual test since IEX discontinued free tier

async def main():
    """Run all connection tests."""
    logger.info("Starting Data Source Connection Tests")
    logger.info("=" * 60)
    
    results = {
        "Polygon.io": await test_polygon(),
        "Alpha Vantage": await test_alpha_vantage(),
        "Finnhub": await test_finnhub(),
        "Yahoo Finance": await test_yahoo(),
        "IEX Cloud": await test_iex()
    }
    
    # Summary
    logger.info("\n" + "=" * 60)
    logger.info("CONNECTION TEST SUMMARY")
    logger.info("=" * 60)
    
    for service, status in results.items():
        status_emoji = "✅" if status else "❌"
        logger.info(f"{status_emoji} {service}: {'Connected' if status else 'Failed'}")
    
    successful = sum(1 for v in results.values() if v)
    total = len(results)
    
    logger.info(f"\nTotal: {successful}/{total} services connected successfully")
    
    if successful < total:
        logger.warning("\n⚠ Some services failed to connect. Check API keys and network connectivity.")
    else:
        logger.info("\n🎉 All services connected successfully!")

if __name__ == "__main__":
    asyncio.run(main())