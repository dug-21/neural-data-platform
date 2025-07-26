"""Test script for Phase 2 Health Check Implementation.

This test validates the enhanced health check system with:
- Circuit breaker functionality
- Code-first approach (no env vars required)
- Comprehensive health status checks
- Prometheus metrics integration

Run this test with:
    python -m pytest tests/test_health_check_phase2.py -v
"""
import asyncio
import aiohttp
import json
import time
from datetime import datetime, timedelta
import pytest
from unittest.mock import Mock, AsyncMock, patch

from utils.health_check import HealthCheckHandler, CircuitBreaker, CircuitBreakerState


class TestCircuitBreaker:
    """Test circuit breaker functionality."""
    
    def test_circuit_breaker_initial_state(self):
        """Test circuit breaker starts in CLOSED state."""
        breaker = CircuitBreaker()
        assert breaker.state == CircuitBreakerState.CLOSED
        assert breaker.should_allow_request() is True
        assert breaker.failure_count == 0
        assert breaker.success_count == 0
    
    def test_circuit_breaker_opens_after_threshold(self):
        """Test circuit breaker opens after failure threshold."""
        breaker = CircuitBreaker(failure_threshold=3)
        
        # First two failures don't open the breaker
        breaker.record_failure()
        assert breaker.state == CircuitBreakerState.CLOSED
        breaker.record_failure()
        assert breaker.state == CircuitBreakerState.CLOSED
        
        # Third failure opens the breaker
        breaker.record_failure()
        assert breaker.state == CircuitBreakerState.OPEN
        assert breaker.should_allow_request() is False
    
    def test_circuit_breaker_recovery(self):
        """Test circuit breaker recovery after timeout."""
        breaker = CircuitBreaker(failure_threshold=1, recovery_timeout=0.1)
        
        # Open the breaker
        breaker.record_failure()
        assert breaker.state == CircuitBreakerState.OPEN
        assert breaker.should_allow_request() is False
        
        # Wait for recovery timeout
        time.sleep(0.2)
        
        # Should transition to HALF_OPEN
        assert breaker.should_allow_request() is True
        assert breaker.state == CircuitBreakerState.HALF_OPEN
    
    def test_circuit_breaker_half_open_to_closed(self):
        """Test circuit breaker transitions from HALF_OPEN to CLOSED."""
        breaker = CircuitBreaker(
            failure_threshold=1, 
            recovery_timeout=0.1, 
            success_threshold=2
        )
        
        # Open the breaker
        breaker.record_failure()
        time.sleep(0.2)
        breaker.should_allow_request()  # Transition to HALF_OPEN
        
        # Record successes
        breaker.record_success()
        assert breaker.state == CircuitBreakerState.HALF_OPEN
        
        breaker.record_success()
        assert breaker.state == CircuitBreakerState.CLOSED
        assert breaker.failure_count == 0


