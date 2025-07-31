# Robust WebSocket Implementation Design for Alpaca Data Ingestion

## Executive Summary

This document presents a comprehensive design for robust WebSocket implementation in the Alpaca data provider, addressing connection reliability, dead connection detection, and seamless data flow monitoring.

## Current State Analysis

### Existing Implementation Strengths
- Uses official Alpaca SDK (StockDataStream)
- Basic reconnection logic with exponential backoff
- Queue-based data buffering (1000 items)
- Handler registration for trades, quotes, and bars
- Symbol subscription management

### Identified Weaknesses
1. **No Dead Connection Detection**: 30-second timeout only logs warnings
2. **Limited State Management**: Simple boolean flag for connection state
3. **No Circuit Breaker**: Continuous retry without intelligent backoff
4. **Missing Health Checks**: No proactive connection monitoring
5. **No Message Queue During Reconnection**: Data loss during disconnects
6. **Basic Error Recovery**: Limited error categorization and handling

## Proposed Architecture

### 1. Connection State Machine

```python
from enum import Enum, auto
from dataclasses import dataclass
from datetime import datetime
import asyncio
from typing import Optional, Dict, Any, List

class ConnectionState(Enum):
    """WebSocket connection states."""
    DISCONNECTED = auto()
    CONNECTING = auto()
    AUTHENTICATING = auto()
    CONNECTED = auto()
    RECONNECTING = auto()
    ERROR = auto()
    TERMINATED = auto()

@dataclass
class ConnectionMetrics:
    """Track connection health metrics."""
    last_message_time: datetime
    messages_received: int
    reconnect_count: int
    error_count: int
    latency_ms: float
    state: ConnectionState
    state_since: datetime
```

### 2. WebSocketManager Class

```python
class WebSocketManager:
    """Manages WebSocket lifecycle with robust error handling."""
    
    def __init__(self, provider: 'AlpacaProvider'):
        self.provider = provider
        self.state = ConnectionState.DISCONNECTED
        self.metrics = ConnectionMetrics(
            last_message_time=datetime.now(),
            messages_received=0,
            reconnect_count=0,
            error_count=0,
            latency_ms=0.0,
            state=ConnectionState.DISCONNECTED,
            state_since=datetime.now()
        )
        
        # Connection parameters
        self.max_reconnect_attempts = 10
        self.base_reconnect_delay = 2.0
        self.max_reconnect_delay = 300.0  # 5 minutes
        self.heartbeat_interval = 15.0  # 15 seconds
        self.dead_connection_timeout = 30.0  # 30 seconds
        
        # Message queue for reconnection
        self.pending_subscriptions: Set[str] = set()
        self.message_buffer: asyncio.Queue = asyncio.Queue(maxsize=5000)
        
        # Tasks
        self._connection_task: Optional[asyncio.Task] = None
        self._heartbeat_task: Optional[asyncio.Task] = None
        self._monitor_task: Optional[asyncio.Task] = None
        
    async def connect(self) -> bool:
        """Establish WebSocket connection with state management."""
        if self.state in [ConnectionState.CONNECTED, ConnectionState.CONNECTING]:
            return self.state == ConnectionState.CONNECTED
            
        self._set_state(ConnectionState.CONNECTING)
        
        try:
            # Initialize StockDataStream
            await self._initialize_stream()
            
            # Start connection task
            self._connection_task = asyncio.create_task(self._connection_loop())
            
            # Wait for connection
            for _ in range(30):  # 30 second timeout
                if self.state == ConnectionState.CONNECTED:
                    # Start monitoring tasks
                    self._heartbeat_task = asyncio.create_task(self._heartbeat_loop())
                    self._monitor_task = asyncio.create_task(self._monitor_loop())
                    return True
                await asyncio.sleep(1)
                
            raise TimeoutError("Connection timeout")
            
        except Exception as e:
            self._set_state(ConnectionState.ERROR)
            self.provider.logger.error(f"Connection failed: {e}")
            return False
    
    async def _connection_loop(self):
        """Main connection loop with automatic reconnection."""
        reconnect_delay = self.base_reconnect_delay
        
        while self.state != ConnectionState.TERMINATED:
            try:
                if self.state == ConnectionState.DISCONNECTED:
                    self._set_state(ConnectionState.CONNECTING)
                
                # Register handlers
                self._register_handlers()
                
                # Subscribe to symbols
                await self._restore_subscriptions()
                
                self._set_state(ConnectionState.CONNECTED)
                self.metrics.reconnect_count = 0
                reconnect_delay = self.base_reconnect_delay
                
                # Run the stream
                await self.provider.stock_stream.run()
                
            except asyncio.CancelledError:
                self.provider.logger.info("Connection loop cancelled")
                break
                
            except Exception as e:
                self.metrics.error_count += 1
                self.provider.logger.error(f"WebSocket error: {e}")
                
                if self.state != ConnectionState.TERMINATED:
                    self._set_state(ConnectionState.RECONNECTING)
                    self.metrics.reconnect_count += 1
                    
                    # Exponential backoff with jitter
                    jitter = random.uniform(0.5, 1.5)
                    wait_time = min(
                        reconnect_delay * jitter,
                        self.max_reconnect_delay
                    )
                    
                    self.provider.logger.info(
                        f"Reconnecting in {wait_time:.1f}s "
                        f"(attempt {self.metrics.reconnect_count})"
                    )
                    
                    await asyncio.sleep(wait_time)
                    reconnect_delay *= 2
    
    def _set_state(self, new_state: ConnectionState):
        """Update connection state with logging."""
        if self.state != new_state:
            old_state = self.state
            self.state = new_state
            self.metrics.state = new_state
            self.metrics.state_since = datetime.now()
            
            self.provider.logger.info(
                f"WebSocket state: {old_state.name} → {new_state.name}"
            )
            
            # Emit state change event
            asyncio.create_task(self._emit_state_change(old_state, new_state))
```

