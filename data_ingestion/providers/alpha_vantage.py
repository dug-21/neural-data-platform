"""Alpha Vantage data provider implementation."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime
import pandas as pd

from .base import BaseProvider, MarketData, DataType
from ..utils.retry import with_retry


class AlphaVantageProvider(BaseProvider):
    """Alpha Vantage data provider for free market data."""
    
    BASE_URL = "https://www.alphavantage.co/query"
    
    # Map intervals to Alpha Vantage functions and intervals
    INTERVAL_MAP = {
        "1min": ("TIME_SERIES_INTRADAY", "1min"),
        "5min": ("TIME_SERIES_INTRADAY", "5min"),
        "15min": ("TIME_SERIES_INTRADAY", "15min"),
        "30min": ("TIME_SERIES_INTRADAY", "30min"),
        "1hour": ("TIME_SERIES_INTRADAY", "60min"),
        "1day": ("TIME_SERIES_DAILY", None),
        "1week": ("TIME_SERIES_WEEKLY", None),
        "1month": ("TIME_SERIES_MONTHLY", None)
    }
    
    def __init__(self):
        super().__init__("alpha_vantage")
        self.api_key = self.settings.alpha_vantage_api_key
        self.session: Optional[aiohttp.ClientSession] = None
        # Alpha Vantage has strict rate limits (5 requests per minute for free tier)
        self._rate_limiter = asyncio.Semaphore(1)  # Force sequential requests
        self._request_interval = 12  # 12 seconds between requests (5 per minute)
    
    async def connect(self):
        """Initialize HTTP session."""
        if not self.api_key:
            self.logger.warning("Alpha Vantage API key not configured - using demo key with limitations")
            self.api_key = "demo"
        
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._connected = True
        self.logger.info("Connected to Alpha Vantage")
    
    async def disconnect(self):
        """Close session."""
        if self.session:
            await self.session.close()
        self._connected = False
        self.logger.info("Disconnected from Alpha Vantage")
    
    @with_retry(max_attempts=3, exceptions=(aiohttp.ClientError,))
    async def _request(self, params: Dict[str, Any]) -> Any:
        """Make request to Alpha Vantage API."""
        async with self._rate_limiter:
            # Enforce rate limit
            await asyncio.sleep(self._request_interval)
            
            params["apikey"] = self.api_key
            
            try:
                async with self.session.get(self.BASE_URL, params=params) as response:
                    response.raise_for_status()
                    data = await response.json()
                    
                    # Check for API errors
                    if "Error Message" in data:
                        raise ValueError(f"API Error: {data['Error Message']}")
                    if "Note" in data:
                        self.logger.warning(f"API Note: {data['Note']}")
                        # Rate limit hit
                        await asyncio.sleep(60)  # Wait a minute
                        return await self._request(params)  # Retry
                    
                    return data
            except aiohttp.ClientError as e:
                self.logger.error("API request failed", error=str(e))
                raise
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data from Alpha Vantage."""
        symbols = self._validate_symbols(symbols)
        
        function, av_interval = self.INTERVAL_MAP.get(interval, ("TIME_SERIES_DAILY", None))
        
        for symbol in symbols:
            try:
                params = {
                    "function": function,
                    "symbol": symbol,
                    "outputsize": "full"  # Get maximum data
                }
                
                if av_interval:
                    params["interval"] = av_interval
                
                data = await self._request(params)
                
                # Parse response based on function type
                time_series_key = self._get_time_series_key(function, av_interval)
                if time_series_key not in data:
                    self.logger.warning(f"No data found for {symbol}")
                    continue
                
                time_series = data[time_series_key]
                
                # Convert to MarketData objects
                for timestamp, values in time_series.items():
                    market_data = self._parse_market_data(timestamp, values, symbol)
                    
                    # Filter by date range
                    if start_time <= market_data.time <= end_time:
                        yield market_data
                        
            except Exception as e:
                self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """
        Alpha Vantage doesn't support real-time streaming.
        This method polls for latest data at regular intervals.
        """
        symbols = self._validate_symbols(symbols)
        
        self.logger.info("Starting polling for real-time data (Alpha Vantage doesn't support streaming)")
        
        while True:
            for symbol in symbols:
                try:
                    # Get latest quote
                    params = {
                        "function": "GLOBAL_QUOTE",
                        "symbol": symbol
                    }
                    
                    data = await self._request(params)
                    
                    if "Global Quote" in data:
                        yield self._parse_quote_data(data["Global Quote"])
                        
                except Exception as e:
                    self.logger.error(f"Failed to fetch quote for {symbol}", error=str(e))
                    continue
            
            # Wait before next poll (respecting rate limits)
            await asyncio.sleep(60)  # Poll every minute
    
    def _get_time_series_key(self, function: str, interval: Optional[str]) -> str:
        """Get the correct time series key from response."""
        if function == "TIME_SERIES_INTRADAY":
            return f"Time Series ({interval})"
        elif function == "TIME_SERIES_DAILY":
            return "Time Series (Daily)"
        elif function == "TIME_SERIES_WEEKLY":
            return "Weekly Time Series"
        elif function == "TIME_SERIES_MONTHLY":
            return "Monthly Time Series"
        else:
            return "Time Series (Daily)"
    
    def _parse_market_data(self, timestamp: str, values: Dict[str, str], symbol: str) -> MarketData:
        """Parse Alpha Vantage time series data."""
        # Parse timestamp
        try:
            time = pd.to_datetime(timestamp)
        except:
            time = datetime.now()
        
        # Alpha Vantage uses different keys with spaces
        return MarketData(
            time=time,
            symbol=symbol,
            open=float(values.get("1. open", 0)),
            high=float(values.get("2. high", 0)),
            low=float(values.get("3. low", 0)),
            close=float(values.get("4. close", 0)),
            volume=int(values.get("5. volume", 0)),
            provider=self.name,
            metadata={
                "adjusted_close": values.get("5. adjusted close"),
                "dividend": values.get("7. dividend amount"),
                "split": values.get("8. split coefficient")
            }
        )
    
    def _parse_quote_data(self, quote: Dict[str, str]) -> MarketData:
        """Parse global quote data."""
        return MarketData(
            time=pd.to_datetime(quote.get("07. latest trading day", datetime.now())),
            symbol=quote.get("01. symbol", ""),
            open=float(quote.get("02. open", 0)),
            high=float(quote.get("03. high", 0)),
            low=float(quote.get("04. low", 0)),
            close=float(quote.get("05. price", 0)),
            volume=int(quote.get("06. volume", 0)),
            provider=self.name,
            metadata={
                "previous_close": quote.get("08. previous close"),
                "change": quote.get("09. change"),
                "change_percent": quote.get("10. change percent")
            }
        )
    
    async def get_technical_indicators(
        self,
        symbol: str,
        indicator: str,
        interval: str = "daily",
        time_period: int = 14,
        series_type: str = "close"
    ) -> Dict[str, Any]:
        """Get technical indicators."""
        params = {
            "function": indicator.upper(),
            "symbol": symbol,
            "interval": interval,
            "time_period": time_period,
            "series_type": series_type
        }
        
        return await self._request(params)
    
    async def get_sma(self, symbol: str, interval: str = "daily", time_period: int = 20) -> Dict[str, Any]:
        """Get Simple Moving Average."""
        return await self.get_technical_indicators(symbol, "SMA", interval, time_period)
    
    async def get_ema(self, symbol: str, interval: str = "daily", time_period: int = 20) -> Dict[str, Any]:
        """Get Exponential Moving Average."""
        return await self.get_technical_indicators(symbol, "EMA", interval, time_period)
    
    async def get_rsi(self, symbol: str, interval: str = "daily", time_period: int = 14) -> Dict[str, Any]:
        """Get Relative Strength Index."""
        return await self.get_technical_indicators(symbol, "RSI", interval, time_period)
    
    async def get_macd(
        self,
        symbol: str,
        interval: str = "daily",
        fast_period: int = 12,
        slow_period: int = 26,
        signal_period: int = 9
    ) -> Dict[str, Any]:
        """Get MACD indicator."""
        params = {
            "function": "MACD",
            "symbol": symbol,
            "interval": interval,
            "fastperiod": fast_period,
            "slowperiod": slow_period,
            "signalperiod": signal_period
        }
        
        return await self._request(params)
    
    async def get_sector_performance(self) -> Dict[str, Any]:
        """Get sector performance data."""
        params = {"function": "SECTOR"}
        return await self._request(params)
    
    async def get_crypto_data(
        self,
        symbol: str,
        market: str = "USD",
        interval: str = "daily"
    ) -> Dict[str, Any]:
        """Get cryptocurrency data."""
        if interval == "daily":
            function = "DIGITAL_CURRENCY_DAILY"
        elif interval == "weekly":
            function = "DIGITAL_CURRENCY_WEEKLY"
        elif interval == "monthly":
            function = "DIGITAL_CURRENCY_MONTHLY"
        else:
            function = "DIGITAL_CURRENCY_DAILY"
        
        params = {
            "function": function,
            "symbol": symbol,
            "market": market
        }
        
        return await self._request(params)