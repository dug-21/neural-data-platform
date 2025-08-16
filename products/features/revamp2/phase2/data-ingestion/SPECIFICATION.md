# Phase 2 Data-Ingestion Python Specification

## Overview

This specification outlines the Python-specific requirements for transforming the data-ingestion service from publishing to a single "market:updates" channel to publishing per-symbol channels (market:AAPL, market:NVDA, etc.).

## Current Implementation Analysis

### Redis Client Architecture
- **Library**: `redis.asyncio` (aioredis)
- **Connection Pool**: Redis ConnectionPool with 50 max connections
- **Configuration**: `/workspaces/neural-trader/data_ingestion/config.py`
- **Initialization**: Async initialization in `RedisStore.connect()`

### Current Channel Structure

#### 1. Unified Channel (PROBLEM)
```python
# Current problematic implementation in realtime_coordinator.py:249
await self.redis.publish("market:updates", json.dumps(market_data, default=str))
```

#### 2. Symbol-Specific Channels (ALREADY IMPLEMENTED)
```python
# Already working correctly in realtime_coordinator.py:245-246
channel = f"market_data:{cleaned['symbol']}"
await self.redis.publish(channel, json.dumps(cleaned, default=str))
```

#### 3. Type-Specific Channels (RedisStore)
```python
# price_updates:AAPL, tick_updates:AAPL, orderbook_updates:AAPL
channel = f"price_updates:{symbol}"
channel = f"tick_updates:{symbol}"
channel = f"orderbook_updates:{symbol}"
```

### Message Format
```json
{
    "symbol": "AAPL",
    "provider": "polygon",
    "time": "2025-01-08T10:30:00Z",
    "timestamp": 1704708600,
    "price": 185.25,
    "volume": 1000,
    "high": 185.50,
    "low": 185.00,
    "open": 185.10,
    "close": 185.25
}
```

## Required Changes

### 1. Channel Migration Strategy

#### Target Channel Format
```python
# Replace this:
await self.redis.publish("market:updates", message)

# With this:
await self.redis.publish(f"market:{symbol}", message)
```

#### Implementation in RealtimeCoordinator
**File**: `/workspaces/neural-trader/data_ingestion/schedulers/realtime_coordinator.py`
**Line**: 249

```python
# BEFORE (line 249)
await self.redis.publish("market:updates", json.dumps(market_data, default=str))

# AFTER
await self.redis.publish(f"market:{cleaned['symbol']}", json.dumps(market_data, default=str))
```

### 2. Backward Compatibility

During transition period, publish to both channels:

```python
# Publish to per-symbol channel (new)
symbol_channel = f"market:{cleaned['symbol']}"
await self.redis.publish(symbol_channel, json.dumps(market_data, default=str))

# Publish to unified channel (deprecated - Phase 3 removal)
if self.settings.enable_legacy_channel:  # Feature flag
    await self.redis.publish("market:updates", json.dumps(market_data, default=str))
```

### 3. Configuration Updates

#### Environment Variables
```bash
# New configuration in config.py
ENABLE_LEGACY_CHANNEL=true  # Phase 2: true, Phase 3: false
REDIS_CHANNEL_PREFIX=market  # Configurable prefix
REDIS_MAX_CONNECTIONS=50     # Current value
REDIS_DECODE_RESPONSES=true  # Current value
```

#### Settings Class Enhancement
```python
class Settings(BaseSettings):
    # Add to existing settings
    enable_legacy_channel: bool = Field(default=True, env="ENABLE_LEGACY_CHANNEL")
    redis_channel_prefix: str = Field(default="market", env="REDIS_CHANNEL_PREFIX")
```

## Performance Requirements

### 1. Async/Await Pattern
- **Requirement**: Maintain full async/await compatibility
- **Implementation**: All Redis operations use `await` with `redis.asyncio`
- **Connection Pooling**: Reuse existing 50-connection pool

### 2. Publishing Frequency
- **Current**: Real-time (every market data update)
- **Target**: Same frequency, but distributed across symbol channels
- **Batching**: No batching required - maintain real-time publishing

### 3. Error Handling
```python
@with_retry(max_attempts=3, exceptions=(redis.RedisError,))
async def publish_market_update(self, symbol: str, market_data: Dict[str, Any]):
    """Publish market update with retry logic."""
    try:
        channel = f"market:{symbol}"
        message = json.dumps(market_data, default=str)
        await self.redis.publish(channel, message)
        
        # Metrics tracking
        metrics.redis_publishes.labels(channel=channel).inc()
        
    except redis.RedisError as e:
        logger.error(f"Failed to publish to {channel}: {e}")
        metrics.redis_publish_errors.labels(channel=channel).inc()
        raise
```

## Implementation Details

### 1. File Changes Required

#### Primary Changes
1. **realtime_coordinator.py:249** - Change unified channel to per-symbol
2. **config.py** - Add backward compatibility settings

#### Secondary Changes (if needed)
1. **redis_store.py** - Add helper methods for channel management
2. **utils/metrics.py** - Add per-symbol channel metrics

### 2. Testing Strategy
- **Unit Tests**: Mock Redis publish calls, verify channel names
- **Integration Tests**: Real Redis instance, verify message routing
- **Load Tests**: Measure performance impact of multiple channels

### 3. Deployment Strategy
1. **Phase 2a**: Dual publishing (both channels)
2. **Phase 2b**: Rust consumer migration to per-symbol channels
3. **Phase 2c**: Remove unified channel publishing

## Compatibility Requirements

### 1. Python Version
- **Current**: Python 3.8+
- **Redis Library**: redis[hiredis]>=4.0.0

### 2. Dependencies
```requirements.txt
redis[hiredis]>=4.0.0
aioredis>=2.0.0  # If used directly
```

### 3. Rust Integration
- **Channel Format**: Must match Rust consumer expectations
- **Message Format**: JSON with timestamp compatibility
- **Error Handling**: Graceful degradation if channels missing

## Validation Criteria

### 1. Functional Validation
- [ ] Messages published to `market:AAPL` for AAPL data
- [ ] Messages published to `market:NVDA` for NVDA data  
- [ ] Message format unchanged from current implementation
- [ ] No message loss during transition

### 2. Performance Validation
- [ ] Publishing latency <= current performance
- [ ] Memory usage increase <= 10%
- [ ] No Redis connection exhaustion
- [ ] Graceful handling of 100+ symbols

### 3. Integration Validation
- [ ] Rust consumer receives messages on new channels
- [ ] Backward compatibility maintained during transition
- [ ] Metrics capture per-symbol publishing rates