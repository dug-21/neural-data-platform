# Minimal AlpacaProvider WebSocket Integration

## Overview
This pseudocode shows ONLY the minimal changes needed to add WebSocket support to the existing AlpacaProvider class. All complex components (ConnectionManager, StreamManager, etc.) are removed. We focus on integrating Alpaca's StockDataStream WebSocket functionality into the existing provider pattern.

## 1. AlpacaProvider Class Modifications

### 1.1 Update __init__ Method

```python
def __init__(self):
    # ... existing initialization ...
    
    # Add WebSocket-specific attributes
    self._ws_connected = False
    self._ws_subscriptions = set()  # Track subscribed symbols
    self._ws_data_queue = asyncio.Queue(maxsize=1000)  # Buffer for incoming data
    self._ws_handlers = {}  # Message type handlers
    
    # Register WebSocket message handlers
    self._register_ws_handlers()
```

### 1.2 Add WebSocket Handler Registration

```python
def _register_ws_handlers(self):
    """Register handlers for different WebSocket message types."""
    # Map Alpaca message handlers to our internal format
    async def on_trade(trade):
        market_data = MarketData(
            time=trade.timestamp,
            symbol=trade.symbol,
            open=float(trade.price),
            high=float(trade.price),
            low=float(trade.price),
            close=float(trade.price),
            volume=int(trade.size),
            provider=self.name,
            metadata={
                "type": "trade",
                "exchange": trade.exchange,
                "conditions": trade.conditions
            }
        )
        await self._ws_data_queue.put(market_data)
    
    async def on_quote(quote):
        # Convert quote to MarketData using bid/ask midpoint
        mid_price = (quote.bid_price + quote.ask_price) / 2
        market_data = MarketData(
            time=quote.timestamp,
            symbol=quote.symbol,
            open=mid_price,
            high=quote.ask_price,  # Ask as high
            low=quote.bid_price,   # Bid as low
            close=mid_price,
            volume=quote.bid_size + quote.ask_size,
            provider=self.name,
            metadata={
                "type": "quote",
                "bid_price": float(quote.bid_price),
                "ask_price": float(quote.ask_price),
                "bid_size": int(quote.bid_size),
                "ask_size": int(quote.ask_size),
                "spread": float(quote.ask_price - quote.bid_price)
            }
        )
        await self._ws_data_queue.put(market_data)
    
    async def on_bar(bar):
        market_data = MarketData(
            time=bar.timestamp,
            symbol=bar.symbol,
            open=float(bar.open),
            high=float(bar.high),
            low=float(bar.low),
            close=float(bar.close),
            volume=int(bar.volume),
            provider=self.name,
            metadata={
                "type": "bar",
                "trade_count": bar.trade_count,
                "vwap": float(bar.vwap) if bar.vwap else None
            }
        )
        await self._ws_data_queue.put(market_data)
    
    # Store handlers
    self._ws_handlers = {
        'trade': on_trade,
        'quote': on_quote,
        'bar': on_bar
    }
```

### 1.3 Add WebSocket Connection Method

```python
async def _connect_websocket(self):
    """Connect to Alpaca WebSocket if not already connected."""
    if self._ws_connected:
        return
        
    try:
        # Register handlers with the StockDataStream instance
        self.stock_stream.subscribe_trades(
            self._ws_handlers['trade'],
            *list(self._ws_subscriptions)
        )
        self.stock_stream.subscribe_quotes(
            self._ws_handlers['quote'],
            *list(self._ws_subscriptions)
        )
        self.stock_stream.subscribe_bars(
            self._ws_handlers['bar'],
            *list(self._ws_subscriptions)
        )
        
        # Start the WebSocket connection in background
        self._stream_task = asyncio.create_task(self._run_websocket())
        self._ws_connected = True
        
        self.logger.info(f"WebSocket connected for symbols: {self._ws_subscriptions}")
        
    except Exception as e:
        self.logger.error(f"Failed to connect WebSocket: {e}")
        raise
```

### 1.4 Add WebSocket Run Method

```python
async def _run_websocket(self):
    """Run the WebSocket connection with automatic reconnection."""
    retry_count = 0
    max_retries = 10
    
    while retry_count < max_retries:
        try:
            # Run the WebSocket stream
            await self.stock_stream.run()
            
        except Exception as e:
            self.logger.error(f"WebSocket error: {e}")
            retry_count += 1
            
            # Exponential backoff
            wait_time = min(2 ** retry_count, 60)
            self.logger.info(f"Reconnecting in {wait_time} seconds... (attempt {retry_count}/{max_retries})")
            await asyncio.sleep(wait_time)
            
            # Reset connection state
            self._ws_connected = False
            
            # Try to reconnect
            if retry_count < max_retries:
                await self._connect_websocket()
```

### 1.5 Replace stream_market_data Method

