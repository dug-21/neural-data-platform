# Alpaca WebSocket API Quick Reference

## WebSocket Endpoints

### Stock Data Streams
```
Free (IEX): wss://stream.data.alpaca.markets/v2/iex
Paid (SIP): wss://stream.data.alpaca.markets/v2/sip
Test:       wss://stream.data.alpaca.markets/v2/test
```

### Other Asset Classes
```
Options: wss://stream.data.alpaca.markets/v1beta1/options
Crypto:  wss://stream.data.alpaca.markets/v1beta3/crypto/us
News:    wss://stream.data.alpaca.markets/v1beta1/news
```

## Authentication

```json
{
  "action": "auth",
  "key": "YOUR_API_KEY_ID",
  "secret": "YOUR_SECRET_KEY"
}
```

**Success Response:**
```json
{
  "T": "success",
  "msg": "authenticated"
}
```

## Subscription Management

### Subscribe
```json
{
  "action": "subscribe",
  "trades": ["AAPL", "TSLA"],
  "quotes": ["AAPL", "TSLA"],
  "bars": ["*"],
  "dailyBars": ["VOO", "SPY"],
  "statuses": ["*"],
  "lulds": ["AAPL", "TSLA"]
}
```

### Unsubscribe
```json
{
  "action": "unsubscribe",
  "trades": ["TSLA"],
  "quotes": ["TSLA"]
}
```

## Message Types

### Trade (T: "t")
```json
{
  "T": "t",
  "S": "AAPL",
  "p": 172.82,
  "s": 100,
  "t": "2024-01-14T14:30:00.123456789Z",
  "c": ["@", "I"],
  "i": 52983525029461,
  "x": "V",
  "z": "C"
}
```

### Quote (T: "q")
```json
{
  "T": "q",
  "S": "AAPL",
  "bx": "V",
  "bp": 172.80,
  "bs": 3,
  "ax": "V",
  "ap": 172.82,
  "as": 2,
  "t": "2024-01-14T14:30:00.234567890Z",
  "c": ["R"],
  "z": "C"
}
```

### Bar (T: "b")
```json
{
  "T": "b",
  "S": "AAPL",
  "o": 172.75,
  "h": 172.85,
  "l": 172.70,
  "c": 172.82,
  "v": 12345,
  "t": "2024-01-14T14:30:00Z",
  "n": 234,
  "vw": 172.78
}
```

## Subscription Limits

| Plan | Max Symbols | Connections | Historical Data |
|------|-------------|-------------|-----------------|
| Free | 30 | 1 | 15 min delayed |
| Algo Trader Plus | Unlimited | Multiple | No restrictions |

## Field Reference

### Trade Fields
- `T`: Message type
- `S`: Symbol
- `p`: Price
- `s`: Size
- `t`: Timestamp
- `c`: Conditions
- `i`: Trade ID
- `x`: Exchange code
- `z`: Tape

### Quote Fields
- `bx`: Bid exchange
- `bp`: Bid price
- `bs`: Bid size
- `ax`: Ask exchange
- `ap`: Ask price
- `as`: Ask size

### Bar Fields
- `o`: Open
- `h`: High
- `l`: Low
- `c`: Close
- `v`: Volume
- `n`: Trade count
- `vw`: VWAP

## Exchange Codes
- `A`: NYSE American
- `B`: NASDAQ BX
- `C`: NSX
- `D`: FINRA
- `E`: Market Independent
- `H`: MIAX
- `I`: ISE
- `J`: EDGA
- `K`: EDGX
- `L`: LTSE
- `M`: CHX
- `N`: NYSE
- `P`: ARCA
- `Q`: NASDAQ
- `S`: NASDAQ Small Cap
- `T`: NASDAQ Int
- `U`: Members Exchange
- `V`: IEX
- `W`: CBOE
- `X`: PSX
- `Y`: BYX
- `Z`: BZX

## Condition Codes
- `@`: Regular Sale
- `A`: Acquisition
- `B`: Bunched Trade
- `C`: Cash Sale
- `D`: Distribution
- `E`: Placeholder
- `F`: Intermarket Sweep
- `G`: Bunched Sold Trade
- `H`: Price Variation Trade
- `I`: Odd Lot Trade
- `K`: Rule 155 Trade
- `L`: Sold Last
- `M`: Market Center Official Close
- `N`: Next Day
- `O`: Opening Prints
- `P`: Prior Reference Price
- `Q`: Market Center Official Open
- `R`: Seller
- `S`: Split Trade
- `T`: Form T
- `U`: Extended Hours
- `V`: Contingent Trade
- `W`: Average Price Trade
- `X`: Cross/Periodic Auction Trade
- `Y`: Yellow Flag
- `Z`: Sold Out of Sequence

## Error Messages

### Connection Errors
```json
{
  "T": "error",
  "code": 400,
  "msg": "invalid syntax"
}
```

### Authentication Errors
```json
{
  "T": "error",
  "code": 401,
  "msg": "not authenticated"
}
```

### Subscription Errors
```json
{
  "T": "error",
  "code": 402,
  "msg": "auth failure"
}
```

## Implementation Tips

1. **Connection Management**
   - Implement exponential backoff for reconnections
   - Use ping/pong frames for keepalive
   - Handle connection drops gracefully

2. **Message Processing**
   - Parse timestamps as nanoseconds
   - Handle out-of-order messages
   - Implement deduplication

3. **Performance**
   - Use message batching
   - Implement backpressure handling
   - Monitor memory usage

4. **Best Practices**
   - Start with IEX feed for development
   - Test with low-volume symbols first
   - Implement comprehensive logging
   - Monitor connection health

## Python Example

```python
import asyncio
import json
import websockets

async def alpaca_stream():
    uri = "wss://stream.data.alpaca.markets/v2/iex"
    
    async with websockets.connect(uri) as websocket:
        # Authenticate
        auth = {
            "action": "auth",
            "key": "YOUR_KEY",
            "secret": "YOUR_SECRET"
        }
        await websocket.send(json.dumps(auth))
        response = await websocket.recv()
        print(f"Auth response: {response}")
        
        # Subscribe
        sub = {
            "action": "subscribe",
            "trades": ["AAPL"],
            "quotes": ["AAPL"],
            "bars": ["AAPL"]
        }
        await websocket.send(json.dumps(sub))
        
        # Receive messages
        async for message in websocket:
            data = json.loads(message)
            print(f"Received: {data}")

if __name__ == "__main__":
    asyncio.run(alpaca_stream())
```