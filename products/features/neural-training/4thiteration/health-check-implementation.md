# Data Ingestion Health Check Implementation

## Overview

The data ingestion service currently lacks a proper health check endpoint. It only exposes Prometheus metrics on port 8001 (mapped to 8000 inside container). This implementation adds a comprehensive health check system that:

1. Provides HTTP health endpoint returning 200/503 status
2. Checks WebSocket connection status for real-time data providers
3. Validates data flow recency to ensure streaming is working
4. Integrates with Docker health check
5. Exposes detailed status in JSON response
6. Maintains compatibility with existing Prometheus metrics

## Analysis of Current State

### Found Issues:
- No health check endpoint in `main.py`
- No health check defined in `docker-compose.yml` for data-ingestion service
- Prometheus metrics server runs on port 8000 (internal) / 8001 (external)
- WebSocket connections tracked in `RealtimeCoordinator` and `StreamManager`
- No unified health status aggregation

### Existing Components to Leverage:
- `StreamManager.stream_health` - tracks stream health metrics
- `RealtimeCoordinator.active_streams` - tracks active WebSocket connections
- `metrics.active_streams` gauge - Prometheus metric for active streams
- `aiohttp` is already installed and used

## Implementation Plan

### 1. Health Check Handler Module

Create `data_ingestion/utils/health_check.py`:

