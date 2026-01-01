"""
Pytest configuration and fixtures for Redis Streams component tests.
"""

import pytest
import asyncio
import time
from typing import Dict, Any, List
from unittest.mock import MagicMock
import uuid
import threading
from collections import defaultdict


class SharedMockRedis:
    """Shared mock Redis instance for consistent testing."""
    
    def __init__(self):
        self.streams = {}
        self.consumer_groups = {}
        self.message_counter = 0
        self._lock = threading.RLock()
        self.operation_latency_ms = 1.0
        
    async def reset(self):
        """Reset all data for clean test state."""
        with self._lock:
            self.streams.clear()
            self.consumer_groups.clear()
            self.message_counter = 0
    
    async def xadd(self, stream_name: str, fields: Dict[str, Any], 
                   id: str = "*", maxlen: int = None, approximate: bool = True):
        """Add message to stream."""
        await asyncio.sleep(self.operation_latency_ms / 1000.0)
        
        with self._lock:
            if stream_name not in self.streams:
                self.streams[stream_name] = []
            
            self.message_counter += 1
            msg_id = f"{int(time.time() * 1000)}-{self.message_counter}" if id == "*" else id
            
            message = {
                'id': msg_id,
                'fields': fields.copy(),
                'timestamp': time.time()
            }
            
            self.streams[stream_name].append(message)
            
            if maxlen and len(self.streams[stream_name]) > maxlen:
                self.streams[stream_name] = self.streams[stream_name][-maxlen:]
            
            return msg_id
    
    async def xread(self, streams: Dict[str, str], count: int = None, block: int = None):
        """Read messages from streams."""
        await asyncio.sleep(self.operation_latency_ms / 1000.0)
        
        result = []
        with self._lock:
            for stream_name, last_id in streams.items():
                if stream_name not in self.streams:
                    continue
                
                stream_messages = []
                for msg in self.streams[stream_name]:
                    if msg['id'] > last_id:
                        stream_messages.append([msg['id'], msg['fields']])
                        if count and len(stream_messages) >= count:
                            break
                
                if stream_messages:
                    result.append([stream_name, stream_messages])
        
        return result
    
    async def xgroup_create(self, stream_name: str, group_name: str, 
                           id: str = "0", mkstream: bool = False):
        """Create consumer group."""
        if mkstream and stream_name not in self.streams:
            self.streams[stream_name] = []
        
        if stream_name not in self.consumer_groups:
            self.consumer_groups[stream_name] = {}
        
        self.consumer_groups[stream_name][group_name] = {
            'last_delivered_id': id,
            'consumers': {},
            'pending_count': 0
        }
        return True
    
    async def xreadgroup(self, group_name: str, consumer_name: str, 
                        streams: Dict[str, str], count: int = None, 
                        block: int = None, noack: bool = False):
        """Read messages using consumer group."""
        return await self.xread(streams, count, block)
    
    async def xinfo_groups(self, stream_name: str):
        """Get consumer group info."""
        if stream_name not in self.consumer_groups:
            return []
        
        groups = []
        for group_name, group_info in self.consumer_groups[stream_name].items():
            groups.append({
                'name': group_name,
                'consumers': len(group_info['consumers']),
                'pending': group_info['pending_count'],
                'last-delivered-id': group_info['last_delivered_id']
            })
        
        return groups
    
    async def exists(self, stream_name: str):
        """Check if stream exists."""
        return stream_name in self.streams


@pytest.fixture(scope="session")
def event_loop():
    """Create event loop for async tests."""
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


@pytest.fixture(scope="function")
async def mock_redis():
    """Fresh mock Redis instance for each test."""
    redis = SharedMockRedis()
    await redis.reset()
    return redis


@pytest.fixture(scope="function") 
async def fast_mock_redis():
    """Fast mock Redis with minimal latency."""
    redis = SharedMockRedis()
    redis.operation_latency_ms = 0.1
    await redis.reset()
    return redis


@pytest.fixture(scope="function")
def sample_market_data():
    """Sample market data messages."""
    return [
        {
            'symbol': 'AAPL',
            'price': 150.75,
            'volume': 1000,
            'timestamp': int(time.time()),
            'source': 'test'
        },
        {
            'symbol': 'GOOGL', 
            'price': 2800.50,
            'volume': 500,
            'timestamp': int(time.time()),
            'source': 'test'
        },
        {
            'symbol': 'MSFT',
            'price': 420.25,
            'volume': 750,
            'timestamp': int(time.time()), 
            'source': 'test'
        }
    ]


@pytest.fixture(scope="function")
def sample_predictions():
    """Sample prediction messages."""
    return [
        {
            'symbol': 'AAPL',
            'prediction': 0.75,
            'confidence': 0.85,
            'model_id': 'lstm_v2',
            'timestamp': int(time.time()),
            'correlation_id': str(uuid.uuid4())
        },
        {
            'symbol': 'GOOGL',
            'prediction': 0.62,
            'confidence': 0.78,
            'model_id': 'transformer_v1', 
            'timestamp': int(time.time()),
            'correlation_id': str(uuid.uuid4())
        }
    ]


@pytest.fixture(scope="function")
def test_channels():
    """Standard test channels."""
    return ['market-data', 'predictions', 'actions', 'monitoring']


@pytest.fixture(scope="function")
def test_consumer_groups():
    """Standard test consumer groups."""
    return {
        'market-data': ['ml-processors', 'analytics', 'monitoring'],
        'predictions': ['trading-engine', 'risk-management', 'analytics'],
        'actions': ['execution-engine', 'compliance', 'audit'],
        'monitoring': ['alerting', 'metrics', 'logging']
    }


@pytest.fixture(scope="function")
def performance_targets():
    """Performance benchmark targets."""
    return {
        'market-data': {
            'throughput_msgs_per_sec': 10000,
            'latency_p99_ms': 50
        },
        'predictions': {
            'throughput_msgs_per_sec': 1000,
            'latency_p99_ms': 100
        },
        'actions': {
            'throughput_msgs_per_sec': 500,
            'latency_p99_ms': 200
        },
        'monitoring': {
            'throughput_msgs_per_sec': 5000,
            'latency_p99_ms': 100
        }
    }


@pytest.fixture(autouse=True)
def cleanup_after_test():
    """Cleanup after each test."""
    yield
    # Force garbage collection
    import gc
    gc.collect()


# Pytest configuration
pytest_plugins = ['pytest_asyncio']


def pytest_configure(config):
    """Configure pytest with custom markers."""
    config.addinivalue_line("markers", "slow: marks tests as slow")
    config.addinivalue_line("markers", "integration: marks tests as integration tests")
    config.addinivalue_line("markers", "benchmark: marks tests as benchmark tests")
    config.addinivalue_line("markers", "throughput: marks tests as throughput tests")


def pytest_collection_modifyitems(config, items):
    """Add markers to tests based on names."""
    for item in items:
        # Mark slow tests
        if "throughput" in item.name or "benchmark" in item.name:
            item.add_marker(pytest.mark.slow)
        
        # Mark integration tests
        if "integration" in item.name:
            item.add_marker(pytest.mark.integration)
        
        # Mark benchmark tests
        if "benchmark" in item.name or "performance" in item.name:
            item.add_marker(pytest.mark.benchmark)
        
        # Mark throughput tests
        if "throughput" in item.name:
            item.add_marker(pytest.mark.throughput)