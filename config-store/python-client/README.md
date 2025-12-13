# Python Config Store Client

A comprehensive Python client for the neural-trader config-store system, implementing advanced features like hybrid configuration loading, fallback mechanisms, security filtering, and deep integration with the Rust config-store service.

## Features

### Core Functionality
- ✅ **Hierarchical Configuration Loading** - Load configurations from multiple sources with prioritization
- ✅ **Environment Variable Fallback** - Automatic fallback to environment variables when service unavailable
- ✅ **Multi-Level Caching** - Intelligent caching with TTL support and automatic eviction
- ✅ **Type Conversion** - Built-in support for string, int, float, bool, list, and dict types
- ✅ **Connection Pooling** - Efficient HTTP connection management with configurable pool sizes
- ✅ **Retry Logic** - Exponential backoff with jitter for robust error handling
- ✅ **Circuit Breaker Pattern** - Prevents cascade failures with automatic recovery
- ✅ **Security Filtering** - Automatic detection and filtering of sensitive data
- ✅ **Health Monitoring** - Service health checks and diagnostics

### Advanced Features
- ✅ **Async/Await Support** - Full asyncio compatibility for non-blocking operations
- ✅ **Context Manager Support** - Proper resource management with async context managers
- ✅ **Configurable Timeouts** - Fine-grained control over connection and request timeouts
- ✅ **Cache Statistics** - Detailed metrics on cache performance and hit ratios
- ✅ **Comprehensive Error Handling** - Specific error types with detailed context
- ✅ **Logging Integration** - Structured logging with configurable levels

## Installation

```bash
pip install httpx redis pydantic pytest pytest-asyncio
```

## Quick Start

```python
import asyncio
from config_store_client import ConfigStoreClient, ConfigStoreConfig

async def main():
    # Configure the client
    config = ConfigStoreConfig(
        service_url="http://localhost:8080",
        timeout=30.0,
        cache_ttl=300,  # 5 minutes
        enable_env_fallback=True,
        env_prefix="NEURAL_TRADER"
    )
    
    # Use client as async context manager
    async with ConfigStoreClient(config) as client:
        # Get configuration with automatic type conversion
        api_key = await client.get_string("trading.api.binance.key")
        timeout = await client.get_int("trading.timeout", default=30)
        enabled = await client.get_bool("trading.enabled", default=True)
        
        # Health check
        health = await client.health_check()
        print(f"Service status: {health['status']}")

asyncio.run(main())
```

## Configuration Options

### ConfigStoreConfig Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `service_url` | str | "http://localhost:8080" | Config store service URL |
| `timeout` | float | 30.0 | Request timeout in seconds |
| `connection_pool_size` | int | 10 | HTTP connection pool size |
| `cache_ttl` | int | 300 | Cache TTL in seconds |
| `cache_max_size` | int | 10000 | Maximum cache entries |
| `cache_enabled` | bool | True | Enable/disable caching |
| `enable_env_fallback` | bool | True | Enable environment variable fallback |
| `env_prefix` | str | "NEURAL_TRADER" | Environment variable prefix |
| `circuit_breaker_enabled` | bool | True | Enable circuit breaker |
| `failure_threshold` | int | 5 | Circuit breaker failure threshold |
| `recovery_timeout` | int | 60 | Circuit breaker recovery timeout |
| `enable_security_filtering` | bool | True | Enable sensitive data filtering |

### RetryConfig Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `max_attempts` | int | 3 | Maximum retry attempts |
| `base_delay` | float | 1.0 | Base delay between retries |
| `backoff_multiplier` | float | 2.0 | Exponential backoff multiplier |
| `max_delay` | float | 60.0 | Maximum delay between retries |
| `jitter_enabled` | bool | True | Add jitter to retry delays |

## API Reference

### ConfigStoreClient Methods

#### Basic Operations

```python
async def get(path: str, default: Any = None, ttl: Optional[int] = None) -> Any
async def set(path: str, value: Any, ttl: Optional[int] = None) -> None
async def delete(path: str) -> None
async def exists(path: str) -> bool
async def list_keys(prefix: str = "") -> List[str]
```

#### Type Conversion Methods

```python
async def get_string(path: str, default: Optional[str] = None) -> str
async def get_int(path: str, default: Optional[int] = None) -> int  
async def get_float(path: str, default: Optional[float] = None) -> float
async def get_bool(path: str, default: Optional[bool] = None) -> bool
async def get_list(path: str, default: Optional[List] = None) -> List
async def get_dict(path: str, default: Optional[Dict] = None) -> Dict
```

#### Utility Methods