```python
"""Health check implementation for data ingestion service."""
import asyncio
from datetime import datetime, timedelta
from typing import Dict, Any, List, Optional, Tuple
from aiohttp import web
import json

from utils.logging import get_logger
from utils.metrics import metrics
from config import get_settings

logger = get_logger(__name__)


class HealthCheckHandler:
    """Handles health check requests and monitors service health."""
    
    def __init__(self):
        self.settings = get_settings()
        self.logger = logger
        
        # Health check thresholds
        self.max_data_age_seconds = 300  # 5 minutes
        self.min_success_rate = 0.8  # 80% success rate
        self.min_active_streams = 1  # At least one active stream
        
        # Component references (set by main service)
        self.realtime_coordinator = None
        self.stream_manager = None
        self.batch_scheduler = None
        self.timescale_db = None
        self.redis_store = None
        
        # Track last successful data timestamps
        self.last_data_timestamps: Dict[str, datetime] = {}
        
        # Health check web app
        self.app = web.Application()
        self._setup_routes()
        self.runner = None
        
    def _setup_routes(self):
        """Setup health check routes."""
        self.app.router.add_get('/health', self.health_check)
        self.app.router.add_get('/health/detailed', self.detailed_health_check)
        self.app.router.add_get('/health/live', self.liveness_probe)
        self.app.router.add_get('/health/ready', self.readiness_probe)
        
    async def start(self, port: int = 8080):
        """Start health check HTTP server."""
        self.runner = web.AppRunner(self.app)
        await self.runner.setup()
        site = web.TCPSite(self.runner, '0.0.0.0', port)
        await site.start()
        self.logger.info(f"Health check server started on port {port}")
        
    async def stop(self):
        """Stop health check server."""
        if self.runner:
            await self.runner.cleanup()
            self.logger.info("Health check server stopped")
    
    def update_data_timestamp(self, provider: str, symbol: str):
        """Update timestamp for last received data."""
        key = f"{provider}:{symbol}"
        self.last_data_timestamps[key] = datetime.now()
        
    async def check_database_health(self) -> Tuple[bool, str]:
        """Check database connectivity."""
        try:
            if self.timescale_db and hasattr(self.timescale_db, 'pool'):
                # Execute simple query to check connection
                async with self.timescale_db.pool.acquire() as conn:
                    result = await conn.fetchval("SELECT 1")
                    return result == 1, "Connected"
            return False, "No database connection"
        except Exception as e:
            return False, f"Database error: {str(e)}"
    
    async def check_redis_health(self) -> Tuple[bool, str]:
        """Check Redis connectivity."""
        try:
            if self.redis_store and hasattr(self.redis_store, 'redis'):
                # Ping Redis
                pong = await self.redis_store.redis.ping()
                return pong, "Connected"
            return False, "No Redis connection"
        except Exception as e:
            return False, f"Redis error: {str(e)}"
    
    def check_websocket_health(self) -> Dict[str, Any]:
        """Check WebSocket connection status."""
        ws_status = {
            'total_providers': 0,
            'active_connections': 0,
            'providers': {}
        }
        
        if self.realtime_coordinator:
            ws_status['total_providers'] = len(self.realtime_coordinator.providers)
            
            for provider_name, provider in self.realtime_coordinator.providers.items():
                provider_status = {
                    'connected': False,
                    'subscribed_symbols': 0,
                    'last_error': None
                }
                
                # Check if provider has WebSocket connection
                if hasattr(provider, 'ws') and provider.ws:
                    provider_status['connected'] = not provider.ws.closed
                    if provider_status['connected']:
                        ws_status['active_connections'] += 1
                        
                # Check subscribed symbols
                if hasattr(provider, 'subscribed_symbols'):
                    provider_status['subscribed_symbols'] = len(provider.subscribed_symbols)
                    
                ws_status['providers'][provider_name] = provider_status
                
        return ws_status
    
    def check_data_flow_health(self) -> Dict[str, Any]:
        """Check data flow recency."""
        now = datetime.now()
        flow_status = {
            'total_flows': len(self.last_data_timestamps),
            'active_flows': 0,
            'stale_flows': 0,
            'oldest_data_age_seconds': None,
            'details': []
        }
        
        for key, timestamp in self.last_data_timestamps.items():
            age_seconds = (now - timestamp).total_seconds()
            is_stale = age_seconds > self.max_data_age_seconds
            
            flow_status['details'].append({
                'flow': key,
                'last_update': timestamp.isoformat(),
                'age_seconds': age_seconds,
                'is_stale': is_stale
            })
            
            if is_stale:
                flow_status['stale_flows'] += 1
            else:
                flow_status['active_flows'] += 1
                
            if flow_status['oldest_data_age_seconds'] is None or age_seconds > flow_status['oldest_data_age_seconds']:
                flow_status['oldest_data_age_seconds'] = age_seconds
                
        return flow_status
    
    def check_stream_health(self) -> Dict[str, Any]:
        """Check stream manager health."""
        stream_status = {
            'total_streams': 0,
            'active_streams': 0,
            'error_streams': 0,
            'success_rate': 0.0,
            'streams': {}
        }
        
        if self.stream_manager:
            stream_status['total_streams'] = len(self.stream_manager.active_streams)
            
            total_operations = 0
            successful_operations = 0
            
            for stream_id, stream_info in self.stream_manager.active_streams.items():
                is_active = stream_info.get('status') == 'running'
                error_count = stream_info.get('error_count', 0)
                success_count = stream_info.get('success_count', 0)
                
                if is_active:
                    stream_status['active_streams'] += 1
                if error_count > success_count:
                    stream_status['error_streams'] += 1
                    
                total_operations += error_count + success_count
                successful_operations += success_count
                
                stream_status['streams'][stream_id] = {
                    'status': stream_info.get('status'),
                    'error_count': error_count,
                    'success_count': success_count,
                    'last_data': stream_info.get('last_data')
                }
            
            if total_operations > 0:
                stream_status['success_rate'] = successful_operations / total_operations
                
        return stream_status
    
    async def get_health_status(self) -> Dict[str, Any]:
        """Get comprehensive health status."""
        # Database health
        db_healthy, db_message = await self.check_database_health()
        
        # Redis health
        redis_healthy, redis_message = await self.check_redis_health()
        
        # WebSocket health
        ws_status = self.check_websocket_health()
        ws_healthy = ws_status['active_connections'] > 0
        
        # Data flow health
        flow_status = self.check_data_flow_health()
        flow_healthy = flow_status['active_flows'] >= self.min_active_streams and flow_status['stale_flows'] == 0
        
        # Stream health
        stream_status = self.check_stream_health()
        stream_healthy = (
            stream_status['active_streams'] >= self.min_active_streams and
            stream_status['success_rate'] >= self.min_success_rate
        )
        
        # Overall health
        is_healthy = all([db_healthy, redis_healthy, ws_healthy or stream_healthy, flow_healthy])
        
        return {
            'status': 'healthy' if is_healthy else 'unhealthy',
            'timestamp': datetime.now().isoformat(),
            'checks': {
                'database': {
                    'healthy': db_healthy,
                    'message': db_message
                },
                'redis': {
                    'healthy': redis_healthy,
                    'message': redis_message
                },
                'websockets': {
                    'healthy': ws_healthy,
                    'details': ws_status
                },
                'data_flow': {
                    'healthy': flow_healthy,
                    'details': flow_status
                },
                'streams': {
                    'healthy': stream_healthy,
                    'details': stream_status
                }
            },
            'metrics': {
                'active_streams': metrics.active_streams._value.get() if hasattr(metrics.active_streams, '_value') else 0,
                'total_data_points': metrics.data_points_processed._value.sum() if hasattr(metrics.data_points_processed, '_value') else 0,
                'total_errors': metrics.processing_errors._value.sum() if hasattr(metrics.processing_errors, '_value') else 0
            }
        }
    
    async def health_check(self, request: web.Request) -> web.Response:
        """Simple health check endpoint."""
        try:
            status = await self.get_health_status()
            is_healthy = status['status'] == 'healthy'
            
            return web.Response(
                text=json.dumps({
                    'status': status['status'],
                    'timestamp': status['timestamp']
                }),
                status=200 if is_healthy else 503,
                content_type='application/json'
            )
        except Exception as e:
            self.logger.error(f"Health check error: {e}")
            return web.Response(
                text=json.dumps({
                    'status': 'error',
                    'message': str(e)
                }),
                status=503,
                content_type='application/json'
            )
    
    async def detailed_health_check(self, request: web.Request) -> web.Response:
        """Detailed health check endpoint."""
        try:
            status = await self.get_health_status()
            is_healthy = status['status'] == 'healthy'
            
            return web.Response(
                text=json.dumps(status, indent=2),
                status=200 if is_healthy else 503,
                content_type='application/json'
            )
        except Exception as e:
            self.logger.error(f"Detailed health check error: {e}")
            return web.Response(
                text=json.dumps({
                    'status': 'error',
                    'message': str(e),
                    'timestamp': datetime.now().isoformat()
                }),
                status=503,
                content_type='application/json'
            )
    
    async def liveness_probe(self, request: web.Request) -> web.Response:
        """Kubernetes liveness probe - checks if service is alive."""
        return web.Response(text='OK', status=200)
    
    async def readiness_probe(self, request: web.Request) -> web.Response:
        """Kubernetes readiness probe - checks if service is ready to accept traffic."""
        try:
            # Quick checks for readiness
            db_healthy, _ = await self.check_database_health()
            redis_healthy, _ = await self.check_redis_health()
            
            if db_healthy and redis_healthy:
                return web.Response(text='OK', status=200)
            else:
                return web.Response(text='Not Ready', status=503)
        except Exception as e:
            return web.Response(text=f'Error: {str(e)}', status=503)
```

