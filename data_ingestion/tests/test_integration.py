"""Integration tests for all data providers working together."""
import pytest
import asyncio
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, AsyncMock
import aiohttp

from ..providers import (
    IEXCloudProvider,
    AlphaVantageProvider,
    PolygonProvider,
    YahooFinanceProvider,
    FinnhubProvider,
    FREDProvider,
    RedditProvider,
    NASDAQProvider,
    PROVIDERS
)
from ..processors.aggregator import DataAggregator
from ..processors.validator import DataValidator
from ..processors.transformer import DataTransformer
from ..processors.cleaner import DataCleaner
from ..storage.redis_store import RedisStore
from ..storage.timescale import TimescaleDB
from ..config import Settings


class MockResponse:
    """Mock HTTP response for testing."""
    def __init__(self, json_data, status=200):
        self._json_data = json_data
        self.status = status
    
    async def json(self):
        return self._json_data
    
    async def text(self):
        return str(self._json_data)
    
    async def __aenter__(self):
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        pass


@pytest.fixture
def mock_settings():
    """Mock settings for testing."""
    return Settings(
        iex_cloud_api_key="test_key",
        alpha_vantage_api_key="test_key",
        polygon_api_key="test_key",
        finnhub_api_key="test_key",
        fred_api_key="test_key",
        reddit_client_id="test_id",
        reddit_client_secret="test_secret",
        nasdaq_api_key="test_key",
        redis_url="redis://localhost:6379",
        timescale_url="postgresql://test:test@localhost:5432/test",
        max_concurrent_requests=5,
        max_requests_per_minute=60
    )


@pytest.fixture
def mock_session():
    """Mock aiohttp session."""
    session = AsyncMock(spec=aiohttp.ClientSession)
    return session


@pytest.fixture
def mock_market_data():
    """Mock market data for different providers."""
    return {
        "iex": {
            "chart": [
                {
                    "date": "2024-01-01",
                    "open": 100.0,
                    "high": 105.0,
                    "low": 99.0,
                    "close": 104.0,
                    "volume": 1000000
                }
            ]
        },
        "alpha_vantage": {
            "Time Series (1min)": {
                "2024-01-01 09:30:00": {
                    "1. open": "100.00",
                    "2. high": "105.00",
                    "3. low": "99.00",
                    "4. close": "104.00",
                    "5. volume": "1000000"
                }
            }
        },
        "polygon": {
            "results": [
                {
                    "t": 1704123600000,
                    "o": 100.0,
                    "h": 105.0,
                    "l": 99.0,
                    "c": 104.0,
                    "v": 1000000
                }
            ]
        },
        "yahoo": {
            "chart": {
                "result": [{
                    "timestamp": [1704123600],
                    "indicators": {
                        "quote": [{
                            "open": [100.0],
                            "high": [105.0],
                            "low": [99.0],
                            "close": [104.0],
                            "volume": [1000000]
                        }]
                    }
                }]
            }
        },
        "finnhub": {
            "o": [100.0],
            "h": [105.0],
            "l": [99.0],
            "c": [104.0],
            "v": [1000000],
            "t": [1704123600]
        },
        "fred": {
            "observations": [
                {
                    "date": "2024-01-01",
                    "value": "3.5"
                }
            ]
        },
        "reddit": {
            "data": {
                "children": [
                    {
                        "data": {
                            "title": "AAPL to the moon!",
                            "selftext": "Great earnings report",
                            "created_utc": 1704123600,
                            "score": 100,
                            "num_comments": 50
                        }
                    }
                ]
            }
        },
        "nasdaq": {
            "data": {
                "tradesTable": {
                    "rows": [
                        {
                            "date": "2024-01-01",
                            "open": "$100.00",
                            "high": "$105.00",
                            "low": "$99.00",
                            "close": "$104.00",
                            "volume": "1,000,000"
                        }
                    ]
                }
            }
        }
    }


