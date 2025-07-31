"""Comprehensive tests for WebSocket resilience features."""
import pytest
import asyncio
import time
from unittest.mock import Mock, AsyncMock, patch, MagicMock
from datetime import datetime, timedelta
from collections import deque

from data_ingestion.providers.alpaca import AlpacaProvider
from data_ingestion.providers.base import MarketData
from data_ingestion.utils.circuit_breaker import CircuitBreaker, CircuitState, CircuitBreakerConfig


class TestCircuitBreaker:
    """Test circuit breaker functionality."""
    
    @pytest.fixture
    def circuit_breaker(self):
        """Create a circuit breaker with test configuration."""
        config = CircuitBreakerConfig(
            failure_threshold=3,
            success_threshold=2,
            timeout=1.0,  # 1 second for faster tests
            half_open_requests=2
        )
        return CircuitBreaker(config)
        
    def test_initial_state(self, circuit_breaker):
        """Test circuit breaker starts in closed state."""
        assert circuit_breaker.is_closed
        assert circuit_breaker.should_allow_request()
        
    @pytest.mark.asyncio
    async def test_circuit_opens_after_failures(self, circuit_breaker):
        """Test circuit opens after reaching failure threshold."""
        # Record failures up to threshold
        for _ in range(3):
            await circuit_breaker.record_failure()
            
        assert circuit_breaker.is_open
        assert not circuit_breaker.should_allow_request()
        
    @pytest.mark.asyncio
    async def test_circuit_transitions_to_half_open(self, circuit_breaker):
        """Test circuit transitions to half-open after timeout."""
        # Open the circuit
        for _ in range(3):
            await circuit_breaker.record_failure()
            
        assert circuit_breaker.is_open
        
        # Wait for timeout
        await asyncio.sleep(1.1)
        
        # Should transition to half-open
        assert circuit_breaker.should_allow_request()
        assert circuit_breaker.is_half_open
        
    @pytest.mark.asyncio
    async def test_circuit_closes_after_successes(self, circuit_breaker):
        """Test circuit closes after successful requests in half-open state."""
        # Open the circuit
        for _ in range(3):
            await circuit_breaker.record_failure()
            
        # Wait for timeout
        await asyncio.sleep(1.1)
        
        # Allow request (transitions to half-open)
        assert circuit_breaker.should_allow_request()
        
        # Record successes
        for _ in range(2):
            await circuit_breaker.record_success()
            
        assert circuit_breaker.is_closed
        
    @pytest.mark.asyncio
    async def test_circuit_reopens_on_half_open_failure(self, circuit_breaker):
        """Test circuit reopens if request fails in half-open state."""
        # Open the circuit
        for _ in range(3):
            await circuit_breaker.record_failure()
            
        # Wait for timeout
        await asyncio.sleep(1.1)
        
        # Allow request (transitions to half-open)
        assert circuit_breaker.should_allow_request()
        
        # Record failure
        await circuit_breaker.record_failure()
        
        assert circuit_breaker.is_open
        
    def test_circuit_stats(self, circuit_breaker):
        """Test circuit breaker statistics tracking."""
        stats = circuit_breaker.get_stats()
        
        assert stats['state'] == 'closed'
        assert stats['total_requests'] == 0
        assert stats['success_rate'] == 0.0
        
    @pytest.mark.asyncio
    async def test_callbacks(self):
        """Test circuit breaker callbacks are invoked."""
        open_called = False
        close_called = False
        half_open_called = False
        
        def on_open():
            nonlocal open_called
            open_called = True
            
        def on_close():
            nonlocal close_called
            close_called = True
            
        def on_half_open():
            nonlocal half_open_called
            half_open_called = True
            
        config = CircuitBreakerConfig(
            failure_threshold=2,
            success_threshold=1,
            timeout=0.5,
            on_open=on_open,
            on_close=on_close,
            on_half_open=on_half_open
        )
        cb = CircuitBreaker(config)
        
        # Trigger open
        await cb.record_failure()
        await cb.record_failure()
        assert open_called
        
        # Trigger half-open
        await asyncio.sleep(0.6)
        cb.should_allow_request()
        assert half_open_called
        
        # Trigger close
        await cb.record_success()
        assert close_called


