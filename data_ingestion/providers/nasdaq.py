"""NASDAQ/Quandl data provider implementation."""
import asyncio
import aiohttp
from typing import List, AsyncIterator, Optional, Dict, Any
from datetime import datetime
import pandas as pd
from urllib.parse import urlencode

from .base import BaseProvider, MarketData, DataType
from ..utils.retry import with_retry


class NASDAQProvider(BaseProvider):
    """NASDAQ/Quandl data provider for comprehensive financial data."""
    
    BASE_URL = "https://www.quandl.com/api/v3"
    
    # Map intervals to Quandl data frequencies
    FREQUENCY_MAP = {
        "1min": "minutely",
        "5min": "5minutely",
        "15min": "15minutely",
        "30min": "30minutely",
        "1hour": "hourly",
        "1day": "daily",
        "1week": "weekly",
        "1month": "monthly",
        "1quarter": "quarterly",
        "1year": "annual"
    }
    
    # Quandl database codes for different data types
    DATABASES = {
        "stocks": "WIKI",  # Wiki EOD Stock Prices
        "futures": "CHRIS",  # Continuous Futures
        "forex": "CURRFX",  # Currency Exchange Rates
        "economics": "FRED",  # Federal Reserve Economic Data
        "commodities": "ODA",  # IMF Cross Country Macroeconomic Statistics
        "nasdaq": "NASDAQOMX"  # NASDAQ OMX Global Index Data
    }
    
    def __init__(self):
        super().__init__("nasdaq")
        self.api_key = self.settings.quandl_api_key
        self.session: Optional[aiohttp.ClientSession] = None
        # Quandl rate limits: 50,000 calls per day for authenticated users
        self._rate_limiter = asyncio.Semaphore(10)  # Allow parallel requests
        self._daily_limit = 50000
        self._daily_calls = 0
        self._last_reset = datetime.now()
    
    async def connect(self):
        """Initialize HTTP session."""
        if not self.api_key:
            raise ValueError("Quandl API key is required for NASDAQ provider")
        
        self.session = aiohttp.ClientSession(
            timeout=aiohttp.ClientTimeout(total=60)
        )
        self._connected = True
        self.logger.info("Connected to NASDAQ/Quandl API")
    
    async def disconnect(self):
        """Close session."""
        if self.session:
            await self.session.close()
            self.session = None
        self._connected = False
        self.logger.info("Disconnected from NASDAQ/Quandl API")
    
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1day"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data from Quandl."""
        if not self._connected:
            raise RuntimeError("Provider not connected")
        
        valid_symbols = self._validate_symbols(symbols)
        frequency = self.FREQUENCY_MAP.get(interval, "daily")
        
        for symbol in valid_symbols:
            try:
                await self._check_daily_limit()
                
                # Determine the appropriate database for the symbol
                database = self._get_database_for_symbol(symbol)
                
                # Build the API endpoint
                endpoint = f"{self.BASE_URL}/datasets/{database}/{symbol}"
                
                params = {
                    "api_key": self.api_key,
                    "start_date": start_time.strftime("%Y-%m-%d"),
                    "end_date": end_time.strftime("%Y-%m-%d"),
                    "collapse": frequency,
                    "order": "asc"
                }
                
                data = await self._fetch_data(endpoint, params)
                
                if data and "dataset" in data:
                    dataset = data["dataset"]
                    columns = dataset.get("column_names", [])
                    data_rows = dataset.get("data", [])
                    
                    # Map column names to indices
                    col_map = {col.lower(): idx for idx, col in enumerate(columns)}
                    
                    for row in data_rows:
                        try:
                            # Parse date
                            date_str = row[col_map.get("date", 0)]
                            timestamp = pd.to_datetime(date_str)
                            
                            # Extract OHLCV data
                            open_price = float(row[col_map.get("open", 1)])
                            high_price = float(row[col_map.get("high", 2)])
                            low_price = float(row[col_map.get("low", 3)])
                            close_price = float(row[col_map.get("close", 4)])
                            volume = int(row[col_map.get("volume", 5)] or 0)
                            
                            yield MarketData(
                                time=timestamp,
                                symbol=symbol,
                                open=open_price,
                                high=high_price,
                                low=low_price,
                                close=close_price,
                                volume=volume,
                                provider=self.name,
                                metadata={
                                    "database": database,
                                    "frequency": frequency,
                                    "source": "quandl"
                                }
                            )
                        except (ValueError, IndexError, KeyError) as e:
                            self.logger.warning(f"Error parsing data row for {symbol}: {e}")
                            continue
                
            except Exception as e:
                self.logger.error(f"Error fetching data for {symbol}: {e}")
                continue
    
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data (not supported by Quandl)."""
        raise NotImplementedError("Quandl does not support real-time streaming data")
    
    async def get_economic_indicators(
        self,
        indicators: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[Dict[str, Any]]:
        """Fetch economic indicators from FRED database."""
        if not self._connected:
            raise RuntimeError("Provider not connected")
        
        for indicator in indicators:
            try:
                await self._check_daily_limit()
                
                # Use FRED database for economic indicators
                endpoint = f"{self.BASE_URL}/datasets/FRED/{indicator}"
                
                params = {
                    "api_key": self.api_key,
                    "start_date": start_time.strftime("%Y-%m-%d"),
                    "end_date": end_time.strftime("%Y-%m-%d"),
                    "order": "asc"
                }
                
                data = await self._fetch_data(endpoint, params)
                
                if data and "dataset" in data:
                    dataset = data["dataset"]
                    columns = dataset.get("column_names", [])
                    data_rows = dataset.get("data", [])
                    
                    for row in data_rows:
                        try:
                            yield {
                                "indicator": indicator,
                                "date": row[0],
                                "value": float(row[1]),
                                "metadata": {
                                    "name": dataset.get("name", ""),
                                    "description": dataset.get("description", ""),
                                    "units": dataset.get("frequency", ""),
                                    "source": "FRED/Quandl"
                                }
                            }
                        except (ValueError, IndexError) as e:
                            self.logger.warning(f"Error parsing indicator data for {indicator}: {e}")
                            continue
                
            except Exception as e:
                self.logger.error(f"Error fetching economic indicator {indicator}: {e}")
                continue
    
    async def get_futures_data(
        self,
        contracts: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[MarketData]:
        """Fetch futures contract data."""
        if not self._connected:
            raise RuntimeError("Provider not connected")
        
        for contract in contracts:
            try:
                await self._check_daily_limit()
                
                # Use CHRIS database for futures
                endpoint = f"{self.BASE_URL}/datasets/CHRIS/{contract}"
                
                params = {
                    "api_key": self.api_key,
                    "start_date": start_time.strftime("%Y-%m-%d"),
                    "end_date": end_time.strftime("%Y-%m-%d"),
                    "order": "asc"
                }
                
                data = await self._fetch_data(endpoint, params)
                
                if data and "dataset" in data:
                    dataset = data["dataset"]
                    columns = dataset.get("column_names", [])
                    data_rows = dataset.get("data", [])
                    
                    # Map column names to indices
                    col_map = {col.lower(): idx for idx, col in enumerate(columns)}
                    
                    for row in data_rows:
                        try:
                            # Parse date
                            date_str = row[col_map.get("date", 0)]
                            timestamp = pd.to_datetime(date_str)
                            
                            # Extract OHLCV data
                            open_price = float(row[col_map.get("open", 1)])
                            high_price = float(row[col_map.get("high", 2)])
                            low_price = float(row[col_map.get("low", 3)])
                            close_price = float(row[col_map.get("settle", col_map.get("close", 4))])
                            volume = int(row[col_map.get("volume", 5)] or 0)
                            
                            yield MarketData(
                                time=timestamp,
                                symbol=contract,
                                open=open_price,
                                high=high_price,
                                low=low_price,
                                close=close_price,
                                volume=volume,
                                provider=self.name,
                                metadata={
                                    "database": "CHRIS",
                                    "type": "futures",
                                    "source": "quandl"
                                }
                            )
                        except (ValueError, IndexError, KeyError) as e:
                            self.logger.warning(f"Error parsing futures data for {contract}: {e}")
                            continue
                
            except Exception as e:
                self.logger.error(f"Error fetching futures data for {contract}: {e}")
                continue
    
    async def search_datasets(
        self,
        query: str,
        database: Optional[str] = None,
        limit: int = 10
    ) -> List[Dict[str, Any]]:
        """Search for available datasets in Quandl."""
        if not self._connected:
            raise RuntimeError("Provider not connected")
        
        await self._check_daily_limit()
        
        endpoint = f"{self.BASE_URL}/datasets"
        params = {
            "api_key": self.api_key,
            "query": query,
            "per_page": limit
        }
        
        if database:
            params["database_code"] = database
        
        data = await self._fetch_data(endpoint, params)
        
        results = []
        if data and "datasets" in data:
            for dataset in data["datasets"]:
                results.append({
                    "id": dataset.get("id"),
                    "database_code": dataset.get("database_code"),
                    "dataset_code": dataset.get("dataset_code"),
                    "name": dataset.get("name"),
                    "description": dataset.get("description"),
                    "refreshed_at": dataset.get("refreshed_at"),
                    "newest_available_date": dataset.get("newest_available_date"),
                    "oldest_available_date": dataset.get("oldest_available_date"),
                    "column_names": dataset.get("column_names", []),
                    "frequency": dataset.get("frequency"),
                    "type": dataset.get("type"),
                    "premium": dataset.get("premium", False)
                })
        
        return results
    
    @with_retry(max_attempts=3, backoff_factor=2)
    async def _fetch_data(self, endpoint: str, params: Dict[str, Any]) -> Optional[Dict[str, Any]]:
        """Fetch data from Quandl API with retry logic."""
        async with self._rate_limiter:
            await self._rate_limit()
            
            try:
                url = f"{endpoint}?{urlencode(params)}"
                
                async with self.session.get(url) as response:
                    self._daily_calls += 1
                    
                    if response.status == 200:
                        return await response.json()
                    elif response.status == 429:
                        # Rate limit exceeded
                        retry_after = response.headers.get("Retry-After", 60)
                        self.logger.warning(f"Rate limit exceeded, retrying after {retry_after}s")
                        await asyncio.sleep(int(retry_after))
                        raise Exception("Rate limit exceeded")
                    elif response.status == 404:
                        self.logger.warning(f"Dataset not found: {endpoint}")
                        return None
                    else:
                        error_data = await response.text()
                        self.logger.error(f"API error {response.status}: {error_data}")
                        raise Exception(f"API error {response.status}: {error_data}")
                        
            except aiohttp.ClientError as e:
                self.logger.error(f"Network error fetching data: {e}")
                raise
    
    def _get_database_for_symbol(self, symbol: str) -> str:
        """Determine the appropriate Quandl database for a symbol."""
        # Simple heuristic - can be extended based on symbol patterns
        if "_" in symbol:  # Likely a futures contract
            return self.DATABASES["futures"]
        elif symbol.startswith("USD") or symbol.endswith("USD"):  # Currency pairs
            return self.DATABASES["forex"]
        elif symbol in ["GDP", "CPI", "UNRATE", "DGS10"]:  # Economic indicators
            return self.DATABASES["economics"]
        else:  # Default to stock data
            return self.DATABASES["stocks"]
    
    async def _check_daily_limit(self):
        """Check and reset daily API call limit."""
        now = datetime.now()
        if (now - self._last_reset).days >= 1:
            self._daily_calls = 0
            self._last_reset = now
            self.logger.info("Daily API call counter reset")
        
        if self._daily_calls >= self._daily_limit:
            # Calculate time until midnight
            tomorrow = now.replace(hour=0, minute=0, second=0, microsecond=0)
            tomorrow = tomorrow.replace(day=tomorrow.day + 1)
            sleep_seconds = (tomorrow - now).total_seconds()
            
            self.logger.error(f"Daily API limit reached ({self._daily_limit}), sleeping until reset in {sleep_seconds/3600:.1f} hours")
            await asyncio.sleep(sleep_seconds)
            self._daily_calls = 0
            self._last_reset = datetime.now()
    
    async def get_metadata(self, database: str, dataset: str) -> Optional[Dict[str, Any]]:
        """Get metadata for a specific dataset."""
        if not self._connected:
            raise RuntimeError("Provider not connected")
        
        await self._check_daily_limit()
        
        endpoint = f"{self.BASE_URL}/datasets/{database}/{dataset}/metadata"
        params = {"api_key": self.api_key}
        
        return await self._fetch_data(endpoint, params)