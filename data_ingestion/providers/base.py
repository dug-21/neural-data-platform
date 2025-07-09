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
        pass
    
    @abstractmethod
    async def disconnect(self):
        """Clean up provider connection."""
        pass
    
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
                    await asyncio.sleep(sleep_time)
                    self._request_count = 0
                    self._last_request_time = asyncio.get_event_loop().time()
            
            self._request_count += 1
            # Rate limiting doesn't need to track metrics - actual API calls will be tracked
    
    def _validate_symbols(self, symbols: List[str]) -> List[str]:
        """Validate and normalize symbols."""
        validated = []
        for symbol in symbols:
            # Basic validation - uppercase and alphanumeric
            clean_symbol = symbol.upper().strip()
            if clean_symbol and clean_symbol.replace("-", "").replace(".", "").isalnum():
                validated.append(clean_symbol)
            else:
                self.logger.warning(f"Invalid symbol: {symbol}")
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