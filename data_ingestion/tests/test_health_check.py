"""Tests for health check implementation."""
import pytest
from unittest.mock import Mock, AsyncMock, MagicMock
from datetime import datetime, timedelta
from aiohttp import web

from utils.health_check import HealthCheckHandler


@pytest.mark.asyncio
async def test_health_check_all_healthy():
    """Test health check when all components are healthy."""
    handler = HealthCheckHandler()
    
    # Mock TimescaleDB
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    
    # Mock connection context manager
    mock_conn = AsyncMock()
    mock_conn.fetchval = AsyncMock(return_value=1)
    handler.timescale_db.pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    handler.timescale_db.pool.acquire.return_value.__aexit__ = AsyncMock()
    
    # Mock Redis
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Mock RealtimeCoordinator with WebSocket connections
    handler.realtime_coordinator = Mock()
    handler.realtime_coordinator.providers = {
        'test_provider': Mock(
            ws=Mock(closed=False),
            subscribed_symbols=['AAPL', 'GOOGL']
        )
    }
    
    # Mock StreamManager
    handler.stream_manager = Mock()
    handler.stream_manager.active_streams = {
        'stream1': {
            'status': 'running',
            'error_count': 5,
            'success_count': 95,
            'last_data': datetime.now().isoformat()
        }
    }
    
    # Update data timestamp to make it fresh
    handler.update_data_timestamp('test_provider', 'AAPL')
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'healthy'
    assert status['checks']['database']['healthy'] is True
    assert status['checks']['redis']['healthy'] is True
    assert status['checks']['websockets']['healthy'] is True
    assert status['checks']['data_flow']['healthy'] is True
    assert status['checks']['streams']['healthy'] is True


@pytest.mark.asyncio
async def test_health_check_database_unhealthy():
    """Test health check when database is down."""
    handler = HealthCheckHandler()
    
    # Mock failed database connection
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock(side_effect=Exception("Connection failed"))
    
    # Mock healthy Redis
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'unhealthy'
    assert status['checks']['database']['healthy'] is False
    assert 'Connection failed' in status['checks']['database']['message']


@pytest.mark.asyncio
async def test_health_check_stale_data():
    """Test health check when data is stale."""
    handler = HealthCheckHandler()
    handler.max_data_age_seconds = 60  # 1 minute for testing
    
    # Mock healthy database and Redis
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    
    mock_conn = AsyncMock()
    mock_conn.fetchval = AsyncMock(return_value=1)
    handler.timescale_db.pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    handler.timescale_db.pool.acquire.return_value.__aexit__ = AsyncMock()
    
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Add old timestamp
    old_time = datetime.now() - timedelta(minutes=5)
    handler.last_data_timestamps['test:AAPL'] = old_time
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'unhealthy'
    assert status['checks']['data_flow']['healthy'] is False
    assert status['checks']['data_flow']['details']['stale_flows'] == 1


@pytest.mark.asyncio
async def test_health_check_websocket_disconnected():
    """Test health check when WebSocket is disconnected."""
    handler = HealthCheckHandler()
    
    # Mock healthy database and Redis
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    
    mock_conn = AsyncMock()
    mock_conn.fetchval = AsyncMock(return_value=1)
    handler.timescale_db.pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    handler.timescale_db.pool.acquire.return_value.__aexit__ = AsyncMock()
    
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Mock disconnected WebSocket
    handler.realtime_coordinator = Mock()
    handler.realtime_coordinator.providers = {
        'test_provider': Mock(
            ws=Mock(closed=True),
            subscribed_symbols=[]
        )
    }
    
    # No active streams
    handler.stream_manager = Mock()
    handler.stream_manager.active_streams = {}
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'unhealthy'
    assert status['checks']['websockets']['healthy'] is False
    assert status['checks']['websockets']['details']['active_connections'] == 0


@pytest.mark.asyncio
async def test_health_check_low_success_rate():
    """Test health check when success rate is low."""
    handler = HealthCheckHandler()
    
    # Mock healthy database and Redis
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    
    mock_conn = AsyncMock()
    mock_conn.fetchval = AsyncMock(return_value=1)
    handler.timescale_db.pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    handler.timescale_db.pool.acquire.return_value.__aexit__ = AsyncMock()
    
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Mock StreamManager with low success rate
    handler.stream_manager = Mock()
    handler.stream_manager.active_streams = {
        'stream1': {
            'status': 'running',
            'error_count': 80,
            'success_count': 20,
            'last_data': datetime.now().isoformat()
        }
    }
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'unhealthy'
    assert status['checks']['streams']['healthy'] is False
    assert status['checks']['streams']['details']['success_rate'] == 0.2


@pytest.mark.asyncio
async def test_liveness_probe():
    """Test Kubernetes liveness probe."""
    handler = HealthCheckHandler()
    
    # Create mock request
    request = Mock(spec=web.Request)
    
    # Call liveness probe
    response = await handler.liveness_probe(request)
    
    assert response.status == 200
    assert response.text == 'OK'


@pytest.mark.asyncio
async def test_readiness_probe_ready():
    """Test Kubernetes readiness probe when ready."""
    handler = HealthCheckHandler()
    
    # Mock healthy database and Redis
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    
    mock_conn = AsyncMock()
    mock_conn.fetchval = AsyncMock(return_value=1)
    handler.timescale_db.pool.acquire.return_value.__aenter__ = AsyncMock(return_value=mock_conn)
    handler.timescale_db.pool.acquire.return_value.__aexit__ = AsyncMock()
    
    handler.redis_store = Mock()
    handler.redis_store.redis = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Create mock request
    request = Mock(spec=web.Request)
    
    # Call readiness probe
    response = await handler.readiness_probe(request)
    
    assert response.status == 200
    assert response.text == 'OK'


@pytest.mark.asyncio
async def test_readiness_probe_not_ready():
    """Test Kubernetes readiness probe when not ready."""
    handler = HealthCheckHandler()
    
    # Mock unhealthy database
    handler.timescale_db = Mock()
    handler.timescale_db.pool = Mock()
    handler.timescale_db.pool.acquire = AsyncMock(side_effect=Exception("Not connected"))
    
    # Create mock request
    request = Mock(spec=web.Request)
    
    # Call readiness probe
    response = await handler.readiness_probe(request)
    
    assert response.status == 503
    assert response.text == 'Not Ready'


def test_update_data_timestamp():
    """Test updating data timestamp."""
    handler = HealthCheckHandler()
    
    # Update timestamp
    handler.update_data_timestamp('test_provider', 'AAPL')
    
    # Check it was recorded
    assert 'test_provider:AAPL' in handler.last_data_timestamps
    assert isinstance(handler.last_data_timestamps['test_provider:AAPL'], datetime)
    
    # Update again
    first_time = handler.last_data_timestamps['test_provider:AAPL']
    handler.update_data_timestamp('test_provider', 'AAPL')
    
    # Should have newer timestamp
    assert handler.last_data_timestamps['test_provider:AAPL'] >= first_time