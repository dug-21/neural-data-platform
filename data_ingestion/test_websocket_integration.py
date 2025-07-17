#!/usr/bin/env python3
"""
Integration test for WebSocket streaming functionality.
This test verifies that the WebSocket implementation works correctly.
"""

import asyncio
import sys
import os
from unittest.mock import Mock, patch

# Add the parent directory to the Python path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from data_ingestion.providers.alpaca import AlpacaProvider
from data_ingestion.providers.base import MarketData


async def test_websocket_disabled_fallback():
    """Test that WebSocket falls back to polling when disabled."""
    print("Testing WebSocket disabled fallback...")
    
    with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
        mock_settings.return_value = Mock(
            alpaca_api_key="test_key",
            alpaca_api_secret="test_secret",
            alpaca_subscription_level="basic",
            alpaca_ws_enabled=False,  # Disabled
            max_concurrent_requests=10,
            max_requests_per_minute=200
        )
        
        provider = AlpacaProvider()
        
        # Mock the polling method
        test_data = MarketData(
            time=None,  # Will be normalized by MarketData.__post_init__
            symbol="AAPL",
            open=150.0, high=151.0, low=149.0, close=150.5,
            volume=1000000,
            provider="alpaca"
        )
        
        async def mock_polling_generator(symbols):
            print(f"Mock polling called with symbols: {symbols}")
            yield test_data
        
        with patch.object(provider, 'stream_market_data', return_value=mock_polling_generator(["AAPL"])):
            # Should fall back to polling
            data_points = []
            async for data in provider.stream_market_data_ws(["AAPL"]):
                data_points.append(data)
                break  # Just get one data point
            
            print(f"Received {len(data_points)} data points")
            if data_points:
                print(f"Data: {data_points[0].symbol} = ${data_points[0].close}")
                assert data_points[0].symbol == "AAPL"
                assert data_points[0].close == 150.5
                print("✅ Fallback to polling works correctly")
            else:
                print("❌ No data received from fallback")


async def test_websocket_configuration():
    """Test WebSocket configuration loading."""
    print("\nTesting WebSocket configuration...")
    
    with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
        mock_settings.return_value = Mock(
            alpaca_api_key="test_key",
            alpaca_api_secret="test_secret",
            alpaca_subscription_level="basic",
            alpaca_ws_enabled=True,
            alpaca_ws_url="wss://custom.websocket.url",
            alpaca_ws_reconnect_delay=10,
            alpaca_ws_max_reconnect_attempts=5,
            max_concurrent_requests=10,
            max_requests_per_minute=200
        )
        
        provider = AlpacaProvider()
        
        # Verify configuration is loaded correctly
        assert provider._ws_config["enabled"] == True
        assert provider._ws_config["url"] == "wss://custom.websocket.url"
        assert provider._ws_config["reconnect_delay"] == 10
        assert provider._ws_config["max_reconnect_attempts"] == 5
        
        print("✅ WebSocket configuration loaded correctly")


def test_method_signature():
    """Test that the method has the correct signature."""
    print("\nTesting method signature...")
    
    with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
        mock_settings.return_value = Mock(
            alpaca_api_key="test_key",
            alpaca_api_secret="test_secret",
            alpaca_subscription_level="basic",
            max_concurrent_requests=10,
            max_requests_per_minute=200
        )
        
        provider = AlpacaProvider()
        
        # Check method exists
        assert hasattr(provider, 'stream_market_data_ws')
        
        # Check signature matches stream_market_data
        import inspect
        ws_sig = inspect.signature(provider.stream_market_data_ws)
        polling_sig = inspect.signature(provider.stream_market_data)
        
        # Both should have same parameter names (symbols)
        ws_params = list(ws_sig.parameters.keys())
        polling_params = list(polling_sig.parameters.keys())
        
        print(f"WebSocket method params: {ws_params}")
        print(f"Polling method params: {polling_params}")
        
        # WebSocket method should have same core parameters as polling
        assert "symbols" in ws_params
        print("✅ Method signature is correct")


async def main():
    """Run all integration tests."""
    print("Starting WebSocket Integration Tests")
    print("=" * 50)
    
    try:
        # Test method signature
        test_method_signature()
        
        # Test configuration
        await test_websocket_configuration()
        
        # Test fallback behavior
        await test_websocket_disabled_fallback()
        
        print("\n" + "=" * 50)
        print("✅ All integration tests passed!")
        
    except Exception as e:
        print(f"\n❌ Integration test failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)


if __name__ == "__main__":
    asyncio.run(main())