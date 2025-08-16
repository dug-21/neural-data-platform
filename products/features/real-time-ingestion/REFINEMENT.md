# SPARC Refinement: AlpacaProvider WebSocket Optimizations

## Overview

This document focuses on specific optimizations for the WebSocket implementation within the AlpacaProvider class. These are practical refinements that can be implemented within the minimal scope of modifying a single file.

## 1. WebSocket Connection Optimizations

### 1.1 Simple Connection Management

```python
class AlpacaProvider(DataProvider):
    """Extended with WebSocket support."""
    
    async def stream_market_data_ws(self, symbols: List[str]):
        """Stream real-time market data via WebSocket."""
        # Simple exponential backoff for reconnection
        retry_delay = 1.0
        max_delay = 60.0
        
        while True:
            try:
                async with websockets.connect(
                    self.ws_url,
                    ssl=self._get_ssl_context(),
                    ping_interval=30,
                    ping_timeout=10
                ) as websocket:
                    # Reset delay on successful connection
                    retry_delay = 1.0
                    
                    # Authenticate
                    await self._authenticate_ws(websocket)
                    
                    # Subscribe to symbols
                    await self._subscribe_symbols(websocket, symbols)
                    
                    # Message processing loop
                    async for message in websocket:
                        data = self._parse_ws_message(message)
                        if data:
                            await self._process_market_data(data)
                            
            except Exception as e:
                self.logger.error(f"WebSocket error: {e}")
                await asyncio.sleep(retry_delay)
                retry_delay = min(retry_delay * 2, max_delay)
```

### 1.2 TCP Socket Optimization

```python
def _get_ssl_context(self):
    """Create optimized SSL context for WebSocket."""
    context = ssl.create_default_context()
    
    # Optimize for low latency
    context.options |= ssl.OP_NO_COMPRESSION
    context.options |= ssl.OP_NO_TICKET
    
    return context
```

## 2. Message Processing Optimizations

### 2.1 Efficient Message Parsing

```python
def _parse_ws_message(self, message: str) -> Optional[MarketData]:
    """Parse Alpaca WebSocket message efficiently."""
    try:
        # Use rapidjson if available for faster parsing
        try:
            import rapidjson as json
        except ImportError:
            import json
            
        data = json.loads(message)
        
        # Fast path for known message types
        msg_type = data.get('T')
        
        if msg_type == 't':  # Trade
            return self._parse_trade(data)
        elif msg_type == 'q':  # Quote
            return self._parse_quote(data)
        elif msg_type == 'b':  # Bar
            return self._parse_bar(data)
        else:
            # Control messages (auth, subscription, etc.)
            return None
            
    except Exception as e:
        self.logger.error(f"Failed to parse message: {e}")
        return None
```

### 2.2 Batch Processing for Storage

```python
async def _process_market_data(self, data: MarketData):
    """Process market data with batching."""
    # Add to batch
    if not hasattr(self, '_batch'):
        self._batch = []
        self._batch_time = time.time()
        
    self._batch.append(data)
    
    # Flush batch if size or time threshold reached
    if len(self._batch) >= 100 or time.time() - self._batch_time > 0.1:
        await self._flush_batch()
        
async def _flush_batch(self):
    """Flush batched data to storage."""
    if self._batch:
        # Use existing storage adapters
        for storage in self.storage_adapters:
            await storage.store_batch(self._batch)
            
        self._batch = []
        self._batch_time = time.time()
```

## 3. Memory Efficiency

### 3.1 Object Reuse Pattern

```python
class AlpacaProvider(DataProvider):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        # Pre-allocate reusable objects
        self._market_data_pool = []
        
    def _get_market_data_object(self):
        """Get reusable MarketData object."""
        if self._market_data_pool:
            return self._market_data_pool.pop()
        return MarketData()
        
    def _release_market_data_object(self, obj):
        """Return object to pool."""
        # Reset object state
        obj.symbol = None
        obj.price = None
        obj.volume = None
        obj.timestamp = None
        
        if len(self._market_data_pool) < 1000:
            self._market_data_pool.append(obj)
```

## 4. Subscription Management

### 4.1 Dynamic Symbol Management

```python
async def _subscribe_symbols(self, websocket, symbols: List[str]):
    """Subscribe to market data for symbols."""
    # Alpaca subscription format
    subscribe_msg = {
        "action": "subscribe",
        "trades": symbols,
        "quotes": symbols,
        "bars": symbols
    }
    
    await websocket.send(json.dumps(subscribe_msg))
    
    # Track subscriptions
    self._active_subscriptions = set(symbols)
    
async def add_symbols(self, symbols: List[str]):
    """Add symbols to active WebSocket subscription."""
    if hasattr(self, '_websocket') and self._websocket:
        new_symbols = set(symbols) - self._active_subscriptions
        if new_symbols:
            subscribe_msg = {
                "action": "subscribe",
                "trades": list(new_symbols),
                "quotes": list(new_symbols),
                "bars": list(new_symbols)
            }
            await self._websocket.send(json.dumps(subscribe_msg))
            self._active_subscriptions.update(new_symbols)
```

