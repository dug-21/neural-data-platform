# Polygon.io WebSocket Migration Guide

## Overview

This guide provides step-by-step instructions for migrating from the current mixed HTTP/WebSocket implementation to the new WebSocket-first architecture.

## Migration Benefits

### Performance Improvements
- **Latency**: Sub-millisecond message delivery vs 100ms+ HTTP requests
- **Throughput**: Handle 10,000+ messages/second vs 100 requests/second
- **Efficiency**: Single persistent connection vs multiple HTTP connections
- **Real-time**: True streaming data vs polling intervals

### Reliability Enhancements
- Automatic reconnection with exponential backoff
- Message buffering during disconnects
- Automatic resubscription after reconnects
- Connection health monitoring
- Circuit breaker for persistent failures

### Feature Additions
- Backpressure handling for high-volume streams
- Message buffering with configurable limits
- Batch subscription management
- Comprehensive metrics and monitoring
- Graceful degradation to HTTP fallback

## Migration Steps

### Step 1: Install New Provider

```python
# Update imports
from data_ingestion.providers.polygon_websocket import (
    PolygonWebSocketProvider,
    WebSocketConfig
)

# Create configuration
config = WebSocketConfig(
    max_reconnect_attempts=10,
    message_buffer_size=10000,
    subscription_batch_size=100
)

# Initialize provider
provider = PolygonWebSocketProvider(config)
```

### Step 2: Update Connection Logic

**Before:**
```python
# Old connection
provider = PolygonProvider()
await provider.connect()
```

**After:**
```python
# New connection with lifecycle management
async with PolygonWebSocketProvider(config) as provider:
    # Provider is connected and ready
    # Automatic cleanup on exit
    pass
```

### Step 3: Update Streaming Methods

**Before:**
```python
# Limited WebSocket support
async for data in provider.stream_market_data(["AAPL"]):
    process(data)
```

**After:**
```python
# Full WebSocket streaming with multiple data types
# Stream market data
async for data in provider.stream_market_data(["AAPL", "GOOGL"]):
    process_bar(data)

# Stream tick data
async for tick in provider.stream_tick_data(["AAPL"]):
    process_tick(tick)

# Stream quotes
async for quote in provider.stream_quotes(["AAPL"]):
    process_quote(quote)
```

### Step 4: Error Handling

**Before:**
```python
try:
    async for data in provider.stream_market_data(symbols):
        process(data)
except Exception as e:
    # Manual reconnection needed
    logger.error(f"Stream failed: {e}")
```

**After:**
```python
# Automatic error recovery
async for data in provider.stream_market_data(symbols):
    process(data)
    # Provider handles reconnections automatically
    # Check provider.get_statistics() for health
```

### Step 5: Monitoring and Metrics

**New capabilities:**
```python
# Get provider statistics
stats = provider.get_statistics()
print(f"Connection state: {stats['connection_state']}")
print(f"Buffer stats: {stats['buffer_stats']}")
print(f"Active subscriptions: {stats['active_subscriptions']}")

# Monitor connection health
if not provider.ws_manager.is_connected:
    logger.warning("WebSocket disconnected")
```

## Code Examples

### Basic Streaming
```python
async def stream_stocks():
    config = WebSocketConfig(
        message_buffer_size=50000,  # Handle high volume
        heartbeat_interval=30.0     # Keep connection alive
    )
    
    async with PolygonWebSocketProvider(config) as provider:
        # Stream multiple symbols
        symbols = ["AAPL", "GOOGL", "MSFT", "AMZN"]
        
        async for data in provider.stream_market_data(symbols):
            print(f"{data.symbol}: ${data.close} @ {data.time}")
```

### Multi-Stream Processing
```python
async def multi_stream():
    async with PolygonWebSocketProvider() as provider:
        # Create tasks for different data types
        tasks = [
            process_bars(provider, ["AAPL"]),
            process_ticks(provider, ["AAPL"]),
            process_quotes(provider, ["AAPL"])
        ]
        
        # Run all streams concurrently
        await asyncio.gather(*tasks)

async def process_bars(provider, symbols):
    async for bar in provider.stream_market_data(symbols):
        # Process aggregate bars
        await save_bar(bar)

async def process_ticks(provider, symbols):
    async for tick in provider.stream_tick_data(symbols):
        # Process individual trades
        await save_tick(tick)

async def process_quotes(provider, symbols):
    async for quote in provider.stream_quotes(symbols):
        # Process quote updates
        await save_quote(quote)
```

