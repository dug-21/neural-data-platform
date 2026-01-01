"""
Redis Streams EventBus Component Tests

This package contains comprehensive unit tests for the Redis Streams EventBus
implementation, covering:

- Stream channel configuration and management
- Message routing and pub/sub patterns  
- Consumer group management and scaling
- Message ordering guarantees
- Throughput benchmarks and performance validation

All tests are designed to be standalone and use mocked Redis implementations
to avoid external dependencies while maintaining realistic behavior.
"""

__version__ = "1.0.0"
__author__ = "Neural Trader QA Team"

# Test configuration constants
DEFAULT_TEST_TIMEOUT = 30  # seconds
MAX_TEST_MESSAGES = 10000
DEFAULT_THROUGHPUT_TARGET = 100000  # messages per second

# Channel types for testing
TEST_CHANNELS = [
    'market-data',
    'predictions', 
    'actions',
    'monitoring'
]

# Consumer groups for testing  
TEST_CONSUMER_GROUPS = {
    'market-data': ['ml-processors', 'analytics', 'monitoring'],
    'predictions': ['trading-engine', 'risk-management', 'analytics'],
    'actions': ['execution-engine', 'compliance', 'audit'],
    'monitoring': ['alerting', 'metrics', 'logging']
}

# Performance benchmarks
PERFORMANCE_TARGETS = {
    'market-data': {
        'throughput_msgs_per_sec': 10000,
        'latency_p99_ms': 50,
        'memory_per_1k_msgs_mb': 2.5
    },
    'predictions': {
        'throughput_msgs_per_sec': 1000, 
        'latency_p99_ms': 100,
        'memory_per_1k_msgs_mb': 5.0
    },
    'actions': {
        'throughput_msgs_per_sec': 500,
        'latency_p99_ms': 200, 
        'memory_per_1k_msgs_mb': 3.0
    },
    'monitoring': {
        'throughput_msgs_per_sec': 5000,
        'latency_p99_ms': 100,
        'memory_per_1k_msgs_mb': 1.5
    }
}