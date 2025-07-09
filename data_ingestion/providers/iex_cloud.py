"""IEX Cloud data provider implementation."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
import pandas as pd

from .base import BaseProvider, MarketData, TickData, DataType
from utils.retry import with_retry


class IEXCloudProvider(BaseProvider):
    """IEX Cloud data provider for real-time and historical market data."""
    
    BASE_URL = "https://cloud.iexapis.com/stable"
    SANDBOX_URL = "https://sandbox.iexapis.com/stable"
    
    def __init__(self, use_sandbox: bool = False):
        super().__init__("iex_cloud")
        self.api_key = self.settings.iex_cloud_api_key
        self.base_url = self.SANDBOX_URL if use_sandbox else self.BASE_URL
        self.session: Optional[aiohttp.ClientSession] = None
        self._ws_session: Optional[aiohttp.ClientSession] = None
        self._ws_connections: Dict[str, aiohttp.ClientWebSocketResponse] = {}
    
    async def connect(self):
        """Initialize HTTP session."""
        if not self.api_key:
            raise ValueError("IEX Cloud API key not configured")
        
        self.session = aiohttp.ClientSession(
            headers={"Accept": "application/json"},
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._ws_session = aiohttp.ClientSession()
        self._connected = True
        self.logger.info("Connected to IEX Cloud")
    
    async def disconnect(self):
        """Close all connections."""
        # Close WebSocket connections
        for ws in self._ws_connections.values():
            await ws.close()
        self._ws_connections.clear()
        
        # Close sessions
        if self.session:
            await self.session.close()
        if self._ws_session:
            await self._ws_session.close()
        
        self._connected = False
        self.logger.info("Disconnected from IEX Cloud")
    
    @with_retry(max_attempts=3, exceptions=(aiohttp.ClientError,))
    async def _request(self, endpoint: str, params: Optional[Dict[str, Any]] = None) -> Any:
        """Make authenticated request to IEX Cloud."""
        await self._rate_limit()
        
        url = f"{self.base_url}{endpoint}"
        params = params or {}
        params["token"] = self.api_key
        
        try:
            async with self.session.get(url, params=params) as response:
                response.raise_for_status()
                data = await response.json()
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
        """Fetch historical market data from IEX Cloud."""
        symbols = self._validate_symbols(symbols)
        interval_info = self._parse_interval(interval)
        
        # IEX Cloud historical data endpoints
        if interval == "1day":
            endpoint_template = "/stock/{symbol}/chart/range"
            range_param = self._calculate_date_range(start_time, end_time)
        else:
            # Intraday data
            endpoint_template = "/stock/{symbol}/intraday-prices"
            range_param = None
        
        for symbol in symbols:
            try:
                endpoint = endpoint_template.format(symbol=symbol.lower())
                params = {}
                
                if interval == "1day":
                    params["range"] = range_param
                else:
                    # For intraday, we need to fetch day by day
                    current_date = start_time.date()
                    end_date = end_time.date()
                    
                    while current_date <= end_date:
                        params = {
                            "chartIEXOnly": "true",
                            "chartInterval": interval_info["minutes"]
                        }
                        
                        # If fetching historical intraday (not today)
                        if current_date < datetime.now().date():
                            params["exactDate"] = current_date.strftime("%Y%m%d")
                        
                        data = await self._request(endpoint, params)
                        
                        if isinstance(data, list):
                            for item in data:
                                yield self._parse_market_data(item, symbol)
                        
                        current_date += timedelta(days=1)
                
            except Exception as e:
                self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data via SSE."""
        symbols = self._validate_symbols(symbols)
        
        # IEX Cloud SSE endpoint
        sse_url = f"https://cloud-sse.iexapis.com/stable/stocksUS"
        params = {
            "token": self.api_key,
            "symbols": ",".join(symbols),
            "on": "true"
        }
        
        async with self.session.get(sse_url, params=params) as response:
            async for line in response.content:
                if line.startswith(b"data: "):
                    try:
                        import json
                        data = json.loads(line[6:])
                        
                        if data.get("type") == "quote":
                            yield self._parse_streaming_data(data)
                    except Exception as e:
                        self.logger.error("Failed to parse streaming data", error=str(e))
                        continue
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch historical tick data (trades)."""
        symbols = self._validate_symbols(symbols)
        
        for symbol in symbols:
            try:
                # IEX Cloud TOPS/Last endpoint for recent trades
                endpoint = f"/stock/{symbol.lower()}/trades"
                data = await self._request(endpoint)
                
                if isinstance(data, list):
                    for trade in data:
                        tick = self._parse_tick_data(trade, symbol)
                        if start_time <= tick.time <= end_time:
                            yield tick
                            
            except Exception as e:
                self.logger.error(f"Failed to fetch tick data for {symbol}", error=str(e))
                continue
    
    def _parse_market_data(self, data: Dict[str, Any], symbol: str) -> MarketData:
        """Parse IEX Cloud response to MarketData."""
        # Handle different response formats
        if "date" in data and "minute" in data:
            # Intraday format
            time_str = f"{data['date']} {data['minute']}"
            time = pd.to_datetime(time_str)
        elif "date" in data:
            # Daily format
            time = pd.to_datetime(data["date"])
        else:
            time = datetime.now()
        
        return MarketData(
            time=time,
            symbol=symbol,
            open=float(data.get("open", 0)),
            high=float(data.get("high", 0)),
            low=float(data.get("low", 0)),
            close=float(data.get("close", 0)),
            volume=int(data.get("volume", 0)),
            provider=self.name,
            metadata={
                "change": data.get("change"),
                "changePercent": data.get("changePercent"),
                "marketCap": data.get("marketCap")
            }
        )
    
    def _parse_streaming_data(self, data: Dict[str, Any]) -> MarketData:
        """Parse streaming quote data."""
        return MarketData(
            time=pd.to_datetime(data.get("latestUpdate", datetime.now()), unit="ms"),
            symbol=data.get("symbol", ""),
            open=float(data.get("open", 0)),
            high=float(data.get("high", 0)),
            low=float(data.get("low", 0)),
            close=float(data.get("latestPrice", 0)),
            volume=int(data.get("volume", 0)),
            provider=self.name,
            metadata={
                "bid": data.get("bidPrice"),
                "ask": data.get("askPrice"),
                "bidSize": data.get("bidSize"),
                "askSize": data.get("askSize")
            }
        )
    
    def _parse_tick_data(self, data: Dict[str, Any], symbol: str) -> TickData:
        """Parse trade data to TickData."""
        return TickData(
            time=pd.to_datetime(data.get("timestamp", datetime.now()), unit="ms"),
            symbol=symbol,
            price=float(data.get("price", 0)),
            size=int(data.get("size", 0)),
            exchange=data.get("exchange"),
            conditions=data.get("conditions"),
            provider=self.name
        )
    
    def _calculate_date_range(self, start_time: datetime, end_time: datetime) -> str:
        """Calculate IEX Cloud range parameter."""
        days_diff = (end_time - start_time).days
        
        if days_diff <= 5:
            return "5d"
        elif days_diff <= 30:
            return "1m"
        elif days_diff <= 90:
            return "3m"
        elif days_diff <= 180:
            return "6m"
        elif days_diff <= 365:
            return "1y"
        elif days_diff <= 730:
            return "2y"
        elif days_diff <= 1825:
            return "5y"
        else:
            return "max"
    
    async def get_company_info(self, symbol: str) -> Dict[str, Any]:
        """Get company information."""
        endpoint = f"/stock/{symbol.lower()}/company"
        return await self._request(endpoint)
    
    async def get_stats(self, symbol: str) -> Dict[str, Any]:
        """Get key statistics."""
        endpoint = f"/stock/{symbol.lower()}/stats"
        return await self._request(endpoint)
    
    async def get_news(self, symbol: str, last: int = 10) -> List[Dict[str, Any]]:
        """Get latest news for a symbol."""
        endpoint = f"/stock/{symbol.lower()}/news/last/{last}"
        return await self._request(endpoint)