### 2. Update Main Service

Modify `data_ingestion/main.py` to integrate health check:

```python
# Add to imports
from utils.health_check import HealthCheckHandler

# In DataIngestionService.__init__
self.health_check_handler = HealthCheckHandler()

# In DataIngestionService.start method, after initializing components:
# Set component references for health checks
self.health_check_handler.realtime_coordinator = self.realtime_coordinator
self.health_check_handler.stream_manager = self.stream_manager
self.health_check_handler.batch_scheduler = self.batch_scheduler
self.health_check_handler.timescale_db = self.realtime_coordinator.timescale if self.realtime_coordinator else None
self.health_check_handler.redis_store = self.realtime_coordinator.redis if self.realtime_coordinator else None

# Start health check server
await self.health_check_handler.start(port=8080)
logger.info("Health check server started on port 8080")

# In DataIngestionService.shutdown method:
# Stop health check server
if self.health_check_handler:
    await self.health_check_handler.stop()
```

### 3. Update Data Processing to Track Health

Modify data processing to update health timestamps:

In `schedulers/realtime_coordinator.py`:
```python
# In _process_market_data method, after successful processing:
if hasattr(self, 'health_check_handler') and self.health_check_handler:
    self.health_check_handler.update_data_timestamp(provider_name, data['symbol'])
```