```python
async def health_check() -> Dict[str, Any]
async def clear_cache(prefix: Optional[str] = None) -> None
def get_cache_stats() -> Dict[str, Any]
```

## Environment Variable Integration

The client automatically maps configuration paths to environment variables:

| Configuration Path | Environment Variable |
|-------------------|---------------------|
| `trading.api.binance.key` | `NEURAL_TRADER_TRADING_API_BINANCE_KEY` |
| `database.host` | `NEURAL_TRADER_DATABASE_HOST` |
| `app.debug` | `NEURAL_TRADER_APP_DEBUG` |

## Error Handling

The client provides specific error types for different failure scenarios:

```python
from config_store_client.errors import (
    ConfigNotFoundError,      # Configuration key not found
    ConfigValidationError,    # Type conversion failed
    ConfigConnectionError,    # Service connection failed
    ConfigTimeoutError,       # Operation timed out
    ConfigSecurityError       # Security constraint violated
)

try:
    value = await client.get_string("missing.key")
except ConfigNotFoundError as e:
    print(f"Key not found: {e.key}")
except ConfigValidationError as e:
    print(f"Validation failed: {e.reason}")
```

## Circuit Breaker Pattern

The client implements a circuit breaker to prevent cascade failures:

- **Closed**: Normal operation, requests pass through
- **Open**: Service is failing, requests are blocked
- **Half-Open**: Testing if service has recovered

```python
# Circuit breaker automatically manages state
health = await client.health_check()
print(f"Circuit breaker state: {health['circuit_breaker_state']}")
```

## Caching System

The client features a multi-level caching system with:

- **TTL-based expiration** - Automatic cache invalidation
- **LRU eviction** - Removes least recently used entries when full
- **Cache statistics** - Monitor cache performance

```python
# Get cache statistics
stats = client.get_cache_stats()
print(f"Cache hit ratio: {stats['cache_hit_ratio']:.2%}")
print(f"Active entries: {stats['active_entries']}")

# Clear cache
await client.clear_cache()  # Clear all
await client.clear_cache("trading")  # Clear prefix
```

## Security Features

### Sensitive Data Filtering

The client automatically detects and filters sensitive data:

```python
# These patterns are automatically filtered:
# - API keys (api_key, api-key)
# - Passwords (password)  
# - Tokens (token)
# - Private keys (-----BEGIN PRIVATE KEY-----)

api_key = await client.get("trading.api.key")
# Returns: "***FILTERED***" instead of actual key
```

### Access Control

Configuration access is controlled through path-based permissions and security contexts.

## Testing

The implementation includes comprehensive tests covering:

- ✅ Basic configuration operations
- ✅ Type conversion and validation
- ✅ Caching behavior and expiration
- ✅ Environment variable fallback
- ✅ Retry logic and circuit breaker
- ✅ Error handling scenarios
- ✅ Security filtering
- ✅ Concurrent request handling

Run tests:

```bash
cd /workspaces/neural-trader/src/config_store_client
python -m pytest test_client.py -v
```

## Performance Characteristics

### Benchmarks

Based on the implementation, expected performance characteristics:

- **Cache Hit Latency**: < 1ms (in-memory lookup)
- **Service Request**: 10-50ms (depending on network)
- **Environment Fallback**: < 1ms (OS environment lookup)
- **Concurrent Requests**: Supports 10+ concurrent connections
- **Memory Usage**: ~100KB base + ~1KB per cached configuration

### Optimization Features

- **Connection Pooling**: Reuses HTTP connections
- **Request Batching**: Efficient bulk operations  
- **Lazy Initialization**: Connections created on demand
- **Memory Management**: Automatic cache eviction

## Integration Examples

### With Data Ingestion Service

```python
async def setup_data_ingestion():
    async with ConfigStoreClient() as config:
        # Load data source configuration
        binance_key = await config.get_string("data.sources.binance.api_key")
        polygon_key = await config.get_string("data.sources.polygon.api_key")
        symbols = await config.get_list("data.ingestion.symbols")
        batch_size = await config.get_int("data.ingestion.batch_size", default=1000)
        
        # Configure data providers
        providers = {
            "binance": {"api_key": binance_key},
            "polygon": {"api_key": polygon_key}
        }
        
        return DataIngestionService(
            providers=providers,
            symbols=symbols,
            batch_size=batch_size
        )
```

### With Neural Trading System

