"""FRED (Federal Reserve Economic Data) provider implementation."""
import aiohttp
import asyncio
from datetime import datetime
from typing import List, Dict, Any, AsyncIterator, Optional
import logging

from .base import BaseProvider, MarketData, DataType
from ..config import get_settings
from ..utils.retry import with_retry


class FREDProvider(BaseProvider):
    """Provider for Federal Reserve Economic Data (FRED)."""
    
    def __init__(self):
        super().__init__("FRED")
        self.base_url = "https://api.stlouisfed.org/fred"
        self._session: Optional[aiohttp.ClientSession] = None
        self.api_key = self.settings.fred_api_key
        
        if not self.api_key:
            raise ValueError("FRED_API_KEY not found in settings")
    
    async def connect(self):
        """Initialize connection to FRED API."""
        if self._connected:
            return
            
        self._session = aiohttp.ClientSession(
            headers={
                "User-Agent": "neural-trader/1.0"
            }
        )
        self._connected = True
        self.logger.info("Connected to FRED API")
    
    async def disconnect(self):
        """Close connection to FRED API."""
        if self._session:
            await self._session.close()
            self._session = None
        self._connected = False
        self.logger.info("Disconnected from FRED API")
    
    def _check_connection(self):
        """Ensure provider is connected."""
        if not self._connected:
            raise RuntimeError("Not connected to FRED API. Call connect() first.")
    
    async def get_series(
        self,
        series_id: str,
        start_date: datetime,
        end_date: datetime,
        frequency: str = "d"  # daily by default
    ) -> AsyncIterator[MarketData]:
        """
        Fetch economic time series data from FRED.
        
        Args:
            series_id: FRED series ID (e.g., 'DGS10' for 10-year Treasury)
            start_date: Start date for data
            end_date: End date for data
            frequency: Data frequency (d=daily, w=weekly, m=monthly, q=quarterly, a=annual)
            
        Yields:
            MarketData objects with economic indicators
        """
        self._check_connection()
        await self._rate_limit()
        
        url = f"{self.base_url}/series/observations"
        params = {
            "series_id": series_id,
            "api_key": self.api_key,
            "file_type": "json",
            "observation_start": start_date.strftime("%Y-%m-%d"),
            "observation_end": end_date.strftime("%Y-%m-%d"),
            "frequency": frequency,
            "units": "lin"  # Linear units (no transformation)
        }
        
        try:
            async with self._session.get(url, params=params) as response:
                if response.status != 200:
                    error_text = await response.text()
                    raise Exception(f"FRED API error ({response.status}): {error_text}")
                
                data = await response.json()
                observations = data.get("observations", [])
                
                for obs in observations:
                    try:
                        # Parse date and value
                        date = datetime.strptime(obs["date"], "%Y-%m-%d")
                        value = float(obs["value"])
                        
                        # FRED data doesn't have OHLC, so we use the value for all
                        yield MarketData(
                            time=date,
                            symbol=series_id,
                            open=value,
                            high=value,
                            low=value,
                            close=value,
                            volume=0,  # No volume for economic data
                            provider=self.name,
                            metadata={
                                "series_id": series_id,
                                "frequency": frequency,
                                "units": obs.get("units", "")
                            }
                        )
                    except (ValueError, KeyError) as e:
                        self.logger.warning(f"Skipping invalid observation: {e}")
                        continue
                        
        except aiohttp.ClientError as e:
            self.logger.error(f"Network error fetching FRED data: {e}")
            raise
        except Exception as e:
            self.logger.error(f"Error fetching FRED series {series_id}: {e}")
            raise
    
    async def search_series(
        self,
        search_text: str,
        limit: int = 100
    ) -> List[Dict[str, Any]]:
        """
        Search for FRED series by text.
        
        Args:
            search_text: Text to search for
            limit: Maximum number of results
            
        Returns:
            List of series metadata
        """
        self._check_connection()
        await self._rate_limit()
        
        url = f"{self.base_url}/series/search"
        params = {
            "search_text": search_text,
            "api_key": self.api_key,
            "file_type": "json",
            "limit": limit
        }
        
        try:
            async with self._session.get(url, params=params) as response:
                if response.status != 200:
                    error_text = await response.text()
                    raise Exception(f"FRED API error ({response.status}): {error_text}")
                
                data = await response.json()
                series_list = data.get("seriess", [])
                
                return [
                    {
                        "id": s["id"],
                        "title": s["title"],
                        "units": s.get("units", ""),
                        "frequency": s.get("frequency", ""),
                        "seasonal_adjustment": s.get("seasonal_adjustment", ""),
                        "last_updated": s.get("last_updated", "")
                    }
                    for s in series_list
                ]
                
        except Exception as e:
            self.logger.error(f"Error searching FRED series: {e}")
            raise
    
    async def get_popular_series(self) -> Dict[str, str]:
        """
        Get popular FRED series for trading.
        
        Returns:
            Dictionary of series_id: description
        """
        return {
            # Interest Rates
            "DGS10": "10-Year Treasury Constant Maturity Rate",
            "DGS2": "2-Year Treasury Constant Maturity Rate", 
            "DGS30": "30-Year Treasury Constant Maturity Rate",
            "DFF": "Effective Federal Funds Rate",
            "SOFR": "Secured Overnight Financing Rate",
            
            # Economic Indicators  
            "UNRATE": "Unemployment Rate",
            "CPIAUCSL": "Consumer Price Index for All Urban Consumers",
            "CPILFESL": "Core CPI (Less Food and Energy)",
            "GDP": "Gross Domestic Product",
            "GDPC1": "Real GDP",
            
            # Market Indicators
            "VIXCLS": "CBOE Volatility Index (VIX)",
            "DEXUSEU": "U.S. / Euro Foreign Exchange Rate",
            "DXY": "U.S. Dollar Index",
            
            # Money Supply
            "M2SL": "M2 Money Supply",
            "BOGMBASE": "Monetary Base",
            
            # Consumer Sentiment
            "UMCSENT": "University of Michigan Consumer Sentiment",
            "DFEDTARU": "Federal Funds Target Rate - Upper Limit"
        }
    
    # Implement required abstract methods (even if not fully used for economic data)
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Fetch market data - for FRED, this wraps get_series."""
        for symbol in symbols:
            # Map interval to FRED frequency
            frequency_map = {
                "1day": "d",
                "1week": "w",
                "1month": "m",
                "3month": "q",
                "1year": "a"
            }
            frequency = frequency_map.get(interval, "d")
            
            async for data in self.get_series(symbol, start_time, end_time, frequency):
                yield data
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """
        Stream real-time data - not applicable for FRED.
        FRED data is updated at various frequencies (daily, weekly, monthly).
        """
        raise NotImplementedError("FRED does not provide real-time streaming data")
    
    def get_update_schedule(self) -> Dict[str, str]:
        """
        Get typical update schedules for popular series.
        
        Returns:
            Dictionary of series_id: update schedule
        """
        return {
            "DGS10": "Daily at 3:00 PM ET",
            "DFF": "Daily at 9:00 AM ET",
            "UNRATE": "Monthly, first Friday at 8:30 AM ET",
            "CPIAUCSL": "Monthly, around the 15th at 8:30 AM ET",
            "GDP": "Quarterly, end of month at 8:30 AM ET",
            "VIXCLS": "Daily at market close"
        }