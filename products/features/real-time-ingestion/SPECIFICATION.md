# SPARC Specification: Real-Time Market Data Ingestion (Minimal Changes)

## 1. Feature Overview

### 1.1 Purpose
Enhance the existing AlpacaProvider in the Neural Trader platform by adding WebSocket streaming alongside the current polling mechanism, reducing data latency from 5 seconds to sub-second updates.

### 1.2 Scope
- Add WebSocket streaming method to existing AlpacaProvider class
- Keep all existing polling functionality as fallback
- Support free (IEX) data feed for stock symbols only
- No changes required to other components (RealtimeCoordinator, StreamManager already support streaming)
- Minimal code changes - extend rather than replace

### 1.3 Success Criteria
- Reduce data latency from 5 seconds to <100ms for stock symbols
- Support existing symbol subscription patterns
- Maintain automatic fallback to polling on WebSocket failure
- Use existing MarketData model without modifications
- No breaking changes to existing provider interface

## 2. Functional Requirements

### 2.1 Minimal AlpacaProvider Enhancement

#### 2.1.1 WebSocket Method Addition
- **REQ-001**: AlpacaProvider SHALL add a stream_market_data_ws() method alongside existing stream_market_data()
- **REQ-002**: WebSocket method SHALL connect to wss://stream.data.alpaca.markets/v2/iex
- **REQ-003**: WebSocket method SHALL authenticate using existing API credentials
- **REQ-004**: WebSocket method SHALL reuse existing MarketData model

#### 2.1.2 Fallback Behavior
- **REQ-005**: System SHALL automatically fallback to polling method on WebSocket failure
- **REQ-006**: System SHALL use simple reconnection with exponential backoff
- **REQ-007**: Both methods SHALL yield identical MarketData objects
- **REQ-008**: Configuration SHALL determine primary method (ws or polling)

### 2.2 Data Subscription

#### 2.2.1 Symbol Management
- **REQ-009**: WebSocket method SHALL subscribe to same symbols as polling method
- **REQ-010**: WebSocket method SHALL handle stock symbols only (no crypto/options)
- **REQ-011**: System SHALL use single WebSocket connection (no distribution needed)
- **REQ-012**: System SHALL respect IEX feed limitations

#### 2.2.2 Data Types
- **REQ-013**: System SHALL subscribe to real-time bars (1-minute aggregates)
- **REQ-014**: System SHALL convert bar data to existing MarketData format
- **REQ-015**: System SHALL maintain OHLCV structure compatibility
- **REQ-016**: System SHALL ignore non-bar message types
- **REQ-017**: System SHALL use same data validation as polling method

### 2.3 Message Processing

#### 2.3.1 Parsing
- **REQ-018**: System SHALL parse Alpaca bar messages using existing JSON parsing
- **REQ-019**: System SHALL extract OHLCV fields from bar messages
- **REQ-020**: System SHALL skip non-bar messages without error
- **REQ-021**: System SHALL use datetime parsing consistent with polling method

#### 2.3.2 Conversion to MarketData
- **REQ-022**: System SHALL populate MarketData fields exactly as polling method does
- **REQ-023**: System SHALL use same timestamp handling as existing code
- **REQ-024**: System SHALL apply same price rounding as polling method
- **REQ-025**: System SHALL yield MarketData objects via async generator

### 2.4 Integration Requirements

#### 2.4.1 AlpacaProvider Extension
- **REQ-026**: AlpacaProvider SHALL add stream_market_data_ws() without modifying existing methods
- **REQ-027**: Both streaming methods SHALL have identical signatures and return types
- **REQ-028**: Provider SHALL select method based on ALPACA_WS_ENABLED config
- **REQ-029**: Provider SHALL fallback to polling if WebSocket fails

#### 2.4.2 No Changes to Storage Layer
- **REQ-030**: WebSocket method SHALL yield data to existing RealtimeCoordinator
- **REQ-031**: No changes needed to Redis/TimescaleDB integration
- **REQ-032**: StreamManager already handles async iterators properly
- **REQ-033**: All downstream components remain unchanged

