"""
Pytest Configuration for Config Store Client Tests

Provides shared fixtures and setup for unit tests following TDD London School approach.
"""

import pytest
import asyncio
import os
from unittest.mock import Mock, AsyncMock, patch


@pytest.fixture(scope="session")
def event_loop():
    """Create an instance of the default event loop for the test session."""
    loop = asyncio.get_event_loop_policy().new_event_loop()
    yield loop
    loop.close()


@pytest.fixture
def mock_grpc_channel():
    """Mock gRPC channel for testing"""
    channel = AsyncMock()
    channel.close = AsyncMock()
    channel.get_state = Mock(return_value="READY")
    channel.channel_ready = AsyncMock(return_value=True)
    return channel


@pytest.fixture
def clean_environment():
    """Clean environment variables for testing"""
    # Store original environment
    original_env = dict(os.environ)
    
    # Clear test-related environment variables
    test_env_vars = [key for key in os.environ.keys() if key.startswith(('DATABASE_', 'TRADING_', 'API_', 'NEURAL_'))]
    for var in test_env_vars:
        if var in os.environ:
            del os.environ[var]
    
    yield
    
    # Restore original environment
    os.environ.clear()
    os.environ.update(original_env)


@pytest.fixture
def sample_trading_config():
    """Sample trading configuration for tests"""
    return {
        "max_position_size": 10000.0,
        "risk_tolerance": 0.02,
        "enable_paper_trading": True,
        "api_key": "test_api_key_12345",
        "symbols": ["BTC/USDT", "ETH/USDT", "ADA/USDT"]
    }


@pytest.fixture
def sample_database_config():
    """Sample database configuration for tests"""
    return {
        "host": "localhost",
        "port": 5432,
        "username": "trader_user",
        "password": "secure_password",
        "pool_size": 10,
        "timeout_seconds": 30
    }


@pytest.fixture
def trading_config_schema():
    """JSON schema for trading configuration"""
    return {
        "type": "object",
        "properties": {
            "max_position_size": {
                "type": "number",
                "minimum": 0,
                "description": "Maximum position size in USD"
            },
            "risk_tolerance": {
                "type": "number", 
                "minimum": 0,
                "maximum": 1,
                "description": "Risk tolerance as decimal (0-1)"
            },
            "enable_paper_trading": {
                "type": "boolean",
                "description": "Enable paper trading mode"
            },
            "api_key": {
                "type": "string",
                "minLength": 10,
                "description": "API key for exchange"
            },
            "symbols": {
                "type": "array",
                "items": {"type": "string"},
                "minItems": 1,
                "description": "List of trading symbols"
            }
        },
        "required": ["max_position_size", "risk_tolerance", "api_key"],
        "additionalProperties": False
    }


@pytest.fixture
def mock_grpc_responses():
    """Mock gRPC response data"""
    return {
        "health_check": {"healthy": True},
        "get_config": {"value": "test_value"},
        "set_config": {"success": True},
        "bulk_config": {
            "configs": {
                "trading.max_position": 10000.0,
                "trading.risk_tolerance": 0.02,
                "database.host": "localhost"
            }
        },
        "list_keys": {
            "keys": [
                "trading.max_position",
                "trading.risk_tolerance",
                "database.host",
                "database.port"
            ]
        }
    }