```python
async def stream_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
    """Stream real-time market data via WebSocket."""
    symbols = self._validate_symbols(symbols)
    
    if not symbols:
        self.logger.warning("No valid symbols to stream")
        return
    
    # Check WebSocket symbol limits
    if self.subscription_level == "basic":
        max_symbols = self._subscription_limits["websocket_symbols"]
        if max_symbols and len(symbols) > max_symbols:
            self.logger.warning(f"Basic plan limited to {max_symbols} WebSocket symbols. Truncating.")
            symbols = symbols[:max_symbols]
    
    # Update subscriptions
    self._ws_subscriptions.update(symbols)
    
    # Connect WebSocket if needed
    if not self._ws_connected:
        await self._connect_websocket()
    else:
        # Subscribe to new symbols on existing connection
        for symbol in symbols:
            if symbol not in self._ws_subscriptions:
                self.stock_stream.subscribe_trades(self._ws_handlers['trade'], symbol)
                self.stock_stream.subscribe_quotes(self._ws_handlers['quote'], symbol)
                self.stock_stream.subscribe_bars(self._ws_handlers['bar'], symbol)
    
    self.logger.info(f"Streaming real-time data for symbols: {symbols}")
    
    # Yield data from queue as it arrives
    while True:
        try:
            # Get data from queue with timeout
            data = await asyncio.wait_for(
                self._ws_data_queue.get(),
                timeout=30.0  # 30 second timeout
            )
            
            # Only yield if symbol is in our requested list
            if data.symbol in symbols:
                yield data
                
        except asyncio.TimeoutError:
            self.logger.warning("No data received for 30 seconds")
            # Check if WebSocket is still alive
            if not self._ws_connected:
                self.logger.error("WebSocket disconnected, attempting reconnect...")
                await self._connect_websocket()
                
        except Exception as e:
            self.logger.error(f"Error processing stream data: {e}")
            await asyncio.sleep(1)
```

### 1.6 Add New WebSocket-Specific Stream Method

```python
async def stream_market_data_ws(
    self,
    symbols: List[str],
    data_types: List[str] = ["trades", "quotes", "bars"]
) -> AsyncIterator[MarketData]:
    """
    Stream specific data types via WebSocket.
    
    Args:
        symbols: List of stock symbols
        data_types: List of data types to stream ("trades", "quotes", "bars")
    """
    symbols = self._validate_symbols(symbols)
    
    if not symbols:
        self.logger.warning("No valid symbols to stream")
        return
    
    # Update subscriptions
    self._ws_subscriptions.update(symbols)
    
    # Connect WebSocket if needed
    if not self._ws_connected:
        await self._connect_websocket()
    
    # Subscribe to requested data types
    for symbol in symbols:
        if "trades" in data_types:
            self.stock_stream.subscribe_trades(self._ws_handlers['trade'], symbol)
        if "quotes" in data_types:
            self.stock_stream.subscribe_quotes(self._ws_handlers['quote'], symbol)
        if "bars" in data_types:
            self.stock_stream.subscribe_bars(self._ws_handlers['bar'], symbol)
    
    self.logger.info(f"Streaming {data_types} for symbols: {symbols}")
    
    # Yield data from queue
    while True:
        try:
            data = await asyncio.wait_for(self._ws_data_queue.get(), timeout=30.0)
            
            # Filter by requested data types
            if data.symbol in symbols:
                data_type = data.metadata.get("type")
                if data_type in data_types:
                    yield data
                    
        except asyncio.TimeoutError:
            self.logger.warning("No data received for 30 seconds")
            if not self._ws_connected:
                await self._connect_websocket()
                
        except Exception as e:
            self.logger.error(f"Error in WebSocket stream: {e}")
            await asyncio.sleep(1)
```

### 1.7 Update disconnect Method

```python
async def disconnect(self):
    """Close all connections including WebSocket."""
    try:
        # Cancel WebSocket streaming task first
        if self._stream_task and not self._stream_task.done():
            self._stream_task.cancel()
            try:
                await self._stream_task
            except asyncio.CancelledError:
                pass
        
        # Close WebSocket stream
        if self.stock_stream and self._ws_connected:
            try:
                await self.stock_stream.close()
                self._ws_connected = False
                self._ws_subscriptions.clear()
            except Exception as e:
                self.logger.error(f"Error closing WebSocket: {e}")
        
        # ... existing disconnect logic ...
        
    except Exception as e:
        self.logger.error(f"Error during disconnect: {e}")
```

## 2. Usage Example

```python
# Initialize provider
provider = AlpacaProvider()
await provider.connect()

# Stream real-time data via WebSocket
async for market_data in provider.stream_market_data_ws(
    symbols=["AAPL", "GOOGL", "MSFT"],
    data_types=["trades", "quotes"]  # Only trades and quotes
):
    print(f"{market_data.symbol}: ${market_data.close} @ {market_data.time}")
    
# Or use the existing stream_market_data (now uses WebSocket)
async for market_data in provider.stream_market_data(["AAPL", "GOOGL"]):
    print(f"Real-time: {market_data.symbol} = ${market_data.close}")
```

## Key Implementation Points

1. **Minimal Changes**: Only modifies AlpacaProvider class, no new classes needed
2. **Reuses Existing SDK**: Uses Alpaca's StockDataStream from the SDK
3. **Async Queue**: Buffers incoming WebSocket data in an async queue
4. **Message Handlers**: Converts Alpaca messages to existing MarketData format
5. **Automatic Reconnection**: Built-in retry logic with exponential backoff
6. **Subscription Management**: Tracks subscribed symbols and respects plan limits
7. **Backward Compatible**: Existing polling methods still work as fallback

## Benefits

- **Simple Integration**: Just adds methods to existing class
- **No Breaking Changes**: Existing code continues to work
- **Leverages SDK**: Uses official Alpaca WebSocket implementation
- **Error Handling**: Includes reconnection and error recovery
- **Plan Aware**: Respects basic/unlimited plan limits