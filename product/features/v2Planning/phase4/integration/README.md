# Python EventBus Bridge Integration

This directory contains the Python EventBus bridge implementation for the neural-trader data-ingestion service, enabling seamless migration from Redis to the high-performance Rust EventBus.

## 🚀 Features

- **Dual Publishing**: Publish to both Redis and EventBus during migration
- **Feature Flags**: Gradual rollout with percentage-based routing
- **Circuit Breaker**: Automatic failover and recovery
- **Comprehensive Monitoring**: Metrics collection and performance tracking
- **Error Handling**: Robust retry mechanisms and fallback strategies
- **Production Ready**: Battle-tested patterns for high-throughput scenarios

## 📁 File Overview

### Core Implementation
- **`python_eventbus_bridge.py`** - Main EventBus bridge with async/await support
- **`data_ingestion_integration.py`** - Data ingestion service integration with migration support

### Configuration & Examples
- **`config_example.py`** - Configuration examples for different environments
- **`requirements.txt`** - Python dependencies

### Testing
- **`test_eventbus_bridge.py`** - Comprehensive unit test suite

## 🔧 Installation

```bash
# Install dependencies
pip install -r requirements.txt

# For development
pip install -r requirements.txt[dev]
```

## 🚀 Quick Start

### Basic Usage

```python
import asyncio
from python_eventbus_bridge import create_eventbus_bridge, Event

async def main():
    async with create_eventbus_bridge() as bridge:
        # Publish an event
        event = Event(
            topic="market.data",
            payload={"symbol": "AAPL", "price": 150.25, "volume": 1000000},
            source="data-ingestion",
            event_type="market_tick"
        )
        
        await bridge.publish(event)
        
        # Subscribe to events
        async def handle_event(event):
            print(f"Received: {event.payload}")
            
        subscriber = await bridge.subscribe("market.data", handle_event)
        
        # Keep running
        await asyncio.sleep(10)

if __name__ == "__main__":
    asyncio.run(main())
```

### Data Ingestion Integration

```python
import asyncio
from data_ingestion_integration import create_data_ingestion_publisher, FeatureFlags

async def main():
    # Configure for gradual migration
    flags = FeatureFlags(
        enable_eventbus=True,
        enable_dual_publish=True,
        eventbus_percentage=0.5,  # 50% to EventBus
        fallback_to_redis=True
    )
    
    async with create_data_ingestion_publisher(feature_flags=flags) as publisher:
        # Publish market data
        result = await publisher.publish_market_data(
            symbol="AAPL",
            price=150.25,
            volume=1000000
        )
        
        print(f"Published successfully: {result.success}")
        print(f"Redis: {result.redis_success}, EventBus: {result.eventbus_success}")
        
        # Get metrics
        metrics = await publisher.get_metrics()
        print(f"Metrics: {metrics}")

if __name__ == "__main__":
    asyncio.run(main())
```

## 🔄 Migration Strategy

### Phase 0: Baseline (Redis Only)
```python
flags = FeatureFlags(
    enable_eventbus=False,
    enable_dual_publish=False,
    eventbus_percentage=0.0
)
```

### Phase 1: Initial Rollout (10% EventBus)
```python
flags = FeatureFlags(
    enable_eventbus=True,
    enable_dual_publish=True,
    eventbus_percentage=0.1
)
```

### Phase 2: Partial Migration (50% EventBus)
```python
flags = FeatureFlags(
    enable_eventbus=True,
    enable_dual_publish=True,
    eventbus_percentage=0.5
)
```

### Phase 3: Near Complete (90% EventBus)
```python
flags = FeatureFlags(
    enable_eventbus=True,
    enable_dual_publish=True,
    eventbus_percentage=0.9
)
```

### Phase 4: Complete Migration (EventBus Only)
```python
flags = FeatureFlags(
    enable_eventbus=True,
    enable_dual_publish=False,
    eventbus_percentage=1.0
)
```

## ⚙️ Configuration

### Environment Variables

```bash
# EventBus Configuration
EVENTBUS_HOST=localhost
EVENTBUS_PORT=8080

# Redis Configuration
REDIS_HOST=localhost
REDIS_PORT=6379
REDIS_DB=0
REDIS_PASSWORD=secret

# Feature Flags
ENABLE_EVENTBUS=true
ENABLE_DUAL_PUBLISH=true
EVENTBUS_PERCENTAGE=0.5
ENABLE_BENCHMARKING=true
FALLBACK_TO_REDIS=true
```

### Programmatic Configuration

```python
from python_eventbus_bridge import EventBusConfig
from data_ingestion_integration import FeatureFlags, RedisConfig

# EventBus Configuration
eventbus_config = EventBusConfig(
    host="localhost",
    port=8080,
    max_retries=5,
    connection_timeout=10.0,
    max_connections=100,
    circuit_breaker_threshold=10,
    enable_metrics=True
)

# Redis Configuration
redis_config = RedisConfig(
    host="localhost",
    port=6379,
    db=0,
    max_connections=50
)

# Feature Flags
feature_flags = FeatureFlags(
    enable_eventbus=True,
    enable_dual_publish=True,
    eventbus_percentage=0.5,
    enable_benchmarking=True,
    fallback_to_redis=True
)
```