## 3. Non-Functional Requirements

### 3.1 Performance

- **NFR-001**: WebSocket latency SHALL be <100ms (better than 5s polling)
- **NFR-002**: Memory overhead SHALL be minimal (single connection)
- **NFR-003**: CPU usage SHALL be comparable to polling method
- **NFR-004**: No performance regression for existing functionality

### 3.2 Reliability

- **NFR-005**: System SHALL maintain existing reliability via fallback
- **NFR-006**: WebSocket failures SHALL not impact system availability
- **NFR-007**: Polling method SHALL remain fully functional
- **NFR-008**: Configuration SHALL allow disabling WebSocket entirely

### 3.3 Simplicity

- **NFR-009**: Implementation SHALL be contained within AlpacaProvider
- **NFR-010**: No new dependencies beyond websocket client library
- **NFR-011**: Code changes SHALL be under 200 lines
- **NFR-012**: No breaking changes to existing interfaces

### 3.4 Security

- **NFR-013**: All connections SHALL use TLS 1.3 encryption
- **NFR-014**: API credentials SHALL be stored securely
- **NFR-015**: System SHALL support key rotation without downtime
- **NFR-016**: System SHALL implement rate limiting per connection

## 4. Interface Specifications

### 4.1 WebSocket Message Format

#### 4.1.1 Authentication Message
```json
{
  "action": "auth",
  "key": "${API_KEY}",
  "secret": "${API_SECRET}"
}
```

#### 4.1.2 Subscription Message
```json
{
  "action": "subscribe",
  "trades": ["AAPL", "GOOGL", "MSFT"],
  "quotes": ["AAPL", "GOOGL", "MSFT"],
  "bars": ["AAPL", "GOOGL", "MSFT"]
}
```

#### 4.1.3 Trade Message Format
```json
{
  "T": "t",              // Message type: trade
  "S": "AAPL",          // Symbol
  "p": 150.25,          // Price
  "s": 100,             // Size
  "t": "2024-01-14T09:30:00.123456789Z",  // Timestamp
  "c": ["@", "F"],      // Conditions
  "i": 12345,           // Trade ID
  "x": "V"              // Exchange code
}
```

### 4.2 Internal Data Schema

#### 4.2.1 MarketData Schema
```python
@dataclass
class MarketData:
    time: datetime       # UTC timestamp
    symbol: str         # Normalized symbol
    open: float        # Opening price
    high: float        # High price
    low: float         # Low price
    close: float       # Closing price
    volume: int        # Volume
    provider: str      # "alpaca"
    metadata: Dict[str, Any]  # Additional fields
```

### 4.3 API Extensions

#### 4.3.1 AlpacaProvider Methods
```python
async def stream_market_data_ws(
    self,
    symbols: List[str]
) -> AsyncIterator[MarketData]:
    """Stream market data via WebSocket.
    
    Same signature as stream_market_data() for drop-in replacement.
    Falls back to polling on WebSocket failure.
    """

# No additional methods needed - keep it minimal
# Existing stream_market_data() remains unchanged
```

## 5. Configuration Specifications

### 5.1 Environment Variables

```bash
# Minimal WebSocket Configuration
ALPACA_WS_ENABLED=false  # Default to false for safety
ALPACA_WS_URL=wss://stream.data.alpaca.markets/v2/iex  # Fixed IEX endpoint

# Optional tuning (with sensible defaults)
ALPACA_WS_RECONNECT_DELAY=5  # Simple 5 second reconnect
ALPACA_WS_MAX_RECONNECT_ATTEMPTS=3  # Fallback to polling after 3 failures
```

### 5.2 No Complex Configuration Needed

- No connection pooling (single connection)
- No load balancing (single instance)
- No circuit breaker (simple fallback to polling)
- Configuration via existing .env file