### 4. Docker Compose Health Check

Update `docker-compose.yml` to add health check for data-ingestion service:

```yaml
data-ingestion:
  # ... existing configuration ...
  ports:
    - "8001:8000"  # Prometheus metrics
    - "8080:8080"  # Health check endpoint
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
    interval: 30s
    timeout: 10s
    retries: 3
    start_period: 60s
```

### 5. Prometheus Metrics Integration

Add health-specific Prometheus metrics in `utils/metrics.py`:

```python
# Add to Metrics class
self.health_check_status = Gauge(
    'data_ingestion_health_status',
    'Overall health status (1=healthy, 0=unhealthy)'
)

self.health_check_component_status = Gauge(
    'data_ingestion_health_component_status',
    'Component health status (1=healthy, 0=unhealthy)',
    ['component']
)

self.data_flow_age_seconds = Gauge(
    'data_ingestion_data_flow_age_seconds',
    'Age of last received data in seconds',
    ['provider', 'symbol']
)
```

Update health check handler to export metrics:

```python
# In HealthCheckHandler.get_health_status method:
# Update Prometheus metrics
metrics.health_check_status.set(1 if is_healthy else 0)
metrics.health_check_component_status.labels(component='database').set(1 if db_healthy else 0)
metrics.health_check_component_status.labels(component='redis').set(1 if redis_healthy else 0)
metrics.health_check_component_status.labels(component='websockets').set(1 if ws_healthy else 0)
metrics.health_check_component_status.labels(component='data_flow').set(1 if flow_healthy else 0)
metrics.health_check_component_status.labels(component='streams').set(1 if stream_healthy else 0)

# Update data flow age metrics
for key, timestamp in self.last_data_timestamps.items():
    provider, symbol = key.split(':', 1)
    age_seconds = (datetime.now() - timestamp).total_seconds()
    metrics.data_flow_age_seconds.labels(provider=provider, symbol=symbol).set(age_seconds)
```

## API Endpoints

### 1. `/health` - Simple Health Check
- Returns 200 if healthy, 503 if unhealthy
- Minimal JSON response with status and timestamp
- Used by Docker health check

### 2. `/health/detailed` - Detailed Health Status
- Returns comprehensive health information
- Includes all component statuses and metrics
- Useful for debugging and monitoring dashboards

### 3. `/health/live` - Liveness Probe
- Always returns 200 if service is running
- Used by Kubernetes liveness probe

### 4. `/health/ready` - Readiness Probe
- Returns 200 if service is ready to accept traffic
- Checks database and Redis connectivity
- Used by Kubernetes readiness probe

## Health Check Response Examples

### Simple Health Check (200 OK):
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:45.123456"
}
```

### Simple Health Check (503 Service Unavailable):
```json
{
  "status": "unhealthy",
  "timestamp": "2024-01-15T10:30:45.123456"
}
```

### Detailed Health Check:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:45.123456",
  "checks": {
    "database": {
      "healthy": true,
      "message": "Connected"
    },
    "redis": {
      "healthy": true,
      "message": "Connected"
    },
    "websockets": {
      "healthy": true,
      "details": {
        "total_providers": 3,
        "active_connections": 2,
        "providers": {
          "polygon": {
            "connected": true,
            "subscribed_symbols": 10,
            "last_error": null
          },
          "alpaca": {
            "connected": true,
            "subscribed_symbols": 5,
            "last_error": null
          },
          "finnhub": {
            "connected": false,
            "subscribed_symbols": 0,
            "last_error": "Connection timeout"
          }
        }
      }
    },
    "data_flow": {
      "healthy": true,
      "details": {
        "total_flows": 15,
        "active_flows": 15,
        "stale_flows": 0,
        "oldest_data_age_seconds": 45.2,
        "details": [
          {
            "flow": "polygon:AAPL",
            "last_update": "2024-01-15T10:30:00.000000",
            "age_seconds": 45.2,
            "is_stale": false
          }
        ]
      }
    },
    "streams": {
      "healthy": true,
      "details": {
        "total_streams": 3,
        "active_streams": 3,
        "error_streams": 0,
        "success_rate": 0.95,
        "streams": {
          "realtime_polygon": {
            "status": "running",
            "error_count": 5,
            "success_count": 95,
            "last_data": "2024-01-15T10:30:40.000000"
          }
        }
      }
    }
  },
  "metrics": {
    "active_streams": 3,
    "total_data_points": 150000,
    "total_errors": 12
  }
}
```