class TestAlpacaWebSocketResilience:
    """Test Alpaca provider WebSocket resilience features."""
    
    @pytest.fixture
    def mock_settings(self):
        """Create mock settings."""
        settings = Mock()
        settings.alpaca_api_key = "test_key"
        settings.alpaca_api_secret = "test_secret"
        settings.alpaca_subscription_level = "basic"
        settings.alpaca_ws_enabled = True
        return settings
        
    @pytest.fixture
    def alpaca_provider(self, mock_settings):
        """Create Alpaca provider with mocked settings."""
        with patch('data_ingestion.providers.alpaca.get_settings', return_value=mock_settings):
            provider = AlpacaProvider()
            provider.logger = Mock()
            return provider
            
    def test_resilience_initialization(self, alpaca_provider):
        """Test resilience features are properly initialized."""
        assert alpaca_provider.max_reconnect_attempts == 100
        assert alpaca_provider.reconnect_delay == 1.0
        assert isinstance(alpaca_provider.message_buffer, deque)
        assert alpaca_provider.message_buffer.maxlen == 10000
        assert isinstance(alpaca_provider.circuit_breaker, CircuitBreaker)
        
    @pytest.mark.asyncio
    async def test_enhanced_reconnect_with_backoff(self, alpaca_provider):
        """Test enhanced reconnection with exponential backoff."""
        # Mock the connect and run methods to fail
        alpaca_provider._connect_websocket = AsyncMock(side_effect=Exception("Connection failed"))
        alpaca_provider.stock_stream = Mock()
        alpaca_provider.stock_stream.run = AsyncMock(side_effect=Exception("Stream failed"))
        
        # Limit attempts for testing
        alpaca_provider.max_reconnect_attempts = 3
        
        start_time = time.time()
        await alpaca_provider._enhanced_reconnect()
        elapsed = time.time() - start_time
        
        # Should have attempted 3 times with exponential backoff
        assert alpaca_provider.reconnect_attempts == 3
        # Minimum time with backoff: 1 + 2 + 4 = 7 seconds (plus jitter)
        assert elapsed >= 6  # Allow some margin
        
    @pytest.mark.asyncio
    async def test_circuit_breaker_integration(self, alpaca_provider):
        """Test circuit breaker prevents connection storms."""
        # Configure circuit breaker to open quickly
        alpaca_provider.circuit_breaker.config.failure_threshold = 2
        
        # Mock connection to fail
        alpaca_provider._connect_websocket = AsyncMock(side_effect=Exception("Connection failed"))
        alpaca_provider.stock_stream = Mock()
        
        # Record initial failures to open circuit
        await alpaca_provider.circuit_breaker.record_failure()
        await alpaca_provider.circuit_breaker.record_failure()
        
        assert alpaca_provider.circuit_breaker.is_open
        
        # Enhanced reconnect should respect circuit breaker
        alpaca_provider.max_reconnect_attempts = 5
        start_time = time.time()
        await alpaca_provider._enhanced_reconnect()
        elapsed = time.time() - start_time
        
        # Should have waited for circuit breaker
        assert elapsed >= 10  # We sleep 10s when circuit is open
        
    @pytest.mark.asyncio
    async def test_message_buffering(self, alpaca_provider):
        """Test message buffering when queue is full."""
        # Create a small queue to test buffering
        alpaca_provider._ws_data_queue = asyncio.Queue(maxsize=2)
        
        # Fill the queue
        await alpaca_provider._ws_data_queue.put("msg1")
        await alpaca_provider._ws_data_queue.put("msg2")
        
        # Create a mock trade
        trade = Mock()
        trade.timestamp = datetime.now()
        trade.symbol = "AAPL"
        trade.price = 150.0
        trade.size = 100
        trade.exchange = "NASDAQ"
        trade.conditions = []
        
        # Call the handler - should buffer the message
        handler = alpaca_provider._ws_handlers['trade']
        await handler(trade)
        
        # Check message was buffered
        assert len(alpaca_provider.message_buffer) == 1
        assert alpaca_provider.message_buffer[0].symbol == "AAPL"
        
    @pytest.mark.asyncio
    async def test_health_check_monitoring(self, alpaca_provider):
        """Test WebSocket health check monitoring."""
        alpaca_provider._ws_connected = True
        alpaca_provider._last_message_time = datetime.now() - timedelta(seconds=65)
        alpaca_provider.stock_stream = Mock()
        alpaca_provider.stock_stream.stop = Mock()
        
        # Create health check task
        health_task = asyncio.create_task(alpaca_provider._websocket_health_check())
        
        # Let it run one iteration
        await asyncio.sleep(0.1)
        
        # Cancel the task
        health_task.cancel()
        try:
            await health_task
        except asyncio.CancelledError:
            pass
            
        # Should have detected stale connection
        alpaca_provider.logger.warning.assert_called()
        
    def test_connection_stats(self, alpaca_provider):
        """Test connection statistics reporting."""
        alpaca_provider._ws_connected = True
        alpaca_provider.reconnect_attempts = 5
        alpaca_provider._connection_start_time = datetime.now() - timedelta(hours=2)
        alpaca_provider._last_message_time = datetime.now() - timedelta(seconds=30)
        alpaca_provider._ws_subscriptions = {"AAPL", "GOOGL"}
        
        stats = alpaca_provider.get_connection_stats()
        
        assert stats['connected'] is True
        assert stats['reconnect_attempts'] == 5
        assert stats['subscribed_symbols'] == 2
        assert 'uptime_hours' in stats
        assert stats['uptime_hours'] >= 2.0
        assert 'last_message_age_seconds' in stats
        assert stats['last_message_age_seconds'] >= 30
        
    @pytest.mark.asyncio
    async def test_message_handler_updates_last_message_time(self, alpaca_provider):
        """Test message handlers update last message time."""
        # Test trade handler
        trade = Mock()
        trade.timestamp = datetime.now()
        trade.symbol = "AAPL"
        trade.price = 150.0
        trade.size = 100
        trade.exchange = "NASDAQ"
        trade.conditions = []
        
        handler = alpaca_provider._ws_handlers['trade']
        await handler(trade)
        
        assert alpaca_provider._last_message_time is not None
        assert (datetime.now() - alpaca_provider._last_message_time).total_seconds() < 1
        
    @pytest.mark.asyncio
    async def test_buffer_drain_on_reconnect(self, alpaca_provider):
        """Test message buffer is drained on reconnection."""
        # Add messages to buffer
        for i in range(5):
            msg = MarketData(
                time=datetime.now(),
                symbol=f"TEST{i}",
                open=100.0,
                high=101.0,
                low=99.0,
                close=100.5,
                volume=1000,
                provider="alpaca"
            )
            alpaca_provider.message_buffer.append(msg)
            
        # Mock stream to fail once then succeed
        alpaca_provider.stock_stream = Mock()
        alpaca_provider.stock_stream.run = AsyncMock(side_effect=[Exception("Failed"), None])
        alpaca_provider._connect_websocket = AsyncMock()
        
        # Run websocket with reconnection
        await alpaca_provider._run_websocket()
        
        # Buffer should have been processed
        assert alpaca_provider.logger.info.call_count > 0
        
    @pytest.mark.asyncio 
    async def test_resilience_under_load(self, alpaca_provider):
        """Test resilience features under high load."""
        # Configure for stress test
        alpaca_provider._ws_data_queue = asyncio.Queue(maxsize=100)
        
        # Generate high volume of messages
        messages_sent = 0
        messages_buffered = 0
        
        for i in range(200):
            trade = Mock()
            trade.timestamp = datetime.now()
            trade.symbol = f"TEST{i}"
            trade.price = 100.0 + i
            trade.size = 100
            trade.exchange = "TEST"
            trade.conditions = []
            
            handler = alpaca_provider._ws_handlers['trade']
            await handler(trade)
            messages_sent += 1
            
            if len(alpaca_provider.message_buffer) > messages_buffered:
                messages_buffered = len(alpaca_provider.message_buffer)
                
        # Should have buffered messages when queue was full
        assert messages_buffered > 0
        assert alpaca_provider._ws_data_queue.qsize() == 100  # Queue at max
        
        
class TestIntegration:
    """Integration tests for WebSocket resilience."""
    
    @pytest.mark.asyncio
    async def test_full_reconnection_flow(self):
        """Test complete reconnection flow with all features."""
        with patch('data_ingestion.providers.alpaca.get_settings') as mock_settings:
            settings = Mock()
            settings.alpaca_api_key = "test"
            settings.alpaca_api_secret = "test"
            settings.alpaca_subscription_level = "basic"
            mock_settings.return_value = settings
            
            provider = AlpacaProvider()
            provider.logger = Mock()
            
            # Mock successful connection after 2 failures
            connect_calls = 0
            async def mock_connect():
                nonlocal connect_calls
                connect_calls += 1
                if connect_calls < 3:
                    raise Exception("Connection failed")
                    
            provider._connect_websocket = mock_connect
            provider.stock_stream = Mock()
            provider.stock_stream.run = AsyncMock()
            
            # Run enhanced reconnect
            await provider._enhanced_reconnect()
            
            # Should have succeeded after retries
            assert connect_calls == 3
            assert provider.reconnect_attempts == 0  # Reset on success
            assert provider.circuit_breaker.is_closed