"""Test suite for FRED (Federal Reserve Economic Data) provider."""
import pytest
from unittest.mock import Mock, patch, AsyncMock, MagicMock
from datetime import datetime, timedelta
import aiohttp
import json

from data_ingestion.providers.fred import FREDProvider
from data_ingestion.providers.base import MarketData


class TestFREDProvider:
    """Test cases for FRED provider implementation."""
    
    @pytest.fixture
    def mock_settings(self):
        """Mock settings for testing."""
        settings = Mock()
        settings.fred_api_key = "test_api_key"
        settings.max_concurrent_requests = 5
        settings.max_requests_per_minute = 120
        return settings
    
    @pytest.fixture
    def provider(self, mock_settings):
        """Create FRED provider instance with mocked settings."""
        with patch('data_ingestion.providers.base.get_settings', return_value=mock_settings):
            with patch('data_ingestion.providers.fred.get_settings', return_value=mock_settings):
                return FREDProvider()
    
    @pytest.mark.asyncio
    async def test_provider_initialization(self, provider):
        """Test provider initializes correctly."""
        assert provider.name == "FRED"
        assert provider.base_url == "https://api.stlouisfed.org/fred"
        assert not provider._connected
    
    @pytest.mark.asyncio
    async def test_connect_success(self, provider):
        """Test successful connection to FRED API."""
        # Mock the session creation
        mock_session = AsyncMock(spec=aiohttp.ClientSession)
        
        with patch('aiohttp.ClientSession', return_value=mock_session):
            await provider.connect()
            
            assert provider._connected is True
            assert provider._session == mock_session
    
    @pytest.mark.asyncio
    async def test_disconnect(self, provider):
        """Test provider disconnection."""
        # Setup connected provider
        mock_session = AsyncMock(spec=aiohttp.ClientSession)
        provider._session = mock_session
        provider._connected = True
        
        await provider.disconnect()
        
        mock_session.close.assert_called_once()
        assert provider._connected is False
        assert provider._session is None
    
    @pytest.mark.asyncio
    async def test_get_series_success(self, provider):
        """Test successful series data retrieval."""
        # Mock response data
        mock_response_data = {
            "observations": [
                {
                    "date": "2024-01-01",
                    "value": "3.5"
                },
                {
                    "date": "2024-01-02", 
                    "value": "3.6"
                }
            ]
        }
        
        # Setup mocked session and response
        mock_response = AsyncMock()
        mock_response.status = 200
        mock_response.json = AsyncMock(return_value=mock_response_data)
        
        # Create a proper async context manager mock
        async def mock_get(*args, **kwargs):
            return mock_response
        
        mock_response.__aenter__ = AsyncMock(return_value=mock_response)
        mock_response.__aexit__ = AsyncMock(return_value=None)
        
        provider._session = AsyncMock()
        provider._session.get = MagicMock(return_value=mock_response)
        provider._connected = True
        
        # Test parameters
        series_id = "DGS10"  # 10-Year Treasury Rate
        start_date = datetime(2024, 1, 1)
        end_date = datetime(2024, 1, 2)
        
        # Call method
        data_points = []
        async for data in provider.get_series(series_id, start_date, end_date):
            data_points.append(data)
        
        # Assertions
        assert len(data_points) == 2
        assert data_points[0].symbol == "DGS10"
        assert data_points[0].close == 3.5
        assert data_points[1].close == 3.6
        
        # Verify API call
        provider._session.get.assert_called_once()
        call_args = provider._session.get.call_args
        assert "series/observations" in call_args[0][0]
        assert call_args[1]["params"]["series_id"] == "DGS10"
    
    @pytest.mark.asyncio
    async def test_get_series_api_error(self, provider):
        """Test handling of API errors."""
        # Setup error response
        mock_response = AsyncMock()
        mock_response.status = 400
        mock_response.text = AsyncMock(return_value="Invalid API key")
        
        # Make it an async context manager
        mock_response.__aenter__ = AsyncMock(return_value=mock_response)
        mock_response.__aexit__ = AsyncMock(return_value=None)
        
        provider._session = AsyncMock()
        provider._session.get = MagicMock(return_value=mock_response)
        provider._connected = True
        
        # Test that exception is raised
        with pytest.raises(Exception) as exc_info:
            async for _ in provider.get_series("INVALID", datetime.now(), datetime.now()):
                pass
        
        assert "FRED API error" in str(exc_info.value)
    
    @pytest.mark.asyncio
    async def test_rate_limiting(self, provider):
        """Test rate limiting functionality."""
        # This should be inherited from base class
        provider._request_count = 119  # Just under limit
        provider._last_request_time = 0
        
        # Mock time
        with patch('asyncio.get_event_loop') as mock_loop:
            mock_loop.return_value.time.return_value = 30  # 30 seconds elapsed
            
            await provider._rate_limit()
            
            assert provider._request_count == 120
    
    @pytest.mark.asyncio
    async def test_get_multiple_series(self, provider):
        """Test fetching multiple economic series."""
        series_ids = ["DGS10", "DFF", "UNRATE"]  # Treasury, Fed Funds, Unemployment
        
        # Mock responses for each series
        mock_responses = {
            "DGS10": {"observations": [{"date": "2024-01-01", "value": "3.5"}]},
            "DFF": {"observations": [{"date": "2024-01-01", "value": "5.33"}]},
            "UNRATE": {"observations": [{"date": "2024-01-01", "value": "3.7"}]}
        }
        
        # Setup mock to return different responses based on series_id
        def create_mock_response(series_id):
            mock_resp = AsyncMock()
            mock_resp.status = 200
            mock_resp.json = AsyncMock(return_value=mock_responses[series_id])
            mock_resp.__aenter__ = AsyncMock(return_value=mock_resp)
            mock_resp.__aexit__ = AsyncMock(return_value=None)
            return mock_resp
        
        provider._session = AsyncMock()
        provider._session.get = MagicMock(side_effect=lambda url, **kwargs: create_mock_response(kwargs["params"]["series_id"]))
        provider._connected = True
        
        # Fetch all series
        results = {}
        for series_id in series_ids:
            data_points = []
            async for data in provider.get_series(series_id, datetime.now(), datetime.now()):
                data_points.append(data)
            results[series_id] = data_points
        
        # Verify we got data for all series
        assert len(results) == 3
        assert results["DGS10"][0].close == 3.5
        assert results["DFF"][0].close == 5.33
        assert results["UNRATE"][0].close == 3.7
    
    @pytest.mark.asyncio
    async def test_search_series(self, provider):
        """Test searching for economic series."""
        mock_response_data = {
            "seriess": [
                {
                    "id": "DGS10",
                    "title": "Market Yield on U.S. Treasury Securities at 10-Year Constant Maturity",
                    "units": "Percent"
                },
                {
                    "id": "DGS30", 
                    "title": "Market Yield on U.S. Treasury Securities at 30-Year Constant Maturity",
                    "units": "Percent"
                }
            ]
        }
        
        mock_response = AsyncMock()
        mock_response.status = 200
        mock_response.json = AsyncMock(return_value=mock_response_data)
        mock_response.__aenter__ = AsyncMock(return_value=mock_response)
        mock_response.__aexit__ = AsyncMock(return_value=None)
        
        provider._session = AsyncMock()
        provider._session.get = MagicMock(return_value=mock_response)
        provider._connected = True
        
        # Search for treasury yields
        results = await provider.search_series("treasury yield")
        
        assert len(results) == 2
        assert results[0]["id"] == "DGS10"
        assert "10-Year" in results[0]["title"]
    
    @pytest.mark.asyncio
    async def test_connection_required(self, provider):
        """Test that methods fail when not connected."""
        provider._connected = False
        
        with pytest.raises(RuntimeError) as exc_info:
            async for _ in provider.get_series("DGS10", datetime.now(), datetime.now()):
                pass
        
        assert "Not connected" in str(exc_info.value)
    
    @pytest.mark.asyncio
    async def test_invalid_series_id(self, provider):
        """Test handling of invalid series IDs."""
        mock_response_data = {
            "observations": []  # Empty observations for invalid series
        }
        
        mock_response = AsyncMock()
        mock_response.status = 200
        mock_response.json = AsyncMock(return_value=mock_response_data)
        mock_response.__aenter__ = AsyncMock(return_value=mock_response)
        mock_response.__aexit__ = AsyncMock(return_value=None)
        
        provider._session = AsyncMock()
        provider._session.get = MagicMock(return_value=mock_response)
        provider._connected = True
        
        data_points = []
        async for data in provider.get_series("INVALID_SERIES", datetime.now(), datetime.now()):
            data_points.append(data)
        
        assert len(data_points) == 0  # No data returned for invalid series