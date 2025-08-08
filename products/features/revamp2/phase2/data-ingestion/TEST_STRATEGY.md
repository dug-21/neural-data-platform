# Phase 2 Data-Ingestion Python Test Strategy

## Overview

This document outlines the comprehensive pytest testing strategy for migrating from unified Redis channel publishing to per-symbol channel publishing in the Python data-ingestion service.

## Current Test Infrastructure Analysis

### Existing Test Structure
```
/workspaces/neural-trader/data_ingestion/tests/
├── fixtures.py              # Test data fixtures
├── test_data_flow.py         # Data pipeline tests  
├── test_integration.py       # Integration tests
├── test_health_check.py      # Health monitoring tests
├── test_metrics_integration.py  # Metrics validation
├── test_secure_settings.py  # Configuration tests
└── test_*.py                # Provider-specific tests
```

### Existing Testing Patterns
```python
# Current test patterns from test_integration.py
@pytest.mark.asyncio
async def test_redis_publish():
    """Test Redis publishing functionality."""
    redis_store = RedisStore()
    await redis_store.connect()
    
    # Test publishing
    await redis_store.publish("test_channel", "test_message")
    
    await redis_store.disconnect()
```

## Test Strategy for Per-Symbol Channel Migration

### 1. Unit Tests - Channel Management

#### 1.1 Channel Manager Tests
**File**: `tests/test_channel_manager.py`

```python
import pytest
from unittest.mock import Mock, AsyncMock
from data_ingestion.utils.channel_manager import ChannelManager
from config import Settings

class TestChannelManager:
    """Test channel management functionality."""
    
    def setup_method(self):
        """Setup test fixtures."""
        self.settings = Mock(spec=Settings)
        self.settings.redis_channel_prefix = "market"
        self.settings.enable_legacy_channel = True
        self.channel_manager = ChannelManager(self.settings)
    
    def test_get_symbol_channel(self):
        """Test per-symbol channel name generation."""
        # Test case 1: Standard symbol
        channel = self.channel_manager.get_symbol_channel("AAPL")
        assert channel == "market:AAPL"
        
        # Test case 2: Symbol with special characters
        channel = self.channel_manager.get_symbol_channel("BRK.A")
        assert channel == "market:BRK.A"
        
        # Test case 3: Custom prefix
        self.settings.redis_channel_prefix = "stocks"
        channel = self.channel_manager.get_symbol_channel("NVDA")
        assert channel == "stocks:NVDA"
    
    def test_get_legacy_channel(self):
        """Test legacy channel name."""
        channel = self.channel_manager.get_legacy_channel()
        assert channel == "market:updates"
    
    def test_legacy_channel_disabled(self):
        """Test behavior when legacy channel is disabled."""
        self.settings.enable_legacy_channel = False
        assert not self.channel_manager.enable_legacy
```

#### 1.2 Redis Publisher Tests
**File**: `tests/test_redis_publisher.py`

```python
import pytest
import json
from unittest.mock import AsyncMock, Mock, patch
from data_ingestion.storage.redis_store import RedisStore

class TestRedisPublisher:
    """Test Redis publishing functionality."""
    
    @pytest.fixture
    async def redis_store(self):
        """Redis store fixture with mocked connection."""
        store = RedisStore()
        store.redis = AsyncMock()
        return store
    
    @pytest.mark.asyncio
    async def test_publish_to_symbol_channel(self, redis_store):
        """Test publishing to per-symbol channels."""
        symbol = "AAPL"
        market_data = {
            "symbol": symbol,
            "price": 185.25,
            "timestamp": 1704708600
        }
        
        # Test publishing
        await redis_store.publish_market_update(symbol, market_data)
        
        # Verify Redis publish was called with correct channel
        expected_channel = f"market:{symbol}"
        expected_message = json.dumps(market_data, default=str)
        
        redis_store.redis.publish.assert_called_once_with(
            expected_channel, 
            expected_message
        )
    
    @pytest.mark.asyncio
    async def test_dual_publishing_compatibility(self, redis_store):
        """Test dual publishing during transition period."""
        symbol = "NVDA"
        market_data = {"symbol": symbol, "price": 450.75}
        
        # Enable legacy channel
        redis_store.settings = Mock()
        redis_store.settings.enable_legacy_channel = True
        
        await redis_store.publish_market_update_dual(symbol, market_data)
        
        # Verify both channels were called
        calls = redis_store.redis.publish.call_args_list
        assert len(calls) == 2
        
        # Check per-symbol channel
        assert calls[0][0][0] == f"market:{symbol}"
        
        # Check legacy channel
        assert calls[1][0][0] == "market:updates"
    
    @pytest.mark.asyncio
    async def test_publish_error_handling(self, redis_store):
        """Test error handling in publishing."""
        import redis
        
        # Mock Redis error
        redis_store.redis.publish.side_effect = redis.RedisError("Connection failed")
        
        with pytest.raises(redis.RedisError):
            await redis_store.publish_market_update("AAPL", {"symbol": "AAPL"})
```