## 5. Error Handling and Fallback

### 5.1 Graceful Degradation

```python
async def get_market_data(self, symbols: List[str], **kwargs):
    """Get market data with automatic fallback."""
    # Try WebSocket first if available
    if self.websocket_enabled and hasattr(self, '_ws_data_cache'):
        # Check if we have recent data from WebSocket
        recent_data = self._get_recent_ws_data(symbols)
        if recent_data:
            return recent_data
    
    # Fallback to REST API
    return await super().get_market_data(symbols, **kwargs)
    
def _get_recent_ws_data(self, symbols: List[str]):
    """Get recent data from WebSocket cache."""
    if not hasattr(self, '_ws_data_cache'):
        return None
        
    now = time.time()
    result = []
    
    for symbol in symbols:
        if symbol in self._ws_data_cache:
            data, timestamp = self._ws_data_cache[symbol]
            # Consider data fresh if less than 1 second old
            if now - timestamp < 1.0:
                result.append(data)
                
    return result if len(result) == len(symbols) else None
```

## 6. Performance Monitoring

### 6.1 Simple Metrics Collection

```python
def _init_metrics(self):
    """Initialize WebSocket metrics."""
    self._ws_metrics = {
        'messages_received': 0,
        'messages_processed': 0,
        'parse_errors': 0,
        'reconnections': 0,
        'last_message_time': None,
        'latency_samples': deque(maxlen=1000)
    }
    
def _update_metrics(self, message_received_time):
    """Update WebSocket metrics."""
    self._ws_metrics['messages_received'] += 1
    self._ws_metrics['last_message_time'] = time.time()
    
    # Simple latency tracking
    if hasattr(self, '_last_process_time'):
        latency = (time.time() - message_received_time) * 1000  # ms
        self._ws_metrics['latency_samples'].append(latency)
```

## 7. Configuration Options

### 7.1 WebSocket Configuration

```python
class AlpacaProvider(DataProvider):
    def __init__(self, *args, websocket_enabled=True, **kwargs):
        super().__init__(*args, **kwargs)
        
        # WebSocket configuration
        self.websocket_enabled = websocket_enabled
        self.ws_config = {
            'ping_interval': kwargs.get('ws_ping_interval', 30),
            'ping_timeout': kwargs.get('ws_ping_timeout', 10),
            'max_reconnect_delay': kwargs.get('ws_max_reconnect_delay', 60),
            'batch_size': kwargs.get('ws_batch_size', 100),
            'batch_timeout': kwargs.get('ws_batch_timeout', 0.1),
            'cache_ttl': kwargs.get('ws_cache_ttl', 1.0)
        }
        
        # WebSocket URL construction
        base_url = self.base_url.replace('https://', 'wss://')
        self.ws_url = f"{base_url}/stream"
```

## 8. Testing Utilities

### 8.1 Mock WebSocket for Testing

```python
class MockWebSocket:
    """Mock WebSocket for testing."""
    
    def __init__(self, test_messages):
        self.test_messages = test_messages
        self.sent_messages = []
        
    async def send(self, message):
        self.sent_messages.append(message)
        
    async def __aiter__(self):
        for message in self.test_messages:
            yield message
            
    async def close(self):
        pass

# Usage in tests
async def test_websocket_parsing():
    provider = AlpacaProvider()
    
    test_messages = [
        '{"T":"t","S":"AAPL","p":150.25,"s":100,"t":"2024-01-01T10:00:00Z"}',
        '{"T":"q","S":"AAPL","bp":150.20,"ap":150.30,"bs":100,"as":200}'
    ]
    
    mock_ws = MockWebSocket(test_messages)
    processed = []
    
    async for message in mock_ws:
        data = provider._parse_ws_message(message)
        if data:
            processed.append(data)
            
    assert len(processed) == 2
```

## Summary

These refinements focus on practical optimizations that can be implemented within the AlpacaProvider class:

1. **Simple reconnection logic** with exponential backoff
2. **Efficient message parsing** with type-specific handlers
3. **Batch processing** to reduce storage overhead
4. **Memory efficiency** through object pooling
5. **Dynamic subscription management** for symbol additions
6. **Graceful fallback** to REST API when needed
7. **Basic metrics** for monitoring WebSocket health
8. **Configuration options** for tuning behavior

All optimizations maintain backward compatibility and can be implemented incrementally within the existing AlpacaProvider structure. The focus is on practical improvements that deliver immediate value without requiring architectural changes.