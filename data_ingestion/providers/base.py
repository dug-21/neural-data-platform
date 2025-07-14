"""Base provider interface for data sources."""
from abc import ABC, abstractmethod
from typing import Dict, List, Any, Optional, AsyncIterator
from datetime import datetime
import asyncio
from dataclasses import dataclass
from enum import Enum

from config import get_settings
from utils.logging import get_logger
from utils.metrics import metrics
from utils.retry import with_retry
from utils.normalization import DataNormalizer


class DataType(Enum):
    """Types of data available from providers."""
    MARKET_DATA = "market_data"
    TICK_DATA = "tick_data"
    ORDER_BOOK = "order_book"
    NEWS = "news"
    FUNDAMENTALS = "fundamentals"
    TECHNICAL = "technical"


@dataclass
class MarketData:
    """Standard market data structure."""
    time: datetime
    symbol: str
    open: float
    high: float
    low: float
    close: float
    volume: int
    provider: str
    metadata: Optional[Dict[str, Any]] = None
    
    def __post_init__(self):
        """Normalize data on creation."""
        # Normalize timestamp to UTC with proper rounding
        self.time = DataNormalizer.normalize_timestamp(self.time)
        
        # Normalize symbol to uppercase
        self.symbol = DataNormalizer.normalize_symbol(self.symbol)
        
        # Normalize prices to 2 decimal places
        self.open = DataNormalizer.normalize_price(self.open)
        self.high = DataNormalizer.normalize_price(self.high)
        self.low = DataNormalizer.normalize_price(self.low)
        self.close = DataNormalizer.normalize_price(self.close)
        
        # Ensure volume is integer
        self.volume = DataNormalizer.normalize_volume(self.volume)
        
        # Normalize provider name
        self.provider = DataNormalizer.normalize_provider_name(self.provider)
        
        # Validate OHLC consistency
        if not DataNormalizer.validate_ohlc_consistency(self.open, self.high, self.low, self.close):
            raise ValueError(f"Invalid OHLC data for {self.symbol}: O={self.open}, H={self.high}, L={self.low}, C={self.close}")


@dataclass
class TickData:
    """Tick-level trade data."""
    time: datetime
    symbol: str
    price: float
    size: int
    exchange: Optional[str] = None
    conditions: Optional[str] = None
    provider: Optional[str] = None
    
    def __post_init__(self):
        """Normalize tick data on creation."""
        self.time = DataNormalizer.normalize_timestamp(self.time)
        self.symbol = DataNormalizer.normalize_symbol(self.symbol)
        self.price = DataNormalizer.normalize_price(self.price)
        self.size = DataNormalizer.normalize_volume(self.size)
        if self.provider:
            self.provider = DataNormalizer.normalize_provider_name(self.provider)


@dataclass
class OrderBookData:
    """Order book snapshot data."""
    time: datetime
    symbol: str
    bid_price: float
    bid_size: int
    ask_price: float
    ask_size: int
    mid_price: float
    spread: float
    provider: str
    
    def __post_init__(self):
        """Normalize order book data on creation."""
        self.time = DataNormalizer.normalize_timestamp(self.time)
        self.symbol = DataNormalizer.normalize_symbol(self.symbol)
        self.bid_price = DataNormalizer.normalize_price(self.bid_price)
        self.ask_price = DataNormalizer.normalize_price(self.ask_price)
        self.bid_size = DataNormalizer.normalize_volume(self.bid_size)
        self.ask_size = DataNormalizer.normalize_volume(self.ask_size)
        self.mid_price = DataNormalizer.normalize_price(self.mid_price)
        self.spread = DataNormalizer.normalize_price(self.spread)
        self.provider = DataNormalizer.normalize_provider_name(self.provider)


