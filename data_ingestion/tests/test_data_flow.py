"""Test data flow between providers and processing components."""
import pytest
import asyncio
from datetime import datetime, timedelta
from unittest.mock import Mock, patch, AsyncMock
from typing import List, Dict, Any
import json

from ..providers import PROVIDERS, BaseProvider
from ..processors.aggregator import DataAggregator
from ..processors.validator import DataValidator
from ..processors.transformer import DataTransformer
from ..processors.cleaner import DataCleaner
from ..storage.redis_store import RedisStore
from ..storage.timescale import TimescaleDB
from ..schedulers.batch_scheduler import BatchScheduler
from ..schedulers.realtime_coordinator import RealtimeCoordinator
from ..config import Settings
from .fixtures import MockDataFixtures


class MockProvider(BaseProvider):
    """Mock provider for testing data flow."""
    
    def __init__(self, name: str, mock_data: Dict[str, Any]):
        super().__init__(name)
        self.mock_data = mock_data
        self._connected = False
    
    async def connect(self):
        """Mock connection."""
        self._connected = True
    
    async def disconnect(self):
        """Mock disconnection."""
        self._connected = False
    
    async def get_market_data(self, symbols, start_time, end_time, interval="1min"):
        """Mock market data retrieval."""
        if not self._connected:
            raise Exception("Provider not connected")
        
        # Simulate some data based on mock_data
        from ..providers.base import MarketData
        
        for symbol in symbols:
            yield MarketData(
                time=start_time,
                symbol=symbol,
                open=100.0,
                high=105.0,
                low=99.0,
                close=104.0,
                volume=1000000,
                provider=self.name
            )
    
    async def stream_market_data(self, symbols):
        """Mock streaming market data."""
        if not self._connected:
            raise Exception("Provider not connected")
        
        from ..providers.base import MarketData
        
        for i in range(5):  # Send 5 updates
            for symbol in symbols:
                yield MarketData(
                    time=datetime.now(),
                    symbol=symbol,
                    open=100.0 + i,
                    high=105.0 + i,
                    low=99.0 + i,
                    close=104.0 + i,
                    volume=1000000 + i * 10000,
                    provider=self.name
                )
            await asyncio.sleep(0.1)  # Small delay between updates


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
        max_concurrent_requests=10,
        max_requests_per_minute=100
    )


@pytest.fixture
def mock_fixtures():
    """Mock data fixtures."""
    return MockDataFixtures()


