"""Data providers for market data ingestion."""
from .base import BaseProvider, MarketData, TickData, OrderBookData, DataType
from .iex_cloud import IEXCloudProvider
from .alpha_vantage import AlphaVantageProvider
from .polygon import PolygonProvider
from .yahoo_finance import YahooFinanceProvider
from .finnhub import FinnhubProvider
from .fred import FREDProvider
from .reddit import RedditProvider
from .nasdaq import NASDAQProvider
from .newsapi import NewsAPIProvider, NewsArticle

# Provider registry
PROVIDERS = {
    "iex_cloud": IEXCloudProvider,
    "alpha_vantage": AlphaVantageProvider,
    "polygon": PolygonProvider,
    "yahoo_finance": YahooFinanceProvider,
    "finnhub": FinnhubProvider,
    "fred": FREDProvider,
    "reddit": RedditProvider,
    "nasdaq": NASDAQProvider,
    "newsapi": NewsAPIProvider
}

__all__ = [
    "BaseProvider",
    "MarketData",
    "TickData",
    "OrderBookData",
    "DataType",
    "IEXCloudProvider",
    "AlphaVantageProvider",
    "PolygonProvider",
    "YahooFinanceProvider",
    "FinnhubProvider",
    "FREDProvider",
    "RedditProvider",
    "NASDAQProvider",
    "NewsAPIProvider",
    "NewsArticle",
    "PROVIDERS"
]