class BaseProvider(ABC):
    """Abstract base class for all data providers."""
    
    def __init__(self, name: str):
        self.name = name
        self.settings = get_settings()
        self.logger = get_logger(f"{__name__}.{name}")
        self._rate_limiter = asyncio.Semaphore(self.settings.max_concurrent_requests)
        self._last_request_time = 0
        self._request_count = 0
        self._connected = False
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self.connect()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.disconnect()
    
    @abstractmethod
    async def connect(self):
        """Initialize provider connection."""
        metrics.active_connections.labels(connection_type=f"provider_{self.name}").inc()
        self._connected = True
    
    @abstractmethod
    async def disconnect(self):
        """Clean up provider connection."""
        if self._connected:
            metrics.active_connections.labels(connection_type=f"provider_{self.name}").dec()
            self._connected = False
    
    @abstractmethod
    async def get_market_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime,
        interval: str = "1min"
    ) -> AsyncIterator[MarketData]:
        """Fetch historical market data."""
        pass
    
    @abstractmethod
    async def stream_market_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[MarketData]:
        """Stream real-time market data."""
        pass
    
    async def get_tick_data(
        self,
        symbols: List[str],
        start_time: datetime,
        end_time: datetime
    ) -> AsyncIterator[TickData]:
        """Fetch tick-level data (optional implementation)."""
        raise NotImplementedError(f"{self.name} does not support tick data")
    
    async def stream_tick_data(
        self,
        symbols: List[str]
    ) -> AsyncIterator[TickData]:
        """Stream real-time tick data (optional implementation)."""
        raise NotImplementedError(f"{self.name} does not support tick streaming")
    
    async def get_order_book(
        self,
        symbols: List[str]
    ) -> AsyncIterator[OrderBookData]:
        """Get order book snapshots (optional implementation)."""
        raise NotImplementedError(f"{self.name} does not support order book data")
    
    async def _rate_limit(self):
        """Implement rate limiting."""
        async with self._rate_limiter:
            current_time = asyncio.get_event_loop().time()
            
            # Reset counter every minute
            if current_time - self._last_request_time > 60:
                self._request_count = 0
                self._last_request_time = current_time
            
            # Check rate limit
            if self._request_count >= self.settings.max_requests_per_minute:
                sleep_time = 60 - (current_time - self._last_request_time)
                if sleep_time > 0:
                    self.logger.warning(f"Rate limit reached, sleeping for {sleep_time:.2f}s")
                    metrics.rate_limit_hits.labels(provider=self.name).inc()
                    await asyncio.sleep(sleep_time)
                    self._request_count = 0
                    self._last_request_time = asyncio.get_event_loop().time()
            
            self._request_count += 1
    
    def _validate_symbols(self, symbols: List[str]) -> List[str]:
        """Validate and normalize symbols."""
        validated = []
        for symbol in symbols:
            try:
                # Use centralized normalization
                clean_symbol = DataNormalizer.normalize_symbol(symbol)
                validated.append(clean_symbol)
            except ValueError as e:
                self.logger.warning(f"Invalid symbol {symbol}: {e}")
        return validated
    
    def _parse_interval(self, interval: str) -> Dict[str, Any]:
        """Parse interval string to provider-specific format."""
        interval_map = {
            "1min": {"minutes": 1, "label": "1min"},
            "5min": {"minutes": 5, "label": "5min"},
            "15min": {"minutes": 15, "label": "15min"},
            "30min": {"minutes": 30, "label": "30min"},
            "1hour": {"minutes": 60, "label": "1hour"},
            "4hour": {"minutes": 240, "label": "4hour"},
            "1day": {"minutes": 1440, "label": "1day"},
        }
        
        return interval_map.get(interval, interval_map["1min"])
    
    def _normalize_timestamp(self, ts, interval: str = "1min") -> datetime:
        """Helper method for timestamp normalization."""
        return DataNormalizer.normalize_timestamp(ts, interval)
    
    def _normalize_price(self, price) -> float:
        """Helper method for price normalization."""
        return DataNormalizer.normalize_price(price)
    
    def _normalize_volume(self, volume) -> int:
        """Helper method for volume normalization."""
        return DataNormalizer.normalize_volume(volume)