```python
async def setup_neural_trader():
    async with ConfigStoreClient() as config:
        # Load trading configuration
        max_position = await config.get_float("trading.limits.max_position")
        risk_tolerance = await config.get_float("trading.risk.tolerance")
        enable_paper_trading = await config.get_bool("trading.paper_trading", default=True)
        
        # Load neural model configuration
        model_path = await config.get_string("neural.model.path")
        training_interval = await config.get_int("neural.training.interval_hours", default=24)
        
        return NeuralTrader(
            max_position=max_position,
            risk_tolerance=risk_tolerance,
            paper_trading=enable_paper_trading,
            model_path=model_path,
            training_interval=training_interval
        )
```

## Production Deployment

### Docker Integration

```dockerfile
FROM python:3.11-slim

WORKDIR /app
COPY requirements.txt .
RUN pip install -r requirements.txt

COPY src/config_store_client ./config_store_client
ENV PYTHONPATH=/app

# Set configuration via environment
ENV NEURAL_TRADER_CONFIG_STORE_URL=http://config-store:8080
ENV NEURAL_TRADER_CACHE_TTL=600
ENV NEURAL_TRADER_ENABLE_CIRCUIT_BREAKER=true

CMD ["python", "-m", "myapp"]
```

### Kubernetes ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: neural-trader-config
data:
  CONFIG_STORE_URL: "http://config-store-service:8080"
  NEURAL_TRADER_CACHE_TTL: "600"
  NEURAL_TRADER_CONNECTION_POOL_SIZE: "20"
  NEURAL_TRADER_ENABLE_ENV_FALLBACK: "true"
```

## Monitoring and Observability

### Health Checks

```python
async def monitor_config_health():
    async with ConfigStoreClient() as client:
        health = await client.health_check()
        
        if health["status"] == "unhealthy":
            # Alert monitoring system
            alert_manager.send_alert(
                message="Config store service unavailable",
                severity="warning"
            )
```

### Metrics Collection

```python
# The client provides built-in metrics
stats = client.get_cache_stats()
metrics = {
    "config_cache_hit_ratio": stats["cache_hit_ratio"],
    "config_cache_size": stats["total_entries"],
    "config_circuit_breaker_state": health["circuit_breaker_state"]
}

# Send to monitoring system
prometheus_client.send_metrics(metrics)
```

## Best Practices

### 1. Use Context Managers
Always use the client as an async context manager to ensure proper cleanup:

```python
async with ConfigStoreClient(config) as client:
    # Use client here
    pass
# Connections automatically closed
```

### 2. Configure Appropriate Timeouts
Set timeouts based on your application's requirements:

```python
config = ConfigStoreConfig(
    timeout=30.0,  # 30 second timeout
    cache_ttl=300,  # 5 minute cache
)
```

### 3. Handle Errors Gracefully
Always provide sensible defaults and handle errors:

```python
try:
    batch_size = await client.get_int("ingestion.batch_size", default=1000)
except ConfigValidationError:
    batch_size = 1000  # Safe fallback
```

### 4. Use Environment Variables for Secrets
Store sensitive configuration in environment variables:

```bash
export NEURAL_TRADER_TRADING_API_KEY="your-secret-key"
export NEURAL_TRADER_DATABASE_PASSWORD="your-password"
```

### 5. Monitor Circuit Breaker State
Include circuit breaker state in health checks:

```python
health = await client.health_check()
if health["circuit_breaker_state"] == "open":
    logger.warning("Config store circuit breaker is open")
```

## Troubleshooting

### Common Issues

1. **Import Error**: Ensure all dependencies are installed
2. **Connection Refused**: Verify config store service is running
3. **Circuit Breaker Open**: Check service health and wait for recovery
4. **Environment Variables**: Verify correct naming convention
5. **Cache Issues**: Clear cache if stale data is returned

### Debug Logging

Enable debug logging for troubleshooting:

```python
import logging
logging.basicConfig(level=logging.DEBUG)

# The client will now log all operations
async with ConfigStoreClient(config) as client:
    value = await client.get("debug.key")
```

## Contributing

The config store client follows the SPARC methodology for development:

1. **Specification** - Requirements analysis and API design
2. **Pseudocode** - Algorithm design and flow specification  
3. **Architecture** - System design and component interaction
4. **Refinement** - TDD implementation and testing
5. **Completion** - Integration and deployment

All changes should include appropriate tests and documentation updates.

## License

This implementation is part of the neural-trader project and follows the same licensing terms.

## Changelog

### v0.1.0 (2024-08-23)
- ✅ Initial implementation with core functionality
- ✅ Environment variable fallback support
- ✅ Multi-level caching with TTL
- ✅ Connection pooling and retry logic  
- ✅ Circuit breaker pattern implementation
- ✅ Security filtering for sensitive data
- ✅ Comprehensive test suite
- ✅ Full async/await support
- ✅ Type conversion methods
- ✅ Health monitoring and diagnostics