## 6. Error Handling Specifications

### 6.1 Connection Errors

| Error Type | Handling Strategy | Recovery Action |
|------------|------------------|-----------------|
| Authentication Failed | Log and alert | Check credentials, retry with backoff |
| Connection Timeout | Exponential backoff | Reconnect with new endpoint |
| Connection Closed | Immediate reconnect | Restore subscriptions |
| Network Unreachable | Circuit breaker | Fallback to polling |

### 6.2 Message Errors

| Error Type | Handling Strategy | Recovery Action |
|------------|------------------|-----------------|
| Invalid JSON | Log and skip | Continue processing |
| Unknown Message Type | Log warning | Skip message |
| Missing Required Field | Use defaults | Process with warning |
| Timestamp Out of Order | Buffer and reorder | Process when in sequence |

## 7. Testing Requirements

### 7.1 Unit Tests
- Test stream_market_data_ws() yields MarketData objects
- Test WebSocket message parsing to MarketData conversion
- Test fallback to polling on connection failure
- Test configuration handling

### 7.2 Integration Tests
- Verify WebSocket and polling methods yield identical data
- Test seamless fallback behavior
- Ensure no impact on existing functionality
- Verify RealtimeCoordinator handles both methods

### 7.3 Minimal Test Additions
- Add tests alongside existing AlpacaProvider tests
- Mock WebSocket connection for unit tests
- Use existing test infrastructure
- No new test frameworks needed

## 8. Migration Strategy

### 8.1 Simple Toggle Approach
- Deploy with ALPACA_WS_ENABLED=false (no change to current behavior)
- Test in development with ALPACA_WS_ENABLED=true
- Monitor logs to verify WebSocket data matches polling data
- Enable in production when confident

### 8.2 No Complex Migration Needed
- Both methods coexist in same AlpacaProvider class
- No data migration required
- No infrastructure changes
- Simple configuration toggle

## 9. Monitoring and Alerting

### 9.1 Simple Metrics
- WebSocket connection status (connected/disconnected)
- Fallback count (number of times fallen back to polling)
- Data latency comparison (WebSocket vs polling)
- Use existing logging infrastructure

### 9.2 Basic Alerts
- Log warning when falling back to polling
- Log info when WebSocket reconnects
- Use existing monitoring tools
- No new alerting infrastructure needed

## 10. Compliance and Risk

### 10.1 Data Compliance
- Respect Alpaca terms of service
- Implement rate limiting
- Secure credential storage
- Audit trail for data access

### 10.2 Risk Mitigation
- Fallback to polling on failure
- Data validation and sanitization
- Circuit breaker implementation
- Gradual rollout strategy

## Appendix A: Implementation Example

### Minimal WebSocket Addition to AlpacaProvider
```python
# In alpaca.py - Add this method to existing class
async def stream_market_data_ws(self, symbols: List[str]) -> AsyncIterator[MarketData]:
    """Stream via WebSocket with fallback to polling."""
    if not self.config.get('ALPACA_WS_ENABLED', False):
        # Use existing polling method
        async for data in self.stream_market_data(symbols):
            yield data
        return
    
    try:
        # Simple WebSocket connection
        async with websockets.connect(self.ws_url) as ws:
            # Authenticate
            await ws.send(json.dumps({
                "action": "auth",
                "key": self.api_key,
                "secret": self.api_secret
            }))
            
            # Subscribe
            await ws.send(json.dumps({
                "action": "subscribe",
                "bars": symbols
            }))
            
            # Stream data
            async for message in ws:
                data = json.loads(message)
                if data.get('T') == 'b':  # Bar message
                    yield self._convert_to_market_data(data)
    except Exception as e:
        self.logger.warning(f"WebSocket failed, falling back to polling: {e}")
        async for data in self.stream_market_data(symbols):
            yield data
```

### That's It!
- ~50 lines of code added to existing provider
- No architectural changes
- No impact on other components
- Simple, reliable, minimal