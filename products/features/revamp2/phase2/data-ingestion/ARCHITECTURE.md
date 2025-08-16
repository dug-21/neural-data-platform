# Phase 2 Data-Ingestion Python Architecture

## Overview

This document describes the Python async architecture for migrating from unified Redis channel publishing to per-symbol channel publishing, focusing on the asyncio/aioredis implementation patterns.

## Current Architecture Analysis

### 1. Async Event Loop Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    DataIngestionService                        │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ RealtimeCoord   │  │ BatchScheduler  │  │ StreamManager   │ │
│  │ (async)         │  │ (async)         │  │ (async)         │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
│         │                       │                       │      │
│         ▼                       ▼                       ▼      │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐ │
│  │ RedisStore      │  │ TimescaleDB     │  │ Providers       │ │
│  │ (redis.asyncio) │  │ (asyncpg)       │  │ (websockets)    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

### 2. Current Redis Publishing Flow

```python
# Current problematic flow in realtime_coordinator.py
async def _process_market_data(self, data: Any, provider_name: str):
    # Data processing (validation, cleaning)
    cleaned = self.cleaner._clean_record(data_dict)
    
    # Store in TimescaleDB  
    await self.timescale.insert_market_data([cleaned])
    
    # Cache in Redis
    await self.redis.set(cache_key, json.dumps(cleaned, default=str), ttl=300)
    
    # CURRENT: Symbol-specific channel (WORKING)
    channel = f"market_data:{cleaned['symbol']}"
    await self.redis.publish(channel, json.dumps(cleaned, default=str))
    
    # PROBLEM: Unified channel (NEEDS CHANGE)
    await self.redis.publish("market:updates", json.dumps(market_data, default=str))
```

### 3. Connection Pool Management

```python
# RedisStore initialization pattern
class RedisStore:
    def __init__(self):
        self._pool: Optional[ConnectionPool] = None
        
    async def connect(self):
        self._pool = redis.ConnectionPool.from_url(
            self.settings.redis_url,
            max_connections=50,          # Reuse existing pool size
            decode_responses=True        # JSON compatibility
        )
        self.redis = redis.Redis(connection_pool=self._pool)
```

## Target Architecture Design

### 1. Enhanced Channel Strategy

```python
class ChannelManager:
    """Manages Redis channel routing for market data."""
    
    def __init__(self, settings: Settings):
        self.prefix = settings.redis_channel_prefix  # "market"
        self.enable_legacy = settings.enable_legacy_channel  # Phase 2: True
        
    def get_symbol_channel(self, symbol: str) -> str:
        """Get per-symbol channel name."""
        return f"{self.prefix}:{symbol}"
        
    def get_legacy_channel(self) -> str:
        """Get unified legacy channel name."""
        return "market:updates"
```

### 2. Publishing Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Market Data Processing                       │
│                                                                 │
│  WebSocket Data → Validation → Cleaning → Symbol Routing       │
│                                                   │             │
│                                                   ▼             │
│  ┌─────────────────────────────────────────────────────┐       │
│  │              Channel Publisher                      │       │
│  │                                                     │       │
│  │  ┌──────────────────┐  ┌──────────────────────┐    │       │
│  │  │ Per-Symbol       │  │ Legacy Channel       │    │       │
│  │  │ market:AAPL      │  │ market:updates       │    │       │
│  │  │ market:NVDA      │  │ (Phase 2 only)      │    │       │
│  │  │ market:TSLA      │  │                      │    │       │
│  │  └──────────────────┘  └──────────────────────┘    │       │
│  └─────────────────────────────────────────────────────┘       │
│                                                                 │
│  ┌─────────────────────────────────────────────────────┐       │
│  │              Consumer Integration                   │       │
│  │                                                     │       │
│  │  ┌──────────────────┐  ┌──────────────────────┐    │       │
│  │  │ Rust Consumer    │  │ Python Subscribers   │    │       │
│  │  │ (main.rs:350)    │  │ (Optional)           │    │       │
│  │  └──────────────────┘  └──────────────────────┘    │       │
│  └─────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

### 3. Async Implementation Pattern

