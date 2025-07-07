"""Tests for NASDAQ/Quandl data provider."""
import pytest
import asyncio
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, AsyncMock
import aiohttp

from data_ingestion.providers.nasdaq import NASDAQProvider
from data_ingestion.providers.base import MarketData


class TestNASDAQProvider:
    """Test suite for NASDAQ/Quandl provider."""
    
    @pytest.fixture
    def provider(self):
        """Create a NASDAQ provider instance."""
        with patch('data_ingestion.providers.nasdaq.get_settings') as mock_settings:
            mock_settings.return_value = Mock(
                quandl_api_key="test_api_key",
                max_concurrent_requests=5,
                max_requests_per_minute=60
            )
            return NASDAQProvider()
    
    @pytest.fixture
    def mock_response_data(self):
        """Mock response data from Quandl API."""
        return {
            "dataset": {
                "column_names": ["Date", "Open", "High", "Low", "Close", "Volume"],
                "data": [
                    ["2023-01-01", 100.0, 105.0, 99.0, 103.0, 1000000],
                    ["2023-01-02", 103.0, 107.0, 102.0, 106.0, 1200000],
                    ["2023-01-03", 106.0, 108.0, 104.0, 107.0, 900000]
                ],
                "name": "Apple Inc. (AAPL) Stock Prices",
                "description": "End of day stock prices for Apple Inc.",
                "database_code": "WIKI",
                "dataset_code": "AAPL",
                "frequency": "daily"
            }
        }
    
    @pytest.fixture
    def mock_economic_data(self):
        """Mock economic indicator data."""
        return {
            "dataset": {
                "column_names": ["Date", "Value"],
                "data": [
                    ["2023-01-01", 3.5],
                    ["2023-02-01", 3.4],
                    ["2023-03-01", 3.5]
                ],
                "name": "Unemployment Rate",
                "description": "Civilian Unemployment Rate, Seasonally Adjusted",
                "frequency": "monthly",
                "units": "Percent"
            }
        }
    
    @pytest.fixture
    def mock_futures_data(self):
        """Mock futures contract data."""
        return {
            "dataset": {
                "column_names": ["Date", "Open", "High", "Low", "Settle", "Volume", "Open Interest"],
                "data": [
                    ["2023-01-01", 75.50, 76.20, 74.80, 75.90, 150000, 280000],
                    ["2023-01-02", 75.90, 77.00, 75.50, 76.80, 180000, 285000]
                ],
                "database_code": "CHRIS",
                "dataset_code": "CME_CL1"
            }
        }
    
    @pytest.fixture
    def mock_search_results(self):
        """Mock dataset search results."""
        return {
            "datasets": [
                {
                    "id": 1234,
                    "database_code": "WIKI",
                    "dataset_code": "AAPL",
                    "name": "Apple Inc. (AAPL) Stock Prices",
                    "description": "End of day stock prices",
                    "refreshed_at": "2023-12-15T00:00:00Z",
                    "newest_available_date": "2023-12-14",
                    "oldest_available_date": "1980-12-12",
                    "column_names": ["Date", "Open", "High", "Low", "Close", "Volume"],
                    "frequency": "daily",
                    "type": "Time Series",
                    "premium": False
                }
            ]
        }
    
    @pytest.mark.asyncio
    async def test_init(self, provider):
        """Test provider initialization."""
        assert provider.name == "nasdaq"
        assert provider.api_key == "test_api_key"
        assert provider.BASE_URL == "https://www.quandl.com/api/v3"
        assert provider._daily_limit == 50000
        assert provider._daily_calls == 0
    
    @pytest.mark.asyncio
    async def test_connect_success(self, provider):
        """Test successful connection."""
        await provider.connect()
        assert provider._connected is True
        assert provider.session is not None
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_connect_no_api_key(self):
        """Test connection without API key."""
        with patch('data_ingestion.providers.nasdaq.get_settings') as mock_settings:
            mock_settings.return_value = Mock(quandl_api_key=None)
            provider = NASDAQProvider()
            
            with pytest.raises(ValueError, match="Quandl API key is required"):
                await provider.connect()
    
    @pytest.mark.asyncio
    async def test_get_market_data(self, provider, mock_response_data):
        """Test fetching market data."""
        await provider.connect()
        
        with patch.object(provider, '_fetch_data', return_value=mock_response_data):
            start_time = datetime(2023, 1, 1)
            end_time = datetime(2023, 1, 3)
            
            data_points = []
            async for data in provider.get_market_data(
                ["AAPL"], start_time, end_time, "1day"
            ):
                data_points.append(data)
            
            assert len(data_points) == 3
            
            # Check first data point
            first = data_points[0]
            assert isinstance(first, MarketData)
            assert first.symbol == "AAPL"
            assert first.open == 100.0
            assert first.high == 105.0
            assert first.low == 99.0
            assert first.close == 103.0
            assert first.volume == 1000000
            assert first.provider == "nasdaq"
            assert first.metadata["database"] == "WIKI"
            assert first.metadata["frequency"] == "daily"
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_economic_indicators(self, provider, mock_economic_data):
        """Test fetching economic indicators."""
        await provider.connect()
        
        with patch.object(provider, '_fetch_data', return_value=mock_economic_data):
            start_time = datetime(2023, 1, 1)
            end_time = datetime(2023, 3, 31)
            
            indicators = []
            async for indicator in provider.get_economic_indicators(
                ["UNRATE"], start_time, end_time
            ):
                indicators.append(indicator)
            
            assert len(indicators) == 3
            
            # Check first indicator
            first = indicators[0]
            assert first["indicator"] == "UNRATE"
            assert first["date"] == "2023-01-01"
            assert first["value"] == 3.5
            assert first["metadata"]["name"] == "Unemployment Rate"
            assert first["metadata"]["source"] == "FRED/Quandl"
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_futures_data(self, provider, mock_futures_data):
        """Test fetching futures data."""
        await provider.connect()
        
        with patch.object(provider, '_fetch_data', return_value=mock_futures_data):
            start_time = datetime(2023, 1, 1)
            end_time = datetime(2023, 1, 2)
            
            futures = []
            async for data in provider.get_futures_data(
                ["CME_CL1"], start_time, end_time
            ):
                futures.append(data)
            
            assert len(futures) == 2
            
            # Check first futures data
            first = futures[0]
            assert isinstance(first, MarketData)
            assert first.symbol == "CME_CL1"
            assert first.open == 75.50
            assert first.high == 76.20
            assert first.low == 74.80
            assert first.close == 75.90  # Uses settle price
            assert first.volume == 150000
            assert first.metadata["type"] == "futures"
            assert first.metadata["database"] == "CHRIS"
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_search_datasets(self, provider, mock_search_results):
        """Test searching for datasets."""
        await provider.connect()
        
        with patch.object(provider, '_fetch_data', return_value=mock_search_results):
            results = await provider.search_datasets("Apple", limit=10)
            
            assert len(results) == 1
            result = results[0]
            assert result["dataset_code"] == "AAPL"
            assert result["name"] == "Apple Inc. (AAPL) Stock Prices"
            assert result["database_code"] == "WIKI"
            assert result["premium"] is False
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_stream_market_data_not_supported(self, provider):
        """Test that streaming is not supported."""
        await provider.connect()
        
        with pytest.raises(NotImplementedError, match="Quandl does not support real-time streaming"):
            async for _ in provider.stream_market_data(["AAPL"]):
                pass
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_database_selection(self, provider):
        """Test database selection logic."""
        assert provider._get_database_for_symbol("AAPL") == "WIKI"
        assert provider._get_database_for_symbol("CME_CL1") == "CHRIS"
        assert provider._get_database_for_symbol("EURUSD") == "CURRFX"
        assert provider._get_database_for_symbol("GDP") == "FRED"
        assert provider._get_database_for_symbol("CPI") == "FRED"
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self, provider):
        """Test rate limiting behavior."""
        await provider.connect()
        
        # Mock response
        mock_response = AsyncMock()
        mock_response.status = 429
        mock_response.headers = {"Retry-After": "1"}
        
        with patch.object(provider.session, 'get', return_value=mock_response):
            with pytest.raises(Exception, match="Rate limit exceeded"):
                await provider._fetch_data("test_endpoint", {})
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_daily_limit_check(self, provider):
        """Test daily API call limit checking."""
        provider._daily_calls = 49999
        provider._last_reset = datetime.now()
        
        # Should not trigger limit
        await provider._check_daily_limit()
        
        # Set to limit
        provider._daily_calls = 50000
        
        # Mock sleep to avoid actual waiting
        with patch('asyncio.sleep', new_callable=AsyncMock):
            await provider._check_daily_limit()
            assert provider._daily_calls == 0
    
    @pytest.mark.asyncio
    async def test_error_handling(self, provider):
        """Test error handling in data fetching."""
        await provider.connect()
        
        # Test 404 response
        mock_response = AsyncMock()
        mock_response.status = 404
        
        with patch.object(provider.session, 'get', return_value=mock_response):
            result = await provider._fetch_data("test_endpoint", {})
            assert result is None
        
        # Test network error
        with patch.object(provider.session, 'get', side_effect=aiohttp.ClientError("Network error")):
            with pytest.raises(aiohttp.ClientError):
                await provider._fetch_data("test_endpoint", {})
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_get_metadata(self, provider):
        """Test fetching dataset metadata."""
        await provider.connect()
        
        mock_metadata = {
            "dataset": {
                "id": 1234,
                "database_code": "WIKI",
                "dataset_code": "AAPL",
                "name": "Apple Inc. Stock Prices",
                "description": "Historical stock prices"
            }
        }
        
        with patch.object(provider, '_fetch_data', return_value=mock_metadata):
            metadata = await provider.get_metadata("WIKI", "AAPL")
            assert metadata["dataset"]["dataset_code"] == "AAPL"
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_invalid_symbols(self, provider):
        """Test handling of invalid symbols."""
        await provider.connect()
        
        # Invalid symbols should be filtered out
        valid_symbols = provider._validate_symbols(["AAPL", "???", "MSFT", "123ABC", ""])
        assert valid_symbols == ["AAPL", "MSFT", "123ABC"]
        
        await provider.disconnect()
    
    @pytest.mark.asyncio
    async def test_parse_interval(self, provider):
        """Test interval parsing."""
        interval_info = provider._parse_interval("1day")
        assert interval_info["minutes"] == 1440
        assert interval_info["label"] == "1day"
        
        # Test default
        interval_info = provider._parse_interval("unknown")
        assert interval_info["label"] == "1min"


@pytest.mark.integration
class TestNASDAQProviderIntegration:
    """Integration tests for NASDAQ provider (requires API key)."""
    
    @pytest.mark.skipif(
        not pytest.config.getoption("--integration", default=False),
        reason="Integration tests disabled"
    )
    @pytest.mark.asyncio
    async def test_real_api_call(self):
        """Test actual API call to Quandl."""
        provider = NASDAQProvider()
        
        if not provider.api_key or provider.api_key == "test_api_key":
            pytest.skip("Real API key required for integration test")
        
        await provider.connect()
        
        try:
            # Search for Apple dataset
            results = await provider.search_datasets("Apple", limit=1)
            assert len(results) > 0
            
            # Get one day of data
            start = datetime.now() - timedelta(days=30)
            end = datetime.now() - timedelta(days=29)
            
            data_count = 0
            async for data in provider.get_market_data(["AAPL"], start, end):
                data_count += 1
                assert isinstance(data, MarketData)
                assert data.symbol == "AAPL"
            
            assert data_count > 0
            
        finally:
            await provider.disconnect()