## 📊 Monitoring & Metrics

### Available Metrics

- **Counters**: Total events, successes, failures by topic and strategy
- **Gauges**: Connection status, circuit breaker state
- **Histograms**: Publish latency, processing time

### Health Check

```python
async with create_data_ingestion_publisher() as publisher:
    health = await publisher.health_check()
    print(f"Overall status: {health['status']}")
    print(f"Redis status: {health['redis']['status']}")
    print(f"EventBus status: {health['eventbus']['status']}")
```

### Performance Benchmarking

```python
# Run benchmark
python data_ingestion_integration.py benchmark
```

## 🛡️ Error Handling

### Circuit Breaker
- Automatically opens after configurable failure threshold
- Prevents cascading failures during EventBus outages
- Automatic recovery after timeout period

### Retry Logic
- Exponential backoff with jitter
- Configurable retry attempts and timeouts
- Different strategies for transient vs persistent errors

### Fallback Strategies
- EventBus Primary: Falls back to Redis on EventBus failure
- Redis Primary: Falls back to EventBus on Redis failure
- Dual Publish: Succeeds if either succeeds

## 🧪 Testing

### Run Tests
```bash
# Run all tests
pytest test_eventbus_bridge.py -v

# Run with coverage
pytest test_eventbus_bridge.py --cov=. --cov-report=html

# Run specific test class
pytest test_eventbus_bridge.py::TestDataIngestionPublisher -v
```

### Test Categories
- **Unit Tests**: Individual component testing
- **Integration Tests**: Cross-component interaction
- **Performance Tests**: Throughput and latency benchmarks
- **Failure Tests**: Circuit breaker and fallback scenarios

## 🐳 Docker Integration

### Environment Variables in Docker
```dockerfile
ENV EVENTBUS_HOST=eventbus-service
ENV EVENTBUS_PORT=8080
ENV REDIS_HOST=redis-service
ENV REDIS_PORT=6379
ENV ENABLE_EVENTBUS=true
ENV EVENTBUS_PERCENTAGE=0.1
```

### Docker Compose Example
```yaml
services:
  data-ingestion:
    build: .
    environment:
      - EVENTBUS_HOST=eventbus
      - REDIS_HOST=redis
      - ENABLE_EVENTBUS=true
      - EVENTBUS_PERCENTAGE=0.5
    depends_on:
      - redis
      - eventbus
```

## ☸️ Kubernetes Deployment

### ConfigMap
```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: eventbus-config
data:
  EVENTBUS_HOST: "eventbus-service"
  EVENTBUS_PORT: "8080"
  REDIS_HOST: "redis-service"
  ENABLE_EVENTBUS: "true"
  EVENTBUS_PERCENTAGE: "0.1"
```

### Deployment
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: data-ingestion
spec:
  template:
    spec:
      containers:
      - name: data-ingestion
        image: neural-trader/data-ingestion:latest
        envFrom:
        - configMapRef:
            name: eventbus-config
```

## 📈 Performance Considerations

### High Throughput Configuration
```python
# Optimized for high throughput
config = EventBusConfig(
    max_connections=200,
    connection_timeout=3.0,
    request_timeout=10.0,
    max_retries=3,
    circuit_breaker_threshold=20
)

flags = FeatureFlags(
    enable_dual_publish=False,  # Single publish for speed
    enable_detailed_logging=False,  # Reduce overhead
    fallback_to_redis=False  # Avoid fallback delays
)
```

### Memory Usage
- Connection pooling to limit memory usage
- Configurable buffer sizes for subscriptions
- Automatic cleanup of completed operations

### Network Optimization
- HTTP/2 support for EventBus connections
- Connection reuse and keepalive
- Compression for large payloads

## 🔐 Security Considerations

### Authentication
- Support for API keys and JWT tokens
- TLS/SSL encryption for all connections
- Certificate validation and pinning

### Data Privacy
- Payload encryption for sensitive data
- Configurable data retention policies
- Audit logging for compliance

## 🚨 Troubleshooting

### Common Issues

1. **Connection Timeouts**
   - Increase `connection_timeout` in config
   - Check network connectivity
   - Verify service health

2. **High Error Rates**
   - Check circuit breaker status
   - Review EventBus service logs
   - Validate event payload format

3. **Memory Usage**
   - Reduce `max_connections`
   - Enable connection cleanup
   - Monitor subscription count

4. **Performance Issues**
   - Disable detailed logging
   - Use single publish strategy
   - Optimize payload size

### Debug Mode

```python
import logging
logging.basicConfig(level=logging.DEBUG)

flags = FeatureFlags(
    enable_detailed_logging=True,
    enable_benchmarking=True
)
```

## 📚 Additional Resources

- [EventBus API Documentation](../../../docs/eventbus-api.md)
- [Migration Playbook](../../../docs/migration-guide.md)
- [Performance Tuning Guide](../../../docs/performance-tuning.md)
- [Monitoring Setup](../../../docs/monitoring-setup.md)

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

---

**Note**: This implementation follows the SPARC methodology and integrates with the neural-trader's Phase 4 EventBus migration strategy. For production deployment, please review the configuration examples and adjust settings according to your specific requirements.