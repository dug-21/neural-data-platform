# Alpaca WebSocket Reliability Analysis

## Executive Summary

The Alpaca WebSocket implementation has significant reliability issues that require sophisticated monitoring and recovery mechanisms. Currently, when the WebSocket fails, it doesn't recover automatically, requiring a container restart. This analysis provides a comprehensive solution for detecting data gaps (30 seconds during trading hours) and implementing automatic recovery without container restarts.

## Current Implementation Analysis

### 1. WebSocket Architecture

**Location:** `/workspaces/neural-trader/data_ingestion/providers/alpaca.py`

#### Key Components:
- **Alpaca SDK WebSocket** (Lines 101-114): Uses `StockDataStream` with configurable feed
- **Direct WebSocket Connection** (Lines 579-674): Fallback implementation using `websockets` library
- **Message Handlers** (Lines 125-192): Processes trades, quotes, and bars
- **Reconnection Logic** (Lines 376-415): Basic exponential backoff with 10 retry attempts

### 2. Current Failure Patterns

#### Identified Issues:

1. **No Heartbeat Mechanism**
   - WebSocket has no ping/pong implementation
   - No detection of stale connections
   - Silent failures when connection appears alive but no data flows

2. **Limited Retry Logic**
   ```python
   # Current implementation (Line 379-380)
   retry_count = 0
   max_retries = 10
   ```
   - Fixed retry count doesn't adapt to different failure scenarios
   - No differentiation between temporary and permanent failures

3. **Queue Timeout Without Recovery**
   ```python
   # Line 454-459
   data = await asyncio.wait_for(
       self._ws_data_queue.get(),
       timeout=30.0  # 30 second timeout
   )
   ```
   - Timeout logs warning but doesn't trigger aggressive recovery
   - No trading hours awareness

4. **Container Restart Dependency**
   - Docker restart policy: `unless-stopped`
   - No internal recovery mechanism for stuck WebSocket
   - Manual intervention required

### 3. Missing Monitoring

#### Current State:
- **No WebSocket-specific metrics** in Prometheus alerts
- **No trading hours awareness** in monitoring
- **No data freshness tracking** per symbol during active trading
- **Generic alerts** don't catch WebSocket-specific failures

## Proposed Solution Architecture

### 1. Enhanced WebSocket Manager

```python
class EnhancedAlpacaWebSocketManager:
    """
    Sophisticated WebSocket management with automatic recovery
    """
    
    def __init__(self):
        self.connection_state = ConnectionState()
        self.heartbeat_manager = HeartbeatManager()
        self.data_monitor = DataFreshnessMonitor()
        self.circuit_breaker = CircuitBreaker()
        self.recovery_strategy = AdaptiveRecoveryStrategy()
```

### 2. Multi-Layer Monitoring System

#### Layer 1: Heartbeat/Keep-Alive
```python
class HeartbeatManager:
    """
    Proactive connection health monitoring
    """
    async def start_heartbeat(self):
        while self.active:
            if await self._check_connection_health():
                await self._send_ping()
            else:
                await self._trigger_recovery()
            await asyncio.sleep(15)  # 15-second intervals
```

#### Layer 2: Data Freshness Monitor
```python
class DataFreshnessMonitor:
    """
    Tracks data freshness per symbol during trading hours
    """
    def __init__(self):
        self.last_data_timestamps = {}
        self.trading_hours = TradingHours()
        
    async def monitor_data_flow(self):
        while True:
            if self.trading_hours.is_market_open():
                stale_symbols = self._check_stale_symbols(
                    threshold_seconds=30
                )
                if stale_symbols:
                    await self._handle_stale_data(stale_symbols)
            await asyncio.sleep(5)  # Check every 5 seconds
```

#### Layer 3: Circuit Breaker Pattern
```python
class CircuitBreaker:
    """
    Prevents cascade failures and manages recovery attempts
    """
    def __init__(self):
        self.states = ['CLOSED', 'OPEN', 'HALF_OPEN']
        self.failure_threshold = 5
        self.recovery_timeout = 60
        self.success_threshold = 3
```

### 3. Adaptive Recovery Strategy

```python
class AdaptiveRecoveryStrategy:
    """
    Intelligent recovery based on failure patterns
    """
    
    async def recover(self, failure_context):
        strategy = self._determine_strategy(failure_context)
        
        if strategy == 'QUICK_RECONNECT':
            return await self._quick_reconnect()
        elif strategy == 'FULL_RESET':
            return await self._full_connection_reset()
        elif strategy == 'FALLBACK_TO_POLLING':
            return await self._activate_polling_mode()
        elif strategy == 'ESCALATE':
            return await self._escalate_to_container_restart()
```

### 4. Trading Hours Awareness

