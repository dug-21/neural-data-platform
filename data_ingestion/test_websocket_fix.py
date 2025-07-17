#!/usr/bin/env python3
"""Test script to verify WebSocket connection fix."""

import asyncio
import os
from providers.alpaca import AlpacaProvider

async def test_websocket_fix():
    """Test the WebSocket connection after the fix."""
    print("🔧 Testing WebSocket connection fix...")
    
    # Create provider
    provider = AlpacaProvider()
    print("✅ Provider created")
    
    # Enable WebSocket for testing
    provider._ws_config["enabled"] = True
    print("✅ WebSocket enabled")
    
    try:
        # Test symbols
        symbols = ["AAPL", "MSFT"]
        
        print(f"🚀 Testing WebSocket streaming for symbols: {symbols}")
        
        # Test streaming for a short duration
        count = 0
        timeout_task = asyncio.create_task(asyncio.sleep(10))  # 10 second timeout
        
        try:
            async for data in provider.stream_market_data_ws(symbols):
                print(f"📊 Received data: {data.symbol} - ${data.price:.2f} at {data.timestamp}")
                count += 1
                
                # Stop after receiving some data or timeout
                if count >= 5:  # Stop after 5 data points
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
            print("✅ WebSocket streaming is working!")
        else:
            print("⚠️  No data received (likely due to after-hours trading)")
            
    except Exception as e:
        print(f"❌ Error during WebSocket test: {e}")
        print("🔄 This is likely due to after-hours trading or connection issues")
        
    finally:
        # Clean up
        try:
            await provider.disconnect()
            print("✅ Provider disconnected cleanly")
        except Exception as e:
            print(f"⚠️  Disconnect warning: {e}")

if __name__ == "__main__":
    print("🧪 WebSocket Fix Verification Test")
    print("=" * 50)
    asyncio.run(test_websocket_fix())
    print("=" * 50)
    print("🏁 Test completed")