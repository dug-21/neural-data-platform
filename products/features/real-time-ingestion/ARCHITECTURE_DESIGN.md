# Real-Time Streaming Architecture Design - Minimal WebSocket Implementation

## Executive Summary

This document presents a minimal architectural design for adding WebSocket support to the existing AlpacaProvider class in the Neural Trader platform. The design focuses solely on enhancing the data_ingestion/providers/alpaca.py file with WebSocket streaming capabilities while maintaining full compatibility with the existing system.

## 1. Scope and Approach

### 1.1 What Changes
- **Only one file**: `data_ingestion/providers/alpaca.py`
- Add WebSocket client using the `websockets` library
- Add a new method `stream_market_data_ws()` for WebSocket streaming
- Reuse existing `_normalize_data()` method for data transformation
- Keep `stream_market_data()` as fallback for polling

### 1.2 What Doesn't Change
- RealtimeCoordinator - already supports streaming providers
- StreamManager - already handles async iterators
- Database models - existing MarketData format is maintained
- Redis integration - no changes needed
- DAA agents - continue to consume data as before

## 2. Minimal Implementation Design

### 2.1 Enhanced AlpacaProvider Class

```python
import websockets
import json
from typing import AsyncIterator, List, Dict, Optional
import asyncio
from datetime import datetime

class AlpacaProvider(BaseProvider):
    """Alpaca market data provider with WebSocket support."""
    
    def __init__(self):
        super().__init__("alpaca")
        self.ws_url = "wss://stream.data.alpaca.markets/v2/iex"
        self.ws_connection: Optional[websockets.WebSocketClientProtocol] = None
        self.reconnect_delay = 1.0
        self.max_reconnect_delay = 60.0
        
    async def stream_market_data_ws(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time data via WebSocket."""
        while True:
            try:
                # Connect and authenticate
                await self._connect_websocket()
                await self._authenticate_websocket()
                await self._subscribe_symbols(symbols)
                
                # Stream messages
                async for message in self._receive_messages():
                    market_data = self._process_ws_message(message)
                    if market_data:
                        yield market_data
                        
            except Exception as e:
                self.logger.error(f"WebSocket error: {e}")
                await self._handle_reconnect()
                # Fall back to polling if needed
                if self.reconnect_delay > 30:
                    async for data in self.stream_market_data(symbols):
                        yield data
                        
    async def _connect_websocket(self):
        """Establish WebSocket connection."""
        self.ws_connection = await websockets.connect(self.ws_url)
        
    async def _authenticate_websocket(self):
        """Send authentication message."""
        auth_msg = {
            "action": "auth",
            "key": self.config.api_key,
            "secret": self.config.api_secret
        }
        await self.ws_connection.send(json.dumps(auth_msg))
        
    async def _subscribe_symbols(self, symbols: List[str]):
        """Subscribe to market data for symbols."""
        sub_msg = {
            "action": "subscribe",
            "trades": symbols,
            "quotes": symbols,
            "bars": symbols
        }
        await self.ws_connection.send(json.dumps(sub_msg))
        
    async def _receive_messages(self):
        """Receive and yield WebSocket messages."""
        async for message in self.ws_connection:
            yield json.loads(message)
            
    def _process_ws_message(self, message: List[Dict]) -> Optional[MarketData]:
        """Process WebSocket message and normalize data."""
        if not isinstance(message, list) or not message:
            return None
            
        msg = message[0]
        msg_type = msg.get('T')
        
        # Map WebSocket message to format expected by _normalize_data
        if msg_type == 't':  # Trade
            data = {
                'symbol': msg['S'],
                'price': msg['p'],
                'volume': msg['s'],
                'timestamp': msg['t']
            }
        elif msg_type == 'q':  # Quote
            data = {
                'symbol': msg['S'],
                'bid': msg['bp'],
                'ask': msg['ap'],
                'bid_size': msg['bs'],
                'ask_size': msg['as'],
                'timestamp': msg['t']
            }
        elif msg_type == 'b':  # Bar
            data = {
                'symbol': msg['S'],
                'open': msg['o'],
                'high': msg['h'],
                'low': msg['l'],
                'close': msg['c'],
                'volume': msg['v'],
                'timestamp': msg['t']
            }
        else:
            return None
            
        # Reuse existing normalization
        return self._normalize_data(data)
        
    async def _handle_reconnect(self):
        """Handle reconnection with exponential backoff."""
        if self.ws_connection:
            await self.ws_connection.close()
            
        await asyncio.sleep(self.reconnect_delay)
        self.reconnect_delay = min(self.reconnect_delay * 2, self.max_reconnect_delay)
```