## Monitoring Integration

### Prometheus Queries

1. Overall health status:
```promql
data_ingestion_health_status
```

2. Component health breakdown:
```promql
data_ingestion_health_component_status
```

3. Data staleness alert:
```promql
data_ingestion_data_flow_age_seconds > 300
```

4. WebSocket connection failures:
```promql
sum(data_ingestion_health_component_status{component="websockets"} == 0) > 0
```

### Grafana Dashboard

Create a health monitoring dashboard with:
1. Overall health status (gauge)
2. Component health grid (stat panels)
3. Data flow age heatmap
4. Active connections over time
5. Success rate trends

## Testing

### Unit Tests

```python
# tests/test_health_check.py
import pytest
from unittest.mock import Mock, AsyncMock
from utils.health_check import HealthCheckHandler

@pytest.mark.asyncio
async def test_health_check_all_healthy():
    handler = HealthCheckHandler()
    
    # Mock components
    handler.timescale_db = Mock()
    handler.timescale_db.pool.acquire = AsyncMock()
    handler.redis_store = Mock()
    handler.redis_store.redis.ping = AsyncMock(return_value=True)
    
    # Mock coordinator and streams
    handler.realtime_coordinator = Mock()
    handler.realtime_coordinator.providers = {
        'test_provider': Mock(ws=Mock(closed=False))
    }
    
    # Update data timestamp
    handler.update_data_timestamp('test_provider', 'AAPL')
    
    # Get health status
    status = await handler.get_health_status()
    
    assert status['status'] == 'healthy'
    assert status['checks']['database']['healthy'] is True
    assert status['checks']['redis']['healthy'] is True

@pytest.mark.asyncio
async def test_health_check_stale_data():
    handler = HealthCheckHandler()
    handler.max_data_age_seconds = 60  # 1 minute for testing
    
    # Add old timestamp
    from datetime import datetime, timedelta
    old_time = datetime.now() - timedelta(minutes=5)
    handler.last_data_timestamps['test:AAPL'] = old_time
    
    flow_status = handler.check_data_flow_health()
    assert flow_status['stale_flows'] == 1
    assert flow_status['active_flows'] == 0
```

### Integration Tests

```bash
# Start service and test health endpoints
curl -f http://localhost:8080/health
curl -f http://localhost:8080/health/detailed
curl -f http://localhost:8080/health/live
curl -f http://localhost:8080/health/ready
```

## Benefits

1. **Comprehensive Health Monitoring**: Tracks all critical components
2. **Docker Integration**: Automatic container restarts on unhealthy state
3. **Kubernetes Ready**: Separate liveness and readiness probes
4. **Detailed Debugging**: Comprehensive status for troubleshooting
5. **Prometheus Integration**: Health metrics exposed for alerting
6. **Real-time Status**: Tracks WebSocket connections and data flow
7. **Configurable Thresholds**: Easy to adjust health criteria

## Future Enhancements

1. Add health check for batch scheduler
2. Implement circuit breaker pattern for failing providers
3. Add historical health metrics storage
4. Create health status WebSocket endpoint for real-time monitoring
5. Add provider-specific health checks (API key validation, rate limit status)
6. Implement automated recovery actions based on health status