### 3. DataFlowMonitor Class

```python
class DataFlowMonitor:
    """Monitor data flow and detect dead connections."""
    
    def __init__(self, manager: WebSocketManager):
        self.manager = manager
        self.check_interval = 5.0  # Check every 5 seconds
        self.warning_threshold = 15.0  # Warn after 15 seconds
        self.dead_threshold = 30.0  # Dead after 30 seconds
        
        # Tracking
        self.last_check = datetime.now()
        self.consecutive_failures = 0
        self.health_status = "healthy"
        
    async def monitor_loop(self):
        """Main monitoring loop."""
        while True:
            try:
                await asyncio.sleep(self.check_interval)
                await self.check_connection_health()
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                self.manager.provider.logger.error(f"Monitor error: {e}")
    
    async def check_connection_health(self):
        """Check if connection is still alive."""
        now = datetime.now()
        time_since_last_message = (
            now - self.manager.metrics.last_message_time
        ).total_seconds()
        
        if time_since_last_message > self.dead_threshold:
            self.health_status = "dead"
            self.consecutive_failures += 1
            
            self.manager.provider.logger.error(
                f"Dead connection detected! No data for {time_since_last_message:.1f}s"
            )
            
            # Trigger reconnection
            if self.manager.state == ConnectionState.CONNECTED:
                await self.manager.reconnect()
                
        elif time_since_last_message > self.warning_threshold:
            self.health_status = "degraded"
            self.manager.provider.logger.warning(
                f"Connection degraded: No data for {time_since_last_message:.1f}s"
            )
            
            # Send ping/heartbeat
            await self.manager.send_heartbeat()
            
        else:
            self.health_status = "healthy"
            self.consecutive_failures = 0
    
    def get_health_report(self) -> Dict[str, Any]:
        """Generate health status report."""
        return {
            "status": self.health_status,
            "last_message_age": (
                datetime.now() - self.manager.metrics.last_message_time
            ).total_seconds(),
            "consecutive_failures": self.consecutive_failures,
            "messages_received": self.manager.metrics.messages_received,
            "error_count": self.manager.metrics.error_count,
            "reconnect_count": self.manager.metrics.reconnect_count,
            "state": self.manager.state.name,
            "state_duration": (
                datetime.now() - self.manager.metrics.state_since
            ).total_seconds()
        }
```

### 4. CircuitBreaker Implementation