@pytest.mark.asyncio
class TestDataFlow:
    """Test complete data flow from providers to storage."""
    
    async def test_full_pipeline_flow(self, mock_settings, mock_fixtures):
        """Test complete data flow through the entire pipeline."""
        # Setup components
        aggregator = DataAggregator()
        validator = DataValidator()
        transformer = DataTransformer()
        cleaner = DataCleaner()
        
        # Mock providers
        provider1 = MockProvider("provider1", mock_fixtures.get_iex_cloud_response())
        provider2 = MockProvider("provider2", mock_fixtures.get_yahoo_finance_response())
        
        provider1.settings = mock_settings
        provider2.settings = mock_settings
        
        # Connect providers
        await provider1.connect()
        await provider2.connect()
        
        # Step 1: Fetch data from multiple providers
        all_data = []
        symbols = ["AAPL", "GOOGL"]
        start_time = datetime(2024, 1, 1)
        end_time = datetime(2024, 1, 2)
        
        # Collect from provider 1
        async for data in provider1.get_market_data(symbols, start_time, end_time):
            all_data.append(data)
        
        # Collect from provider 2
        async for data in provider2.get_market_data(symbols, start_time, end_time):
            all_data.append(data)
        
        assert len(all_data) == 4  # 2 symbols * 2 providers
        
        # Step 2: Validate data
        validated_data = []
        for data in all_data:
            if validator.validate_market_data(data):
                validated_data.append(data)
        
        assert len(validated_data) == len(all_data)  # All should be valid
        
        # Step 3: Transform data
        transformed_data = []
        for data in validated_data:
            transformed = transformer.normalize_market_data(data)
            transformed_data.append(transformed)
        
        assert len(transformed_data) == len(validated_data)
        
        # Step 4: Clean data
        cleaned_data = []
        for data in transformed_data:
            cleaned = cleaner.clean_market_data(data)
            cleaned_data.append(cleaned)
        
        assert len(cleaned_data) == len(transformed_data)
        
        # Step 5: Aggregate data
        aggregated = aggregator.aggregate_market_data(cleaned_data)
        
        assert aggregated is not None
        assert len(aggregated) <= len(cleaned_data)  # May be fewer after aggregation
        
        # Cleanup
        await provider1.disconnect()
        await provider2.disconnect()
    
    async def test_realtime_data_flow(self, mock_settings, mock_fixtures):
        """Test real-time data flow with streaming."""
        # Setup components
        coordinator = RealtimeCoordinator(mock_settings)
        validator = DataValidator()
        
        # Mock provider
        provider = MockProvider("realtime_provider", mock_fixtures.get_streaming_data())
        provider.settings = mock_settings
        
        await provider.connect()
        
        # Start streaming
        streamed_data = []
        async for data in provider.stream_market_data(["AAPL"]):
            # Validate each piece of data as it comes in
            if validator.validate_market_data(data):
                streamed_data.append(data)
            
            # Stop after 3 updates for testing
            if len(streamed_data) >= 3:
                break
        
        assert len(streamed_data) == 3
        assert all(data.symbol == "AAPL" for data in streamed_data)
        
        # Verify data is in chronological order
        timestamps = [data.time for data in streamed_data]
        assert timestamps == sorted(timestamps)
        
        await provider.disconnect()
    
    async def test_batch_processing_flow(self, mock_settings, mock_fixtures):
        """Test batch processing workflow."""
        # Setup batch scheduler
        scheduler = BatchScheduler(mock_settings)
        
        # Mock multiple providers
        providers = [
            MockProvider("batch_provider1", mock_fixtures.get_iex_cloud_response()),
            MockProvider("batch_provider2", mock_fixtures.get_yahoo_finance_response()),
            MockProvider("batch_provider3", mock_fixtures.get_polygon_response())
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            await provider.connect()
        
        # Setup batch job
        symbols = ["AAPL", "GOOGL", "MSFT"]
        start_time = datetime(2024, 1, 1)
        end_time = datetime(2024, 1, 2)
        
        # Collect data from all providers
        batch_data = []
        for provider in providers:
            async for data in provider.get_market_data(symbols, start_time, end_time):
                batch_data.append(data)
        
        # Should have data from all providers and symbols
        expected_count = len(providers) * len(symbols)
        assert len(batch_data) == expected_count
        
        # Verify data diversity
        provider_names = set(data.provider for data in batch_data)
        assert len(provider_names) == len(providers)
        
        symbol_names = set(data.symbol for data in batch_data)
        assert len(symbol_names) == len(symbols)
        
        # Cleanup
        for provider in providers:
            await provider.disconnect()
    
    async def test_error_handling_in_flow(self, mock_settings, mock_fixtures):
        """Test error handling throughout the data flow."""
        # Mock provider that fails
        class FailingProvider(MockProvider):
            def __init__(self, name: str, fail_after: int = 2):
                super().__init__(name, {})
                self.fail_after = fail_after
                self.call_count = 0
            
            async def get_market_data(self, symbols, start_time, end_time, interval="1min"):
                self.call_count += 1
                if self.call_count > self.fail_after:
                    raise Exception("Provider failed")
                
                # Return some data first
                from ..providers.base import MarketData
                yield MarketData(
                    time=start_time,
                    symbol=symbols[0],
                    open=100.0,
                    high=105.0,
                    low=99.0,
                    close=104.0,
                    volume=1000000,
                    provider=self.name
                )
        
        failing_provider = FailingProvider("failing_provider", fail_after=1)
        failing_provider.settings = mock_settings
        
        # Also have a working provider
        working_provider = MockProvider("working_provider", mock_fixtures.get_iex_cloud_response())
        working_provider.settings = mock_settings
        
        await failing_provider.connect()
        await working_provider.connect()
        
        # Try to get data from both
        successful_data = []
        failed_providers = []
        
        providers = [failing_provider, working_provider]
        
        for provider in providers:
            try:
                async for data in provider.get_market_data(["AAPL"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
                    successful_data.append(data)
            except Exception as e:
                failed_providers.append((provider.name, str(e)))
        
        # Should have some successful data and some failures
        assert len(successful_data) > 0
        assert len(failed_providers) > 0
        
        # Working provider should have succeeded
        successful_provider_names = set(data.provider for data in successful_data)
        assert "working_provider" in successful_provider_names
        
        # Failing provider should be in failed list
        failed_provider_names = set(name for name, _ in failed_providers)
        assert "failing_provider" in failed_provider_names
        
        await failing_provider.disconnect()
        await working_provider.disconnect()
    
    async def test_data_consistency_across_providers(self, mock_settings, mock_fixtures):
        """Test data consistency when aggregating from multiple providers."""
        # Setup multiple providers with slightly different data
        class ConsistentProvider(MockProvider):
            def __init__(self, name: str, base_price: float):
                super().__init__(name, {})
                self.base_price = base_price
            
            async def get_market_data(self, symbols, start_time, end_time, interval="1min"):
                from ..providers.base import MarketData
                
                for symbol in symbols:
                    # Return slightly different prices to test consistency
                    yield MarketData(
                        time=start_time,
                        symbol=symbol,
                        open=self.base_price,
                        high=self.base_price + 5.0,
                        low=self.base_price - 1.0,
                        close=self.base_price + 4.0,
                        volume=1000000,
                        provider=self.name
                    )
        
        providers = [
            ConsistentProvider("provider_a", 100.0),
            ConsistentProvider("provider_b", 100.1),  # Slightly different
            ConsistentProvider("provider_c", 99.9)    # Slightly different
        ]
        
        for provider in providers:
            provider.settings = mock_settings
            await provider.connect()
        
        # Collect all data
        all_data = []
        for provider in providers:
            async for data in provider.get_market_data(["AAPL"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
                all_data.append(data)
        
        # Check for consistency
        assert len(all_data) == 3
        
        # Prices should be close but not identical
        close_prices = [data.close for data in all_data]
        assert max(close_prices) - min(close_prices) < 1.0  # Within reasonable range
        
        # Volumes should be identical (since we set them the same)
        volumes = [data.volume for data in all_data]
        assert all(v == volumes[0] for v in volumes)
        
        # Test aggregation
        aggregator = DataAggregator()
        aggregated = aggregator.aggregate_market_data(all_data)
        
        # Should produce consensus data
        assert aggregated is not None
        
        for provider in providers:
            await provider.disconnect()
    
    async def test_storage_integration_flow(self, mock_settings, mock_fixtures):
        """Test complete flow including storage."""
        # Mock storage backends
        with patch('redis.Redis') as mock_redis, \
             patch('psycopg2.connect') as mock_postgres:
            
            # Setup storage
            redis_store = RedisStore(mock_settings)
            timescale_db = TimescaleDB(mock_settings)
            
            # Mock storage methods
            redis_store.store_market_data = AsyncMock()
            timescale_db.store_market_data = AsyncMock()
            
            # Setup provider
            provider = MockProvider("storage_provider", mock_fixtures.get_iex_cloud_response())
            provider.settings = mock_settings
            await provider.connect()
            
            # Get data
            data_to_store = []
            async for data in provider.get_market_data(["AAPL"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
                data_to_store.append(data)
            
            # Store in both backends
            for data in data_to_store:
                await redis_store.store_market_data(data)
                await timescale_db.store_market_data(data)
            
            # Verify storage was called
            assert redis_store.store_market_data.call_count == len(data_to_store)
            assert timescale_db.store_market_data.call_count == len(data_to_store)
            
            await provider.disconnect()
    
    async def test_performance_under_load(self, mock_settings, mock_fixtures):
        """Test data flow performance under load."""
        # Setup multiple providers and many symbols
        providers = [
            MockProvider(f"load_provider_{i}", mock_fixtures.get_iex_cloud_response())
            for i in range(5)
        ]
        
        symbols = [f"TEST{i:03d}" for i in range(20)]  # 20 symbols
        
        for provider in providers:
            provider.settings = mock_settings
            await provider.connect()
        
        # Measure time for concurrent data fetching
        start_time = asyncio.get_event_loop().time()
        
        # Create tasks for concurrent execution
        tasks = []
        for provider in providers:
            task = self._collect_provider_data(provider, symbols)
            tasks.append(task)
        
        # Execute all tasks concurrently
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        end_time = asyncio.get_event_loop().time()
        execution_time = end_time - start_time
        
        # Verify results
        successful_results = [r for r in results if not isinstance(r, Exception)]
        assert len(successful_results) == len(providers)
        
        # Check that we got data from all providers
        total_data_points = sum(len(result) for result in successful_results)
        expected_points = len(providers) * len(symbols)
        assert total_data_points == expected_points
        
        # Performance should be reasonable (concurrent execution)
        assert execution_time < 5.0  # Should complete within 5 seconds
        
        # Cleanup
        for provider in providers:
            await provider.disconnect()
    
    async def _collect_provider_data(self, provider, symbols):
        """Helper method to collect data from a provider."""
        data = []
        async for item in provider.get_market_data(symbols, datetime(2024, 1, 1), datetime(2024, 1, 2)):
            data.append(item)
        return data
    
    async def test_data_quality_monitoring(self, mock_settings, mock_fixtures):
        """Test data quality monitoring throughout the flow."""
        # Setup components
        validator = DataValidator()
        
        # Mock provider with some bad data
        class QualityProvider(MockProvider):
            def __init__(self, name: str):
                super().__init__(name, {})
                self.data_count = 0
            
            async def get_market_data(self, symbols, start_time, end_time, interval="1min"):
                from ..providers.base import MarketData
                
                for symbol in symbols:
                    self.data_count += 1
                    
                    # Every 3rd data point is bad
                    if self.data_count % 3 == 0:
                        # Bad data - negative price
                        yield MarketData(
                            time=start_time,
                            symbol=symbol,
                            open=-100.0,  # Invalid
                            high=105.0,
                            low=99.0,
                            close=104.0,
                            volume=1000000,
                            provider=self.name
                        )
                    else:
                        # Good data
                        yield MarketData(
                            time=start_time,
                            symbol=symbol,
                            open=100.0,
                            high=105.0,
                            low=99.0,
                            close=104.0,
                            volume=1000000,
                            provider=self.name
                        )
        
        provider = QualityProvider("quality_provider")
        provider.settings = mock_settings
        await provider.connect()
        
        # Collect and validate data
        all_data = []
        valid_data = []
        invalid_data = []
        
        async for data in provider.get_market_data(["AAPL", "GOOGL", "MSFT"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
            all_data.append(data)
            
            if validator.validate_market_data(data):
                valid_data.append(data)
            else:
                invalid_data.append(data)
        
        # Should have some valid and some invalid data
        assert len(all_data) == 3  # 3 symbols
        assert len(valid_data) == 2  # 2 valid (1st and 2nd)
        assert len(invalid_data) == 1  # 1 invalid (3rd)
        
        # Quality metrics
        quality_ratio = len(valid_data) / len(all_data)
        assert 0.5 <= quality_ratio <= 1.0  # Should be reasonable
        
        await provider.disconnect()


@pytest.mark.asyncio
class TestProviderCoordination:
    """Test coordination between different types of providers."""
    
    async def test_market_data_economic_data_correlation(self, mock_settings, mock_fixtures):
        """Test correlating market data with economic indicators."""
        # Mock market data provider
        market_provider = MockProvider("market_provider", mock_fixtures.get_yahoo_finance_response())
        
        # Mock economic data provider
        class EconomicProvider(MockProvider):
            async def get_economic_data(self, indicators, start_time, end_time):
                """Mock economic data."""
                from ..providers.base import MarketData  # Reuse for simplicity
                
                for indicator in indicators:
                    yield MarketData(
                        time=start_time,
                        symbol=indicator,
                        open=3.5,  # Interest rate
                        high=3.6,
                        low=3.4,
                        close=3.5,
                        volume=0,  # N/A for economic data
                        provider=self.name
                    )
        
        econ_provider = EconomicProvider("economic_provider", mock_fixtures.get_fred_response())
        
        # Setup providers
        for provider in [market_provider, econ_provider]:
            provider.settings = mock_settings
            await provider.connect()
        
        # Fetch correlated data
        market_data = []
        async for data in market_provider.get_market_data(["SPY"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
            market_data.append(data)
        
        economic_data = []
        async for data in econ_provider.get_economic_data(["FEDFUNDS"], datetime(2024, 1, 1), datetime(2024, 1, 2)):
            economic_data.append(data)
        
        # Should have both types of data
        assert len(market_data) > 0
        assert len(economic_data) > 0
        
        # Verify different data types
        assert market_data[0].symbol == "SPY"
        assert economic_data[0].symbol == "FEDFUNDS"
        
        # In a real scenario, you would analyze correlation
        # For testing, just verify both datasets exist
        assert market_data[0].close > 50  # Market price
        assert economic_data[0].close < 10  # Interest rate
        
        for provider in [market_provider, econ_provider]:
            await provider.disconnect()
    
    async def test_multi_timeframe_coordination(self, mock_settings, mock_fixtures):
        """Test coordinating data across different timeframes."""
        # Mock provider with different timeframes
        class TimeframeProvider(MockProvider):
            async def get_market_data(self, symbols, start_time, end_time, interval="1min"):
                from ..providers.base import MarketData
                
                # Simulate different intervals
                if interval == "1min":
                    # High frequency data
                    for i in range(60):  # 60 minutes
                        yield MarketData(
                            time=start_time + timedelta(minutes=i),
                            symbol=symbols[0],
                            open=100.0 + i * 0.1,
                            high=105.0 + i * 0.1,
                            low=99.0 + i * 0.1,
                            close=104.0 + i * 0.1,
                            volume=1000 + i * 10,
                            provider=self.name
                        )
                elif interval == "1h":
                    # Lower frequency data
                    for i in range(24):  # 24 hours
                        yield MarketData(
                            time=start_time + timedelta(hours=i),
                            symbol=symbols[0],
                            open=100.0 + i * 0.5,
                            high=105.0 + i * 0.5,
                            low=99.0 + i * 0.5,
                            close=104.0 + i * 0.5,
                            volume=60000 + i * 1000,
                            provider=self.name
                        )
        
        provider = TimeframeProvider("timeframe_provider", {})
        provider.settings = mock_settings
        await provider.connect()
        
        # Fetch different timeframes
        minute_data = []
        async for data in provider.get_market_data(["AAPL"], datetime(2024, 1, 1), datetime(2024, 1, 2), "1min"):
            minute_data.append(data)
            if len(minute_data) >= 10:  # Limit for testing
                break
        
        hourly_data = []
        async for data in provider.get_market_data(["AAPL"], datetime(2024, 1, 1), datetime(2024, 1, 2), "1h"):
            hourly_data.append(data)
            if len(hourly_data) >= 5:  # Limit for testing
                break
        
        # Verify different granularities
        assert len(minute_data) == 10
        assert len(hourly_data) == 5
        
        # Minute data should have higher frequency
        minute_times = [data.time for data in minute_data]
        hourly_times = [data.time for data in hourly_data]
        
        # Time differences should be different
        minute_diff = minute_times[1] - minute_times[0]
        hourly_diff = hourly_times[1] - hourly_times[0]
        
        assert minute_diff < hourly_diff
        
        await provider.disconnect()