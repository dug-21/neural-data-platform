#!/usr/bin/env python3
"""
Simple integration test for WebSocket streaming functionality.
"""

import asyncio
import sys
import os
from unittest.mock import Mock, patch

# Add the parent directory to the Python path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from data_ingestion.providers.alpaca import AlpacaProvider


def test_websocket_method_exists():
    """Test that the WebSocket method exists and has correct signature."""
    print("Testing WebSocket method existence...")
    
    # Test without mocking first - just check the method exists
    try:
        provider = AlpacaProvider()
    except ValueError as e:
        # Expected if no API keys configured
        print(f"Note: {e} (this is expected in test environment)")
        # We can still test the class structure
        pass
    
    # Check method exists
    assert hasattr(AlpacaProvider, 'stream_market_data_ws')
    print("✅ stream_market_data_ws method exists")
    
    # Check method signature
    import inspect
    method = getattr(AlpacaProvider, 'stream_market_data_ws')
    sig = inspect.signature(method)
    
    # Should have 'self' and 'symbols' parameters
    params = list(sig.parameters.keys())
    print(f"Method parameters: {params}")
    assert 'symbols' in params
    print("✅ Method signature is correct")


def test_websocket_configuration_structure():
    """Test WebSocket configuration structure."""
    print("\nTesting WebSocket configuration structure...")
    
    # Mock the settings to avoid credential requirements
    with patch('data_ingestion.providers.base.get_settings') as mock_settings:
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
        
        # Check WebSocket configuration exists
        assert hasattr(provider, '_ws_config')
        assert isinstance(provider._ws_config, dict)
        
        # Check configuration values
        assert provider._ws_config["enabled"] == True
        assert provider._ws_config["url"] == "wss://custom.websocket.url"
        assert provider._ws_config["reconnect_delay"] == 10
        assert provider._ws_config["max_reconnect_attempts"] == 5
        
        print("✅ WebSocket configuration loaded correctly")


async def test_websocket_fallback_behavior():
    """Test WebSocket fallback to polling when disabled."""
    print("\nTesting WebSocket fallback behavior...")
    
    with patch('data_ingestion.providers.base.get_settings') as mock_settings:
        mock_settings.return_value = Mock(
            alpaca_api_key="test_key",
            alpaca_api_secret="test_secret",
            alpaca_subscription_level="basic",
            alpaca_ws_enabled=False,  # Disabled - should fallback
            max_concurrent_requests=10,
            max_requests_per_minute=200
        )
        
        provider = AlpacaProvider()
        
        # Mock the polling method to track if it's called
        fallback_called = False
        
        async def mock_polling_stream(symbols):
            nonlocal fallback_called
            fallback_called = True
            print(f"Fallback polling called for symbols: {symbols}")
            # Don't yield anything to end the test quickly
            return
            yield  # unreachable, but makes this a generator
        
        # Replace the polling method
        provider.stream_market_data = mock_polling_stream
        
        # Call WebSocket method - should fallback to polling
        try:
            async for data in provider.stream_market_data_ws(["AAPL"]):
                break  # Exit immediately
        except:
            pass  # Expected since our mock doesn't yield real data
        
        assert fallback_called, "Fallback to polling was not called"
        print("✅ WebSocket correctly falls back to polling when disabled")


def test_message_conversion_method():
    """Test the WebSocket message conversion method."""
    print("\nTesting WebSocket message conversion...")
    
    with patch('data_ingestion.providers.base.get_settings') as mock_settings:
        mock_settings.return_value = Mock(
            alpaca_api_key="test_key",
            alpaca_api_secret="test_secret",
            alpaca_subscription_level="basic",
            max_concurrent_requests=10,
            max_requests_per_minute=200
        )
        
        provider = AlpacaProvider()
        
        # Test message conversion
        test_bar_msg = {
            "T": "b",  # Bar message type
            "S": "AAPL",  # Symbol
            "o": 150.0,  # Open
            "h": 151.0,  # High
            "l": 149.0,  # Low
            "c": 150.5,  # Close
            "v": 1000000,  # Volume
            "t": "2024-01-01T10:00:00Z",  # Timestamp
            "n": 500,  # Trade count
            "vw": 150.25  # VWAP
        }
        
        # Convert message to MarketData
        market_data = provider._convert_ws_bar_to_market_data(test_bar_msg)
        
        assert market_data is not None
        assert market_data.symbol == "AAPL"
        assert market_data.open == 150.0
        assert market_data.high == 151.0
        assert market_data.low == 149.0
        assert market_data.close == 150.5
        assert market_data.volume == 1000000
        assert market_data.provider == "alpaca"
        assert market_data.metadata["trades"] == 500
        assert market_data.metadata["vwap"] == 150.25
        assert market_data.metadata["source"] == "websocket"
        
        print("✅ WebSocket message conversion works correctly")


async def main():
    """Run all integration tests."""
    print("WebSocket Implementation Integration Tests")
    print("=" * 50)
    
    try:
        # Test method existence and signature
        test_websocket_method_exists()
        
        # Test configuration
        test_websocket_configuration_structure()
        
        # Test message conversion
        test_message_conversion_method()
        
        # Test fallback behavior
        await test_websocket_fallback_behavior()
        
        print("\n" + "=" * 50)
        print("✅ All integration tests passed!")
        print("✅ WebSocket implementation is working correctly!")
        
    except Exception as e:
        print(f"\n❌ Integration test failed: {e}")
        import traceback
        traceback.print_exc()
        return False
    
    return True


if __name__ == "__main__":
    success = asyncio.run(main())
    sys.exit(0 if success else 1)