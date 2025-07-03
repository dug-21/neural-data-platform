"""
Neural Trader Data Ingestion Service

A comprehensive Python data ingestion system for financial market data.
Supports multiple data providers, real-time streaming, and batch processing.
"""

from .providers import (
    BaseProvider,
    IEXCloudProvider,
    AlphaVantageProvider,
    PolygonProvider,
    YahooFinanceProvider,
    FinnhubProvider,
    PROVIDERS
)

from .processors import (
    DataCleaner,
    DataValidator,
    DataTransformer,
    DataAggregator
)

from .schedulers import (
    RealtimeCoordinator,
    BatchScheduler,
    StreamManager
)

from .storage import (
    TimescaleDB,
    RedisStore
)

from .config import get_settings

__version__ = "1.0.0"

__all__ = [
    # Providers
    "BaseProvider",
    "IEXCloudProvider",
    "AlphaVantageProvider",
    "PolygonProvider",
    "YahooFinanceProvider",
    "FinnhubProvider",
    "PROVIDERS",
    
    # Processors
    "DataCleaner",
    "DataValidator",
    "DataTransformer",
    "DataAggregator",
    
    # Schedulers
    "RealtimeCoordinator",
    "BatchScheduler",
    "StreamManager",
    
    # Storage
    "TimescaleDB",
    "RedisStore",
    
    # Config
    "get_settings"
]