"""Unit tests for Polygon WebSocket implementation."""
import asyncio
import pytest
import json
from unittest.mock import Mock, AsyncMock, patch, MagicMock
from datetime import datetime
import aiohttp

from data_ingestion.providers.polygon_websocket import (
    PolygonWebSocketProvider,
    WebSocketConfig,
    WebSocketManager,
    ConnectionState,
    SubscriptionManager,
    MessageProcessor,
    StreamBuffer,
    MessageType
)
from data_ingestion.providers.base import MarketData, TickData, OrderBookData


@pytest.fixture
def mock_config():
    """Mock WebSocket configuration."""
    return WebSocketConfig(
        url="wss://test.polygon.io",
        max_reconnect_attempts=3,
        initial_reconnect_delay=0.1,
        heartbeat_interval=1.0,
        message_buffer_size=100
    )


@pytest.fixture
def mock_api_key():
    """Mock API key."""
    return "test_api_key_123"


class TestStreamBuffer:
    """Test StreamBuffer functionality."""
    
    @pytest.mark.asyncio
    async def test_push_pop(self):
        """Test basic push and pop operations."""
        buffer = StreamBuffer(max_size=5)
        
        # Push messages
        assert await buffer.push({"id": 1})
        assert await buffer.push({"id": 2})
        assert await buffer.push({"id": 3})
        
        # Pop messages
        assert await buffer.pop() == {"id": 1}
        assert await buffer.pop() == {"id": 2}
        assert await buffer.pop() == {"id": 3}
        assert await buffer.pop() is None
    
    @pytest.mark.asyncio
    async def test_overflow(self):
        """Test buffer overflow handling."""
        buffer = StreamBuffer(max_size=2)
        
        # Fill buffer
        assert await buffer.push({"id": 1})
        assert await buffer.push({"id": 2})
        
        # Overflow
        assert not await buffer.push({"id": 3})
        assert buffer.overflow_count == 1
    
    @pytest.mark.asyncio
    async def test_pop_batch(self):
        """Test batch pop operations."""
        buffer = StreamBuffer(max_size=10)
        
        # Push 5 messages
        for i in range(5):
            await buffer.push({"id": i})
        
        # Pop batch
        batch = await buffer.pop_batch(3)
        assert len(batch) == 3
        assert batch[0]["id"] == 0
        assert batch[2]["id"] == 2
        
        # Pop remaining
        batch = await buffer.pop_batch(10)
        assert len(batch) == 2
    
    def test_statistics(self):
        """Test buffer statistics."""
        buffer = StreamBuffer(max_size=10)
        stats = buffer.get_stats()
        
        assert stats["current_size"] == 0
        assert stats["max_size"] == 10
        assert stats["total_messages"] == 0
        assert stats["overflow_count"] == 0