```python
class TradingHours:
    """
    Market hours tracking with holiday awareness
    """
    def __init__(self):
        self.market_open = time(9, 30)  # 9:30 AM ET
        self.market_close = time(16, 0)  # 4:00 PM ET
        self.timezone = pytz.timezone('America/New_York')
        
    def is_market_open(self):
        now = datetime.now(self.timezone)
        if now.weekday() >= 5:  # Weekend
            return False
        if self._is_market_holiday(now.date()):
            return False
        return self.market_open <= now.time() <= self.market_close
```

### 5. Enhanced Metrics and Alerting

```yaml
# Additional Prometheus alerts for WebSocket monitoring
- alert: AlpacaWebSocketNoData
  expr: |
    (time() - alpaca_websocket_last_data_timestamp) > 30
    and hour() >= 14 and hour() < 21  # 9:30 AM - 4:00 PM ET in UTC
    and dayofweek() >= 1 and dayofweek() <= 5
  for: 30s
  labels:
    severity: critical
    component: alpaca-websocket
  annotations:
    summary: "No Alpaca WebSocket data during trading hours"
    description: "No data received for {{ $value }} seconds during active trading"

- alert: AlpacaWebSocketConnectionFailures
  expr: |
    rate(alpaca_websocket_reconnection_attempts_total[5m]) > 0.5
  for: 2m
  labels:
    severity: warning
  annotations:
    summary: "Frequent WebSocket reconnections"
    description: "WebSocket reconnecting {{ $value }} times per minute"
```

## Implementation Plan

### Phase 1: Monitoring Infrastructure (Week 1)
1. Implement `DataFreshnessMonitor` class
2. Add WebSocket-specific Prometheus metrics
3. Create trading hours awareness
4. Deploy enhanced alerting rules

### Phase 2: Recovery Mechanisms (Week 2)
1. Implement `HeartbeatManager` for proactive health checks
2. Add `CircuitBreaker` pattern
3. Create `AdaptiveRecoveryStrategy`
4. Test recovery scenarios

### Phase 3: Integration and Testing (Week 3)
1. Integrate with existing Alpaca provider
2. Add comprehensive logging
3. Load testing during market hours
4. Failover scenario testing

### Phase 4: Production Deployment (Week 4)
1. Gradual rollout with feature flags
2. Monitor new metrics
3. Tune recovery parameters
4. Document operational procedures

## Operational Procedures

### 1. Automatic Recovery Flow
```
1. Data gap detected (30 seconds)
   ↓
2. Check if during trading hours
   ↓
3. Verify connection state
   ↓
4. Execute recovery strategy:
   - Quick reconnect (< 3 failures)
   - Full reset (3-5 failures)
   - Fallback to polling (5-10 failures)
   - Container restart (> 10 failures)
```

### 2. Manual Intervention Triggers
- Circuit breaker in OPEN state for > 5 minutes
- Repeated fallback to polling mode
- Error rate > 50% for WebSocket operations
- Data gaps > 2 minutes during critical trading periods

### 3. Monitoring Dashboard
Key metrics to display:
- WebSocket connection uptime
- Data freshness by symbol
- Recovery attempts and success rate
- Circuit breaker state
- Current recovery strategy

## Best Practices

### 1. Connection Management
- Use connection pooling for multiple symbols
- Implement graceful shutdown procedures
- Maintain connection state metadata
- Log all state transitions

### 2. Error Handling
- Categorize errors (network, auth, rate limit, etc.)
- Implement error-specific recovery strategies
- Maintain error history for pattern detection
- Alert on new error patterns

### 3. Performance Optimization
- Batch symbol subscriptions
- Use efficient data structures for timestamp tracking
- Implement backpressure handling
- Monitor memory usage

## Testing Strategy

### 1. Unit Tests
- Test each monitoring component independently
- Mock WebSocket failures
- Verify recovery strategy selection
- Test trading hours calculations

### 2. Integration Tests
- Simulate network failures
- Test during market hours simulation
- Verify metric collection
- Test alert triggering

### 3. Load Tests
- Subscribe to maximum symbols
- Simulate high data rates
- Test recovery under load
- Verify no data loss during recovery

### 4. Chaos Engineering
- Random connection drops
- Partial message delivery
- Authentication failures
- Rate limiting scenarios

## Conclusion

The current Alpaca WebSocket implementation lacks sophisticated monitoring and recovery mechanisms necessary for production trading systems. This proposed solution provides:

1. **Proactive Monitoring**: Multiple layers of health checks
2. **Intelligent Recovery**: Adaptive strategies based on failure patterns
3. **Trading Hours Awareness**: Critical for financial markets
4. **Operational Excellence**: Clear procedures and comprehensive monitoring

Implementation of these enhancements will eliminate the need for manual container restarts and ensure continuous data flow during critical trading hours.