### 2. Integration Tests - End-to-End Flow

#### 2.1 Realtime Coordinator Integration
**File**: `tests/test_realtime_coordinator_integration.py`

```python
import pytest
import asyncio
import json
from unittest.mock import AsyncMock, Mock, patch
from data_ingestion.schedulers.realtime_coordinator import RealtimeCoordinator

class TestRealtimeCoordinatorIntegration:
    """Integration tests for realtime coordinator with channel migration."""
    
    @pytest.fixture
    async def coordinator(self):
        """Setup coordinator with mocked dependencies."""
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Mock()
        coordinator.settings.enable_legacy_channel = True
        return coordinator
    
    @pytest.mark.asyncio
    async def test_market_data_processing_flow(self, coordinator):
        """Test complete market data processing with new channels."""
        # Setup test data
        market_data = {
            "symbol": "AAPL",
            "price": 185.25,
            "volume": 1000,
            "time": "2025-01-08T10:30:00Z"
        }
        
        # Process market data
        await coordinator._process_market_data(market_data, "polygon")
        
        # Verify TimescaleDB insert
        coordinator.timescale.insert_market_data.assert_called_once()
        
        # Verify Redis publishing calls
        publish_calls = coordinator.redis.publish.call_args_list
        
        # Should have 3 calls: market_data:AAPL, market:AAPL, market:updates
        assert len(publish_calls) >= 2
        
        # Check channels
        channels = [call[0][0] for call in publish_calls]
        assert "market:AAPL" in channels
        assert "market_data:AAPL" in channels
        
        # If legacy enabled, should also have market:updates
        if coordinator.settings.enable_legacy_channel:
            assert "market:updates" in channels
    
    @pytest.mark.asyncio
    async def test_multiple_symbols_processing(self, coordinator):
        """Test processing multiple symbols concurrently."""
        symbols = ["AAPL", "NVDA", "TSLA", "MSFT", "GOOGL"]
        market_data_list = [
            {"symbol": symbol, "price": 100 + i, "volume": 1000}
            for i, symbol in enumerate(symbols)
        ]
        
        # Process all market data concurrently
        tasks = [
            coordinator._process_market_data(data, "polygon")
            for data in market_data_list
        ]
        await asyncio.gather(*tasks)
        
        # Verify each symbol got its own channel
        publish_calls = coordinator.redis.publish.call_args_list
        channels = [call[0][0] for call in publish_calls]
        
        for symbol in symbols:
            assert f"market:{symbol}" in channels
            assert f"market_data:{symbol}" in channels
```

#### 2.2 Real Redis Integration Tests
**File**: `tests/test_redis_integration.py`