class TestWebSocketManager:
    """Test WebSocketManager functionality."""
    
    @pytest.mark.asyncio
    async def test_connect_success(self, mock_config, mock_api_key):
        """Test successful connection."""
        manager = WebSocketManager(mock_config, mock_api_key)
        
        # Mock session and connection
        mock_session = AsyncMock()
        mock_ws = AsyncMock()
        mock_ws.closed = False
        
        # Mock successful auth response
        auth_response = {"status": "auth_success", "message": "conn_123"}
        mock_ws.__aiter__.return_value = [
            MagicMock(type=aiohttp.WSMsgType.TEXT, data=json.dumps(auth_response))
        ]
        
        mock_session.ws_connect.return_value = mock_ws
        
        with patch('aiohttp.ClientSession', return_value=mock_session):
            result = await manager.connect()
            
            assert result is True
            assert manager.state == ConnectionState.CONNECTED
            assert manager._connection_id == "conn_123"
    
    @pytest.mark.asyncio
    async def test_connect_timeout(self, mock_config, mock_api_key):
        """Test connection timeout."""
        mock_config.connection_timeout = 0.1
        manager = WebSocketManager(mock_config, mock_api_key)
        
        mock_session = AsyncMock()
        mock_session.ws_connect.side_effect = asyncio.TimeoutError()
        
        with patch('aiohttp.ClientSession', return_value=mock_session):
            result = await manager.connect()
            
            assert result is False
            assert manager.state == ConnectionState.FAILED
    
    @pytest.mark.asyncio
    async def test_reconnect_with_backoff(self, mock_config, mock_api_key):
        """Test reconnection with exponential backoff."""
        mock_config.max_reconnect_attempts = 2
        mock_config.initial_reconnect_delay = 0.01
        manager = WebSocketManager(mock_config, mock_api_key)
        
        # Mock failed connections
        connect_attempts = []
        
        async def mock_connect():
            connect_attempts.append(datetime.now())
            return False
        
        manager.connect = mock_connect
        manager.disconnect = AsyncMock()
        
        result = await manager.reconnect()
        
        assert result is False
        assert len(connect_attempts) == 2
        assert manager.state == ConnectionState.FAILED
        
        # Check backoff timing
        if len(connect_attempts) > 1:
            delay = (connect_attempts[1] - connect_attempts[0]).total_seconds()
            assert delay >= 0.01  # At least initial delay
    
    @pytest.mark.asyncio
    async def test_send_receive(self, mock_config, mock_api_key):
        """Test sending and receiving messages."""
        manager = WebSocketManager(mock_config, mock_api_key)
        
        # Mock connection
        mock_ws = AsyncMock()
        mock_ws.closed = False
        manager._connection = mock_ws
        
        # Test send
        message = {"action": "subscribe", "params": "T.AAPL"}
        await manager.send(message)
        mock_ws.send_json.assert_called_once_with(message)
        
        # Test receive
        mock_messages = [
            MagicMock(type=aiohttp.WSMsgType.TEXT, data='{"ev": "T", "sym": "AAPL"}'),
            MagicMock(type=aiohttp.WSMsgType.TEXT, data='[{"ev": "Q"}, {"ev": "A"}]'),
            MagicMock(type=aiohttp.WSMsgType.CLOSED, data=None)
        ]
        mock_ws.__aiter__.return_value = iter(mock_messages)
        
        received = []
        async for msg in manager.receive():
            received.append(msg)
        
        assert len(received) == 3
        assert received[0] == {"ev": "T", "sym": "AAPL"}
        assert received[1] == {"ev": "Q"}
        assert received[2] == {"ev": "A"}