```python
class EnhancedRealtimeCoordinator(RealtimeCoordinator):
    """Enhanced coordinator with per-symbol channel support."""
    
    def __init__(self):
        super().__init__()
        self.channel_manager = ChannelManager(self.settings)
        
    async def _process_market_data(self, data: Any, provider_name: str):
        """Enhanced processing with per-symbol publishing."""
        try:
            # Existing processing logic (unchanged)
            cleaned = self.cleaner._clean_record(data_dict)
            await self.timescale.insert_market_data([cleaned])
            
            # Enhanced publishing strategy
            await self._publish_market_data(cleaned, provider_name)
            
        except Exception as e:
            logger.error(f"Failed to process market data: {e}")
            metrics.processing_errors.labels(provider=provider_name).inc()
    
    async def _publish_market_data(self, cleaned: Dict[str, Any], provider: str):
        """Enhanced publishing with channel strategy."""
        symbol = cleaned['symbol']
        message = json.dumps(cleaned, default=str)
        
        # Primary: Per-symbol channel (NEW)
        symbol_channel = self.channel_manager.get_symbol_channel(symbol)
        await self.redis.publish(symbol_channel, message)
        metrics.redis_publishes.labels(channel=symbol_channel, provider=provider).inc()
        
        # Secondary: Legacy channel (PHASE 2 ONLY)
        if self.channel_manager.enable_legacy:
            legacy_channel = self.channel_manager.get_legacy_channel()
            await self.redis.publish(legacy_channel, message)
            metrics.redis_publishes.labels(channel=legacy_channel, provider=provider).inc()
        
        # Existing: market_data:SYMBOL (KEEP UNCHANGED)
        market_data_channel = f"market_data:{symbol}"
        await self.redis.publish(market_data_channel, message)
```

## Implementation Strategy

### 1. Phased Migration Approach

#### Phase 2a: Dual Publishing (Backward Compatible)
```python
async def _publish_market_data_phase2a(self, cleaned, provider):
    """Phase 2a: Publish to both old and new channels."""
    symbol = cleaned['symbol']
    message = json.dumps(cleaned, default=str)
    
    # New per-symbol channels
    await self.redis.publish(f"market:{symbol}", message)
    
    # Legacy unified channel (maintain compatibility)
    await self.redis.publish("market:updates", message)
    
    # Existing channels (unchanged)
    await self.redis.publish(f"market_data:{symbol}", message)
```

#### Phase 2b: Consumer Migration Support
```python
async def _publish_market_data_phase2b(self, cleaned, provider):
    """Phase 2b: Configurable legacy channel support."""
    symbol = cleaned['symbol']
    message = json.dumps(cleaned, default=str)
    
    # Primary: Per-symbol channels
    await self.redis.publish(f"market:{symbol}", message)
    
    # Configurable: Legacy channel
    if self.settings.enable_legacy_channel:
        await self.redis.publish("market:updates", message)
```

#### Phase 2c: Pure Per-Symbol (Final)
```python
async def _publish_market_data_phase2c(self, cleaned, provider):
    """Phase 2c: Pure per-symbol publishing."""
    symbol = cleaned['symbol']
    message = json.dumps(cleaned, default=str)
    
    # Only per-symbol channels
    await self.redis.publish(f"market:{symbol}", message)
    await self.redis.publish(f"market_data:{symbol}", message)
```

### 2. Configuration Management

```python
# Enhanced settings in config.py
class Settings(BaseSettings):
    # Existing Redis settings
    redis_url: str = Field(default="redis://localhost:6379", env="REDIS_URL")
    
    # New Phase 2 settings
    enable_legacy_channel: bool = Field(default=True, env="ENABLE_LEGACY_CHANNEL")
    redis_channel_prefix: str = Field(default="market", env="REDIS_CHANNEL_PREFIX")
    redis_dual_publish: bool = Field(default=True, env="REDIS_DUAL_PUBLISH")
    
    # Performance settings
    redis_max_connections: int = Field(default=50, env="REDIS_MAX_CONNECTIONS")
    redis_publish_timeout: int = Field(default=5, env="REDIS_PUBLISH_TIMEOUT")
```

### 3. Error Handling & Resilience