```python
import pytest
import asyncio
import json
import redis.asyncio as redis
from data_ingestion.storage.redis_store import RedisStore
from data_ingestion.config import get_settings

@pytest.mark.integration
class TestRedisIntegration:
    """Integration tests with real Redis instance."""
    
    @pytest.fixture(scope="session")
    async def redis_store(self):
        """Real Redis store for integration testing."""
        settings = get_settings()
        # Use test Redis DB (different from production)
        test_redis_url = f"{settings.redis_url}/15"  # Use DB 15 for tests
        
        store = RedisStore()
        store.settings.redis_url = test_redis_url
        await store.connect()
        
        yield store
        
        # Cleanup
        await store.redis.flushdb()  # Clear test database
        await store.disconnect()
    
    @pytest.mark.asyncio
    async def test_real_redis_publishing(self, redis_store):
        """Test publishing to real Redis instance."""
        symbol = "AAPL"
        market_data = {
            "symbol": symbol,
            "price": 185.25,
            "timestamp": 1704708600,
            "volume": 1000
        }
        
        # Setup subscriber to verify message delivery
        pubsub = redis_store.redis.pubsub()
        await pubsub.subscribe(f"market:{symbol}")
        
        # Publish message
        await redis_store.publish_market_update(symbol, market_data)
        
        # Verify message received
        message = await pubsub.get_message(timeout=5.0)
        assert message is not None
        assert message['type'] == 'message'
        
        received_data = json.loads(message['data'])
        assert received_data['symbol'] == symbol
        assert received_data['price'] == 185.25
        
        await pubsub.unsubscribe(f"market:{symbol}")
        await pubsub.close()
    
    @pytest.mark.asyncio
    async def test_channel_isolation(self, redis_store):
        """Test that symbols are isolated to their own channels."""
        symbols = ["AAPL", "NVDA", "TSLA"]
        received_messages = {symbol: [] for symbol in symbols}
        
        # Setup subscribers for each symbol
        pubsubs = {}
        for symbol in symbols:
            pubsub = redis_store.redis.pubsub()
            await pubsub.subscribe(f"market:{symbol}")
            pubsubs[symbol] = pubsub
        
        # Publish to different symbols
        for i, symbol in enumerate(symbols):
            market_data = {
                "symbol": symbol,
                "price": 100 + i,
                "unique_id": f"test_{symbol}_{i}"
            }
            await redis_store.publish_market_update(symbol, market_data)
        
        # Verify each subscriber only gets their symbol's messages
        for symbol in symbols:
            pubsub = pubsubs[symbol]
            message = await pubsub.get_message(timeout=2.0)
            
            assert message is not None
            received_data = json.loads(message['data'])
            assert received_data['symbol'] == symbol
            assert received_data['unique_id'] == f"test_{symbol}_{symbols.index(symbol)}"
            
            # Verify no cross-contamination
            extra_message = await pubsub.get_message(timeout=1.0)
            assert extra_message is None or extra_message['type'] != 'message'
        
        # Cleanup
        for pubsub in pubsubs.values():
            await pubsub.close()
```

### 3. Load Testing

#### 3.1 Performance Tests
**File**: `tests/test_performance.py`

```python
import pytest
import asyncio
import time
from unittest.mock import AsyncMock
from data_ingestion.schedulers.realtime_coordinator import RealtimeCoordinator

@pytest.mark.performance
class TestPerformance:
    """Performance tests for channel migration."""
    
    @pytest.fixture
    async def mock_coordinator(self):
        """Coordinator with mocked I/O for pure processing tests."""
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Mock()
        coordinator.settings.enable_legacy_channel = True
        return coordinator
    
    @pytest.mark.asyncio
    async def test_high_throughput_publishing(self, mock_coordinator):
        """Test publishing throughput with multiple symbols."""
        symbols = [f"SYMBOL_{i:03d}" for i in range(100)]
        messages_per_symbol = 10
        total_messages = len(symbols) * messages_per_symbol
        
        start_time = time.time()
        
        # Generate and process messages
        tasks = []
        for symbol in symbols:
            for i in range(messages_per_symbol):
                market_data = {
                    "symbol": symbol,
                    "price": 100 + (i * 0.1),
                    "volume": 1000 + i,
                    "timestamp": int(time.time())
                }
                task = mock_coordinator._process_market_data(market_data, "test_provider")
                tasks.append(task)
        
        # Execute all tasks concurrently
        await asyncio.gather(*tasks)
        
        end_time = time.time()
        duration = end_time - start_time
        throughput = total_messages / duration
        
        # Performance assertions
        assert throughput > 1000, f"Throughput too low: {throughput:.2f} msg/sec"
        assert duration < 10, f"Processing took too long: {duration:.2f} seconds"
        
        # Verify all messages were published
        assert mock_coordinator.redis.publish.call_count >= total_messages
    
    @pytest.mark.asyncio
    async def test_memory_usage_scaling(self, mock_coordinator):
        """Test memory usage doesn't grow excessively with more symbols."""
        import psutil
        import os
        
        process = psutil.Process(os.getpid())
        initial_memory = process.memory_info().rss / 1024 / 1024  # MB
        
        # Process messages for many symbols
        symbols = [f"STOCK_{i:04d}" for i in range(1000)]
        
        for symbol in symbols:
            market_data = {
                "symbol": symbol,
                "price": 150.0,
                "volume": 1000
            }
            await mock_coordinator._process_market_data(market_data, "test_provider")
        
        final_memory = process.memory_info().rss / 1024 / 1024  # MB
        memory_increase = final_memory - initial_memory
        
        # Memory increase should be reasonable (< 100MB for 1000 symbols)
        assert memory_increase < 100, f"Memory usage increased by {memory_increase:.2f} MB"
```