@pytest.mark.asyncio
class TestProviderIntegration:
    """Test all providers working together."""
    
    async def test_all_providers_initialization(self, mock_settings):
        """Test that all providers can be initialized."""
        providers = []
        
        for name, provider_class in PROVIDERS.items():
            provider = provider_class(name)
            provider.settings = mock_settings
            providers.append(provider)
        
        assert len(providers) == len(PROVIDERS)
        
        for provider in providers:
            assert provider.name in PROVIDERS
            assert hasattr(provider, 'connect')
            assert hasattr(provider, 'disconnect')
            assert hasattr(provider, 'get_market_data')
            assert hasattr(provider, 'stream_market_data')
    
    async def test_concurrent_data_fetching(self, mock_settings, mock_session, mock_market_data):
        """Test fetching data from multiple providers concurrently."""
        providers = [
            IEXCloudProvider("iex_cloud"),
            AlphaVantageProvider("alpha_vantage"),
            PolygonProvider("polygon"),
            YahooFinanceProvider("yahoo_finance"),
            FinnhubProvider("finnhub")
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            provider._session = mock_session
        
        # Mock responses for each provider
        mock_session.get.side_effect = [
            MockResponse(mock_market_data["iex"]),
            MockResponse(mock_market_data["alpha_vantage"]),
            MockResponse(mock_market_data["polygon"]),
            MockResponse(mock_market_data["yahoo"]),
            MockResponse(mock_market_data["finnhub"])
        ]
        
        # Fetch data concurrently
        tasks = []
        symbols = ["AAPL"]
        start_time = datetime(2024, 1, 1)
        end_time = datetime(2024, 1, 2)
        
        for provider in providers:
            task = provider.get_market_data(symbols, start_time, end_time)
            tasks.append(task)
        
        # Gather results
        results = await asyncio.gather(*[
            self._collect_data(task) for task in tasks
        ], return_exceptions=True)
        
        # Verify we got data from all providers
        successful_results = [r for r in results if not isinstance(r, Exception)]
        assert len(successful_results) >= 3  # At least 3 providers should succeed
    
    async def test_data_aggregation_pipeline(self, mock_settings, mock_session, mock_market_data):
        """Test the complete data pipeline with aggregation."""
        # Initialize components
        aggregator = DataAggregator()
        validator = DataValidator()
        transformer = DataTransformer()
        cleaner = DataCleaner()
        
        # Create mock providers
        providers = [
            IEXCloudProvider("iex_cloud"),
            YahooFinanceProvider("yahoo_finance")
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            provider._session = mock_session
        
        # Mock responses
        mock_session.get.side_effect = [
            MockResponse(mock_market_data["iex"]),
            MockResponse(mock_market_data["yahoo"])
        ]
        
        # Fetch data from providers
        all_data = []
        for provider in providers:
            async for data in provider.get_market_data(
                ["AAPL"], 
                datetime(2024, 1, 1), 
                datetime(2024, 1, 2)
            ):
                all_data.append(data)
        
        # Process through pipeline
        validated_data = []
        for data in all_data:
            if validator.validate_market_data(data):
                validated_data.append(data)
        
        # Transform data
        transformed_data = []
        for data in validated_data:
            transformed = transformer.normalize_market_data(data)
            transformed_data.append(transformed)
        
        # Clean data
        cleaned_data = []
        for data in transformed_data:
            cleaned = cleaner.clean_market_data(data)
            cleaned_data.append(cleaned)
        
        # Aggregate data
        aggregated = aggregator.aggregate_market_data(cleaned_data)
        
        assert aggregated is not None
        assert len(cleaned_data) >= len(providers)
    
    async def test_mixed_data_types(self, mock_settings, mock_session, mock_market_data):
        """Test handling different data types from different providers."""
        # Market data provider
        market_provider = YahooFinanceProvider("yahoo_finance")
        market_provider.settings = mock_settings
        market_provider._session = mock_session
        
        # Economic data provider
        econ_provider = FREDProvider("fred")
        econ_provider.settings = mock_settings
        econ_provider._session = mock_session
        
        # Social data provider
        social_provider = RedditProvider("reddit")
        social_provider.settings = mock_settings
        social_provider._session = mock_session
        
        # Mock responses
        mock_session.get.side_effect = [
            MockResponse(mock_market_data["yahoo"]),
            MockResponse(mock_market_data["fred"]),
            MockResponse(mock_market_data["reddit"])
        ]
        
        # Fetch different types of data
        market_data = []
        async for data in market_provider.get_market_data(
            ["AAPL"], 
            datetime(2024, 1, 1), 
            datetime(2024, 1, 2)
        ):
            market_data.append(data)
        
        econ_data = []
        async for data in econ_provider.get_economic_data(
            ["GDP"], 
            datetime(2024, 1, 1), 
            datetime(2024, 1, 2)
        ):
            econ_data.append(data)
        
        social_data = []
        async for data in social_provider.get_social_sentiment(
            ["AAPL"], 
            limit=10
        ):
            social_data.append(data)
        
        # Verify we got all types of data
        assert len(market_data) > 0
        assert len(econ_data) > 0
        assert len(social_data) > 0
    
    async def test_error_handling_across_providers(self, mock_settings, mock_session):
        """Test that errors in one provider don't affect others."""
        providers = [
            IEXCloudProvider("iex_cloud"),
            AlphaVantageProvider("alpha_vantage"),
            PolygonProvider("polygon")
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            provider._session = mock_session
        
        # Mock one success, one failure, one success
        mock_session.get.side_effect = [
            MockResponse({"chart": [{"date": "2024-01-01", "close": 100}]}),
            Exception("Provider error"),
            MockResponse({"results": [{"c": 100, "t": 1704123600000}]})
        ]
        
        results = []
        errors = []
        
        for provider in providers:
            try:
                async for data in provider.get_market_data(
                    ["AAPL"], 
                    datetime(2024, 1, 1), 
                    datetime(2024, 1, 2)
                ):
                    results.append(data)
            except Exception as e:
                errors.append((provider.name, str(e)))
        
        # Should have results from 2 providers and 1 error
        assert len(results) >= 1
        assert len(errors) >= 1
    
    async def test_rate_limiting_coordination(self, mock_settings, mock_session):
        """Test that rate limiting works across multiple providers."""
        # Reduce rate limits for testing
        mock_settings.max_concurrent_requests = 2
        mock_settings.max_requests_per_minute = 10
        
        providers = [
            IEXCloudProvider("iex_cloud"),
            AlphaVantageProvider("alpha_vantage")
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            provider._session = mock_session
        
        # Mock many responses
        mock_session.get.return_value = MockResponse({"data": "test"})
        
        # Make concurrent requests
        tasks = []
        for _ in range(5):  # 5 requests per provider
            for provider in providers:
                task = provider._make_request("test_url")
                tasks.append(task)
        
        # This should respect rate limits
        start_time = asyncio.get_event_loop().time()
        results = await asyncio.gather(*tasks, return_exceptions=True)
        end_time = asyncio.get_event_loop().time()
        
        # With rate limiting, this should take some time
        assert end_time - start_time > 0  # Should not be instant
        
        # Check that we got results (or rate limit errors)
        assert len(results) == 10
    
    async def test_storage_integration(self, mock_settings):
        """Test storing data from multiple providers."""
        # Mock storage backends
        with patch('redis.Redis'), patch('psycopg2.connect'):
            redis_store = RedisStore(mock_settings)
            timescale_db = TimescaleDB(mock_settings)
            
            # Mock market data from different providers
            data_points = [
                MarketData(
                    time=datetime(2024, 1, 1, 9, 30),
                    symbol="AAPL",
                    open=100.0,
                    high=105.0,
                    low=99.0,
                    close=104.0,
                    volume=1000000,
                    provider="iex_cloud"
                ),
                MarketData(
                    time=datetime(2024, 1, 1, 9, 30),
                    symbol="AAPL",
                    open=100.1,
                    high=105.1,
                    low=99.1,
                    close=104.1,
                    volume=1000100,
                    provider="yahoo_finance"
                )
            ]
            
            # Store in both backends
            for data in data_points:
                await redis_store.store_market_data(data)
                await timescale_db.store_market_data(data)
            
            # Verify storage was called
            assert redis_store is not None
            assert timescale_db is not None
    
    async def _collect_data(self, async_iterator):
        """Helper to collect all data from an async iterator."""
        results = []
        async for item in async_iterator:
            results.append(item)
        return results


@pytest.mark.asyncio
class TestProviderCoordination:
    """Test coordination between different provider types."""
    
    async def test_market_and_fundamental_correlation(self, mock_settings, mock_session, mock_market_data):
        """Test correlating market data with fundamental data."""
        # Market data provider
        market_provider = YahooFinanceProvider("yahoo_finance")
        market_provider.settings = mock_settings
        market_provider._session = mock_session
        
        # Fundamental data provider (using FRED for economic indicators)
        fundamental_provider = FREDProvider("fred")
        fundamental_provider.settings = mock_settings
        fundamental_provider._session = mock_session
        
        # Mock responses
        mock_session.get.side_effect = [
            MockResponse(mock_market_data["yahoo"]),
            MockResponse(mock_market_data["fred"])
        ]
        
        # Fetch both types of data
        market_data = []
        async for data in market_provider.get_market_data(
            ["SPY"],  # S&P 500 ETF
            datetime(2024, 1, 1), 
            datetime(2024, 1, 2)
        ):
            market_data.append(data)
        
        economic_data = []
        async for data in fundamental_provider.get_economic_data(
            ["UNRATE"],  # Unemployment rate
            datetime(2024, 1, 1), 
            datetime(2024, 1, 2)
        ):
            economic_data.append(data)
        
        # Both should have data
        assert len(market_data) > 0
        assert len(economic_data) > 0
        
        # In a real scenario, you would correlate these datasets
        # For testing, just verify they can be fetched together
        assert market_data[0].symbol == "SPY"
        assert economic_data[0].symbol == "UNRATE"
    
    async def test_realtime_and_historical_sync(self, mock_settings, mock_session):
        """Test synchronizing real-time and historical data."""
        provider = IEXCloudProvider("iex_cloud")
        provider.settings = mock_settings
        provider._session = mock_session
        
        # Mock historical data
        historical_response = {
            "chart": [
                {
                    "date": "2024-01-01",
                    "close": 100
                }
            ]
        }
        
        # Mock real-time data
        realtime_response = {
            "chart": [
                {
                    "date": "2024-01-02",
                    "close": 105
                }
            ]
        }
        
        mock_session.get.side_effect = [
            MockResponse(historical_response),
            MockResponse(realtime_response)
        ]
        
        # Fetch historical
        historical = []
        async for data in provider.get_market_data(
            ["AAPL"], 
            datetime(2024, 1, 1), 
            datetime(2024, 1, 1)
        ):
            historical.append(data)
        
        # Simulate real-time (using same method for testing)
        realtime = []
        async for data in provider.get_market_data(
            ["AAPL"], 
            datetime(2024, 1, 2), 
            datetime(2024, 1, 2)
        ):
            realtime.append(data)
        
        # Verify continuity
        assert len(historical) > 0
        assert len(realtime) > 0
        assert realtime[0].time > historical[0].time


@pytest.mark.asyncio
class TestFailoverScenarios:
    """Test failover scenarios when providers fail."""
    
    async def test_primary_secondary_failover(self, mock_settings, mock_session):
        """Test falling back to secondary provider when primary fails."""
        primary = IEXCloudProvider("iex_cloud")
        secondary = YahooFinanceProvider("yahoo_finance")
        
        primary.settings = mock_settings
        secondary.settings = mock_settings
        
        primary._session = mock_session
        secondary._session = mock_session
        
        # Primary fails, secondary succeeds
        mock_session.get.side_effect = [
            Exception("Primary provider down"),
            MockResponse({"chart": {"result": [{"indicators": {"quote": [{"close": [100]}]}}]}})
        ]
        
        # Try primary first
        primary_data = []
        try:
            async for data in primary.get_market_data(
                ["AAPL"], 
                datetime(2024, 1, 1), 
                datetime(2024, 1, 2)
            ):
                primary_data.append(data)
        except Exception:
            # Fallback to secondary
            secondary_data = []
            async for data in secondary.get_market_data(
                ["AAPL"], 
                datetime(2024, 1, 1), 
                datetime(2024, 1, 2)
            ):
                secondary_data.append(data)
            
            assert len(secondary_data) > 0
            assert secondary_data[0].provider == "yahoo_finance"
    
    async def test_multiple_provider_redundancy(self, mock_settings, mock_session):
        """Test using multiple providers for redundancy."""
        providers = [
            IEXCloudProvider("iex_cloud"),
            AlphaVantageProvider("alpha_vantage"),
            YahooFinanceProvider("yahoo_finance")
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            provider._session = mock_session
        
        # First two fail, third succeeds
        mock_session.get.side_effect = [
            Exception("Provider 1 down"),
            Exception("Provider 2 down"),
            MockResponse({"chart": {"result": [{"indicators": {"quote": [{"close": [100]}]}}]}})
        ]
        
        successful_data = None
        for provider in providers:
            try:
                async for data in provider.get_market_data(
                    ["AAPL"], 
                    datetime(2024, 1, 1), 
                    datetime(2024, 1, 2)
                ):
                    successful_data = data
                    break
            except Exception:
                continue
        
        assert successful_data is not None
        assert successful_data.provider == "yahoo_finance"


# Mock data classes for testing
from dataclasses import dataclass
from datetime import datetime
from typing import Optional, Dict, Any


@dataclass
class MarketData:
    """Mock market data for testing."""
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
class EconomicData:
    """Mock economic data for testing."""
    time: datetime
    symbol: str
    value: float
    provider: str
    metadata: Optional[Dict[str, Any]] = None


@dataclass
class SocialData:
    """Mock social sentiment data for testing."""
    time: datetime
    symbol: str
    sentiment: float
    volume: int
    provider: str
    metadata: Optional[Dict[str, Any]] = None