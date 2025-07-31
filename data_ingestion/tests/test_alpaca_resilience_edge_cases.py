"""Edge case tests for Alpaca WebSocket resilience."""
import pytest
import asyncio
from unittest.mock import Mock, AsyncMock, patch
from datetime import datetime, timedelta
import websockets

from data_ingestion.providers.alpaca import AlpacaProvider
from data_ingestion.utils.circuit_breaker import CircuitBreaker, CircuitState


class TestEdgeCases:
    """Test edge cases and error conditions."""
    
    @pytest.fixture
    def alpaca_provider(self):
        """Create provider with mocked settings."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            settings = Mock()
            settings.alpaca_api_key = "test"
            settings.alpaca_api_secret = "test"
            settings.alpaca_subscription_level = "basic"
            mock_settings.return_value = settings
            
            provider = AlpacaProvider()
            provider.logger = Mock()
            return provider
            
    @pytest.mark.asyncio
    async def test_reconnect_with_none_stock_stream(self, alpaca_provider):
        """Test reconnection when stock_stream is None."""
        alpaca_provider.stock_stream = None
        
        await alpaca_provider._run_websocket()
        
        # Should log error and return early
        alpaca_provider.logger.error.assert_called_with(
            "StockDataStream not available for WebSocket connection"
        )
        
    @pytest.mark.asyncio
    async def test_health_check_with_exceptions(self, alpaca_provider):
        """Test health check handles exceptions gracefully."""
        alpaca_provider._ws_connected = True
        
        # Mock datetime to raise exception
        with patch('data_ingestion.providers.alpaca.datetime') as mock_dt:
            mock_dt.now.side_effect = Exception("Time error")
            
            # Run health check
            health_task = asyncio.create_task(alpaca_provider._websocket_health_check())
            await asyncio.sleep(0.1)
            
            # Cancel task
            health_task.cancel()
            try:
                await health_task
            except asyncio.CancelledError:
                pass
                
            # Should have logged error
            alpaca_provider.logger.error.assert_called()
            
    @pytest.mark.asyncio
    async def test_circuit_breaker_prevents_rapid_retries(self, alpaca_provider):
        """Test circuit breaker prevents connection storms."""
        # Set up circuit breaker to open immediately
        alpaca_provider.circuit_breaker.config.failure_threshold = 1
        alpaca_provider.max_reconnect_attempts = 10
        
        # Mock connection to always fail
        alpaca_provider._connect_websocket = AsyncMock(
            side_effect=Exception("Connection failed")
        )
        
        # Track time between attempts
        attempt_times = []
        original_sleep = asyncio.sleep
        
        async def track_sleep(duration):
            attempt_times.append(duration)
            # Speed up test by reducing actual sleep
            await original_sleep(min(duration, 0.1))
            
        with patch('asyncio.sleep', track_sleep):
            await alpaca_provider._enhanced_reconnect()
            
        # Should have backed off exponentially
        assert len(attempt_times) > 0
        # Later attempts should have longer delays
        if len(attempt_times) > 2:
            assert attempt_times[-1] > attempt_times[0]
            
    @pytest.mark.asyncio
    async def test_message_buffer_overflow(self, alpaca_provider):
        """Test message buffer handles overflow correctly."""
        # Fill buffer to capacity
        for i in range(10001):  # Buffer maxlen is 10000
            alpaca_provider.message_buffer.append(f"msg{i}")
            
        # Buffer should maintain max size
        assert len(alpaca_provider.message_buffer) == 10000
        # Oldest messages should be dropped
        assert alpaca_provider.message_buffer[0] == "msg1"
        
    @pytest.mark.asyncio
    async def test_concurrent_health_check_tasks(self, alpaca_provider):
        """Test multiple health check tasks don't interfere."""
        alpaca_provider._ws_connected = True
        alpaca_provider._connection_start_time = datetime.now()
        
        # Start multiple health check tasks
        tasks = []
        for _ in range(3):
            task = asyncio.create_task(alpaca_provider._websocket_health_check())
            tasks.append(task)
            
        # Let them run briefly
        await asyncio.sleep(0.1)
        
        # Cancel all tasks
        for task in tasks:
            task.cancel()
            
        # Wait for cancellation
        for task in tasks:
            try:
                await task
            except asyncio.CancelledError:
                pass
                
        # Should not have any errors
        assert True  # If we get here, no exceptions were raised
        
    @pytest.mark.asyncio
    async def test_websocket_with_invalid_credentials(self, alpaca_provider):
        """Test handling of authentication failures."""
        # Mock WebSocket connection
        mock_ws = AsyncMock()
        mock_ws.recv = AsyncMock(side_effect=[
            '{"T": "error", "msg": "authentication failed"}',
        ])
        mock_ws.closed = False
        
        with patch('websockets.connect', AsyncMock(return_value=mock_ws)):
            with pytest.raises(ConnectionError, match="authentication failed"):
                async for _ in alpaca_provider._stream_via_websocket(["AAPL"]):
                    pass
                    
    def test_connection_stats_with_no_connection(self, alpaca_provider):
        """Test stats when never connected."""
        stats = alpaca_provider.get_connection_stats()
        
        assert stats['connected'] is False
        assert 'uptime_hours' not in stats
        assert 'last_message_age_seconds' not in stats
        
    @pytest.mark.asyncio
    async def test_cleanup_on_disconnect(self, alpaca_provider):
        """Test proper cleanup on disconnect."""
        # Set up connected state
        alpaca_provider._ws_connected = True
        alpaca_provider._stream_task = asyncio.create_task(asyncio.sleep(10))
        alpaca_provider.stock_stream = Mock()
        alpaca_provider.stock_stream.close = AsyncMock()
        
        # Disconnect
        await alpaca_provider.disconnect()
        
        # Should have cleaned up
        assert alpaca_provider._ws_connected is False
        assert alpaca_provider._stream_task.cancelled()
        alpaca_provider.stock_stream.close.assert_called()
        
    @pytest.mark.asyncio
    async def test_queue_put_exception_handling(self, alpaca_provider):
        """Test handling of queue put exceptions."""
        # Mock queue to raise exception
        alpaca_provider._ws_data_queue = Mock()
        alpaca_provider._ws_data_queue.put_nowait = Mock(
            side_effect=Exception("Queue error")
        )
        
        # Create mock trade
        trade = Mock()
        trade.timestamp = datetime.now()
        trade.symbol = "AAPL"
        trade.price = 150.0
        trade.size = 100
        trade.exchange = "NASDAQ"
        trade.conditions = []
        
        # Should not raise exception
        handler = alpaca_provider._ws_handlers['trade']
        await handler(trade)
        
        # Message should still be buffered
        assert len(alpaca_provider.message_buffer) > 0