class TestHealthCheckHandler:
    """Test enhanced health check handler."""
    
    @pytest.fixture
    async def handler(self):
        """Create health check handler instance."""
        handler = HealthCheckHandler(port=8080)
        # Mock components
        handler.timescale_db = AsyncMock()
        handler.redis_store = AsyncMock()
        handler.realtime_coordinator = Mock()
        handler.stream_manager = Mock()
        yield handler
        if handler.runner:
            await handler.stop()
    
    async def test_health_check_without_env_vars(self, handler):
        """Test health check works without environment variables."""
        # Handler should initialize without requiring env vars
        assert handler.port == 8080
        assert handler.max_data_age_seconds == 300
        assert handler.min_success_rate == 0.8
        assert handler.min_active_streams == 1
        
        # Circuit breakers should be initialized
        assert 'database' in handler.circuit_breakers
        assert 'redis' in handler.circuit_breakers
        assert 'websocket' in handler.circuit_breakers
        assert 'data_flow' in handler.circuit_breakers
    
    async def test_database_health_with_circuit_breaker(self, handler):
        """Test database health check with circuit breaker."""
        # Mock successful database connection
        handler.timescale_db.pool.acquire.return_value.__aenter__.return_value.fetchval.return_value = 1
        
        # First check should succeed
        healthy, message = await handler.check_database_health()
        assert healthy is True
        assert message == "Connected"
        
        # Simulate failures
        handler.timescale_db.pool.acquire.side_effect = Exception("Connection failed")
        
        # Record failures up to threshold
        for _ in range(5):
            healthy, message = await handler.check_database_health()
            assert healthy is False
        
        # Circuit breaker should be open
        breaker = handler.circuit_breakers['database']
        assert breaker.state == CircuitBreakerState.OPEN
        
        # Next request should be blocked by circuit breaker
        healthy, message = await handler.check_database_health()
        assert healthy is False
        assert "Circuit breaker OPEN" in message
    
    async def test_redis_health_with_timeout(self, handler):
        """Test Redis health check with timeout."""
        # Mock Redis ping that times out
        async def slow_ping():
            await asyncio.sleep(5)  # Longer than timeout
            return True
        
        handler.redis_store.redis.ping = slow_ping
        
        # Should timeout and record failure
        healthy, message = await handler.check_redis_health()
        assert healthy is False
        assert "timeout" in message.lower()
    
    async def test_websocket_health_tracking(self, handler):
        """Test WebSocket health status tracking."""
        # Mock WebSocket providers
        mock_provider = Mock()
        mock_provider.ws = Mock(closed=False)
        mock_provider.subscribed_symbols = ['AAPL', 'GOOGL']
        
        handler.realtime_coordinator.providers = {
            'alpaca': mock_provider
        }
        
        # Check WebSocket health
        ws_status = handler.check_websocket_health()
        
        assert ws_status['total_providers'] == 1
        assert ws_status['active_connections'] == 1
        assert ws_status['healthy'] is True
        assert ws_status['circuit_breaker'] == 'closed'
        assert 'alpaca' in ws_status['providers']
        assert ws_status['providers']['alpaca']['connected'] is True
        assert ws_status['providers']['alpaca']['subscribed_symbols'] == 2
    
    async def test_data_flow_freshness(self, handler):
        """Test data flow freshness monitoring."""
        # Add fresh data timestamp
        now = datetime.now()
        handler.update_data_timestamp('alpaca', 'AAPL')
        
        # Check data flow health
        flow_status = handler.check_data_flow_health()
        
        assert flow_status['total_flows'] == 1
        assert flow_status['active_flows'] == 1
        assert flow_status['stale_flows'] == 0
        assert flow_status['healthy'] is True
        
        # Simulate stale data
        old_timestamp = now - timedelta(minutes=10)
        handler.last_data_timestamps['alpaca:AAPL'] = old_timestamp
        
        flow_status = handler.check_data_flow_health()
        assert flow_status['stale_flows'] == 1
        assert flow_status['healthy'] is False
    
    async def test_comprehensive_health_status(self, handler):
        """Test comprehensive health status aggregation."""
        # Mock all components as healthy
        handler.timescale_db.pool.acquire.return_value.__aenter__.return_value.fetchval.return_value = 1
        handler.redis_store.redis.ping.return_value = True
        
        mock_provider = Mock()
        mock_provider.ws = Mock(closed=False)
        handler.realtime_coordinator.providers = {'alpaca': mock_provider}
        
        handler.update_data_timestamp('alpaca', 'AAPL')
        
        handler.stream_manager.active_streams = {
            'stream1': {
                'status': 'running',
                'error_count': 0,
                'success_count': 100
            }
        }
        
        # Get comprehensive status
        status = await handler.get_health_status()
        
        assert status['status'] == 'healthy'
        assert 'timestamp' in status
        assert status['checks']['database']['healthy'] is True
        assert status['checks']['redis']['healthy'] is True
        assert status['checks']['websockets']['healthy'] is True
        assert status['checks']['data_flow']['healthy'] is True
        assert status['checks']['streams']['healthy'] is True
    
    async def test_health_endpoint_response(self, handler):
        """Test health check endpoint response format."""
        # Start the health check server
        await handler.start(port=8081)  # Use different port to avoid conflicts
        
        # Mock healthy components
        handler.timescale_db.pool.acquire.return_value.__aenter__.return_value.fetchval.return_value = 1
        handler.redis_store.redis.ping.return_value = True
        
        # Make health check request
        async with aiohttp.ClientSession() as session:
            async with session.get('http://localhost:8081/health') as response:
                assert response.status == 200
                data = await response.json()
                
                assert 'status' in data
                assert 'timestamp' in data
                assert 'circuit_breakers' in data
                
                # Check circuit breaker status
                for component in ['database', 'redis', 'websocket', 'data_flow']:
                    assert component in data['circuit_breakers']
                    assert 'state' in data['circuit_breakers'][component]
                    assert 'failures' in data['circuit_breakers'][component]


async def test_integration_scenario():
    """Integration test simulating real-world scenario."""
    handler = HealthCheckHandler(port=8082)
    
    try:
        # Start health check server
        await handler.start()
        
        # Simulate component initialization
        handler.timescale_db = AsyncMock()
        handler.redis_store = AsyncMock()
        handler.timescale_db.pool.acquire.return_value.__aenter__.return_value.fetchval.return_value = 1
        handler.redis_store.redis.ping.return_value = True
        
        # Test multiple health checks
        async with aiohttp.ClientSession() as session:
            # First check should be healthy
            async with session.get('http://localhost:8082/health') as response:
                assert response.status == 200
                data = await response.json()
                assert data['status'] == 'healthy'
            
            # Simulate database failure
            handler.timescale_db.pool.acquire.side_effect = Exception("DB down")
            
            # Health check should reflect unhealthy status
            async with session.get('http://localhost:8082/health') as response:
                assert response.status == 503
                data = await response.json()
                assert data['status'] == 'unhealthy'
            
            # Detailed health check should show component status
            async with session.get('http://localhost:8082/health/detailed') as response:
                assert response.status == 503
                data = await response.json()
                assert data['checks']['database']['healthy'] is False
                assert "DB down" in data['checks']['database']['message']
    
    finally:
        await handler.stop()


if __name__ == "__main__":
    # Run basic integration test
    print("Running Phase 2 Health Check Tests...")
    asyncio.run(test_integration_scenario())
    print("✅ Integration test passed!")
    
    print("\nFor full test suite, run:")
    print("  python -m pytest tests/test_health_check_phase2.py -v")