```python
class CircuitBreaker:
    """Circuit breaker pattern for connection management."""
    
    def __init__(self, 
                 failure_threshold: int = 5,
                 recovery_timeout: float = 60.0,
                 expected_exception: type = Exception):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.expected_exception = expected_exception
        
        self.failure_count = 0
        self.last_failure_time = None
        self.state = "closed"  # closed, open, half-open
        
    async def call(self, func, *args, **kwargs):
        """Execute function with circuit breaker protection."""
        if self.state == "open":
            if self._should_attempt_reset():
                self.state = "half-open"
            else:
                raise ConnectionError("Circuit breaker is OPEN")
        
        try:
            result = await func(*args, **kwargs)
            self._on_success()
            return result
            
        except self.expected_exception as e:
            self._on_failure()
            raise
    
    def _on_success(self):
        """Reset failure count on success."""
        self.failure_count = 0
        self.state = "closed"
        
    def _on_failure(self):
        """Increment failure count and potentially open circuit."""
        self.failure_count += 1
        self.last_failure_time = datetime.now()
        
        if self.failure_count >= self.failure_threshold:
            self.state = "open"
            
    def _should_attempt_reset(self) -> bool:
        """Check if we should try half-open state."""
        return (
            self.last_failure_time and
            (datetime.now() - self.last_failure_time).total_seconds() 
            >= self.recovery_timeout
        )
```

### 5. Enhanced AlpacaProvider Integration

```python
class AlpacaProvider(BaseProvider):
    """Enhanced Alpaca provider with robust WebSocket support."""
    
    def __init__(self):
        super().__init__("alpaca")
        
        # ... existing initialization ...
        
        # Enhanced WebSocket components
        self._ws_manager: Optional[WebSocketManager] = None
        self._data_monitor: Optional[DataFlowMonitor] = None
        self._circuit_breaker = CircuitBreaker(
            failure_threshold=5,
            recovery_timeout=60.0
        )
        
        # Message queue during reconnection
        self._reconnection_buffer = asyncio.Queue(maxsize=10000)
        self._buffer_task: Optional[asyncio.Task] = None
        
    async def connect(self):
        """Enhanced connection with WebSocket manager."""
        await super().connect()  # Existing SDK initialization
        
        # Initialize WebSocket manager
        self._ws_manager = WebSocketManager(self)
        self._data_monitor = DataFlowMonitor(self._ws_manager)
        
        # Start buffer processor
        self._buffer_task = asyncio.create_task(self._process_buffer())
        
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream with enhanced reliability."""
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        # Ensure WebSocket is connected
        if not self._ws_manager or self._ws_manager.state != ConnectionState.CONNECTED:
            connected = await self._circuit_breaker.call(
                self._ws_manager.connect
            )
            if not connected:
                raise ConnectionError("Failed to establish WebSocket connection")
        
        # Update subscriptions
        await self._ws_manager.update_subscriptions(symbols)
        
        # Yield data with monitoring
        while True:
            try:
                # Try to get data with timeout
                data = await asyncio.wait_for(
                    self._ws_data_queue.get(),
                    timeout=5.0
                )
                
                # Update metrics
                self._ws_manager.metrics.last_message_time = datetime.now()
                self._ws_manager.metrics.messages_received += 1
                
                # Filter and yield
                if data.symbol in symbols:
                    yield data
                    
            except asyncio.TimeoutError:
                # Check connection health
                health = self._data_monitor.get_health_report()
                if health["status"] == "dead":
                    self.logger.error("Dead connection detected, reconnecting...")
                    await self._ws_manager.reconnect()
                    
            except Exception as e:
                self.logger.error(f"Stream error: {e}")
                await asyncio.sleep(1)
    
    async def _process_buffer(self):
        """Process messages from reconnection buffer."""
        while True:
            try:
                # Move buffered messages to main queue
                while not self._reconnection_buffer.empty():
                    msg = await self._reconnection_buffer.get()
                    await self._ws_data_queue.put(msg)
                    
                await asyncio.sleep(0.1)
                
            except asyncio.CancelledError:
                break
            except Exception as e:
                self.logger.error(f"Buffer processing error: {e}")
```

### 6. Event Handlers and Callbacks

