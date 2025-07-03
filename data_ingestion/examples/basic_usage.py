#!/usr/bin/env python3
"""
Basic usage example for the Neural Trader Data Ingestion Service.

This script demonstrates how to:
1. Fetch historical data
2. Stream real-time data
3. Process and store data
4. Query stored data
"""

import asyncio
import os
from datetime import datetime, timedelta
from pathlib import Path

# Add parent directory to path
import sys
sys.path.append(str(Path(__file__).parent.parent.parent))

from data_ingestion import (
    YahooFinanceProvider,
    FinnhubProvider,
    DataCleaner,
    DataValidator,
    DataTransformer,
    TimescaleDB,
    RedisStore
)
from data_ingestion.config import get_settings


async def example_historical_data():
    """Example: Fetch and process historical data."""
    print("\n=== Historical Data Example ===")
    
    # Initialize provider
    provider = YahooFinanceProvider()
    await provider.connect()
    
    # Fetch data for multiple symbols
    symbols = ['AAPL', 'MSFT', 'GOOGL']
    end_time = datetime.now()
    start_time = end_time - timedelta(days=30)
    
    print(f"Fetching 30 days of data for {symbols}")
    
    all_data = []
    async for market_data in provider.get_market_data(
        symbols, start_time, end_time, interval='1day'
    ):
        all_data.append(market_data.__dict__)
    
    print(f"Fetched {len(all_data)} data points")
    
    # Process data
    cleaner = DataCleaner()
    validator = DataValidator()
    
    # Clean data
    cleaned_data = cleaner.clean_market_data(all_data)
    print(f"Cleaned data: {len(cleaned_data)} records remain")
    
    # Validate data
    valid_data, invalid_data = validator.validate_batch(cleaned_data)
    print(f"Valid: {len(valid_data)}, Invalid: {len(invalid_data)}")
    
    # Transform data (add technical indicators)
    if valid_data:
        import pandas as pd
        df = pd.DataFrame(valid_data)
        
        transformer = DataTransformer()
        df_with_indicators = transformer.add_technical_indicators(df)
        
        print("\nTechnical indicators added:")
        print(df_with_indicators[['symbol', 'close', 'rsi', 'macd']].tail())
    
    await provider.disconnect()


async def example_real_time_streaming():
    """Example: Stream real-time data."""
    print("\n=== Real-Time Streaming Example ===")
    
    # Use Finnhub for WebSocket streaming
    provider = FinnhubProvider()
    await provider.connect()
    
    # Stream data for a few symbols
    symbols = ['AAPL', 'MSFT']
    print(f"Starting real-time stream for {symbols}")
    print("Streaming for 30 seconds...")
    
    # Set up Redis for real-time storage
    redis = RedisStore()
    await redis.connect()
    
    # Stream with timeout
    count = 0
    start_time = asyncio.get_event_loop().time()
    
    try:
        async for market_data in provider.stream_market_data(symbols):
            # Store in Redis
            await redis.set_latest_price(
                market_data.symbol,
                {
                    'price': market_data.close,
                    'volume': market_data.volume,
                    'time': market_data.time.isoformat()
                }
            )
            
            count += 1
            print(f"Received: {market_data.symbol} @ ${market_data.close}")
            
            # Stop after 30 seconds
            if asyncio.get_event_loop().time() - start_time > 30:
                break
    
    except Exception as e:
        print(f"Streaming error: {e}")
    
    print(f"Received {count} real-time updates")
    
    # Get latest prices from cache
    print("\nLatest cached prices:")
    for symbol in symbols:
        price_data = await redis.get_latest_price(symbol)
        if price_data:
            print(f"{symbol}: ${price_data.get('price', 'N/A')}")
    
    await redis.disconnect()
    await provider.disconnect()


async def example_multi_provider_aggregation():
    """Example: Aggregate data from multiple providers."""
    print("\n=== Multi-Provider Aggregation Example ===")
    
    from data_ingestion.processors import DataAggregator
    
    # Initialize multiple providers
    providers = {
        'yahoo': YahooFinanceProvider(),
        'finnhub': FinnhubProvider()
    }
    
    # Connect all providers
    for provider in providers.values():
        await provider.connect()
    
    # Fetch data from each provider
    symbol = 'AAPL'
    end_time = datetime.now()
    start_time = end_time - timedelta(days=1)
    
    provider_data = {}
    
    for name, provider in providers.items():
        print(f"Fetching from {name}...")
        data = []
        
        try:
            async for market_data in provider.get_market_data(
                [symbol], start_time, end_time, interval='1hour'
            ):
                data.append(market_data.__dict__)
            
            if data:
                import pandas as pd
                provider_data[name] = pd.DataFrame(data)
                print(f"{name}: {len(data)} records")
        except Exception as e:
            print(f"{name} error: {e}")
    
    # Aggregate data
    if len(provider_data) > 1:
        aggregator = DataAggregator()
        
        # Try different aggregation methods
        for method in ['priority', 'average', 'consensus']:
            print(f"\nAggregation method: {method}")
            aggregated = aggregator.merge_market_data(provider_data, method=method)
            
            if not aggregated.empty:
                print(aggregated[['time', 'symbol', 'close', 'provider']].head())
    
    # Disconnect all providers
    for provider in providers.values():
        await provider.disconnect()


async def example_database_operations():
    """Example: Store and query data from TimescaleDB."""
    print("\n=== Database Operations Example ===")
    
    # Initialize database
    db = TimescaleDB()
    await db.connect()
    
    # Fetch some data
    provider = YahooFinanceProvider()
    await provider.connect()
    
    symbol = 'AAPL'
    end_time = datetime.now()
    start_time = end_time - timedelta(days=7)
    
    print(f"Fetching 7 days of data for {symbol}")
    
    data = []
    async for market_data in provider.get_market_data(
        [symbol], start_time, end_time, interval='1hour'
    ):
        data.append(market_data.__dict__)
    
    print(f"Storing {len(data)} records in TimescaleDB")
    
    # Store data
    if data:
        await db.insert_market_data(data)
        
        # Query back the data
        print("\nQuerying stored data...")
        df = await db.query_market_data(
            symbol,
            start_time,
            end_time
        )
        
        print(f"Retrieved {len(df)} records")
        print("\nFirst few records:")
        print(df.head())
        
        # Get latest price
        latest = await db.get_latest_price(symbol)
        if latest:
            print(f"\nLatest price: ${latest['price']} at {latest['time']}")
    
    await provider.disconnect()
    await db.disconnect()


async def main():
    """Run all examples."""
    # Check if we have necessary environment variables
    settings = get_settings()
    
    print("Neural Trader Data Ingestion Examples")
    print("=" * 50)
    
    # Run examples
    try:
        await example_historical_data()
        
        # Only run streaming example if we have API key
        if settings.finnhub_api_key and settings.finnhub_api_key != "free":
            await example_real_time_streaming()
        else:
            print("\n[Skipping real-time example - no Finnhub API key]")
        
        await example_multi_provider_aggregation()
        
        # Only run database example if TimescaleDB is available
        try:
            await example_database_operations()
        except Exception as e:
            print(f"\n[Skipping database example - {e}]")
            
    except KeyboardInterrupt:
        print("\nExamples interrupted by user")


if __name__ == "__main__":
    # Run the examples
    asyncio.run(main())