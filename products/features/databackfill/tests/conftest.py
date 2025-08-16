"""
Pytest configuration for backfill tests
"""
import pytest
import asyncio
import os
from pathlib import Path
import tempfile
import shutil
from unittest.mock import AsyncMock, Mock

# Add parent directory to path for imports
import sys
sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent))


# Pytest markers
def pytest_configure(config):
    """Register custom markers"""
    config.addinivalue_line(
        "markers", "performance: mark test as performance test"
    )
    config.addinivalue_line(
        "markers", "stress: mark test as stress test"
    )
    config.addinivalue_line(
        "markers", "integration: mark test as integration test"
    )
    config.addinivalue_line(
        "markers", "slow: mark test as slow running"
    )


# Event loop configuration
@pytest.fixture(scope="session")
def event_loop():
    """Create event loop for async tests"""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


# Temporary directories
@pytest.fixture
def temp_dir():
    """Create temporary directory for test files"""
    temp_dir = tempfile.mkdtemp()
    yield Path(temp_dir)
    shutil.rmtree(temp_dir)


# Mock providers
@pytest.fixture
def mock_yahoo_provider():
    """Mock Yahoo Finance provider"""
    provider = AsyncMock()
    provider.name = "yahoo"
    provider.get_market_data = AsyncMock()
    provider.get_historical_data = AsyncMock()
    return provider


@pytest.fixture
def mock_alpaca_provider():
    """Mock Alpaca provider"""
    provider = AsyncMock()
    provider.name = "alpaca"
    provider.get_market_data = AsyncMock()
    provider.get_bars = AsyncMock()
    return provider


@pytest.fixture
def mock_polygon_provider():
    """Mock Polygon provider"""
    provider = AsyncMock()
    provider.name = "polygon"
    provider.get_market_data = AsyncMock()
    provider.get_aggregates = AsyncMock()
    return provider


# Mock storage
@pytest.fixture
def mock_timescale_db():
    """Mock TimescaleDB storage"""
    db = AsyncMock()
    db.store_market_data = AsyncMock()
    db.batch_insert = AsyncMock()
    db.query = AsyncMock()
    db.execute = AsyncMock()
    return db


@pytest.fixture
def mock_s3_client():
    """Mock S3 client"""
    client = AsyncMock()
    client.put_object = AsyncMock(return_value={'ETag': '"test-etag"'})
    client.get_object = AsyncMock()
    client.list_objects_v2 = AsyncMock()
    client.head_bucket = AsyncMock()
    client.create_multipart_upload = AsyncMock()
    client.upload_part = AsyncMock()
    client.complete_multipart_upload = AsyncMock()
    return client


# Environment setup
@pytest.fixture(autouse=True)
def setup_test_env(monkeypatch):
    """Setup test environment variables"""
    test_env = {
        'POLYGON_API_KEY': 'test_polygon_key',
        'ALPACA_API_KEY': 'test_alpaca_key',
        'ALPACA_SECRET_KEY': 'test_alpaca_secret',
        'ALPHA_VANTAGE_API_KEY': 'test_av_key',
        'IEX_API_KEY': 'test_iex_key',
        'YAHOO_API_KEY': 'test_yahoo_key',
        'AWS_ACCESS_KEY_ID': 'test_aws_key',
        'AWS_SECRET_ACCESS_KEY': 'test_aws_secret',
        'AWS_REGION': 'us-east-1',
        'S3_BUCKET': 'test-trading-backfill',
        'DATABASE_URL': 'postgresql://test:test@localhost:5432/test_trading',
        'REDIS_URL': 'redis://localhost:6379/0',
        'LOG_LEVEL': 'DEBUG',
        'ENVIRONMENT': 'test'
    }
    
    for key, value in test_env.items():
        monkeypatch.setenv(key, value)


# Test data fixtures
@pytest.fixture
def sample_ohlcv_data():
    """Sample OHLCV data for testing"""
    from datetime import datetime, timedelta
    from data_ingestion.providers.base import MarketData
    
    data = []
    base_time = datetime(2023, 1, 2, 9, 30)
    
    for i in range(100):
        data.append(MarketData(
            time=base_time + timedelta(minutes=i),
            symbol="AAPL",
            open=150.0 + (i % 5),
            high=152.0 + (i % 5),
            low=148.0 + (i % 5),
            close=151.0 + (i % 5),
            volume=1000000 + (i * 10000)
        ))
    
    return data


@pytest.fixture
def sample_tick_data():
    """Sample tick data for testing"""
    from datetime import datetime, timedelta
    from data_ingestion.providers.base import TickData
    
    data = []
    base_time = datetime(2023, 1, 2, 9, 30)
    
    for i in range(1000):
        data.append(TickData(
            time=base_time + timedelta(seconds=i * 0.1),
            symbol="AAPL",
            price=150.05 + (i % 100) * 0.01,
            size=100 + (i % 10) * 10,
            bid=150.04 + (i % 100) * 0.01,
            ask=150.06 + (i % 100) * 0.01,
            conditions=[]
        ))
    
    return data


# Utility functions
def assert_valid_market_data(data):
    """Assert that market data is valid"""
    assert data.time is not None
    assert data.symbol is not None
    assert data.open > 0
    assert data.high >= data.low
    assert data.high >= data.open
    assert data.high >= data.close
    assert data.low <= data.open
    assert data.low <= data.close
    assert data.volume >= 0


def assert_valid_tick_data(tick):
    """Assert that tick data is valid"""
    assert tick.time is not None
    assert tick.symbol is not None
    assert tick.price > 0
    assert tick.size > 0
    assert tick.bid <= tick.price
    assert tick.ask >= tick.price


# Performance monitoring
@pytest.fixture
def performance_monitor():
    """Monitor test performance metrics"""
    import time
    import psutil
    
    class PerformanceMonitor:
        def __init__(self):
            self.start_time = None
            self.start_memory = None
            self.process = psutil.Process()
        
        def start(self):
            self.start_time = time.time()
            self.start_memory = self.process.memory_info().rss / 1024 / 1024  # MB
        
        def stop(self):
            elapsed = time.time() - self.start_time
            end_memory = self.process.memory_info().rss / 1024 / 1024  # MB
            memory_used = end_memory - self.start_memory
            
            return {
                'elapsed_seconds': elapsed,
                'memory_used_mb': memory_used,
                'cpu_percent': self.process.cpu_percent()
            }
    
    return PerformanceMonitor()