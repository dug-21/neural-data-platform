"""Alpaca Markets data provider implementation using official SDK (stocks only)."""
import asyncio
import json
import websockets
from typing import List, AsyncIterator, Optional
from datetime import datetime, timedelta

from alpaca.data.historical import StockHistoricalDataClient
from alpaca.data.live import StockDataStream
from alpaca.data.requests import (
    StockBarsRequest, 
    StockTradesRequest,
    StockLatestQuoteRequest
)
from alpaca.data.timeframe import TimeFrame, TimeFrameUnit
from alpaca.data.models import Bar, Trade, Quote
from alpaca.data.enums import DataFeed

from .base import BaseProvider, MarketData, TickData, OrderBookData


class AlpacaProvider(BaseProvider):
    """Alpaca Markets data provider using official SDK for stocks."""
    
    # Map intervals to Alpaca TimeFrame parameters
    INTERVAL_MAP = {
        "1min": (1, "Minute"),
        "5min": (5, "Minute"),
        "15min": (15, "Minute"),
        "30min": (30, "Minute"),
        "1hour": (1, "Hour"),
        "4hour": (4, "Hour"),
        "1day": (1, "Day"),
        "1week": (1, "Week"),
        "1month": (1, "Month")
    }
    
    # Alpaca subscription levels
    SUBSCRIPTION_LIMITS = {
        "basic": {
            "websocket_symbols": 30,
            "historical_calls_per_minute": 200,
            "historical_data_age_limit": timedelta(minutes=15),
            "feed": DataFeed.IEX  # Free accounts only have IEX access
        },
        "unlimited": {
            "websocket_symbols": None,  # Unlimited
            "historical_calls_per_minute": 10000,
            "historical_data_age_limit": None,  # No limit
            "feed": DataFeed.SIP  # Paid accounts get SIP access
        }
    }
    
    def __init__(self):
        super().__init__("alpaca")
        self.api_key = self.settings.alpaca_api_key
        self.api_secret = self.settings.alpaca_api_secret
        self.subscription_level = self.settings.alpaca_subscription_level or "basic"
        
        # WebSocket configuration (for direct WebSocket connection)
        self._ws_config = {
            "enabled": getattr(self.settings, 'alpaca_ws_enabled', False),
            "url": getattr(self.settings, 'alpaca_ws_url', 'wss://stream.data.alpaca.markets/v2/iex'),
            "reconnect_delay": getattr(self.settings, 'alpaca_ws_reconnect_delay', 5),
            "max_reconnect_attempts": getattr(self.settings, 'alpaca_ws_max_reconnect_attempts', 3)
        }
        
        # Initialize clients
        self.stock_client: Optional[StockHistoricalDataClient] = None
        self.stock_stream: Optional[StockDataStream] = None
        
        self._subscription_limits = self.SUBSCRIPTION_LIMITS[self.subscription_level]
        self._stream_task = None
        self._data_queue = None
        
        # Add WebSocket-specific attributes (for SDK-based WebSocket)
        self._ws_connected = False
        self._ws_subscriptions = set()  # Track subscribed symbols
        self._ws_data_queue = asyncio.Queue(maxsize=1000)  # Buffer for incoming data
        self._ws_handlers = {}  # Message type handlers
        
        # Register WebSocket message handlers
        self._register_ws_handlers()
    
    async def connect(self):
        """Initialize Alpaca SDK clients."""
        if not self.api_key or not self.api_secret:
            raise ValueError("Alpaca API key and secret not configured")
        
        try:
            # Initialize historical data client with both key and secret
            self.stock_client = StockHistoricalDataClient(
                api_key=self.api_key,
                secret_key=self.api_secret
            )
            
            # Initialize streaming client with both key and secret
            feed_enum = self._subscription_limits["feed"]
            
            try:
                self.stock_stream = StockDataStream(
                    api_key=self.api_key,
                    secret_key=self.api_secret,
                    feed=feed_enum
                )
                self.logger.info(f"StockDataStream initialized with feed={feed_enum}")
            except Exception as e:
                self.logger.error(f"Failed to create StockDataStream: {e}")
                # Try without feed parameter as fallback
                self.stock_stream = StockDataStream(
                    api_key=self.api_key,
                    secret_key=self.api_secret
                )
                self.logger.info("StockDataStream initialized with default feed")
            
            self._connected = True
            self.logger.info(f"Connected to Alpaca Markets SDK ({self.subscription_level} plan, feed={feed_enum})")
        except Exception as e:
            self.logger.error(f"Failed to initialize Alpaca SDK: {str(e)}")
            self.logger.error(f"Error type: {type(e).__name__}")
            import traceback
            self.logger.error(f"Traceback: {traceback.format_exc()}")
            raise
    
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
            
            # Then close the HTTP stream if it exists
            if self.stock_stream:
                try:
                    await self.stock_stream.close()
                except Exception as e:
                    self.logger.error(f"Error closing stream: {e}")
            
            self._connected = False
            self.logger.info("Disconnected from Alpaca Markets")
        except Exception as e:
            self.logger.error(f"Error during disconnect: {e}")
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Fetch market data from Alpaca - uses latest quotes for real-time, historical bars for batched requests."""
        symbols = self._validate_symbols(symbols)
        
        # Determine if this is a real-time request or historical batch request
        now = datetime.now()
        time_diff = (now - end_time).total_seconds()
        
        # If end_time is very recent (within 5 minutes), treat as real-time request
        is_realtime_request = time_diff < 300  # 5 minutes
        
        if is_realtime_request:
            # Use latest quotes for current/real-time pricing
            self.logger.info("Using real-time quotes for current pricing")
            async for data in self._get_current_market_data(symbols):
                yield data
        else:
            # Use historical bars for batch/historical requests
            self.logger.info(f"Using historical bars for batch request ({start_time} to {end_time})")
            async for data in self._get_historical_market_data(symbols, start_time, end_time, interval):
                yield data
    
    async def _get_current_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Get current market data using latest quotes."""
        for symbol in symbols:
            try:
                # Get latest quote for current pricing
                feed_enum = self._subscription_limits["feed"]
                request = StockLatestQuoteRequest(
                    symbol_or_symbols=symbol,
                    feed=feed_enum
                )
                
                self.logger.debug(f"Fetching latest quote for {symbol} with feed={feed_enum}")
                quotes = self.stock_client.get_stock_latest_quote(request)
                
                if symbol in quotes:
                    quote = quotes[symbol]
                    # Convert quote to MarketData format using bid/ask as OHLC
                    current_price = (quote.bid_price + quote.ask_price) / 2 if quote.bid_price and quote.ask_price else quote.bid_price or quote.ask_price
                    
                    market_data = MarketData(
                        time=quote.timestamp,  # Use actual quote timestamp
                        symbol=symbol,
                        open=current_price,
                        high=current_price,
                        low=current_price,
                        close=current_price,
                        volume=quote.bid_size + quote.ask_size,  # Combined bid/ask volume
                        provider=self.name,
                        metadata={
                            "bid_price": float(quote.bid_price) if quote.bid_price else None,
                            "ask_price": float(quote.ask_price) if quote.ask_price else None,
                            "bid_size": int(quote.bid_size) if quote.bid_size else None,
                            "ask_size": int(quote.ask_size) if quote.ask_size else None,
                            "spread": float(quote.ask_price - quote.bid_price) if quote.bid_price and quote.ask_price else None,
                            "data_type": "latest_quote"
                        }
                    )
                    
                    yield market_data
                    self.logger.info(f"Retrieved current quote for {symbol}: ${current_price:.2f} (bid: ${quote.bid_price:.2f}, ask: ${quote.ask_price:.2f}, timestamp: {quote.timestamp})")
                else:
                    self.logger.warning(f"No quote data returned for {symbol}")
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch current data for {symbol}", error=str(e))
                continue
    
    async def _get_historical_market_data(self, symbols: List[str], start_time: datetime, end_time: datetime, interval: str) -> AsyncIterator[MarketData]:
        """Get historical market data using bars."""
        # Get timeframe parameters and create TimeFrame object
        tf_params = self.INTERVAL_MAP.get(interval, (1, "Minute"))
        timeframe = TimeFrame(tf_params[0], TimeFrameUnit[tf_params[1]])
        
        # Check subscription limits for historical data
        if self.subscription_level == "basic":
            age_limit = self._subscription_limits["historical_data_age_limit"]
            if age_limit and datetime.now() - start_time > age_limit:
                self.logger.warning(f"Basic plan only allows data from last {age_limit}. Adjusting start_time.")
                start_time = datetime.now() - age_limit
        
        for symbol in symbols:
            try:
                # Fetch stock bars with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockBarsRequest(
                    symbol_or_symbols=symbol,
                    timeframe=timeframe,
                    start=start_time,
                    end=end_time,
                    feed=feed_enum
                )
                
                self.logger.debug(f"Fetching historical bars for {symbol} from {start_time} to {end_time} with feed={feed_enum}")
                bars = self.stock_client.get_stock_bars(request)
                
                if symbol in bars and len(bars[symbol]) > 0:
                    bar_count = 0
                    for bar in bars[symbol]:
                        yield self._parse_bar(bar, symbol)
                        bar_count += 1
                    self.logger.info(f"Retrieved {bar_count} historical bars for {symbol}")
                else:
                    self.logger.warning(f"No historical data returned for {symbol}")
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch historical data for {symbol}", error=str(e))
                continue
    
    async def _connect_websocket(self):
        """Connect to Alpaca WebSocket if not already connected."""
        if self._ws_connected:
            return
            
        # Ensure the provider is connected first
        if not hasattr(self, 'stock_stream') or self.stock_stream is None:
            self.logger.error("StockDataStream not initialized. Call connect() first.")
            raise RuntimeError("StockDataStream not initialized. Call connect() first.")
            
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
            
            # Mark as connected - the stream will be started separately
            self._ws_connected = True
            
            self.logger.info(f"WebSocket connected for symbols: {self._ws_subscriptions}")
            
        except Exception as e:
            self.logger.error(f"Failed to connect WebSocket: {e}")
            raise
    
    async def _run_websocket(self):
        """Run the WebSocket connection with automatic reconnection."""
        retry_count = 0
        max_retries = 10
        
        # Ensure stream is available
        if not hasattr(self, 'stock_stream') or self.stock_stream is None:
            self.logger.error("StockDataStream not available for WebSocket connection")
            return
        
        while retry_count < max_retries:
            try:
                # Run the WebSocket stream
                self.logger.info("Starting WebSocket stream...")
                await self.stock_stream.run()
                # If we reach here, the stream ended normally
                self.logger.info("WebSocket stream ended normally")
                break
                
            except asyncio.CancelledError:
                self.logger.info("WebSocket stream cancelled")
                break
                
            except Exception as e:
                self.logger.error(f"WebSocket error: {e}")
                retry_count += 1
                
                if retry_count >= max_retries:
                    self.logger.error(f"Max reconnection attempts ({max_retries}) reached")
                    break
                
                # Exponential backoff
                wait_time = min(2 ** retry_count, 60)
                self.logger.info(f"Reconnecting in {wait_time} seconds... (attempt {retry_count}/{max_retries})")
                await asyncio.sleep(wait_time)
                
                # Reset connection state for retry
                self._ws_connected = False
                
        self.logger.info("WebSocket stream task completed")
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
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
    
    async def stream_market_data_ws(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """
        Stream market data via direct WebSocket connection.
        
        Same signature as stream_market_data() for drop-in replacement.
        Falls back to polling on WebSocket failure.
        
        Args:
            symbols: List of stock symbols to stream
        """
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        # Check if WebSocket is enabled in configuration
        if not self._ws_config["enabled"]:
            self.logger.info("WebSocket disabled, falling back to polling method")
            async for data in self.stream_market_data(symbols):
                yield data
            return
        
        # Ensure provider is connected
        if not hasattr(self, 'stock_stream') or self.stock_stream is None:
            self.logger.info("Provider not connected, establishing connection for WebSocket")
            await self.connect()
        
        # Attempt WebSocket streaming with fallback to polling
        try:
            async for data in self._stream_via_websocket(symbols):
                yield data
        except Exception as e:
            self.logger.warning(f"WebSocket streaming failed: {e}, falling back to polling")
            async for data in self.stream_market_data(symbols):
                yield data
    
    async def _stream_via_sdk_websocket(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Stream data using Alpaca SDK WebSocket with proper pattern."""
        # Set up subscriptions
        self._ws_subscriptions = set(symbols)
        
        # Register handlers before starting stream
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
        
        self.logger.info(f"Starting SDK WebSocket stream for symbols: {symbols}")
        
        # Create background task for stream
        async def run_stream():
            try:
                await self.stock_stream.run()
            except Exception as e:
                self.logger.error(f"Stream error: {e}")
                raise
        
        stream_task = asyncio.create_task(run_stream())
        
        try:
            # Give the stream a moment to connect
            await asyncio.sleep(2)
            
            # Yield data from queue
            while True:
                try:
                    # Get data from queue with timeout
                    data = await asyncio.wait_for(self._ws_data_queue.get(), timeout=30.0)
                    yield data
                except asyncio.TimeoutError:
                    # Check if stream is still running
                    if stream_task.done():
                        exception = stream_task.exception()
                        if exception:
                            raise exception
                        else:
                            self.logger.info("WebSocket stream completed normally")
                            break
                    # Continue waiting if stream is still running
                    self.logger.debug("No data received in 30s, continuing to wait...")
                    continue
        finally:
            # Clean up
            self.logger.info("Cleaning up SDK WebSocket stream")
            self.stock_stream.stop()
            if not stream_task.done():
                stream_task.cancel()
                try:
                    await stream_task
                except asyncio.CancelledError:
                    pass
    
    async def _stream_via_websocket(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Stream data via direct WebSocket connection to Alpaca."""
        ws_url = self._ws_config["url"]
        reconnect_delay = self._ws_config["reconnect_delay"]
        max_attempts = self._ws_config["max_reconnect_attempts"]
        
        attempt = 0
        while attempt < max_attempts:
            try:
                self.logger.info(f"Connecting to Alpaca WebSocket: {ws_url}")
                async with websockets.connect(ws_url) as websocket:
                    # First, get the connection confirmation
                    connect_response = await websocket.recv()
                    self.logger.info(f"Connection response: {connect_response}")
                    
                    connect_data = json.loads(connect_response)
                    if not isinstance(connect_data, list):
                        connect_data = [connect_data]
                    
                    # Check for successful connection
                    connected = any(
                        msg.get("T") == "success" and "connected" in msg.get("msg", "")
                        for msg in connect_data
                    )
                    
                    if not connected:
                        self.logger.error(f"Connection failed. Response: {connect_data}")
                        raise ConnectionError("WebSocket connection failed")
                    
                    self.logger.info("WebSocket connected, sending authentication...")
                    
                    # Now authenticate
                    auth_message = {
                        "action": "auth",
                        "key": self.api_key,
                        "secret": self.api_secret
                    }
                    await websocket.send(json.dumps(auth_message))
                    
                    # Wait for authentication confirmation
                    auth_response = await websocket.recv()
                    self.logger.info(f"Auth response received: {auth_response}")
                    
                    auth_data = json.loads(auth_response)
                    if not isinstance(auth_data, list):
                        auth_data = [auth_data]
                    
                    # Check for successful authentication
                    auth_success = any(
                        msg.get("T") == "success" and "authenticated" in msg.get("msg", "")
                        for msg in auth_data
                    )
                    
                    if not auth_success:
                        self.logger.error(f"Authentication failed. Response: {auth_data}")
                        raise ConnectionError("WebSocket authentication failed")
                    
                    self.logger.info("WebSocket authenticated successfully")
                    
                    # Subscribe to bars for the symbols
                    subscribe_message = {
                        "action": "subscribe",
                        "bars": symbols
                    }
                    await websocket.send(json.dumps(subscribe_message))
                    
                    self.logger.info(f"Subscribed to bars for symbols: {symbols}")
                    
                    # Stream messages
                    async for message in websocket:
                        try:
                            data = json.loads(message)
                            if not isinstance(data, list):
                                data = [data]
                            
                            for msg in data:
                                # Process bar messages only
                                if msg.get("T") == "b":  # Bar message
                                    market_data = self._convert_ws_bar_to_market_data(msg)
                                    if market_data and market_data.symbol in symbols:
                                        yield market_data
                                        
                        except json.JSONDecodeError as e:
                            self.logger.warning(f"Invalid JSON message received: {e}")
                            continue
                        except Exception as e:
                            self.logger.error(f"Error processing WebSocket message: {e}")
                            continue
                            
            except (ConnectionError, websockets.exceptions.ConnectionClosed) as e:
                attempt += 1
                self.logger.warning(f"WebSocket connection failed (attempt {attempt}/{max_attempts}): {e}")
                if attempt < max_attempts:
                    await asyncio.sleep(reconnect_delay)
                else:
                    raise ConnectionError(f"WebSocket connection failed after {max_attempts} attempts")
    
    def _convert_ws_bar_to_market_data(self, bar_msg: dict) -> Optional[MarketData]:
        """Convert WebSocket bar message to MarketData object."""
        try:
            # Parse timestamp
            timestamp_str = bar_msg.get("t")
            if timestamp_str:
                # Handle both ISO format and epoch timestamp
                if isinstance(timestamp_str, str):
                    if timestamp_str.endswith('Z'):
                        timestamp = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
                    else:
                        timestamp = datetime.fromisoformat(timestamp_str)
                else:
                    timestamp = datetime.fromtimestamp(timestamp_str)
            else:
                timestamp = datetime.utcnow()
            
            # Create MarketData object
            market_data = MarketData(
                time=timestamp,
                symbol=bar_msg.get("S", ""),
                open=float(bar_msg.get("o", 0)),
                high=float(bar_msg.get("h", 0)),
                low=float(bar_msg.get("l", 0)),
                close=float(bar_msg.get("c", 0)),
                volume=int(bar_msg.get("v", 0)),
                provider="alpaca",
                metadata={
                    "trades": bar_msg.get("n"),  # Trade count
                    "vwap": bar_msg.get("vw"),   # Volume weighted average price
                    "source": "websocket"
                }
            )
            
            return market_data
            
        except (ValueError, KeyError, TypeError) as e:
            self.logger.error(f"Error converting WebSocket bar message to MarketData: {e}")
            return None
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch historical tick/trade data."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                # Fetch stock trades with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockTradesRequest(
                    symbol_or_symbols=symbol,
                    start=start_time,
                    end=end_time,
                    feed=feed_enum
                )
                trades = self.stock_client.get_stock_trades(request)
                
                if symbol in trades:
                    for trade in trades[symbol]:
                        yield self._parse_trade(trade, symbol)
                        
            except Exception as e:
                self.logger.error(f"Failed to fetch tick data for {symbol}", error=str(e))
                continue
    
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Stream real-time tick/trade data via WebSocket."""
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        # For now, we'll use polling with get_tick_data as a workaround
        self.logger.warning("Alpaca WebSocket tick streaming is currently disabled due to SDK limitations.")
        self.logger.info(f"Using polling mode for tick data: {symbols}")
        
        # Poll every 2 seconds for recent trades
        while True:
            try:
                end_time = datetime.now()
                start_time = end_time - timedelta(seconds=10)
                
                # Get recent trades
                async for tick in self.get_tick_data(symbols, start_time, end_time):
                    yield tick
                
                # Wait before next poll
                await asyncio.sleep(2)
                
            except Exception as e:
                self.logger.error(f"Tick polling error: {str(e)}")
                await asyncio.sleep(5)  # Wait longer on error
    
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Get order book snapshots (latest quotes)."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                # Get latest stock quote with feed specification
                feed_enum = self._subscription_limits["feed"]
                request = StockLatestQuoteRequest(
                    symbol_or_symbols=symbol,
                    feed=feed_enum
                )
                quotes = self.stock_client.get_stock_latest_quote(request)
                
                if symbol in quotes:
                    yield self._parse_quote(quotes[symbol], symbol)
                        
            except Exception as e:
                self.logger.error(f"Failed to fetch order book for {symbol}", error=str(e))
                continue
    
    def _parse_bar(self, bar: Bar, symbol: str) -> MarketData:
        """Parse Alpaca SDK Bar object to MarketData."""
        return MarketData(
            time=bar.timestamp,
            symbol=symbol,
            open=float(bar.open),
            high=float(bar.high),
            low=float(bar.low),
            close=float(bar.close),
            volume=int(bar.volume),
            provider=self.name,
            metadata={
                "trade_count": bar.trade_count if hasattr(bar, 'trade_count') else None,
                "vwap": float(bar.vwap) if hasattr(bar, 'vwap') and bar.vwap else None
            }
        )
    
    def _parse_trade(self, trade: Trade, symbol: str) -> TickData:
        """Parse Alpaca SDK Trade object to TickData."""
        return TickData(
            time=trade.timestamp,
            symbol=symbol,
            price=float(trade.price),
            size=int(trade.size),
            exchange=trade.exchange if hasattr(trade, 'exchange') else None,
            conditions=",".join(trade.conditions) if hasattr(trade, 'conditions') and trade.conditions else None,
            provider=self.name
        )
    
    def _parse_quote(self, quote: Quote, symbol: str) -> OrderBookData:
        """Parse Alpaca SDK Quote object to OrderBookData."""
        bid_price = float(quote.bid_price)
        ask_price = float(quote.ask_price)
        
        return OrderBookData(
            time=quote.timestamp,
            symbol=symbol,
            bid_price=bid_price,
            bid_size=int(quote.bid_size),
            ask_price=ask_price,
            ask_size=int(quote.ask_size),
            mid_price=(bid_price + ask_price) / 2 if bid_price and ask_price else 0,
            spread=ask_price - bid_price if bid_price and ask_price else 0,
            provider=self.name
        )