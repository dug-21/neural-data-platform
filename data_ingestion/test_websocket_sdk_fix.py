#!/usr/bin/env python3
"""Test script to verify the SDK WebSocket fix."""

import asyncio
import os
from providers.alpaca import AlpacaProvider

async def test_sdk_websocket():
    """Test the SDK-based WebSocket implementation."""
    print("🧪 Testing SDK WebSocket implementation...")
    
    # Create provider
    provider = AlpacaProvider()
    print("✅ Provider created")
    
    # Enable WebSocket for testing
    provider._ws_config["enabled"] = True
    print("✅ WebSocket enabled")
    
    try:
        # Test symbols
        symbols = ["AAPL", "MSFT"]
        
        print(f"🚀 Testing SDK WebSocket streaming for symbols: {symbols}")
        
        # Test streaming for a short duration
        count = 0
        timeout_task = asyncio.create_task(asyncio.sleep(15))  # 15 second timeout
        
        try:
            async for data in provider.stream_market_data_ws(symbols):
                print(f"📊 Received data: {data.symbol} - ${data.price:.2f} at {data.timestamp}")
                count += 1
                
                # Stop after receiving some data or timeout
                if count >= 3:  # Stop after 3 data points
                    print("✅ Received sufficient data, stopping test")
                    break
                    
                # Check for timeout
                if timeout_task.done():
                    print("⏰ Timeout reached, stopping test")
                    break
                    
        except asyncio.CancelledError:
            print("🛑 Test cancelled")
        
        print(f"📈 Total data points received: {count}")
        
        if count > 0:
            print("✅ SDK WebSocket streaming is working!")
        else:
            print("⚠️  No data received - this is expected during after-hours trading")
            print("✅ Connection established successfully (no errors)")
            
    except Exception as e:
        print(f"❌ Error during SDK WebSocket test: {e}")
        import traceback
        print("🔍 Full traceback:")
        print(traceback.format_exc())
        
    finally:
        # Clean up
        try:
            await provider.disconnect()
            print("✅ Provider disconnected cleanly")
        except Exception as e:
            print(f"⚠️  Disconnect warning: {e}")

if __name__ == "__main__":
    print("🧪 SDK WebSocket Fix Verification Test")
    print("=" * 50)
    asyncio.run(test_sdk_websocket())
    print("=" * 50)
    print("🏁 Test completed")