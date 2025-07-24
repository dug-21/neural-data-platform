"""Enhanced Polygon.io data provider with robust WebSocket support."""
import asyncio
import aiohttp
import json
from typing import List, AsyncIterator, Optional, Dict, Any, Set
from datetime import datetime, timedelta
import pandas as pd
from decimal import Decimal
from enum import Enum
import time

from .base import BaseProvider, MarketData, TickData, OrderBookData, DataType
from utils.retry import with_retry


class ConnectionState(Enum):
    """WebSocket connection states."""
    DISCONNECTED = "disconnected"
    CONNECTING = "connecting"
    CONNECTED = "connected"
    AUTHENTICATED = "authenticated"
    SUBSCRIBED = "subscribed"
    RECONNECTING = "reconnecting"
    FAILED = "failed"


class PolygonProvider(BaseProvider):
    """Enhanced Polygon.io data provider with robust WebSocket support."""
    
    BASE_URL = "https://api.polygon.io"
    WS_URL_REALTIME = "wss://socket.polygon.io/stocks"
    WS_URL_DELAYED = "wss://delayed.polygon.io/stocks"
    
    # Map intervals to Polygon multiplier and timespan
    INTERVAL_MAP = {
        "1min": (1, "minute"),
        "5min": (5, "minute"),
        "15min": (15, "minute"),
        "30min": (30, "minute"),
        "1hour": (1, "hour"),
        "4hour": (4, "hour"),
        "1day": (1, "day"),
        "1week": (1, "week"),
        "1month": (1, "month")
    }
    
    # WebSocket configuration
    WS_CONFIG = {
        "reconnect_delay": 5,
        "max_reconnect_delay": 300,
        "reconnect_decay": 1.5,
        "max_reconnect_attempts": 10,
        "heartbeat_interval": 30,
        "subscription_batch_size": 100,
        "message_buffer_size": 10000,
        "fallback_polling_interval": 60  # 1 minute polling fallback
    }
    
    def __init__(self):
        super().__init__("polygon")
        self.api_key = self.settings.polygon_api_key
        self.use_delayed_feed = getattr(self.settings, 'polygon_use_delayed', False)
        
        # HTTP session
        self.session: Optional[aiohttp.ClientSession] = None
        
        # WebSocket attributes
        self._ws_connection: Optional[aiohttp.ClientWebSocketResponse] = None
        self._ws_session: Optional[aiohttp.ClientSession] = None
        self._ws_state = ConnectionState.DISCONNECTED
        self._subscribed_symbols: Set[str] = set()
        self._pending_subscriptions: Set[str] = set()
        self._reconnect_count = 0
        self._last_heartbeat = time.time()
        
        # Message handling
        self._message_buffer = asyncio.Queue(maxsize=self.WS_CONFIG["message_buffer_size"])
        self._ws_task: Optional[asyncio.Task] = None
        self._heartbeat_task: Optional[asyncio.Task] = None
        self._fallback_polling_task: Optional[asyncio.Task] = None
        
        # Fallback state
        self._use_fallback = False
        self._fallback_active = False
        
        # Statistics
        self._stats = {
            "messages_received": 0,
            "messages_processed": 0,
            "reconnections": 0,
            "errors": 0,
            "last_message_time": None
        }
    
    async def connect(self):
        """Initialize HTTP and WebSocket sessions."""
        if not self.api_key:
            raise ValueError("Polygon API key not configured")
        
        # Initialize HTTP session
        self.session = aiohttp.ClientSession(
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Accept": "application/json"
            },
            timeout=aiohttp.ClientTimeout(total=30)
        )
        
        # Initialize WebSocket session
        self._ws_session = aiohttp.ClientSession()
        
        self._connected = True
        self.logger.info("Connected to Polygon.io")
    
    async def disconnect(self):
        """Close all connections gracefully."""
        self._connected = False
        
        # Cancel tasks
        for task in [self._ws_task, self._heartbeat_task, self._fallback_polling_task]:
            if task and not task.done():
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
        
        # Close WebSocket
        if self._ws_connection and not self._ws_connection.closed:
            await self._ws_connection.close()
        
        # Close sessions
        if self._ws_session:
            await self._ws_session.close()
        if self.session:
            await self.session.close()
        
        self._ws_state = ConnectionState.DISCONNECTED
        self.logger.info("Disconnected from Polygon.io")
    
    async def stream_market_data_ws(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data via WebSocket with fallback to polling.
        
        This method is an alias for stream_market_data to maintain compatibility
        with the RealtimeCoordinator interface.
        """
        async for data in self.stream_market_data(symbols):
            yield data
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data with WebSocket fallback to polling."""
        symbols = self._validate_symbols(symbols)
        
        if not symbols:
            self.logger.warning("No valid symbols to stream")
            return
        
        # Update subscription list
        self._pending_subscriptions.update(symbols)
        
        # Start WebSocket connection if not running
        if not self._ws_task or self._ws_task.done():
            self._ws_task = asyncio.create_task(self._run_websocket())
        
        # Start heartbeat monitor
        if not self._heartbeat_task or self._heartbeat_task.done():
            self._heartbeat_task = asyncio.create_task(self._monitor_heartbeat())
        
        # Yield data from buffer
        while self._connected:
            try:
                # Check if we should use fallback
                if self._should_use_fallback():
                    if not self._fallback_active:
                        await self._start_fallback_polling(symbols)
                    
                    # Yield from fallback
                    async for data in self._poll_market_data(symbols):
                        yield data
                        # Check if WebSocket recovered
                        if self._ws_state == ConnectionState.SUBSCRIBED:
                            self._fallback_active = False
                            break
                else:
                    # Get data from WebSocket buffer
                    try:
                        data = await asyncio.wait_for(
                            self._message_buffer.get(),
                            timeout=30.0
                        )
                        
                        if data.symbol in symbols:
                            yield data
                            self._stats["messages_processed"] += 1
                            
                    except asyncio.TimeoutError:
                        self.logger.warning("No WebSocket data received for 30 seconds")
                        continue
                        
            except Exception as e:
                self.logger.error(f"Error in stream_market_data: {e}")
                await asyncio.sleep(1)
    
    async def _run_websocket(self):
        """Run WebSocket connection with automatic reconnection."""
        while self._connected:
            try:
                await self._connect_websocket()
                await self._authenticate_websocket()
                await self._subscribe_websocket()
                await self._process_websocket_messages()
                
            except Exception as e:
                self.logger.error(f"WebSocket error: {e}")
                self._ws_state = ConnectionState.RECONNECTING
                self._stats["errors"] += 1
                
                # Calculate backoff delay
                delay = min(
                    self.WS_CONFIG["reconnect_delay"] * (self.WS_CONFIG["reconnect_decay"] ** self._reconnect_count),
                    self.WS_CONFIG["max_reconnect_delay"]
                )
                
                self._reconnect_count += 1
                
                if self._reconnect_count > self.WS_CONFIG["max_reconnect_attempts"]:
                    self.logger.error("Max reconnection attempts reached, switching to fallback")
                    self._ws_state = ConnectionState.FAILED
                    self._use_fallback = True
                    break
                
                self.logger.info(f"Reconnecting in {delay:.1f} seconds (attempt {self._reconnect_count})")
                await asyncio.sleep(delay)
    
    async def _connect_websocket(self):
        """Establish WebSocket connection."""
        if self._ws_connection and not self._ws_connection.closed:
            return
        
        self._ws_state = ConnectionState.CONNECTING
        
        ws_url = self.WS_URL_DELAYED if self.use_delayed_feed else self.WS_URL_REALTIME
        
        self.logger.info(f"Connecting to WebSocket: {ws_url}")
        
        self._ws_connection = await self._ws_session.ws_connect(
            ws_url,
            heartbeat=self.WS_CONFIG["heartbeat_interval"]
        )
        
        self._ws_state = ConnectionState.CONNECTED
        self.logger.info("WebSocket connected")
    
    async def _authenticate_websocket(self):
        """Authenticate WebSocket connection."""
        auth_message = {
            "action": "auth",
            "params": self.api_key
        }
        
        await self._ws_connection.send_json(auth_message)
        
        # Wait for auth response
        auth_response = await self._ws_connection.receive_json()
        
        if isinstance(auth_response, list):
            auth_response = auth_response[0]
        
        if auth_response.get("status") == "auth_success":
            self._ws_state = ConnectionState.AUTHENTICATED
            self.logger.info("WebSocket authenticated")
            self._reconnect_count = 0  # Reset on successful auth
        else:
            raise ConnectionError(f"WebSocket authentication failed: {auth_response}")
    
    async def _subscribe_websocket(self):
        """Subscribe to symbols on WebSocket."""
        # Combine pending and existing subscriptions
        all_symbols = self._subscribed_symbols | self._pending_subscriptions
        
        if not all_symbols:
            return
        
        # Subscribe in batches
        symbol_list = list(all_symbols)
        batch_size = self.WS_CONFIG["subscription_batch_size"]
        
        for i in range(0, len(symbol_list), batch_size):
            batch = symbol_list[i:i + batch_size]
            subscriptions = []
            
            # Subscribe to minute aggregates (AM) and second aggregates (AS)
            for symbol in batch:
                subscriptions.extend([f"AM.{symbol}", f"AS.{symbol}"])
            
            subscribe_message = {
                "action": "subscribe",
                "params": ",".join(subscriptions)
            }
            
            await self._ws_connection.send_json(subscribe_message)
            self.logger.info(f"Subscribed to {len(batch)} symbols")
        
        # Update subscription state
        self._subscribed_symbols = all_symbols
        self._pending_subscriptions.clear()
        self._ws_state = ConnectionState.SUBSCRIBED
    
    async def _process_websocket_messages(self):
        """Process incoming WebSocket messages."""
        async for msg in self._ws_connection:
            try:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = json.loads(msg.data)
                    
                    if isinstance(data, list):
                        for item in data:
                            await self._handle_websocket_message(item)
                    else:
                        await self._handle_websocket_message(data)
                        
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    self.logger.error(f"WebSocket error: {msg.data}")
                    
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    self.logger.warning("WebSocket connection closed")
                    break
                    
            except Exception as e:
                self.logger.error(f"Error processing WebSocket message: {e}")
                continue
    
    async def _handle_websocket_message(self, message: Dict[str, Any]):
        """Handle individual WebSocket message."""
        msg_type = message.get("ev")
        
        # Update heartbeat
        self._last_heartbeat = time.time()
        self._stats["messages_received"] += 1
        self._stats["last_message_time"] = datetime.now()
        
        # Process aggregate messages
        if msg_type in ["AM", "AS"]:  # Minute or second aggregates
            try:
                market_data = self._parse_streaming_aggregate(message)
                
                # Add to buffer if not full
                if not self._message_buffer.full():
                    await self._message_buffer.put(market_data)
                else:
                    self.logger.warning("Message buffer full, dropping message")
                    
            except Exception as e:
                self.logger.error(f"Error parsing aggregate message: {e}")
    
    def _parse_streaming_aggregate(self, message: Dict[str, Any]) -> MarketData:
        """Parse streaming aggregate data."""
        # Handle both millisecond (s) and nanosecond timestamps
        timestamp = message.get("s", message.get("t", 0))
        if timestamp > 1e12:  # Nanoseconds
            timestamp = timestamp / 1e9
        else:  # Milliseconds
            timestamp = timestamp / 1e3
            
        return MarketData(
            time=pd.to_datetime(timestamp, unit="s"),
            symbol=message.get("sym", ""),
            open=float(message.get("o", 0)),
            high=float(message.get("h", 0)),
            low=float(message.get("l", 0)),
            close=float(message.get("c", 0)),
            volume=int(message.get("v", 0)),
            provider=self.name,
            metadata={
                "type": "aggregate",
                "interval": "1s" if message.get("ev") == "AS" else "1m",
                "vwap": message.get("vw"),
                "average_size": message.get("z"),
                "accumulated_volume": message.get("av"),
                "otc": message.get("otc", False)
            }
        )
    
    async def _monitor_heartbeat(self):
        """Monitor WebSocket health and trigger reconnection if needed."""
        while self._connected:
            await asyncio.sleep(self.WS_CONFIG["heartbeat_interval"])
            
            # Check last heartbeat
            time_since_heartbeat = time.time() - self._last_heartbeat
            
            if time_since_heartbeat > self.WS_CONFIG["heartbeat_interval"] * 2:
                self.logger.warning(f"No heartbeat for {time_since_heartbeat:.1f} seconds")
                
                # Force reconnection
                if self._ws_connection and not self._ws_connection.closed:
                    await self._ws_connection.close()
                    
                self._ws_state = ConnectionState.RECONNECTING
    
    def _should_use_fallback(self) -> bool:
        """Determine if fallback polling should be used."""
        return (
            self._use_fallback or
            self._ws_state in [ConnectionState.FAILED, ConnectionState.DISCONNECTED] or
            (self._ws_state == ConnectionState.RECONNECTING and self._reconnect_count > 3)
        )
    
    async def _start_fallback_polling(self, symbols: List[str]):
        """Start fallback polling task."""
        if self._fallback_polling_task and not self._fallback_polling_task.done():
            return
            
        self._fallback_active = True
        self.logger.info("Starting fallback polling mode")
        
        self._fallback_polling_task = asyncio.create_task(
            self._fallback_polling_loop(symbols)
        )
    
    async def _fallback_polling_loop(self, symbols: List[str]):
        """Fallback polling loop."""
        while self._fallback_active and self._connected:
            try:
                # Poll for latest data
                async for data in self._poll_market_data(symbols):
                    await self._message_buffer.put(data)
                
                # Wait before next poll
                await asyncio.sleep(self.WS_CONFIG["fallback_polling_interval"])
                
            except Exception as e:
                self.logger.error(f"Fallback polling error: {e}")
                await asyncio.sleep(5)
    
    async def _poll_market_data(self, symbols: List[str]) -> AsyncIterator[MarketData]:
        """Poll for latest market data (1-minute bars)."""
        end_time = datetime.now()
        start_time = end_time - timedelta(minutes=2)  # Get last 2 minutes
        
        async for data in self.get_market_data(symbols, start_time, end_time, "1min"):
            yield data
    
    @with_retry(max_attempts=3, exceptions=(aiohttp.ClientError,))
    async def _request(self, endpoint: str, params: Optional[Dict[str, Any]] = None) -> Any:
        """Make authenticated request to Polygon API."""
        await self._rate_limit()
        
        url = f"{self.BASE_URL}{endpoint}"
        params = params or {}
        
        try:
            async with self.session.get(url, params=params) as response:
                response.raise_for_status()
                data = await response.json()
                
                if data.get("status") == "ERROR":
                    raise ValueError(f"API Error: {data.get('error', 'Unknown error')}")
                
                return data
        except aiohttp.ClientError as e:
            self.logger.error(f"API request failed: {endpoint}", error=str(e))
            raise
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data with 1-minute aggregate support."""
        symbols = self._validate_symbols(symbols)
        
        # For Basic plan, only daily data might be available
        if interval == "1min" and getattr(self.settings, 'polygon_basic_plan', True):
            self.logger.warning("Polygon Basic plan may not support minute aggregates, trying daily instead")
            interval = "1day"
            
        multiplier, timespan = self.INTERVAL_MAP.get(interval, (1, "minute"))
        
        # Convert times to milliseconds
        start_ms = int(start_time.timestamp() * 1000)
        end_ms = int(end_time.timestamp() * 1000)
        
        for symbol in symbols:
            try:
                endpoint = f"/v2/aggs/ticker/{symbol}/range/{multiplier}/{timespan}/{start_ms}/{end_ms}"
                params = {
                    "adjusted": "true",
                    "sort": "asc",
                    "limit": 50000  # Max limit for better backfill
                }
                
                data = await self._request(endpoint, params)
                
                if data.get("status") == "OK" and "results" in data:
                    for bar in data["results"]:
                        yield self._parse_market_data(bar, symbol)
                
                # Handle pagination for complete data retrieval
                while data.get("next_url"):
                    next_url = data["next_url"].replace(self.BASE_URL, "")
                    data = await self._request(next_url)
                    
                    if "results" in data:
                        for bar in data["results"]:
                            yield self._parse_market_data(bar, symbol)
                            
            except Exception as e:
                self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
                continue
    
    def _parse_market_data(self, bar: Dict[str, Any], symbol: str) -> MarketData:
        """Parse Polygon bar data."""
        return MarketData(
            time=pd.to_datetime(bar.get("t", 0), unit="ms"),
            symbol=symbol,
            open=float(bar.get("o", 0)),
            high=float(bar.get("h", 0)),
            low=float(bar.get("l", 0)),
            close=float(bar.get("c", 0)),
            volume=int(bar.get("v", 0)),
            provider=self.name,
            metadata={
                "vwap": bar.get("vw"),
                "transactions": bar.get("n"),
                "otc": bar.get("otc", False)
            }
        )
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch historical tick data."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                endpoint = f"/v3/trades/{symbol}"
                params = {
                    "timestamp.gte": start_time.isoformat(),
                    "timestamp.lte": end_time.isoformat(),
                    "limit": 50000
                }
                
                data = await self._request(endpoint, params)
                
                if data.get("status") == "OK" and "results" in data:
                    for trade in data["results"]:
                        yield self._parse_tick_data(trade, symbol)
                
                # Handle pagination
                while data.get("next_url"):
                    next_url = data["next_url"].replace(self.BASE_URL, "")
                    data = await self._request(next_url)
                    
                    if "results" in data:
                        for trade in data["results"]:
                            yield self._parse_tick_data(trade, symbol)
                            
            except Exception as e:
                self.logger.error(f"Failed to fetch tick data for {symbol}", error=str(e))
                continue
    
    def _parse_tick_data(self, trade: Dict[str, Any], symbol: str) -> TickData:
        """Parse trade data."""
        return TickData(
            time=pd.to_datetime(trade.get("participant_timestamp", trade.get("sip_timestamp", 0)), unit="ns"),
            symbol=symbol,
            price=float(trade.get("price", 0)),
            size=int(trade.get("size", 0)),
            exchange=trade.get("exchange"),
            conditions=",".join(trade.get("conditions", [])),
            provider=self.name
        )
    
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Stream real-time tick data via WebSocket."""
        # For now, use the same stream as market data
        # In production, would subscribe to trade channel
        self.logger.info("Tick streaming uses aggregate data stream")
        async for data in self.stream_market_data(symbols):
            # Convert aggregate to tick format
            yield TickData(
                time=data.time,
                symbol=data.symbol,
                price=data.close,
                size=data.volume,
                provider=self.name
            )
    
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Get order book snapshots."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                endpoint = f"/v3/quotes/{symbol}"
                params = {"limit": 1}
                
                data = await self._request(endpoint, params)
                
                if data.get("status") == "OK" and "results" in data and data["results"]:
                    quote = data["results"][0]
                    yield self._parse_order_book(quote, symbol)
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch order book for {symbol}", error=str(e))
                continue
    
    def _parse_order_book(self, quote: Dict[str, Any], symbol: str) -> OrderBookData:
        """Parse quote data to order book."""
        bid_price = float(quote.get("bid_price", 0))
        ask_price = float(quote.get("ask_price", 0))
        
        return OrderBookData(
            time=pd.to_datetime(quote.get("participant_timestamp", 0), unit="ns"),
            symbol=symbol,
            bid_price=bid_price,
            bid_size=int(quote.get("bid_size", 0)),
            ask_price=ask_price,
            ask_size=int(quote.get("ask_size", 0)),
            mid_price=(bid_price + ask_price) / 2 if bid_price and ask_price else 0,
            spread=ask_price - bid_price if bid_price and ask_price else 0,
            provider=self.name
        )
    
    def get_stats(self) -> Dict[str, Any]:
        """Get WebSocket statistics."""
        return {
            **self._stats,
            "state": self._ws_state.value,
            "subscribed_symbols": len(self._subscribed_symbols),
            "buffer_size": self._message_buffer.qsize(),
            "fallback_active": self._fallback_active,
            "reconnect_count": self._reconnect_count
        }