### 2.2 Integration Points

The existing RealtimeCoordinator will automatically use the WebSocket streaming:

```python
# In RealtimeCoordinator._stream_provider
if hasattr(provider, 'stream_market_data_ws'):
    # Use WebSocket streaming if available
    async for market_data in provider.stream_market_data_ws(symbols):
        await self._process_market_data(market_data, provider_name)
else:
    # Fall back to polling
    async for market_data in provider.stream_market_data(symbols):
        await self._process_market_data(market_data, provider_name)
```

## 3. Message Format Mapping

### 3.1 Alpaca WebSocket Message Types

| Type | Description | Fields Used |
|------|-------------|-------------|
| `t` | Trade | symbol, price, size, timestamp |
| `q` | Quote | symbol, bid, ask, bid_size, ask_size, timestamp |
| `b` | Bar | symbol, open, high, low, close, volume, timestamp |

### 3.2 Normalization Example

WebSocket message:
```json
{
  "T": "t",
  "S": "AAPL",
  "p": 150.25,
  "s": 100,
  "t": "2024-01-15T10:30:00Z"
}
```

Normalized to MarketData:
```python
MarketData(
    provider="alpaca",
    symbol="AAPL",
    price=150.25,
    volume=100,
    timestamp=datetime(2024, 1, 15, 10, 30, 0),
    data_type="trade"
)
```

## 4. Error Handling and Reliability

### 4.1 Connection Management
- Automatic reconnection with exponential backoff (1s → 60s)
- Graceful fallback to polling after 30 seconds of failures
- Connection state tracking

### 4.2 Message Handling
- Validate message format before processing
- Skip invalid messages without disrupting stream
- Log errors for monitoring

## 5. Configuration

Add to `.env`:
```bash
# WebSocket Configuration (optional - defaults shown)
ALPACA_WS_URL=wss://stream.data.alpaca.markets/v2/iex
ALPACA_WS_RECONNECT_DELAY=1.0
ALPACA_WS_MAX_RECONNECT_DELAY=60.0
```

## 6. Testing Approach

### 6.1 Unit Tests
```python
async def test_websocket_message_parsing():
    """Test WebSocket message parsing."""
    provider = AlpacaProvider()
    
    # Test trade message
    trade_msg = [{"T": "t", "S": "AAPL", "p": 150.25, "s": 100, "t": "2024-01-15T10:30:00Z"}]
    market_data = provider._process_ws_message(trade_msg)
    
    assert market_data.symbol == "AAPL"
    assert market_data.price == 150.25
    assert market_data.volume == 100
```

### 6.2 Integration Tests
```python
async def test_websocket_streaming():
    """Test WebSocket streaming integration."""
    provider = AlpacaProvider()
    
    count = 0
    async for market_data in provider.stream_market_data_ws(["AAPL"]):
        assert isinstance(market_data, MarketData)
        count += 1
        if count >= 10:
            break
```

## 7. Implementation Steps

1. **Add websockets dependency** to requirements.txt:
   ```
   websockets>=12.0
   ```

2. **Update AlpacaProvider** with WebSocket methods as shown above

3. **Test WebSocket connection** with simple script

4. **Add fallback logic** to ensure reliability

5. **Update tests** to cover new functionality

## 8. Performance Considerations

- WebSocket provides sub-second latency (vs 1-5 second polling)
- Single connection handles multiple symbol subscriptions
- Minimal memory overhead - reuses existing data structures
- No additional database load - same storage pattern

## 9. Monitoring

Log these metrics using existing logging:
- WebSocket connection status
- Message receive rate
- Reconnection attempts
- Fallback activations

## 10. Summary

This minimal implementation adds WebSocket streaming to AlpacaProvider while:
- Maintaining backward compatibility
- Reusing existing infrastructure
- Providing automatic fallback
- Requiring no changes to other components

The implementation is focused, testable, and integrates seamlessly with the existing Neural Trader architecture.