```python
class WebSocketEventHandlers:
    """Async event handlers for WebSocket events."""
    
    def __init__(self, provider: AlpacaProvider):
        self.provider = provider
        
    async def on_connect(self):
        """Handle successful connection."""
        self.provider.logger.info("WebSocket connected successfully")
        
        # Notify monitoring systems
        await self._notify_redis("websocket.connected", {
            "provider": "alpaca",
            "timestamp": datetime.now().isoformat()
        })
        
    async def on_disconnect(self, reason: str):
        """Handle disconnection."""
        self.provider.logger.warning(f"WebSocket disconnected: {reason}")
        
        # Buffer incoming messages
        if self.provider._ws_manager.state == ConnectionState.RECONNECTING:
            self.provider.logger.info("Buffering messages during reconnection")
            
    async def on_error(self, error: Exception):
        """Handle WebSocket errors."""
        error_type = type(error).__name__
        
        if error_type in ["ConnectionResetError", "ConnectionAbortedError"]:
            self.provider.logger.error("Connection lost, triggering reconnection")
            await self.provider._ws_manager.reconnect()
            
        elif error_type == "AuthenticationError":
            self.provider.logger.error("Authentication failed, check credentials")
            self.provider._ws_manager._set_state(ConnectionState.ERROR)
            
        else:
            self.provider.logger.error(f"Unexpected error: {error}")
            
    async def on_message(self, message: Dict[str, Any]):
        """Handle incoming messages with validation."""
        try:
            # Validate message structure
            if not self._validate_message(message):
                self.provider.logger.warning(f"Invalid message: {message}")
                return
                
            # Process based on message type
            msg_type = message.get("T")
            
            if msg_type == "error":
                await self.on_error(Exception(message.get("msg", "Unknown error")))
            elif msg_type == "subscription":
                await self._handle_subscription_update(message)
            else:
                # Regular data message
                await self._process_data_message(message)
                
        except Exception as e:
            self.provider.logger.error(f"Message processing error: {e}")
```

### 7. Redis Integration for Monitoring

```python
class RedisEventBus:
    """Publish WebSocket events to Redis for monitoring."""
    
    def __init__(self, redis_client):
        self.redis = redis_client
        self.channel_prefix = "websocket:alpaca:"
        
    async def publish_event(self, event_type: str, data: Dict[str, Any]):
        """Publish event to Redis channel."""
        channel = f"{self.channel_prefix}{event_type}"
        message = json.dumps({
            "timestamp": datetime.now().isoformat(),
            "event": event_type,
            "data": data
        })
        
        await self.redis.publish(channel, message)
        
    async def publish_metrics(self, metrics: ConnectionMetrics):
        """Publish connection metrics."""
        await self.publish_event("metrics", {
            "state": metrics.state.name,
            "messages_received": metrics.messages_received,
            "reconnect_count": metrics.reconnect_count,
            "error_count": metrics.error_count,
            "latency_ms": metrics.latency_ms,
            "last_message_age": (
                datetime.now() - metrics.last_message_time
            ).total_seconds()
        })
```

### 8. Grafana Metrics Export

```python
class PrometheusMetrics:
    """Export metrics for Prometheus/Grafana."""
    
    def __init__(self):
        # Connection metrics
        self.ws_connection_state = Gauge(
            'websocket_connection_state',
            'WebSocket connection state',
            ['provider']
        )
        
        self.ws_messages_total = Counter(
            'websocket_messages_total',
            'Total WebSocket messages received',
            ['provider', 'message_type']
        )
        
        self.ws_reconnects_total = Counter(
            'websocket_reconnects_total',
            'Total WebSocket reconnection attempts',
            ['provider']
        )
        
        self.ws_errors_total = Counter(
            'websocket_errors_total',
            'Total WebSocket errors',
            ['provider', 'error_type']
        )
        
        self.ws_latency_seconds = Histogram(
            'websocket_latency_seconds',
            'WebSocket message latency',
            ['provider']
        )
        
        self.ws_last_message_age = Gauge(
            'websocket_last_message_age_seconds',
            'Age of last received message',
            ['provider']
        )
        
    def update_metrics(self, provider: str, metrics: ConnectionMetrics):
        """Update Prometheus metrics."""
        # Map state to numeric value
        state_map = {
            ConnectionState.DISCONNECTED: 0,
            ConnectionState.CONNECTING: 1,
            ConnectionState.CONNECTED: 2,
            ConnectionState.RECONNECTING: 3,
            ConnectionState.ERROR: -1
        }
        
        self.ws_connection_state.labels(provider).set(
            state_map.get(metrics.state, -2)
        )
        
        self.ws_last_message_age.labels(provider).set(
            (datetime.now() - metrics.last_message_time).total_seconds()
        )
```

## Implementation Timeline

### Phase 1: Core Components (Week 1)
- [ ] Implement ConnectionState and WebSocketManager
- [ ] Add state machine logic
- [ ] Create message buffering system

### Phase 2: Monitoring (Week 2)
- [ ] Implement DataFlowMonitor
- [ ] Add health check mechanisms
- [ ] Create CircuitBreaker

