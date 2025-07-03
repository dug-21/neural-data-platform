"""Polygon.io data provider implementation."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
import pandas as pd
from decimal import Decimal

from .base import BaseProvider, MarketData, TickData, OrderBookData, DataType
from ..utils.retry import with_retry


class PolygonProvider(BaseProvider):
    """Polygon.io data provider for comprehensive market data."""
    
    BASE_URL = "https://api.polygon.io"
    WS_URL = "wss://socket.polygon.io"
    
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
    
    def __init__(self):
        super().__init__("polygon")
        self.api_key = self.settings.polygon_api_key
        self.session: Optional[aiohttp.ClientSession] = None
        self._ws_connection: Optional[aiohttp.ClientWebSocketResponse] = None
        self._ws_session: Optional[aiohttp.ClientSession] = None
        self._subscribed_symbols: set = set()
    
    async def connect(self):
        """Initialize HTTP and WebSocket sessions."""
        if not self.api_key:
            raise ValueError("Polygon API key not configured")
        
        self.session = aiohttp.ClientSession(
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Accept": "application/json"
            },
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._ws_session = aiohttp.ClientSession()
        self._connected = True
        self.logger.info("Connected to Polygon.io")
    
    async def disconnect(self):
        """Close all connections."""
        if self._ws_connection:
            await self._ws_connection.close()
        if self._ws_session:
            await self._ws_session.close()
        if self.session:
            await self.session.close()
        
        self._connected = False
        self.logger.info("Disconnected from Polygon.io")
    
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
        """Fetch historical market data from Polygon."""
        symbols = self._validate_symbols(symbols)
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
                    "limit": 50000  # Max limit
                }
                
                data = await self._request(endpoint, params)
                
                if data.get("status") == "OK" and "results" in data:
                    for bar in data["results"]:
                        yield self._parse_market_data(bar, symbol)
                
                # Handle pagination if needed
                while data.get("next_url"):
                    next_url = data["next_url"].replace(self.BASE_URL, "")
                    data = await self._request(next_url)
                    
                    if "results" in data:
                        for bar in data["results"]:
                            yield self._parse_market_data(bar, symbol)
                            
            except Exception as e:
                self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data via WebSocket."""
        symbols = self._validate_symbols(symbols)
        
        # Connect to WebSocket
        await self._connect_websocket()
        
        # Subscribe to symbols
        await self._subscribe_symbols(symbols, ["AM"])  # Aggregate bars
        
        # Stream data
        async for message in self._stream_messages():
            if message.get("ev") == "AM":  # Aggregate minute bar
                yield self._parse_streaming_bar(message)
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch historical tick data."""
        symbols = self._validate_symbols(symbols)
        
        # Convert to date strings
        start_date = start_time.strftime("%Y-%m-%d")
        
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
    
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Stream real-time tick data via WebSocket."""
        symbols = self._validate_symbols(symbols)
        
        # Connect to WebSocket
        await self._connect_websocket()
        
        # Subscribe to symbols
        await self._subscribe_symbols(symbols, ["T"])  # Trades
        
        # Stream data
        async for message in self._stream_messages():
            if message.get("ev") == "T":  # Trade
                yield self._parse_streaming_trade(message)
    
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Get order book snapshots."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                endpoint = f"/v3/quotes/{symbol}"
                params = {"limit": 1}  # Get latest quote
                
                data = await self._request(endpoint, params)
                
                if data.get("status") == "OK" and "results" in data and data["results"]:
                    quote = data["results"][0]
                    yield self._parse_order_book(quote, symbol)
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch order book for {symbol}", error=str(e))
                continue
    
    async def _connect_websocket(self):
        """Connect to Polygon WebSocket."""
        if self._ws_connection and not self._ws_connection.closed:
            return
        
        ws_url = f"{self.WS_URL}/stocks"
        
        self._ws_connection = await self._ws_session.ws_connect(ws_url)
        
        # Authenticate
        auth_message = {
            "action": "auth",
            "params": self.api_key
        }
        await self._ws_connection.send_json(auth_message)
        
        # Wait for auth response
        auth_response = await self._ws_connection.receive_json()
        if auth_response[0].get("status") != "auth_success":
            raise ConnectionError("WebSocket authentication failed")
        
        self.logger.info("Connected to Polygon WebSocket")
    
    async def _subscribe_symbols(self, symbols: List[str], channels: List[str]):
        """Subscribe to symbols on WebSocket."""
        subscriptions = []
        
        for symbol in symbols:
            for channel in channels:
                subscriptions.append(f"{channel}.{symbol}")
        
        subscribe_message = {
            "action": "subscribe",
            "params": ",".join(subscriptions)
        }
        
        await self._ws_connection.send_json(subscribe_message)
        self._subscribed_symbols.update(symbols)
        
        self.logger.info(f"Subscribed to {len(subscriptions)} channels")
    
    async def _stream_messages(self) -> AsyncIterator[Dict[str, Any]]:
        """Stream messages from WebSocket."""
        try:
            async for msg in self._ws_connection:
                if msg.type == aiohttp.WSMsgType.TEXT:
                    data = msg.json()
                    if isinstance(data, list):
                        for item in data:
                            yield item
                    else:
                        yield data
                elif msg.type == aiohttp.WSMsgType.ERROR:
                    self.logger.error(f"WebSocket error: {msg.data}")
                elif msg.type == aiohttp.WSMsgType.CLOSED:
                    self.logger.warning("WebSocket connection closed")
                    break
        except Exception as e:
            self.logger.error("WebSocket streaming error", error=str(e))
            # Attempt to reconnect
            await self._connect_websocket()
    
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
                "vwap": bar.get("vw"),  # Volume weighted average price
                "transactions": bar.get("n")  # Number of transactions
            }
        )
    
    def _parse_streaming_bar(self, message: Dict[str, Any]) -> MarketData:
        """Parse streaming bar data."""
        return MarketData(
            time=pd.to_datetime(message.get("s", 0), unit="ms"),
            symbol=message.get("sym", ""),
            open=float(message.get("o", 0)),
            high=float(message.get("h", 0)),
            low=float(message.get("l", 0)),
            close=float(message.get("c", 0)),
            volume=int(message.get("v", 0)),
            provider=self.name,
            metadata={
                "vwap": message.get("vw"),
                "average_size": message.get("av")
            }
        )
    
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
    
    def _parse_streaming_trade(self, message: Dict[str, Any]) -> TickData:
        """Parse streaming trade data."""
        return TickData(
            time=pd.to_datetime(message.get("t", 0), unit="ns"),
            symbol=message.get("sym", ""),
            price=float(message.get("p", 0)),
            size=int(message.get("s", 0)),
            exchange=str(message.get("x", "")),
            conditions=",".join(message.get("c", [])),
            provider=self.name
        )
    
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
    
    async def get_snapshot(self, symbols: List[str]) -> Dict[str, Any]:
        """Get snapshot of current prices."""
        symbols = self._validate_symbols(symbols)
        
        endpoint = "/v2/snapshot/locale/us/markets/stocks/tickers"
        params = {"tickers": ",".join(symbols)}
        
        return await self._request(endpoint, params)
    
    async def get_exchanges(self) -> List[Dict[str, Any]]:
        """Get list of exchanges."""
        endpoint = "/v3/reference/exchanges"
        data = await self._request(endpoint)
        return data.get("results", [])