### Advanced Configuration
```python
# Custom configuration for high-frequency trading
hft_config = WebSocketConfig(
    message_buffer_size=100000,      # Large buffer for bursts
    subscription_batch_size=200,     # Batch more subscriptions
    heartbeat_interval=10.0,         # More frequent heartbeats
    max_reconnect_delay=5.0,         # Faster reconnects
    connection_timeout=5.0           # Faster initial connect
)

async with PolygonWebSocketProvider(hft_config) as provider:
    # Subscribe to many symbols
    symbols = load_symbol_universe()  # Could be 1000+ symbols
    
    # Provider handles batching automatically
    async for tick in provider.stream_tick_data(symbols):
        await process_hft_tick(tick)
```

## Testing Your Migration

### Unit Tests
```python
import pytest
from unittest.mock import AsyncMock

@pytest.mark.asyncio
async def test_websocket_streaming():
    provider = PolygonWebSocketProvider()
    
    # Mock the WebSocket manager
    provider.ws_manager = AsyncMock()
    provider.ws_manager.is_connected = True
    
    # Test subscription
    await provider.subscription_manager.subscribe_trades(["AAPL"])
    
    # Verify subscription was sent
    assert "T.AAPL" in provider.subscription_manager._active_subscriptions
```

### Integration Tests
```python
@pytest.mark.asyncio
@pytest.mark.integration
async def test_live_streaming():
    # Test with real connection (requires API key)
    async with PolygonWebSocketProvider() as provider:
        data_received = []
        
        # Stream for 5 seconds
        start_time = time.time()
        async for data in provider.stream_market_data(["AAPL"]):
            data_received.append(data)
            if time.time() - start_time > 5:
                break
        
        assert len(data_received) > 0
```

## Rollback Plan

If issues arise during migration:

1. **Feature Flag**: Use environment variable to switch providers
```python
USE_NEW_WEBSOCKET = os.getenv("POLYGON_USE_NEW_WS", "false").lower() == "true"

if USE_NEW_WEBSOCKET:
    provider = PolygonWebSocketProvider()
else:
    provider = PolygonProvider()  # Old provider
```

2. **Gradual Rollout**: Migrate specific symbols first
```python
# Start with low-volume symbols
test_symbols = ["AAPL", "GOOGL"]
production_symbols = load_all_symbols()

if symbol in test_symbols:
    use_new_provider()
else:
    use_old_provider()
```

3. **Monitoring**: Track metrics during rollout
```python
# Log provider performance
metrics.websocket_latency.observe(latency)
metrics.websocket_messages.inc()
metrics.websocket_errors.inc()
```

## Common Issues and Solutions

### Issue: High Memory Usage
**Solution**: Reduce buffer size
```python
config = WebSocketConfig(message_buffer_size=1000)  # Smaller buffer
```

### Issue: Frequent Disconnections
**Solution**: Increase reconnection attempts and delays
```python
config = WebSocketConfig(
    max_reconnect_attempts=20,
    max_reconnect_delay=120.0
)
```

### Issue: Message Processing Lag
**Solution**: Process messages in batches
```python
async for batch in provider.stream_buffer.pop_batch(100):
    await process_batch(batch)  # Process 100 at a time
```

## Performance Tuning

### For Low Latency
```python
config = WebSocketConfig(
    message_buffer_size=1000,        # Small buffer
    heartbeat_interval=10.0,         # Frequent heartbeats
    connection_timeout=2.0           # Fast timeout
)
```

### For High Throughput
```python
config = WebSocketConfig(
    message_buffer_size=100000,      # Large buffer
    subscription_batch_size=500,     # Large batches
    heartbeat_interval=60.0          # Less overhead
)
```

### For Reliability
```python
config = WebSocketConfig(
    max_reconnect_attempts=50,       # Many retries
    max_reconnect_delay=300.0,       # Long backoff
    message_buffer_size=50000        # Buffer during disconnects
)
```

## Checklist

- [ ] Update provider imports
- [ ] Configure WebSocket settings
- [ ] Update connection logic
- [ ] Migrate streaming methods
- [ ] Add error handling
- [ ] Implement monitoring
- [ ] Update tests
- [ ] Deploy with feature flag
- [ ] Monitor metrics
- [ ] Remove old provider code