```python
class ResilientPublisher:
    """Publisher with error handling and fallback strategies."""
    
    @with_retry(max_attempts=3, exceptions=(redis.RedisError,))
    async def publish_with_fallback(
        self, 
        primary_channel: str,
        fallback_channel: str,
        message: str
    ):
        """Publish with fallback strategy."""
        try:
            # Try primary channel
            await self.redis.publish(primary_channel, message)
            metrics.redis_publishes.labels(channel=primary_channel).inc()
            
        except redis.RedisError as primary_error:
            logger.warning(f"Primary channel {primary_channel} failed: {primary_error}")
            
            try:
                # Fallback to legacy channel
                await self.redis.publish(fallback_channel, message)
                metrics.redis_fallbacks.labels(
                    from_channel=primary_channel,
                    to_channel=fallback_channel
                ).inc()
                
            except redis.RedisError as fallback_error:
                logger.error(f"Both channels failed: {fallback_error}")
                metrics.redis_publish_errors.inc()
                raise
```

## Performance Considerations

### 1. Connection Pool Optimization

```python
# Optimized connection pool configuration
REDIS_POOL_CONFIG = {
    'max_connections': 50,              # Current value (sufficient)
    'connection_kwargs': {
        'decode_responses': True,       # JSON compatibility
        'socket_keepalive': True,       # Connection stability
        'socket_keepalive_options': {}, # OS-specific keepalive
        'health_check_interval': 30,    # Health monitoring
    }
}
```

### 2. Async Concurrency Management

```python
async def _publish_concurrent(self, data_batch: List[Dict[str, Any]]):
    """Concurrent publishing for high-throughput scenarios."""
    publish_tasks = []
    
    for data in data_batch:
        symbol = data['symbol']
        channel = f"market:{symbol}"
        message = json.dumps(data, default=str)
        
        # Create concurrent publish tasks
        task = asyncio.create_task(
            self.redis.publish(channel, message)
        )
        publish_tasks.append(task)
    
    # Wait for all publications
    results = await asyncio.gather(*publish_tasks, return_exceptions=True)
    
    # Handle any failures
    for result in results:
        if isinstance(result, Exception):
            logger.error(f"Publish failed: {result}")
            metrics.concurrent_publish_errors.inc()
```

### 3. Memory Management

```python
class MemoryEfficientPublisher:
    """Memory-efficient message publishing."""
    
    def __init__(self):
        self._message_cache = {}  # Symbol -> compiled message template
        self._cache_ttl = 300     # 5 minutes
        
    async def publish_optimized(self, symbol: str, data: Dict[str, Any]):
        """Optimized publishing with message template caching."""
        # Use template caching for high-frequency symbols
        if symbol in self._message_cache:
            template = self._message_cache[symbol]
        else:
            template = self._create_message_template(symbol)
            self._message_cache[symbol] = template
            
        # Format and publish
        message = template.format(**data)
        await self.redis.publish(f"market:{symbol}", message)
```

## Integration Points

### 1. Rust Consumer Integration

```python
# Message format compatibility for Rust consumer
def format_for_rust_consumer(market_data: Dict[str, Any]) -> str:
    """Format message for Rust consumer compatibility."""
    # Ensure timestamp is integer (Rust expects this)
    if 'time' in market_data:
        if hasattr(market_data['time'], 'timestamp'):
            market_data['timestamp'] = int(market_data['time'].timestamp())
    
    return json.dumps(market_data, default=str)
```

### 2. Health Check Integration

```python
async def health_check_channels(self) -> Dict[str, bool]:
    """Verify channel health for all active symbols."""
    health_status = {}
    
    for symbol in self.subscribed_symbols:
        channel = f"market:{symbol}"
        try:
            # Test channel by publishing a health check message
            await self.redis.publish(f"{channel}:health", "ping")
            health_status[channel] = True
        except redis.RedisError:
            health_status[channel] = False
            
    return health_status
```

## Migration Timeline

### Week 1: Infrastructure Preparation
- [ ] Add configuration support for dual publishing
- [ ] Implement ChannelManager class
- [ ] Add enhanced metrics for per-symbol channels

### Week 2: Implementation
- [ ] Modify realtime_coordinator.py line 249
- [ ] Add backward compatibility layer
- [ ] Implement error handling and fallback

### Week 3: Testing & Validation
- [ ] Unit tests for channel routing
- [ ] Integration tests with real Redis
- [ ] Performance benchmarks

### Week 4: Deployment & Monitoring
- [ ] Deploy with dual publishing enabled
- [ ] Monitor channel metrics
- [ ] Coordinate with Rust team for consumer migration