class TestSubscriptionManager:
    """Test SubscriptionManager functionality."""
    
    @pytest.mark.asyncio
    async def test_subscribe_trades(self, mock_config, mock_api_key):
        """Test trade subscription."""
        ws_manager = AsyncMock()
        ws_manager.is_connected = True
        ws_manager.config = mock_config
        
        sub_manager = SubscriptionManager(ws_manager)
        
        # Subscribe to trades
        await sub_manager.subscribe_trades(["AAPL", "GOOGL"])
        
        # Check subscription was sent
        ws_manager.send.assert_called_once()
        call_args = ws_manager.send.call_args[0][0]
        assert call_args["action"] == "subscribe"
        assert "T.AAPL" in call_args["params"]
        assert "T.GOOGL" in call_args["params"]
    
    @pytest.mark.asyncio
    async def test_batch_subscriptions(self, mock_config, mock_api_key):
        """Test subscription batching."""
        mock_config.subscription_batch_size = 2
        ws_manager = AsyncMock()
        ws_manager.is_connected = True
        ws_manager.config = mock_config
        
        sub_manager = SubscriptionManager(ws_manager)
        
        # Subscribe to many symbols
        symbols = [f"SYM{i}" for i in range(5)]
        await sub_manager.subscribe_trades(symbols)
        
        # Should be sent in 3 batches (2, 2, 1)
        assert ws_manager.send.call_count == 3
    
    @pytest.mark.asyncio
    async def test_unsubscribe(self, mock_config, mock_api_key):
        """Test unsubscription."""
        ws_manager = AsyncMock()
        ws_manager.is_connected = True
        ws_manager.config = mock_config
        
        sub_manager = SubscriptionManager(ws_manager)
        
        # First subscribe
        await sub_manager.subscribe_trades(["AAPL", "GOOGL"])
        sub_manager._active_subscriptions.add("T.AAPL")
        sub_manager._active_subscriptions.add("T.GOOGL")
        
        # Then unsubscribe
        ws_manager.send.reset_mock()
        await sub_manager.unsubscribe(["AAPL"])
        
        # Check unsubscription was sent
        ws_manager.send.assert_called_once()
        call_args = ws_manager.send.call_args[0][0]
        assert call_args["action"] == "unsubscribe"
        assert "T.AAPL" in call_args["params"]
        assert "T.GOOGL" not in call_args["params"]
    
    @pytest.mark.asyncio
    async def test_resubscribe_all(self, mock_config, mock_api_key):
        """Test resubscription after reconnect."""
        ws_manager = AsyncMock()
        ws_manager.is_connected = True
        ws_manager.config = mock_config
        
        sub_manager = SubscriptionManager(ws_manager)
        
        # Setup existing subscriptions
        sub_manager._subscriptions["trades"].symbols = {"AAPL", "GOOGL"}
        sub_manager._subscriptions["quotes"].symbols = {"AAPL"}
        
        # Resubscribe all
        await sub_manager.resubscribe_all()
        
        # Check all subscriptions were sent
        assert ws_manager.send.call_count >= 1
        
        # Verify subscriptions
        all_params = []
        for call in ws_manager.send.call_args_list:
            params = call[0][0]["params"]
            all_params.extend(params.split(","))
        
        assert "T.AAPL" in all_params
        assert "T.GOOGL" in all_params
        assert "Q.AAPL" in all_params


class TestMessageProcessor:
    """Test MessageProcessor functionality."""
    
    @pytest.mark.asyncio
    async def test_process_trade(self):
        """Test trade message processing."""
        processor = MessageProcessor("polygon")
        
        trade_msg = {
            "ev": "T",
            "sym": "AAPL",
            "t": 1234567890123456789,  # nanoseconds
            "p": 150.25,
            "s": 100,
            "x": 4,
            "c": [0, 12]
        }
        
        result = await processor.process(trade_msg)
        
        assert isinstance(result, TickData)
        assert result.symbol == "AAPL"
        assert result.price == 150.25
        assert result.size == 100
        assert result.exchange == "4"
        assert result.conditions == "0,12"
    
    @pytest.mark.asyncio
    async def test_process_quote(self):
        """Test quote message processing."""
        processor = MessageProcessor("polygon")
        
        quote_msg = {
            "ev": "Q",
            "sym": "AAPL",
            "t": 1234567890123456789,
            "bp": 150.00,
            "bs": 100,
            "ap": 150.10,
            "as": 200
        }
        
        result = await processor.process(quote_msg)
        
        assert isinstance(result, OrderBookData)
        assert result.symbol == "AAPL"
        assert result.bid_price == 150.00
        assert result.bid_size == 100
        assert result.ask_price == 150.10
        assert result.ask_size == 200
        assert result.mid_price == 150.05
        assert result.spread == 0.10
    
    @pytest.mark.asyncio
    async def test_process_aggregate(self):
        """Test aggregate bar message processing."""
        processor = MessageProcessor("polygon")
        
        agg_msg = {
            "ev": "AM",
            "sym": "AAPL",
            "s": 1234567890000,  # milliseconds
            "o": 150.00,
            "h": 151.00,
            "l": 149.50,
            "c": 150.75,
            "v": 1000000,
            "vw": 150.25,
            "av": 250
        }
        
        result = await processor.process(agg_msg)
        
        assert isinstance(result, MarketData)
        assert result.symbol == "AAPL"
        assert result.open == 150.00
        assert result.high == 151.00
        assert result.low == 149.50
        assert result.close == 150.75
        assert result.volume == 1000000
        assert result.metadata["vwap"] == 150.25
        assert result.metadata["average_size"] == 250
    
    @pytest.mark.asyncio
    async def test_process_unknown_message(self):
        """Test handling of unknown message types."""
        processor = MessageProcessor("polygon")
        
        unknown_msg = {"ev": "UNKNOWN", "data": "test"}
        result = await processor.process(unknown_msg)
        
        assert result is None
    
    @pytest.mark.asyncio
    async def test_process_error_handling(self):
        """Test error handling in message processing."""
        processor = MessageProcessor("polygon")
        
        # Invalid trade message (missing required fields)
        invalid_msg = {"ev": "T", "sym": "AAPL"}
        
        result = await processor.process(invalid_msg)
        assert result is None  # Should handle gracefully


