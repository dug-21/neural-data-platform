# Data Ingestion Service Integration Guide

## Required Updates for Monitoring Support

To support the new production configuration, the data-ingestion service needs the following updates:

### 1. Health Check Endpoint

Implement a health check endpoint at `/health` on port 8001:

```python
from fastapi import FastAPI
from datetime import datetime
import os

app = FastAPI()

@app.get("/health")
async def health_check():
    """Health check endpoint for monitoring"""
    health_status = {
        "status": "healthy",
        "timestamp": datetime.utcnow().isoformat(),
        "service": "data-ingestion",
        "version": os.getenv("SERVICE_VERSION", "1.0.0"),
        "checks": {
            "database": check_database_connection(),
            "redis": check_redis_connection(),
            "websocket": check_websocket_status()
        }
    }
    
    # Return 200 if all checks pass, 503 if any fail
    all_healthy = all(health_status["checks"].values())
    status_code = 200 if all_healthy else 503
    
    return JSONResponse(content=health_status, status_code=status_code)
```

### 2. Metrics Endpoint

Implement Prometheus metrics endpoint at `/metrics` on port 9091:

```python
from prometheus_client import Counter, Histogram, Gauge, generate_latest
from prometheus_client.core import CollectorRegistry
from fastapi.responses import PlainTextResponse

# Metrics registry
registry = CollectorRegistry()

# Define metrics
api_calls = Counter(
    'data_ingestion_api_calls_total',
    'Total API calls made',
    ['provider', 'endpoint'],
    registry=registry
)

api_errors = Counter(
    'data_ingestion_api_errors_total',
    'Total API errors',
    ['provider', 'error_type'],
    registry=registry
)

processing_duration = Histogram(
    'data_ingestion_processing_duration_seconds',
    'Time spent processing data',
    ['provider', 'data_type'],
    registry=registry
)

queue_size = Gauge(
    'data_ingestion_queue_size',
    'Current queue size',
    ['provider'],
    registry=registry
)

websocket_connections = Gauge(
    'websocket_connections_active',
    'Active WebSocket connections',
    ['provider'],
    registry=registry
)

websocket_messages = Counter(
    'websocket_messages_total',
    'Total WebSocket messages received',
    ['provider', 'message_type'],
    registry=registry
)

@app.get("/metrics", response_class=PlainTextResponse)
async def metrics():
    """Prometheus metrics endpoint"""
    return generate_latest(registry)
```

### 3. WebSocket Health Monitoring

Add WebSocket health check functionality:

```python
import asyncio
from typing import Dict, Any

class WebSocketHealthMonitor:
    def __init__(self):
        self.connection_status: Dict[str, Any] = {}
        self.last_message_time: Dict[str, datetime] = {}
        self.reconnection_count: Dict[str, int] = {}
    
    async def monitor_health(self, provider: str):
        """Monitor WebSocket connection health"""
        while True:
            try:
                # Check connection status
                if provider in self.connection_status:
                    status = self.connection_status[provider]
                    
                    # Update metrics
                    if status.get('connected'):
                        websocket_connections.labels(provider=provider).set(1)
                    else:
                        websocket_connections.labels(provider=provider).set(0)
                    
                    # Check for stale connections
                    last_msg = self.last_message_time.get(provider)
                    if last_msg:
                        time_since_last = (datetime.utcnow() - last_msg).seconds
                        if time_since_last > 60:  # No message for 60 seconds
                            await self.reconnect_websocket(provider)
                
                await asyncio.sleep(30)  # Check every 30 seconds
                
            except Exception as e:
                logger.error(f"Health monitor error for {provider}: {e}")
                await asyncio.sleep(30)
    
    async def reconnect_websocket(self, provider: str):
        """Reconnect WebSocket with tracking"""
        self.reconnection_count[provider] = self.reconnection_count.get(provider, 0) + 1
        websocket_reconnections.labels(provider=provider).inc()
        # Implement reconnection logic here
```

### 4. Environment Variable Configuration

Add support for the new environment variables:

```python
import os

# Health check configuration
HEALTH_CHECK_ENABLED = os.getenv('HEALTH_CHECK_ENABLED', 'true').lower() == 'true'
HEALTH_CHECK_PORT = int(os.getenv('HEALTH_CHECK_PORT', '8001'))

# Metrics configuration
METRICS_ENABLED = os.getenv('METRICS_ENABLED', 'true').lower() == 'true'
METRICS_PORT = int(os.getenv('METRICS_PORT', '9091'))

# WebSocket health configuration
WEBSOCKET_HEALTH_CHECK_ENABLED = os.getenv('WEBSOCKET_HEALTH_CHECK_ENABLED', 'true').lower() == 'true'
WEBSOCKET_HEALTH_CHECK_INTERVAL = int(os.getenv('WEBSOCKET_HEALTH_CHECK_INTERVAL', '30'))
```

### 5. Service Startup

Update the main service startup to include both servers:

```python
import uvicorn
from multiprocessing import Process

def start_health_server():
    """Start health check server"""
    if HEALTH_CHECK_ENABLED:
        uvicorn.run(
            "health_app:app",
            host="0.0.0.0",
            port=HEALTH_CHECK_PORT,
            log_level="info"
        )

def start_metrics_server():
    """Start metrics server"""
    if METRICS_ENABLED:
        uvicorn.run(
            "metrics_app:app",
            host="0.0.0.0",
            port=METRICS_PORT,
            log_level="info"
        )

if __name__ == "__main__":
    # Start health and metrics servers in separate processes
    health_process = Process(target=start_health_server)
    metrics_process = Process(target=start_metrics_server)
    
    health_process.start()
    metrics_process.start()
    
    # Start main application
    # ... your main app startup code ...
```

### 6. Metric Instrumentation

Instrument your code to track metrics:

```python
# In API call functions
async def call_provider_api(provider: str, endpoint: str):
    api_calls.labels(provider=provider, endpoint=endpoint).inc()
    
    try:
        with processing_duration.labels(provider=provider, data_type='api').time():
            response = await make_api_call(provider, endpoint)
        return response
    except Exception as e:
        api_errors.labels(provider=provider, error_type=type(e).__name__).inc()
        raise

# In WebSocket handlers
async def on_websocket_message(provider: str, message: dict):
    websocket_messages.labels(
        provider=provider, 
        message_type=message.get('type', 'unknown')
    ).inc()
    
    # Update last message time for health monitoring
    health_monitor.last_message_time[provider] = datetime.utcnow()
```

### 7. Testing the Integration

Test the new endpoints:

```bash
# Test health check
curl http://localhost:8001/health

# Test metrics
curl http://localhost:9091/metrics

# Verify in Prometheus
# Check http://localhost:9090/targets to see if endpoints are being scraped
```

## Implementation Checklist

- [ ] Implement health check endpoint with comprehensive checks
- [ ] Implement metrics endpoint with all required metrics
- [ ] Add WebSocket health monitoring
- [ ] Support new environment variables
- [ ] Update service startup for multiple servers
- [ ] Instrument code with metric tracking
- [ ] Test all endpoints
- [ ] Verify Prometheus scraping
- [ ] Update documentation

## Notes

- Keep health checks lightweight to avoid impacting performance
- Use appropriate metric types (Counter, Gauge, Histogram)
- Consider adding custom business metrics specific to your use case
- Implement graceful degradation if monitoring systems fail
- Test metric cardinality to avoid explosion