### Phase 3: Integration (Week 3)
- [ ] Integrate with existing AlpacaProvider
- [ ] Add Redis event publishing
- [ ] Implement Prometheus metrics

### Phase 4: Testing & Optimization (Week 4)
- [ ] Comprehensive testing
- [ ] Performance optimization
- [ ] Documentation updates

## Testing Strategy

### Unit Tests
```python
class TestWebSocketManager:
    """Test WebSocket manager functionality."""
    
    async def test_connection_state_transitions(self):
        """Test state machine transitions."""
        manager = WebSocketManager(mock_provider)
        
        assert manager.state == ConnectionState.DISCONNECTED
        
        # Test connection flow
        await manager.connect()
        assert manager.state == ConnectionState.CONNECTED
        
        # Test disconnection
        await manager.disconnect()
        assert manager.state == ConnectionState.DISCONNECTED
    
    async def test_reconnection_backoff(self):
        """Test exponential backoff logic."""
        manager = WebSocketManager(mock_provider)
        
        # Simulate multiple failures
        for i in range(5):
            await manager._handle_connection_error(Exception("Test"))
            
        assert manager.metrics.reconnect_count == 5
        # Verify backoff timing
    
    async def test_message_buffering(self):
        """Test message buffering during reconnection."""
        manager = WebSocketManager(mock_provider)
        
        # Simulate disconnection
        manager._set_state(ConnectionState.RECONNECTING)
        
        # Add messages to buffer
        for i in range(10):
            await manager.buffer_message({"test": i})
            
        # Verify buffer contents
        assert manager.message_buffer.qsize() == 10
```

### Integration Tests
```python
async def test_dead_connection_detection():
    """Test dead connection detection and recovery."""
    provider = AlpacaProvider()
    await provider.connect()
    
    # Simulate no messages for 35 seconds
    provider._ws_manager.metrics.last_message_time = (
        datetime.now() - timedelta(seconds=35)
    )
    
    # Monitor should detect and reconnect
    monitor = DataFlowMonitor(provider._ws_manager)
    await monitor.check_connection_health()
    
    assert monitor.health_status == "dead"
    assert provider._ws_manager.state == ConnectionState.RECONNECTING
```

## Monitoring Dashboard

### Grafana Panel Configuration
```json
{
  "dashboard": {
    "title": "Alpaca WebSocket Health",
    "panels": [
      {
        "title": "Connection State",
        "targets": [{
          "expr": "websocket_connection_state{provider='alpaca'}"
        }]
      },
      {
        "title": "Message Flow Rate",
        "targets": [{
          "expr": "rate(websocket_messages_total{provider='alpaca'}[1m])"
        }]
      },
      {
        "title": "Last Message Age",
        "targets": [{
          "expr": "websocket_last_message_age_seconds{provider='alpaca'}"
        }],
        "thresholds": [
          {"value": 15, "color": "yellow"},
          {"value": 30, "color": "red"}
        ]
      },
      {
        "title": "Error Rate",
        "targets": [{
          "expr": "rate(websocket_errors_total{provider='alpaca'}[5m])"
        }]
      }
    ]
  }
}
```

## Alert Configuration

```yaml
groups:
  - name: websocket_alerts
    rules:
      - alert: WebSocketDead
        expr: websocket_last_message_age_seconds{provider='alpaca'} > 30
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Alpaca WebSocket connection is dead"
          description: "No messages received for {{ $value }} seconds"
      
      - alert: WebSocketHighErrorRate
        expr: rate(websocket_errors_total{provider='alpaca'}[5m]) > 0.1
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High WebSocket error rate"
          description: "Error rate: {{ $value }} errors/sec"
      
      - alert: WebSocketFrequentReconnects
        expr: increase(websocket_reconnects_total{provider='alpaca'}[1h]) > 10
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Frequent WebSocket reconnections"
          description: "{{ $value }} reconnects in the last hour"
```

## Benefits

1. **Reliability**: State machine ensures predictable behavior
2. **Observability**: Comprehensive metrics and logging
3. **Resilience**: Circuit breaker prevents cascading failures
4. **Data Integrity**: Message buffering during reconnections
5. **Proactive Monitoring**: Dead connection detection
6. **Performance**: Optimized reconnection strategies

## Conclusion

This implementation provides enterprise-grade WebSocket reliability for the Alpaca data provider, ensuring continuous data flow with minimal disruption and comprehensive monitoring capabilities.