### 4. Mocking Strategies

#### 4.1 Redis Mocking Patterns
```python
# fixtures.py - Shared test fixtures
import pytest
from unittest.mock import AsyncMock, Mock
from data_ingestion.storage.redis_store import RedisStore

@pytest.fixture
async def mock_redis_store():
    """Redis store with mocked Redis client."""
    store = RedisStore()
    store.redis = AsyncMock()
    store.pubsub = AsyncMock()
    store.settings = Mock()
    store.settings.redis_url = "redis://localhost:6379"
    return store

@pytest.fixture
def mock_redis_publish():
    """Mock Redis publish method with call tracking."""
    mock_publish = AsyncMock()
    mock_publish.side_effect = lambda channel, message: None  # Success
    return mock_publish

@pytest.fixture
def redis_error_scenario():
    """Redis error scenario for failure testing."""
    import redis
    error = redis.RedisError("Connection timeout")
    return error
```

#### 4.2 Provider Mocking
```python
@pytest.fixture
def mock_market_data():
    """Generate realistic market data for testing."""
    def _generate(symbol="AAPL", price=185.25, volume=1000):
        return {
            "symbol": symbol,
            "price": price,
            "volume": volume,
            "high": price * 1.02,
            "low": price * 0.98,
            "open": price * 1.01,
            "close": price,
            "time": "2025-01-08T10:30:00Z",
            "timestamp": 1704708600,
            "provider": "mock_provider"
        }
    return _generate
```

### 5. Backward Compatibility Tests

#### 5.1 Legacy Channel Tests
**File**: `tests/test_backward_compatibility.py`

```python
import pytest
from unittest.mock import AsyncMock, Mock
from data_ingestion.schedulers.realtime_coordinator import RealtimeCoordinator

class TestBackwardCompatibility:
    """Test backward compatibility during migration."""
    
    @pytest.mark.asyncio
    async def test_legacy_channel_still_works(self):
        """Test that legacy consumers still receive messages."""
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Mock()
        coordinator.settings.enable_legacy_channel = True
        
        market_data = {"symbol": "AAPL", "price": 185.25}
        
        await coordinator._process_market_data(market_data, "polygon")
        
        # Verify legacy channel still gets messages
        publish_calls = coordinator.redis.publish.call_args_list
        legacy_calls = [call for call in publish_calls if call[0][0] == "market:updates"]
        assert len(legacy_calls) > 0, "Legacy channel should still receive messages"
    
    @pytest.mark.asyncio
    async def test_graceful_legacy_disable(self):
        """Test graceful behavior when legacy channel is disabled."""
        coordinator = RealtimeCoordinator()
        coordinator.redis = AsyncMock()
        coordinator.timescale = AsyncMock()
        coordinator.settings = Mock()
        coordinator.settings.enable_legacy_channel = False
        
        market_data = {"symbol": "AAPL", "price": 185.25}
        
        await coordinator._process_market_data(market_data, "polygon")
        
        # Verify legacy channel gets no messages
        publish_calls = coordinator.redis.publish.call_args_list
        legacy_calls = [call for call in publish_calls if call[0][0] == "market:updates"]
        assert len(legacy_calls) == 0, "Legacy channel should not receive messages when disabled"
        
        # But per-symbol channels should still work
        symbol_calls = [call for call in publish_calls if call[0][0] == "market:AAPL"]
        assert len(symbol_calls) > 0, "Per-symbol channels should still work"
```