class TestPolygonWebSocketProvider:
    """Test PolygonWebSocketProvider integration."""
    
    @pytest.mark.asyncio
    async def test_provider_lifecycle(self, mock_config):
        """Test provider connection lifecycle."""
        with patch('data_ingestion.providers.polygon_websocket.WebSocketManager') as MockWSManager:
            with patch('config.get_settings') as mock_settings:
                mock_settings.return_value.polygon_api_key = "test_key"
                
                # Setup mocks
                mock_ws_instance = AsyncMock()
                mock_ws_instance.connect.return_value = True
                mock_ws_instance.is_connected = True
                MockWSManager.return_value = mock_ws_instance
                
                provider = PolygonWebSocketProvider(mock_config)
                
                # Test connect
                await provider.connect()
                assert provider._streaming is True
                assert provider._stream_task is not None
                
                # Test disconnect
                await provider.disconnect()
                assert provider._streaming is False
    
    @pytest.mark.asyncio
    async def test_stream_market_data(self, mock_config):
        """Test streaming market data."""
        with patch('data_ingestion.providers.polygon_websocket.WebSocketManager'):
            with patch('config.get_settings') as mock_settings:
                mock_settings.return_value.polygon_api_key = "test_key"
                
                provider = PolygonWebSocketProvider(mock_config)
                provider._streaming = True
                
                # Add test data to buffer
                test_data = MarketData(
                    time=datetime.now(),
                    symbol="AAPL",
                    open=150.0,
                    high=151.0,
                    low=149.0,
                    close=150.5,
                    volume=1000000,
                    provider="polygon_ws"
                )
                
                await provider.stream_buffer.push(test_data)
                
                # Stream data
                streamed = []
                stream_task = asyncio.create_task(
                    provider.stream_market_data(["AAPL"])
                )
                
                # Collect one item
                async for data in stream_task:
                    streamed.append(data)
                    break
                
                assert len(streamed) == 1
                assert streamed[0].symbol == "AAPL"
                assert streamed[0].close == 150.5
    
    @pytest.mark.asyncio
    async def test_statistics(self, mock_config):
        """Test provider statistics."""
        with patch('data_ingestion.providers.polygon_websocket.WebSocketManager'):
            with patch('config.get_settings') as mock_settings:
                mock_settings.return_value.polygon_api_key = "test_key"
                
                provider = PolygonWebSocketProvider(mock_config)
                provider.ws_manager.state = ConnectionState.CONNECTED
                provider.ws_manager.is_connected = True
                
                stats = provider.get_statistics()
                
                assert stats["connection_state"] == "CONNECTED"
                assert stats["is_connected"] is True
                assert "buffer_stats" in stats
                assert "active_subscriptions" in stats


# Integration test with mock WebSocket server
class TestPolygonWebSocketIntegration:
    """Integration tests with mock WebSocket server."""
    
    @pytest.mark.asyncio
    @pytest.mark.integration
    async def test_full_streaming_flow(self):
        """Test complete streaming flow with mock server."""
        # This would require a mock WebSocket server
        # For now, we'll skip the actual implementation
        pass


if __name__ == "__main__":
    pytest.main([__file__, "-v"])