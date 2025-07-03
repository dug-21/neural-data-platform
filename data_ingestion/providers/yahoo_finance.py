"""Yahoo Finance data provider implementation (free tier)."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime, timedelta
import pandas as pd
import yfinance as yf
from concurrent.futures import ThreadPoolExecutor

from .base import BaseProvider, MarketData, DataType
from ..utils.retry import with_retry


class YahooFinanceProvider(BaseProvider):
    """Yahoo Finance data provider using yfinance library."""
    
    # Map intervals to yfinance format
    INTERVAL_MAP = {
        "1min": "1m",
        "2min": "2m",
        "5min": "5m",
        "15min": "15m",
        "30min": "30m",
        "1hour": "60m",
        "1day": "1d",
        "5day": "5d",
        "1week": "1wk",
        "1month": "1mo",
        "3month": "3mo"
    }
    
    def __init__(self):
        super().__init__("yahoo_finance")
        self.session: Optional[aiohttp.ClientSession] = None
        self._executor = ThreadPoolExecutor(max_workers=5)
        # No API key needed for Yahoo Finance
    
    async def connect(self):
        """Initialize session."""
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=30)
        )
        self._connected = True
        self.logger.info("Connected to Yahoo Finance")
    
    async def disconnect(self):
        """Close session and executor."""
        if self.session:
            await self.session.close()
        self._executor.shutdown(wait=True)
        self._connected = False
        self.logger.info("Disconnected from Yahoo Finance")
    
    def _fetch_data_sync(
        self,
        symbol: str,
        start: datetime,
        end: datetime,
        interval: str
    ) -> pd.DataFrame:
        """Synchronous method to fetch data using yfinance."""
        try:
            ticker = yf.Ticker(symbol)
            
            # For intraday data, Yahoo Finance only provides last 60 days
            if interval in ["1m", "2m", "5m", "15m", "30m", "60m"]:
                max_days = 60
                if (datetime.now() - start).days > max_days:
                    self.logger.warning(
                        f"Yahoo Finance only provides {max_days} days of intraday data. "
                        f"Adjusting start date for {symbol}"
                    )
                    start = datetime.now() - timedelta(days=max_days)
            
            # Download data
            df = ticker.history(
                start=start,
                end=end,
                interval=interval,
                auto_adjust=True,
                prepost=False,
                actions=False
            )
            
            return df
            
        except Exception as e:
            self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
            return pd.DataFrame()
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data from Yahoo Finance."""
        symbols = self._validate_symbols(symbols)
        yf_interval = self.INTERVAL_MAP.get(interval, "1d")
        
        for symbol in symbols:
            try:
                # Run yfinance in thread pool to avoid blocking
                loop = asyncio.get_event_loop()
                df = await loop.run_in_executor(
                    self._executor,
                    self._fetch_data_sync,
                    symbol,
                    start_time,
                    end_time,
                    yf_interval
                )
                
                if df.empty:
                    self.logger.warning(f"No data returned for {symbol}")
                    continue
                
                # Convert DataFrame to MarketData objects
                for timestamp, row in df.iterrows():
                    yield MarketData(
                        time=timestamp,
                        symbol=symbol,
                        open=float(row.get("Open", 0)),
                        high=float(row.get("High", 0)),
                        low=float(row.get("Low", 0)),
                        close=float(row.get("Close", 0)),
                        volume=int(row.get("Volume", 0)),
                        provider=self.name,
                        metadata={
                            "dividends": row.get("Dividends"),
                            "splits": row.get("Stock Splits")
                        }
                    )
                    
            except Exception as e:
                self.logger.error(f"Failed to fetch data for {symbol}", error=str(e))
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """
        Yahoo Finance doesn't provide real WebSocket streaming.
        This method polls for latest data at regular intervals.
        """
        symbols = self._validate_symbols(symbols)
        
        self.logger.info("Starting polling for real-time data (Yahoo Finance doesn't support true streaming)")
        
        while True:
            for symbol in symbols:
                try:
                    # Get latest quote using yfinance
                    loop = asyncio.get_event_loop()
                    ticker = await loop.run_in_executor(
                        self._executor,
                        yf.Ticker,
                        symbol
                    )
                    
                    info = await loop.run_in_executor(
                        self._executor,
                        lambda: ticker.info
                    )
                    
                    if info:
                        yield self._parse_quote_data(info, symbol)
                        
                except Exception as e:
                    self.logger.error(f"Failed to fetch quote for {symbol}", error=str(e))
                    continue
            
            # Poll every 10 seconds (be respectful to free service)
            await asyncio.sleep(10)
    
    def _parse_quote_data(self, info: Dict[str, Any], symbol: str) -> MarketData:
        """Parse Yahoo Finance quote data."""
        return MarketData(
            time=datetime.now(),
            symbol=symbol,
            open=float(info.get("regularMarketOpen", info.get("open", 0))),
            high=float(info.get("regularMarketDayHigh", info.get("dayHigh", 0))),
            low=float(info.get("regularMarketDayLow", info.get("dayLow", 0))),
            close=float(info.get("regularMarketPrice", info.get("currentPrice", 0))),
            volume=int(info.get("regularMarketVolume", info.get("volume", 0))),
            provider=self.name,
            metadata={
                "market_cap": info.get("marketCap"),
                "pe_ratio": info.get("trailingPE"),
                "dividend_yield": info.get("dividendYield"),
                "52_week_high": info.get("fiftyTwoWeekHigh"),
                "52_week_low": info.get("fiftyTwoWeekLow"),
                "average_volume": info.get("averageVolume"),
                "beta": info.get("beta"),
                "currency": info.get("currency", "USD")
            }
        )
    
    async def get_company_info(self, symbol: str) -> Dict[str, Any]:
        """Get company information."""
        try:
            loop = asyncio.get_event_loop()
            ticker = await loop.run_in_executor(
                self._executor,
                yf.Ticker,
                symbol
            )
            
            info = await loop.run_in_executor(
                self._executor,
                lambda: ticker.info
            )
            
            return {
                "symbol": symbol,
                "name": info.get("longName", info.get("shortName", "")),
                "sector": info.get("sector"),
                "industry": info.get("industry"),
                "description": info.get("longBusinessSummary"),
                "website": info.get("website"),
                "employees": info.get("fullTimeEmployees"),
                "country": info.get("country"),
                "currency": info.get("currency", "USD")
            }
            
        except Exception as e:
            self.logger.error(f"Failed to fetch company info for {symbol}", error=str(e))
            return {}
    
    async def get_options_chain(self, symbol: str) -> Dict[str, Any]:
        """Get options chain data."""
        try:
            loop = asyncio.get_event_loop()
            ticker = await loop.run_in_executor(
                self._executor,
                yf.Ticker,
                symbol
            )
            
            # Get available expiration dates
            expirations = await loop.run_in_executor(
                self._executor,
                lambda: ticker.options
            )
            
            if not expirations:
                return {}
            
            # Get options for nearest expiration
            options = await loop.run_in_executor(
                self._executor,
                ticker.option_chain,
                expirations[0]
            )
            
            return {
                "symbol": symbol,
                "expirations": expirations,
                "calls": options.calls.to_dict("records"),
                "puts": options.puts.to_dict("records")
            }
            
        except Exception as e:
            self.logger.error(f"Failed to fetch options for {symbol}", error=str(e))
            return {}
    
    async def get_recommendations(self, symbol: str) -> List[Dict[str, Any]]:
        """Get analyst recommendations."""
        try:
            loop = asyncio.get_event_loop()
            ticker = await loop.run_in_executor(
                self._executor,
                yf.Ticker,
                symbol
            )
            
            recommendations = await loop.run_in_executor(
                self._executor,
                lambda: ticker.recommendations
            )
            
            if recommendations is not None and not recommendations.empty:
                return recommendations.reset_index().to_dict("records")
            
            return []
            
        except Exception as e:
            self.logger.error(f"Failed to fetch recommendations for {symbol}", error=str(e))
            return []
    
    async def get_institutional_holders(self, symbol: str) -> List[Dict[str, Any]]:
        """Get institutional holders data."""
        try:
            loop = asyncio.get_event_loop()
            ticker = await loop.run_in_executor(
                self._executor,
                yf.Ticker,
                symbol
            )
            
            holders = await loop.run_in_executor(
                self._executor,
                lambda: ticker.institutional_holders
            )
            
            if holders is not None and not holders.empty:
                return holders.to_dict("records")
            
            return []
            
        except Exception as e:
            self.logger.error(f"Failed to fetch institutional holders for {symbol}", error=str(e))
            return []
    
    async def get_earnings(self, symbol: str) -> Dict[str, Any]:
        """Get earnings data."""
        try:
            loop = asyncio.get_event_loop()
            ticker = await loop.run_in_executor(
                self._executor,
                yf.Ticker,
                symbol
            )
            
            earnings = await loop.run_in_executor(
                self._executor,
                lambda: ticker.earnings
            )
            
            quarterly_earnings = await loop.run_in_executor(
                self._executor,
                lambda: ticker.quarterly_earnings
            )
            
            result = {"symbol": symbol}
            
            if earnings is not None and not earnings.empty:
                result["annual"] = earnings.to_dict("records")
            
            if quarterly_earnings is not None and not quarterly_earnings.empty:
                result["quarterly"] = quarterly_earnings.to_dict("records")
            
            return result
            
        except Exception as e:
            self.logger.error(f"Failed to fetch earnings for {symbol}", error=str(e))
            return {"symbol": symbol}