## Test Configuration and Setup

### 1. Pytest Configuration
**File**: `pytest.ini`

```ini
[tool:pytest]
asyncio_mode = auto
markers =
    integration: Integration tests requiring external services
    performance: Performance and load tests
    redis: Tests requiring Redis connection
testpaths = tests
python_files = test_*.py
python_functions = test_*
python_classes = Test*
addopts = 
    --strict-markers
    --strict-config
    --tb=short
    -v
filterwarnings =
    ignore::DeprecationWarning
    ignore::PendingDeprecationWarning
```

### 2. Test Environment Setup
**File**: `tests/conftest.py`

```python
import pytest
import asyncio
import os
from data_ingestion.config import get_settings, reset_settings

@pytest.fixture(scope="session")
def event_loop():
    """Create event loop for async tests."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()

@pytest.fixture(autouse=True)
def reset_config():
    """Reset configuration between tests."""
    yield
    reset_settings()

@pytest.fixture
def test_settings():
    """Test-specific settings."""
    os.environ["ENVIRONMENT"] = "test"
    os.environ["REDIS_URL"] = "redis://localhost:6379/15"  # Test database
    os.environ["ENABLE_LEGACY_CHANNEL"] = "true"
    os.environ["LOG_LEVEL"] = "DEBUG"
    
    settings = get_settings()
    yield settings
    
    # Cleanup environment
    test_env_vars = ["ENVIRONMENT", "REDIS_URL", "ENABLE_LEGACY_CHANNEL", "LOG_LEVEL"]
    for var in test_env_vars:
        os.environ.pop(var, None)
```

## Test Execution Strategy

### 1. Test Categories
```bash
# Unit tests (fast, no external dependencies)
pytest tests/test_channel_manager.py -v

# Integration tests (Redis required)
pytest tests/test_redis_integration.py -m integration

# Performance tests
pytest tests/test_performance.py -m performance

# All tests
pytest tests/ -v

# Coverage report
pytest tests/ --cov=data_ingestion --cov-report=html
```

### 2. CI/CD Integration
```yaml
# GitHub Actions example
name: Phase 2 Channel Migration Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v3
      - name: Set up Python
        uses: actions/setup-python@v4
        with:
          python-version: "3.9"
      
      - name: Install dependencies
        run: |
          pip install -r requirements.txt
          pip install pytest pytest-asyncio pytest-cov
      
      - name: Run unit tests
        run: pytest tests/test_channel_manager.py -v
      
      - name: Run integration tests
        run: pytest tests/test_redis_integration.py -m integration -v
        env:
          REDIS_URL: redis://localhost:6379
      
      - name: Generate coverage report
        run: pytest tests/ --cov=data_ingestion --cov-report=xml
```

### 3. Test Data Management
```python
# Test data fixtures
TEST_SYMBOLS = ["AAPL", "NVDA", "TSLA", "MSFT", "GOOGL"]

SAMPLE_MARKET_DATA = {
    "AAPL": {"symbol": "AAPL", "price": 185.25, "volume": 1000},
    "NVDA": {"symbol": "NVDA", "price": 450.75, "volume": 500},
    "TSLA": {"symbol": "TSLA", "price": 248.50, "volume": 800},
}

ERROR_SCENARIOS = [
    "redis.ConnectionError",
    "redis.TimeoutError", 
    "json.JSONDecodeError",
    "KeyError",
]
```

This comprehensive test strategy ensures that the channel migration is thoroughly validated across all scenarios, from unit-level channel management to full end-to-end integration testing with real Redis instances.