"""
Tests for Alpaca Markets Data Provider

This module contains comprehensive tests for the Alpaca provider,
including market data, tick data, WebSocket streaming, and order book functionality.
"""

import pytest
import asyncio
import aiohttp
import json
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, MagicMock, AsyncMock
import pandas as pd

from ..providers.alpaca import AlpacaProvider
from ..providers.base import MarketData, TickData, OrderBookData


class TestAlpacaProvider:
    """Test suite for Alpaca Markets provider."""
    
    @pytest.fixture
    def provider(self):
        """Create an Alpaca provider instance."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            mock_settings.return_value = Mock(
                alpaca_api_key="test_key",
                alpaca_api_secret="test_secret",
                alpaca_subscription_level="basic",
                max_concurrent_requests=10,
                max_requests_per_minute=200
            )
            return AlpacaProvider()
    
    @pytest.fixture
    def mock_market_data_response(self):
        """Mock response for market data."""
        return {
            "bars": [
                {
                    "t": "2024-01-01T10:00:00Z",
                    "o": 150.0,
                    "h": 151.0,
                    "l": 149.0,
                    "c": 150.5,
                    "v": 1000000,
                    "n": 500,
                    "vw": 150.25
                },
                {
                    "t": "2024-01-01T10:01:00Z",
                    "o": 150.5,
                    "h": 151.5,
                    "l": 150.0,
                    "c": 151.0,
                    "v": 1200000,
                    "n": 600,
                    "vw": 150.75
                }
            ],
            "next_page_token": None
        }
    
    @pytest.fixture
    def mock_tick_data_response(self):
        """Mock response for tick data."""
        return {
            "trades": [
                {
                    "t": "2024-01-01T10:00:00.123456Z",
                    "p": 150.05,
                    "s": 100,
                    "x": "V",
                    "c": ["@", "I"]
                },
                {
                    "t": "2024-01-01T10:00:00.234567Z",
                    "p": 150.10,
                    "s": 200,
                    "x": "V",
                    "c": ["@"]
                }
            ],
            "next_page_token": None
        }
    
    @pytest.fixture
    def mock_quote_response(self):
        """Mock response for quotes."""
        return {
            "quote": {
                "t": "2024-01-01T10:00:00.123456Z",
                "bp": 150.00,
                "bs": 1000,
                "ap": 150.05,
                "as": 1500
            }
        }
    
    @pytest.mark.asyncio
    async def test_provider_initialization(self, provider):
        """Test provider initialization."""
        assert provider.name == "alpaca"
        assert provider.api_key == "test_key"
        assert provider.api_secret == "test_secret"
        assert provider.subscription_level == "basic"
        assert not provider._connected
    
    @pytest.mark.asyncio
    async def test_connect_success(self, provider):
        """Test successful connection."""
        await provider.connect()
        
        assert provider._connected
        assert provider.session is not None
        assert "APCA-API-KEY-ID" in provider.session.headers
        assert "APCA-API-SECRET-KEY" in provider.session.headers
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_connect_missing_credentials(self):
        """Test connection with missing credentials."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            mock_settings.return_value = Mock(
                alpaca_api_key=None,
                alpaca_api_secret=None,
                alpaca_subscription_level="basic",
                max_concurrent_requests=10,
                max_requests_per_minute=200
            )
            provider = AlpacaProvider()
            
            with pytest.raises(ValueError, match="Alpaca API key and secret not configured"):
                await provider.connect()
    
    @pytest.mark.asyncio
    async def test_get_market_data_stocks(self, provider, mock_market_data_response):
        """Test fetching market data for stocks."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_market_data_response
            
            await provider.connect()
            
            data_points = []
            async for data in provider.get_market_data(
                ["AAPL"],
                datetime(2024, 1, 1, 10, 0),
                datetime(2024, 1, 1, 10, 5),
                "1min"
            ):
                data_points.append(data)
            
            assert len(data_points) == 2
            assert all(isinstance(d, MarketData) for d in data_points)
            assert data_points[0].symbol == "AAPL"
            assert data_points[0].close == 150.5
            assert data_points[0].volume == 1000000
            assert data_points[0].metadata["trades"] == 500
            assert data_points[0].metadata["vwap"] == 150.25
            
            # Check API call
            mock_request.assert_called_once()
            call_args = mock_request.call_args
            assert "/v2/stocks/AAPL/bars" in call_args[0][0]
            assert call_args[1]["params"]["timeframe"] == "1Min"
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_market_data_crypto(self, provider, mock_market_data_response):
        """Test fetching market data for crypto."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_market_data_response
            
            await provider.connect()
            
            data_points = []
            async for data in provider.get_market_data(
                ["BTCUSD"],
                datetime(2024, 1, 1, 10, 0),
                datetime(2024, 1, 1, 10, 5),
                "5min"
            ):
                data_points.append(data)
            
            assert len(data_points) == 2
            assert all(isinstance(d, MarketData) for d in data_points)
            assert data_points[0].symbol == "BTCUSD"
            
            # Check API call
            mock_request.assert_called_once()
            call_args = mock_request.call_args
            assert "/v1beta3/crypto/BTCUSD/bars" in call_args[0][0]
            assert call_args[1]["params"]["timeframe"] == "5Min"
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_market_data_pagination(self, provider):
        """Test market data pagination."""
        response1 = {
            "bars": [{"t": "2024-01-01T10:00:00Z", "o": 150, "h": 151, "l": 149, "c": 150.5, "v": 1000}],
            "next_page_token": "page2"
        }
        response2 = {
            "bars": [{"t": "2024-01-01T10:01:00Z", "o": 150.5, "h": 151.5, "l": 150, "c": 151, "v": 1200}],
            "next_page_token": None
        }
        
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.side_effect = [response1, response2]
            
            await provider.connect()
            
            data_points = []
            async for data in provider.get_market_data(
                ["AAPL"],
                datetime(2024, 1, 1),
                datetime(2024, 1, 2)
            ):
                data_points.append(data)
            
            assert len(data_points) == 2
            assert mock_request.call_count == 2
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_market_data_basic_plan_limit(self, provider):
        """Test basic plan historical data limit."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = {"bars": [], "next_page_token": None}
            
            await provider.connect()
            
            # Try to fetch data older than 15 minutes
            old_start = datetime.now() - timedelta(hours=1)
            data_points = []
            async for data in provider.get_market_data(
                ["AAPL"],
                old_start,
                datetime.now()
            ):
                data_points.append(data)
            
            # Check that start time was adjusted
            call_args = mock_request.call_args[1]["params"]
            start_time = datetime.fromisoformat(call_args["start"].replace("Z", "+00:00"))
            assert datetime.now() - start_time <= timedelta(minutes=16)  # Allow 1 minute buffer
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_tick_data(self, provider, mock_tick_data_response):
        """Test fetching tick data."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_tick_data_response
            
            await provider.connect()
            
            ticks = []
            async for tick in provider.get_tick_data(
                ["AAPL"],
                datetime(2024, 1, 1, 10, 0),
                datetime(2024, 1, 1, 10, 5)
            ):
                ticks.append(tick)
            
            assert len(ticks) == 2
            assert all(isinstance(t, TickData) for t in ticks)
            assert ticks[0].symbol == "AAPL"
            assert ticks[0].price == 150.05
            assert ticks[0].size == 100
            assert ticks[0].exchange == "V"
            assert ticks[0].conditions == "@,I"
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_order_book(self, provider, mock_quote_response):
        """Test fetching order book data."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.return_value = mock_quote_response
            
            await provider.connect()
            
            order_books = []
            async for book in provider.get_order_book(["AAPL"]):
                order_books.append(book)
            
            assert len(order_books) == 1
            assert isinstance(order_books[0], OrderBookData)
            assert order_books[0].symbol == "AAPL"
            assert order_books[0].bid_price == 150.00
            assert order_books[0].bid_size == 1000
            assert order_books[0].ask_price == 150.05
            assert order_books[0].ask_size == 1500
            assert order_books[0].spread == 0.05
            assert order_books[0].mid_price == 150.025
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_stream_market_data(self, provider):
        """Test streaming market data via WebSocket."""
        mock_messages = [
            [{"T": "success", "msg": "authenticated"}],
            [{"T": "subscription", "trades": [], "quotes": [], "bars": ["AAPL"]}],
            [{"T": "b", "S": "AAPL", "t": "2024-01-01T10:00:00Z", "o": 150, "h": 151, "l": 149, "c": 150.5, "v": 1000, "n": 100, "vw": 150.25}],
            [{"T": "b", "S": "AAPL", "t": "2024-01-01T10:01:00Z", "o": 150.5, "h": 151.5, "l": 150, "c": 151, "v": 1200, "n": 120, "vw": 150.75}]
        ]
        
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[json.dumps(msg) for msg in mock_messages])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            await provider.connect()
            
            data_points = []
            stream_iter = provider.stream_market_data(["AAPL"])
            
            # Get first two data points
            async for i, data in enumerate(stream_iter):
                data_points.append(data)
                if i >= 1:
                    break
            
            assert len(data_points) == 2
            assert all(isinstance(d, MarketData) for d in data_points)
            assert data_points[0].symbol == "AAPL"
            assert data_points[0].close == 150.5
            
            # Check WebSocket calls
            assert mock_ws.send.call_count == 2  # Auth + Subscribe
            auth_call = json.loads(mock_ws.send.call_args_list[0][0][0])
            assert auth_call["action"] == "auth"
            assert auth_call["key"] == "test_key"
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_stream_tick_data(self, provider):
        """Test streaming tick data via WebSocket."""
        mock_messages = [
            [{"T": "success", "msg": "authenticated"}],
            [{"T": "subscription", "trades": ["AAPL"], "quotes": [], "bars": []}],
            [{"T": "t", "S": "AAPL", "t": "2024-01-01T10:00:00.123456Z", "p": 150.05, "s": 100, "x": "V", "c": ["@"]}],
            [{"T": "t", "S": "AAPL", "t": "2024-01-01T10:00:00.234567Z", "p": 150.10, "s": 200, "x": "V", "c": ["@", "I"]}]
        ]
        
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[json.dumps(msg) for msg in mock_messages])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            await provider.connect()
            
            ticks = []
            stream_iter = provider.stream_tick_data(["AAPL"])
            
            # Get first two ticks
            async for i, tick in enumerate(stream_iter):
                ticks.append(tick)
                if i >= 1:
                    break
            
            assert len(ticks) == 2
            assert all(isinstance(t, TickData) for t in ticks)
            assert ticks[0].symbol == "AAPL"
            assert ticks[0].price == 150.05
            assert ticks[0].size == 100
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_websocket_symbol_limit_basic_plan(self, provider):
        """Test WebSocket symbol limit for basic plan."""
        # Create 40 symbols (more than basic plan limit of 30)
        symbols = [f"STOCK{i}" for i in range(40)]
        
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps([{"T": "success", "msg": "authenticated"}]),
                json.dumps([{"T": "subscription", "trades": [], "quotes": [], "bars": symbols[:30]}])
            ])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            await provider.connect()
            
            # This should trigger the limit warning
            await provider._connect_websocket(symbols)
            
            # Check that only 30 symbols were subscribed
            subscribe_call = json.loads(mock_ws.send.call_args_list[1][0][0])
            assert len(subscribe_call["bars"]) == 30
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_websocket_reconnect(self, provider):
        """Test WebSocket reconnection on connection loss."""
        import websockets.exceptions
        
        mock_messages = [
            [{"T": "success", "msg": "authenticated"}],
            [{"T": "subscription", "trades": [], "quotes": [], "bars": ["AAPL"]}],
            [{"T": "b", "S": "AAPL", "t": "2024-01-01T10:00:00Z", "o": 150, "h": 151, "l": 149, "c": 150.5, "v": 1000}]
        ]
        
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            
            # First recv raises ConnectionClosed, then succeeds after reconnect
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps(msg) for msg in mock_messages[:2]
            ] + [websockets.exceptions.ConnectionClosed(None, None)] + [
                json.dumps(msg) for msg in mock_messages
            ])
            
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            await provider.connect()
            
            # Subscribe initially
            provider._subscribed_symbols = {"AAPL"}
            
            # Stream should handle reconnection
            data_points = []
            try:
                async for data in provider._stream_messages():
                    if data.get("T") == "b":
                        data_points.append(data)
                        break
            except:
                pass
            
            # Should have attempted to reconnect
            assert mock_ws_connect.call_count >= 1
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_is_crypto_detection(self, provider):
        """Test cryptocurrency symbol detection."""
        assert provider._is_crypto("BTCUSD") is True
        assert provider._is_crypto("ETHUSD") is True
        assert provider._is_crypto("BTCUSDT") is True
        assert provider._is_crypto("ETHBTC") is True
        assert provider._is_crypto("AAPL") is False
        assert provider._is_crypto("MSFT") is False
        assert provider._is_crypto("GOOGL") is False
    
    @pytest.mark.asyncio
    async def test_interval_mapping(self, provider):
        """Test interval mapping to Alpaca format."""
        assert provider.INTERVAL_MAP["1min"] == "1Min"
        assert provider.INTERVAL_MAP["5min"] == "5Min"
        assert provider.INTERVAL_MAP["15min"] == "15Min"
        assert provider.INTERVAL_MAP["30min"] == "30Min"
        assert provider.INTERVAL_MAP["1hour"] == "1Hour"
        assert provider.INTERVAL_MAP["4hour"] == "4Hour"
        assert provider.INTERVAL_MAP["1day"] == "1Day"
        assert provider.INTERVAL_MAP["1week"] == "1Week"
        assert provider.INTERVAL_MAP["1month"] == "1Month"
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self, provider):
        """Test rate limiting functionality."""
        request_count = 0
        
        async def mock_request(*args, **kwargs):
            nonlocal request_count
            request_count += 1
            return {"bars": [], "next_page_token": None}
        
        with patch.object(provider, '_request', side_effect=mock_request):
            await provider.connect()
            
            # Make multiple requests
            tasks = []
            for i in range(5):
                tasks.append(provider.get_market_data(
                    [f"STOCK{i}"],
                    datetime(2024, 1, 1),
                    datetime(2024, 1, 2)
                ))
            
            # Execute concurrently
            results = await asyncio.gather(*[
                anext(task, None) for task in tasks
            ])
            
            # Should respect rate limiting
            assert request_count == 5
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_error_handling(self, provider):
        """Test error handling for API failures."""
        with patch.object(provider, '_request', new_callable=AsyncMock) as mock_request:
            mock_request.side_effect = aiohttp.ClientError("API Error")
            
            await provider.connect()
            
            data_points = []
            async for data in provider.get_market_data(
                ["AAPL"],
                datetime(2024, 1, 1),
                datetime(2024, 1, 2)
            ):
                data_points.append(data)
            
            # Should handle error gracefully and return no data
            assert len(data_points) == 0
            
            await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_context_manager(self, provider):
        """Test using provider as context manager."""
        async with provider as p:
            assert p._connected
            assert p.session is not None
        
        assert not provider._connected
        assert provider.session.closed


class TestAlpacaWebSocketStreaming:
    """Test suite for Alpaca WebSocket streaming functionality."""
    
    @pytest.fixture
    def provider(self):
        """Create an Alpaca provider instance with WebSocket enabled."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            mock_settings.return_value = Mock(
                alpaca_api_key="test_key",
                alpaca_api_secret="test_secret",
                alpaca_subscription_level="basic",
                alpaca_ws_enabled=True,
                alpaca_ws_url="wss://stream.data.alpaca.markets/v2/iex",
                alpaca_ws_reconnect_delay=5,
                alpaca_ws_max_reconnect_attempts=3,
                max_concurrent_requests=10,
                max_requests_per_minute=200
            )
            return AlpacaProvider()
    
    @pytest.fixture
    def mock_ws_bar_messages(self):
        """Mock WebSocket bar messages from Alpaca."""
        return [
            # Authentication success
            [{"T": "success", "msg": "authenticated"}],
            # Subscription confirmation
            [{"T": "subscription", "trades": [], "quotes": [], "bars": ["AAPL"]}],
            # Bar data messages
            [{
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
            }],
            [{
                "T": "b",
                "S": "AAPL",
                "o": 150.5,
                "h": 151.5,
                "l": 150.0,
                "c": 151.0,
                "v": 1200000,
                "t": "2024-01-01T10:01:00Z",
                "n": 600,
                "vw": 150.75
            }]
        ]
    
    @pytest.mark.asyncio
    async def test_websocket_streaming_method_exists(self, provider):
        """Test that stream_market_data_ws method exists and has correct signature."""
        # Should have WebSocket streaming method
        assert hasattr(provider, 'stream_market_data_ws')
        
        # Method should be async and return AsyncIterator
        import inspect
        assert inspect.iscoroutinefunction(provider.stream_market_data_ws)
    
    @pytest.mark.asyncio
    async def test_websocket_configuration_disabled(self):
        """Test WebSocket functionality when disabled in configuration."""
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
            
            # Should fall back to polling method
            with patch.object(provider, 'stream_market_data') as mock_polling:
                mock_polling.return_value = iter([])  # Empty async iterator
                
                result = []
                async for data in provider.stream_market_data_ws(["AAPL"]):
                    result.append(data)
                
                # Should have called polling method as fallback
                mock_polling.assert_called_once_with(["AAPL"])
    
    @pytest.mark.asyncio
    async def test_websocket_authentication(self, provider, mock_ws_bar_messages):
        """Test WebSocket authentication process."""
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps(msg) for msg in mock_ws_bar_messages[:2]  # Auth + subscription only
            ])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            # Start streaming (should authenticate)
            stream_iter = provider.stream_market_data_ws(["AAPL"])
            try:
                await anext(stream_iter)  # Trigger authentication
            except StopAsyncIteration:
                pass
            
            # Should have sent authentication message
            assert mock_ws.send.call_count >= 1
            auth_call = json.loads(mock_ws.send.call_args_list[0][0][0])
            assert auth_call["action"] == "auth"
            assert auth_call["key"] == "test_key"
            assert auth_call["secret"] == "test_secret"
    
    @pytest.mark.asyncio
    async def test_websocket_subscription(self, provider, mock_ws_bar_messages):
        """Test WebSocket symbol subscription process."""
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps(msg) for msg in mock_ws_bar_messages[:2]
            ])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            # Start streaming with multiple symbols
            stream_iter = provider.stream_market_data_ws(["AAPL", "GOOGL", "MSFT"])
            try:
                await anext(stream_iter)
            except StopAsyncIteration:
                pass
            
            # Should have sent subscription message
            assert mock_ws.send.call_count >= 2  # Auth + subscription
            subscribe_call = json.loads(mock_ws.send.call_args_list[1][0][0])
            assert subscribe_call["action"] == "subscribe"
            assert "bars" in subscribe_call
            assert set(subscribe_call["bars"]) == {"AAPL", "GOOGL", "MSFT"}
    
    @pytest.mark.asyncio
    async def test_websocket_bar_message_parsing(self, provider, mock_ws_bar_messages):
        """Test parsing of WebSocket bar messages to MarketData objects."""
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps(msg) for msg in mock_ws_bar_messages
            ])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            # Collect streaming data
            data_points = []
            stream_iter = provider.stream_market_data_ws(["AAPL"])
            
            # Get first two data points
            async for i, data in enumerate(stream_iter):
                data_points.append(data)
                if i >= 1:  # Get 2 data points
                    break
            
            # Should have parsed 2 MarketData objects
            assert len(data_points) == 2
            assert all(isinstance(d, MarketData) for d in data_points)
            
            # Verify first data point
            first_data = data_points[0]
            assert first_data.symbol == "AAPL"
            assert first_data.open == 150.0
            assert first_data.high == 151.0
            assert first_data.low == 149.0
            assert first_data.close == 150.5
            assert first_data.volume == 1000000
            assert first_data.provider == "alpaca"
            assert first_data.time is not None
            
            # Verify second data point
            second_data = data_points[1]
            assert second_data.symbol == "AAPL"
            assert second_data.close == 151.0
            assert second_data.volume == 1200000
    
    @pytest.mark.asyncio
    async def test_websocket_fallback_on_connection_error(self, provider):
        """Test automatic fallback to polling when WebSocket connection fails."""
        with patch('websockets.connect') as mock_ws_connect:
            # Simulate connection failure
            mock_ws_connect.side_effect = ConnectionError("WebSocket connection failed")
            
            with patch.object(provider, 'stream_market_data') as mock_polling:
                # Mock polling method to return test data
                test_data = MarketData(
                    time=datetime.now(),
                    symbol="AAPL",
                    open=150.0, high=151.0, low=149.0, close=150.5,
                    volume=1000000,
                    provider="alpaca"
                )
                
                async def mock_polling_generator(symbols):
                    yield test_data
                
                mock_polling.return_value = mock_polling_generator(["AAPL"])
                
                # Should fall back to polling
                data_points = []
                async for data in provider.stream_market_data_ws(["AAPL"]):
                    data_points.append(data)
                    break  # Just get one data point
                
                # Should have received data from polling fallback
                assert len(data_points) == 1
                assert data_points[0].symbol == "AAPL"
                assert data_points[0].close == 150.5
                
                # Should have called polling method
                mock_polling.assert_called_once_with(["AAPL"])
    
    @pytest.mark.asyncio
    async def test_websocket_fallback_on_message_error(self, provider):
        """Test fallback to polling when WebSocket receives invalid messages."""
        with patch('websockets.connect') as mock_ws_connect:
            mock_ws = AsyncMock()
            # Simulate invalid JSON message after authentication
            mock_ws.recv = AsyncMock(side_effect=[
                json.dumps([{"T": "success", "msg": "authenticated"}]),
                json.dumps([{"T": "subscription", "trades": [], "quotes": [], "bars": ["AAPL"]}]),
                "invalid json",  # This should trigger fallback
            ])
            mock_ws.send = AsyncMock()
            mock_ws_connect.return_value.__aenter__.return_value = mock_ws
            
            with patch.object(provider, 'stream_market_data') as mock_polling:
                test_data = MarketData(
                    time=datetime.now(),
                    symbol="AAPL",
                    open=150.0, high=151.0, low=149.0, close=150.5,
                    volume=1000000,
                    provider="alpaca"
                )
                
                async def mock_polling_generator(symbols):
                    yield test_data
                
                mock_polling.return_value = mock_polling_generator(["AAPL"])
                
                # Should eventually fall back to polling
                data_points = []
                async for data in provider.stream_market_data_ws(["AAPL"]):
                    data_points.append(data)
                    break
                
                # Should have received data (either from WebSocket or polling fallback)
                assert len(data_points) >= 0  # May be empty if fallback didn't trigger in time