class TestRaceConditions:
    """Test for potential race conditions."""
    
    @pytest.mark.asyncio
    async def test_concurrent_reconnection_attempts(self):
        """Test multiple reconnection attempts don't conflict."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            settings = Mock()
            settings.alpaca_api_key = "test"
            settings.alpaca_api_secret = "test"
            mock_settings.return_value = settings
            
            provider = AlpacaProvider()
            provider.logger = Mock()
            
            # Mock connection to track concurrent calls
            connection_count = 0
            connection_lock = asyncio.Lock()
            
            async def mock_connect():
                nonlocal connection_count
                async with connection_lock:
                    connection_count += 1
                    await asyncio.sleep(0.1)
                    
            provider._connect_websocket = mock_connect
            provider.stock_stream = Mock()
            provider.stock_stream.run = AsyncMock()
            
            # Start multiple reconnection attempts
            tasks = []
            for _ in range(5):
                task = asyncio.create_task(provider._enhanced_reconnect())
                tasks.append(task)
                
            # Wait for all to complete
            await asyncio.gather(*tasks, return_exceptions=True)
            
            # Should have handled concurrent attempts gracefully
            assert connection_count > 0
            
    @pytest.mark.asyncio
    async def test_circuit_breaker_thread_safety(self):
        """Test circuit breaker handles concurrent access."""
        cb = CircuitBreaker()
        
        # Create concurrent failure recordings
        async def record_failures():
            for _ in range(10):
                await cb.record_failure()
                await asyncio.sleep(0.001)
                
        # Run multiple tasks concurrently
        tasks = [record_failures() for _ in range(5)]
        await asyncio.gather(*tasks)
        
        # Circuit should be open
        assert cb.is_open
        # Stats should be consistent
        stats = cb.get_stats()
        assert stats['total_failures'] == 50  # 5 tasks * 10 failures