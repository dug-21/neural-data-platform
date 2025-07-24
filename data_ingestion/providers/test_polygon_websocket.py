"""Test script for Polygon WebSocket integration."""
import asyncio
import logging
from datetime import datetime, timedelta

from polygon import PolygonProvider
from config import get_settings

# Set up logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)


async def test_websocket_streaming():
    """Test WebSocket streaming with a few symbols."""
    # Initialize provider
    provider = PolygonProvider()
    
    try:
        # Connect
        print("Connecting to Polygon...")
        await provider.connect()
        
        # Test symbols
        symbols = ["AAPL", "MSFT", "GOOGL"]
        
        print(f"Starting WebSocket stream for: {symbols}")
        print("Streaming data (press Ctrl+C to stop)...")
        
        # Track statistics
        message_count = 0
        start_time = datetime.now()
        
        # Stream data
        async for data in provider.stream_market_data_ws(symbols):
            message_count += 1
            
            # Display data
            print(f"\n[{message_count}] {data.symbol} @ {data.time}")
            print(f"  OHLC: ${data.open:.2f} / ${data.high:.2f} / ${data.low:.2f} / ${data.close:.2f}")
            print(f"  Volume: {data.volume:,}")
            if data.metadata:
                print(f"  Metadata: {data.metadata}")
            
            # Show stats every 10 messages
            if message_count % 10 == 0:
                elapsed = (datetime.now() - start_time).total_seconds()
                rate = message_count / elapsed if elapsed > 0 else 0
                print(f"\n📊 Stats: {message_count} messages, {rate:.1f} msg/sec")
                
                # Get provider stats
                stats = provider.get_stats()
                print(f"  WebSocket State: {stats['state']}")
                print(f"  Buffer Size: {stats['buffer_size']}")
                print(f"  Errors: {stats['errors']}")
                print(f"  Fallback Active: {stats['fallback_active']}")
                
    except KeyboardInterrupt:
        print("\n\nStopping stream...")
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        # Disconnect
        await provider.disconnect()
        print("Disconnected.")


async def test_historical_data():
    """Test historical data retrieval with 1-minute aggregates."""
    provider = PolygonProvider()
    
    try:
        # Connect
        print("Connecting to Polygon...")
        await provider.connect()
        
        # Get last 2 hours of 1-minute data
        end_time = datetime.now()
        start_time = end_time - timedelta(hours=2)
        symbols = ["AAPL"]
        
        print(f"\nFetching 1-minute data for {symbols[0]}")
        print(f"From: {start_time}")
        print(f"To: {end_time}")
        
        data_points = []
        async for data in provider.get_market_data(symbols, start_time, end_time, "1min"):
            data_points.append(data)
        
        print(f"\nRetrieved {len(data_points)} data points")
        
        if data_points:
            # Show first and last few
            print("\nFirst 3 data points:")
            for dp in data_points[:3]:
                print(f"  {dp.time}: ${dp.close:.2f} (vol: {dp.volume:,})")
            
            print("\nLast 3 data points:")
            for dp in data_points[-3:]:
                print(f"  {dp.time}: ${dp.close:.2f} (vol: {dp.volume:,})")
                
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        await provider.disconnect()


async def test_fallback_behavior():
    """Test WebSocket fallback to polling."""
    provider = PolygonProvider()
    
    try:
        # Connect
        print("Connecting to Polygon...")
        await provider.connect()
        
        # Force fallback by setting failed state
        provider._ws_state = provider.ConnectionState.FAILED
        provider._use_fallback = True
        
        symbols = ["AAPL"]
        print(f"\nTesting fallback polling for: {symbols}")
        print("Should poll every minute...")
        
        message_count = 0
        async for data in provider.stream_market_data_ws(symbols):
            message_count += 1
            print(f"\n[Fallback] {data.symbol} @ {data.time}")
            print(f"  Price: ${data.close:.2f}, Volume: {data.volume:,}")
            
            if message_count >= 3:
                print("\nFallback test complete.")
                break
                
    except Exception as e:
        print(f"Error: {e}")
        import traceback
        traceback.print_exc()
    finally:
        await provider.disconnect()


async def main():
    """Run all tests."""
    print("=" * 60)
    print("Polygon WebSocket Integration Test")
    print("=" * 60)
    
    # Check if API key is configured
    settings = get_settings()
    if not settings.polygon_api_key:
        print("\n❌ ERROR: POLYGON_API_KEY not set in environment")
        print("Please set your Polygon API key:")
        print("  export POLYGON_API_KEY='your-api-key'")
        return
    
    print("\n1. Testing Historical Data Retrieval")
    print("-" * 40)
    await test_historical_data()
    
    print("\n\n2. Testing WebSocket Streaming")
    print("-" * 40)
    await test_websocket_streaming()
    
    print("\n\n3. Testing Fallback Behavior")
    print("-" * 40)
    await test_fallback_behavior()
    
    print("\n\n✅ All tests completed!")


if __name__ == "__main__":
    asyncio.run(main())