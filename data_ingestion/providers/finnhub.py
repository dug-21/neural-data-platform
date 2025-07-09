"""Finnhub data provider implementation (free tier)."""
import asyncio
import aiohttp
import websockets
import json
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
import pandas as pd

from .base import BaseProvider, MarketData, TickData, DataType
from utils.retry import with_retry


class FinnhubProvider(BaseProvider):
    """Finnhub data provider for free real-time and historical data."""
    
    BASE_URL = "https://finnhub.io/api/v1"
    WS_URL = "wss://ws.finnhub.io"
    
    # Resolution mapping
    RESOLUTION_MAP = {
        "1min": "1",
        "5min": "5",
        "15min": "15",
        "30min": "30",
        "1hour": "60",
        "1day": "D",
        "1week": "W",
        "1month": "M"
    }
    
    def __init__(self):
        super().__init__("finnhub")
        self.api_key = self.settings.finnhub_api_key
        self.session: Optional[aiohttp.ClientSession] = None
        self._ws_connection = None
        self._ws_subscriptions = set()
    
    async def connect(self):
        """Initialize HTTP session."""
        if not self.api_key:
            self.logger.warning("Finnhub API key not configured - some features may be limited")
            self.api_key = "free"  # Limited free tier
        
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._connected = True
        self.logger.info("Connected to Finnhub")
    
    async def disconnect(self):
        """Close all connections."""
        if self._ws_connection:
            await self._ws_connection.close()
        if self.session:
            await self.session.close()
        
        self._connected = False
        self.logger.info("Disconnected from Finnhub")
    
    @with_retry(max_attempts=3, exceptions=(aiohttp.ClientError,))
    async def _request(self, endpoint: str, params: Optional[Dict[str, Any]] = None) -> Any:
        """Make request to Finnhub API."""
        await self._rate_limit()
        
        url = f"{self.BASE_URL}{endpoint}"
        params = params or {}
        params["token"] = self.api_key
        
        try:
            async with self.session.get(url, params=params) as response:
                response.raise_for_status()
                data = await response.json()
                
                # Check for errors
                if isinstance(data, dict) and data.get("error"):
                    raise ValueError(f"API Error: {data['error']}")
                
                return data
        except aiohttp.ClientError as e:
            self.logger.error(f"API request failed: {endpoint}", error=str(e))
            raise
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data from Finnhub."""
        symbols = self._validate_symbols(symbols)
        resolution = self.RESOLUTION_MAP.get(interval, "D")
        
        # Convert to timestamps
        from_ts = int(start_time.timestamp())
        to_ts = int(end_time.timestamp())
        
        for symbol in symbols:
            try:
                endpoint = "/stock/candle"
                params = {
                    "symbol": symbol,
                    "resolution": resolution,
                    "from": from_ts,
                    "to": to_ts
                }
                
                data = await self._request(endpoint, params)
                
                if data.get("s") == "ok" and "t" in data:
                    # Parse candle data
                    timestamps = data["t"]
                    opens = data["o"]
                    highs = data["h"]
                    lows = data["l"]
                    closes = data["c"]
                    volumes = data["v"]
                    
                    for i in range(len(timestamps)):
                        yield MarketData(
                            time=pd.to_datetime(timestamps[i], unit="s"),
                            symbol=symbol,
                            open=float(opens[i]),
                            high=float(highs[i]),
                            low=float(lows[i]),
                            close=float(closes[i]),
                            volume=int(volumes[i]),
                            provider=self.name
                        )
                elif data.get("s") == "no_data":
                    self.logger.warning(f"No data available for {symbol}")
                else:
                    self.logger.error(f"Unexpected response for {symbol}: {data}")
                    
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
        for symbol in symbols:
            await self._subscribe_symbol(symbol)
        
        # Stream data
        async for message in self._stream_messages():
            if message.get("type") == "trade":
                yield self._parse_trade_message(message)
    
    async def _connect_websocket(self):
        """Connect to Finnhub WebSocket."""
        if self._ws_connection:
            return
        
        ws_url = f"{self.WS_URL}?token={self.api_key}"
        self._ws_connection = await websockets.connect(ws_url)
        self.logger.info("Connected to Finnhub WebSocket")
    
    async def _subscribe_symbol(self, symbol: str):
        """Subscribe to a symbol on WebSocket."""
        if symbol in self._ws_subscriptions:
            return
        
        subscribe_msg = {
            "type": "subscribe",
            "symbol": symbol
        }
        
        await self._ws_connection.send(json.dumps(subscribe_msg))
        self._ws_subscriptions.add(symbol)
        self.logger.info(f"Subscribed to {symbol}")
    
    async def _stream_messages(self) -> AsyncIterator[Dict[str, Any]]:
        """Stream messages from WebSocket."""
        try:
            while True:
                message = await self._ws_connection.recv()
                data = json.loads(message)
                
                if data.get("type") == "trade" and "data" in data:
                    for trade in data["data"]:
                        yield trade
                elif data.get("type") == "ping":
                    # Respond to ping
                    pong_msg = {"type": "pong"}
                    await self._ws_connection.send(json.dumps(pong_msg))
                    
        except websockets.exceptions.ConnectionClosed:
            self.logger.warning("WebSocket connection closed")
            self._ws_connection = None
            # Attempt to reconnect
            await self._connect_websocket()
        except Exception as e:
            self.logger.error("WebSocket error", error=str(e))
    
    def _parse_trade_message(self, trade: Dict[str, Any]) -> MarketData:
        """Parse WebSocket trade message."""
        return MarketData(
            time=pd.to_datetime(trade.get("t", datetime.now()), unit="ms"),
            symbol=trade.get("s", ""),
            open=float(trade.get("p", 0)),  # Use price as all OHLC for tick
            high=float(trade.get("p", 0)),
            low=float(trade.get("p", 0)),
            close=float(trade.get("p", 0)),
            volume=int(trade.get("v", 0)),
            provider=self.name,
            metadata={
                "conditions": trade.get("c", [])
            }
        )
    
    async def get_quote(self, symbol: str) -> Dict[str, Any]:
        """Get current quote for a symbol."""
        endpoint = "/quote"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)
    
    async def get_company_profile(self, symbol: str) -> Dict[str, Any]:
        """Get company profile information."""
        endpoint = "/stock/profile2"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)
    
    async def get_peers(self, symbol: str) -> List[str]:
        """Get company peers."""
        endpoint = "/stock/peers"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)
    
    async def get_recommendation_trends(self, symbol: str) -> List[Dict[str, Any]]:
        """Get analyst recommendation trends."""
        endpoint = "/stock/recommendation"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)
    
    async def get_price_target(self, symbol: str) -> Dict[str, Any]:
        """Get analyst price targets."""
        endpoint = "/stock/price-target"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)
    
    async def get_earnings_calendar(
        self,
        from_date: Optional[datetime] = None,
        to_date: Optional[datetime] = None,
        symbol: Optional[str] = None
    ) -> Dict[str, Any]:
        """Get earnings calendar."""
        endpoint = "/calendar/earnings"
        params = {}
        
        if from_date:
            params["from"] = from_date.strftime("%Y-%m-%d")
        if to_date:
            params["to"] = to_date.strftime("%Y-%m-%d")
        if symbol:
            params["symbol"] = symbol
        
        return await self._request(endpoint, params)
    
    async def get_ipo_calendar(
        self,
        from_date: Optional[datetime] = None,
        to_date: Optional[datetime] = None
    ) -> Dict[str, Any]:
        """Get IPO calendar."""
        endpoint = "/calendar/ipo"
        params = {}
        
        if from_date:
            params["from"] = from_date.strftime("%Y-%m-%d")
        if to_date:
            params["to"] = to_date.strftime("%Y-%m-%d")
        
        return await self._request(endpoint, params)
    
    async def get_economic_calendar(self) -> List[Dict[str, Any]]:
        """Get economic calendar events."""
        endpoint = "/calendar/economic"
        return await self._request(endpoint)
    
    async def get_market_news(self, category: str = "general") -> List[Dict[str, Any]]:
        """Get market news."""
        endpoint = "/news"
        params = {"category": category}
        return await self._request(endpoint, params)
    
    async def get_company_news(
        self,
        symbol: str,
        from_date: datetime,
        to_date: datetime
    ) -> List[Dict[str, Any]]:
        """Get company-specific news."""
        endpoint = "/company-news"
        params = {
            "symbol": symbol,
            "from": from_date.strftime("%Y-%m-%d"),
            "to": to_date.strftime("%Y-%m-%d")
        }
        return await self._request(endpoint, params)
    
    async def get_sentiment(self, symbol: str) -> Dict[str, Any]:
        """Get social sentiment for a symbol."""
        endpoint = "/stock/social-sentiment"
        params = {"symbol": symbol}
        return await self._request(endpoint, params)