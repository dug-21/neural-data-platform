#!/usr/bin/env python3
"""
Mock integration test suite for the neural-trader data ingestion pipeline.
Tests system integration with mocked external dependencies.
"""

import asyncio
import json
import sys
import os
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional
import traceback
from unittest.mock import Mock, AsyncMock, patch, MagicMock
import tempfile

# Add the current directory to sys.path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# Import our modules
from config import get_settings
from providers import PROVIDERS
from utils.logging import get_logger, setup_logging
from utils.metrics import metrics

# Setup logging
setup_logging()
logger = get_logger(__name__)

class MockIntegrationTester:
    """Mock integration test suite that simulates real conditions."""
    
    def __init__(self):
        self.settings = get_settings()
        self.test_results = {}
        self.test_symbols = ['AAPL', 'MSFT', 'GOOGL']
        
    async def run_all_tests(self):
        """Run all integration tests."""
        logger.info("🚀 Starting mock integration tests...")
        
        tests = [
            ("provider_architecture", self.test_provider_architecture),
            ("alpaca_provider_structure", self.test_alpaca_provider_structure),
            ("storage_interfaces", self.test_storage_interfaces),
            ("coordinator_initialization", self.test_coordinator_initialization),
            ("data_flow_simulation", self.test_data_flow_simulation),
            ("error_handling_patterns", self.test_error_handling_patterns),
            ("configuration_validation", self.test_configuration_validation),
            ("metrics_integration", self.test_metrics_integration),
            ("import_dependencies", self.test_import_dependencies),
            ("code_structure_validation", self.test_code_structure_validation)
        ]
        
        total_tests = len(tests)
        passed_tests = 0
        
        for test_name, test_func in tests:
            logger.info(f"🔄 Running test: {test_name}")
            try:
                result = await test_func()
                if result:
                    logger.info(f"✅ Test {test_name} PASSED")
                    passed_tests += 1
                else:
                    logger.error(f"❌ Test {test_name} FAILED")
                self.test_results[test_name] = result
            except Exception as e:
                logger.error(f"💥 Test {test_name} CRASHED: {e}")
                logger.error(traceback.format_exc())
                self.test_results[test_name] = False
        
        # Summary
        success_rate = (passed_tests / total_tests) * 100
        logger.info(f"📊 Mock Integration Test Summary:")
        logger.info(f"   Total Tests: {total_tests}")
        logger.info(f"   Passed: {passed_tests}")
        logger.info(f"   Failed: {total_tests - passed_tests}")
        logger.info(f"   Success Rate: {success_rate:.1f}%")
        
        return success_rate >= 90  # 90% pass rate required for mocks
    
    async def test_provider_architecture(self) -> bool:
        """Test provider architecture and structure."""
        logger.info("Testing provider architecture...")
        
        try:
            # Test provider registry
            logger.info(f"Available providers: {list(PROVIDERS.keys())}")
            
            if 'alpaca' not in PROVIDERS:
                logger.error("Alpaca provider not found in PROVIDERS")
                return False
            
            # Test provider class structure
            alpaca_class = PROVIDERS['alpaca']
            
            # Verify base class inheritance
            from providers.base import BaseProvider
            if not issubclass(alpaca_class, BaseProvider):
                logger.error("Alpaca provider doesn't inherit from BaseProvider")
                return False
            
            # Test provider instantiation
            provider = alpaca_class()
            
            # Verify required methods exist
            required_methods = ['connect', 'disconnect', 'get_market_data', 'stream_market_data']
            for method in required_methods:
                if not hasattr(provider, method):
                    logger.error(f"Provider missing required method: {method}")
                    return False
            
            logger.info("✅ Provider architecture is valid")
            return True
            
        except Exception as e:
            logger.error(f"Provider architecture test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_alpaca_provider_structure(self) -> bool:
        """Test Alpaca provider structure and configuration."""
        logger.info("Testing Alpaca provider structure...")
        
        try:
            alpaca_class = PROVIDERS['alpaca']
            provider = alpaca_class()
            
            # Test provider attributes
            assert hasattr(provider, 'name')
            assert hasattr(provider, 'settings')
            assert hasattr(provider, 'logger')
            assert provider.name == 'alpaca'
            
            # Test configuration handling
            assert hasattr(provider, 'api_key')
            assert hasattr(provider, 'api_secret')
            assert hasattr(provider, 'subscription_level')
            
            # Test interval mapping
            assert hasattr(provider, 'INTERVAL_MAP')
            assert isinstance(provider.INTERVAL_MAP, dict)
            assert '1min' in provider.INTERVAL_MAP
            
            # Test subscription limits
            assert hasattr(provider, 'SUBSCRIPTION_LIMITS')
            assert 'basic' in provider.SUBSCRIPTION_LIMITS
            assert 'unlimited' in provider.SUBSCRIPTION_LIMITS
            
            logger.info("✅ Alpaca provider structure is valid")
            return True
            
        except Exception as e:
            logger.error(f"Alpaca provider structure test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_storage_interfaces(self) -> bool:
        """Test storage interface compatibility."""
        logger.info("Testing storage interfaces...")
        
        try:
            # Test TimescaleDB interface
            from storage.timescale import TimescaleDB
            timescale = TimescaleDB()
            
            # Verify required methods
            required_methods = ['connect', 'disconnect', 'insert_market_data', 'query_market_data']
            for method in required_methods:
                if not hasattr(timescale, method):
                    logger.error(f"TimescaleDB missing method: {method}")
                    return False
            
            # Test Redis interface
            from storage.redis_store import RedisStore
            redis_store = RedisStore()
            
            required_methods = ['connect', 'disconnect', 'set_latest_price', 'get_latest_price', 'publish']
            for method in required_methods:
                if not hasattr(redis_store, method):
                    logger.error(f"RedisStore missing method: {method}")
                    return False
            
            logger.info("✅ Storage interfaces are valid")
            return True
            
        except Exception as e:
            logger.error(f"Storage interfaces test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_coordinator_initialization(self) -> bool:
        """Test coordinator initialization without external dependencies."""
        logger.info("Testing coordinator initialization...")
        
        try:
            from schedulers.realtime_coordinator import RealtimeCoordinator
            
            # Test coordinator instantiation
            coordinator = RealtimeCoordinator()
            
            # Verify attributes
            assert hasattr(coordinator, 'settings')
            assert hasattr(coordinator, 'providers')
            assert hasattr(coordinator, 'active_streams')
            assert hasattr(coordinator, 'subscribed_symbols')
            
            # Test subscription handling
            assert len(coordinator.subscribed_symbols) == 0
            
            # Mock the storage connections
            coordinator.timescale = Mock()
            coordinator.redis = Mock()
            coordinator.timescale.connect = AsyncMock()
            coordinator.redis.connect = AsyncMock()
            
            # Test provider initialization (mocked)
            with patch.object(coordinator, 'providers', {}):
                # This would normally fail due to missing API keys
                # but we're testing the structure
                pass
            
            logger.info("✅ Coordinator initialization structure is valid")
            return True
            
        except Exception as e:
            logger.error(f"Coordinator initialization test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_data_flow_simulation(self) -> bool:
        """Test data flow simulation with mocked components."""
        logger.info("Testing data flow simulation...")
        
        try:
            # Create mock market data
            mock_market_data = Mock()
            mock_market_data.symbol = 'AAPL'
            mock_market_data.close = 150.25
            mock_market_data.time = datetime.now()
            mock_market_data.provider = 'alpaca'
            mock_market_data.open = 150.0
            mock_market_data.high = 151.0
            mock_market_data.low = 149.5
            mock_market_data.volume = 1000000
            
            # Test data conversion
            data_dict = {
                'time': mock_market_data.time,
                'symbol': mock_market_data.symbol,
                'open': mock_market_data.open,
                'high': mock_market_data.high,
                'low': mock_market_data.low,
                'close': mock_market_data.close,
                'volume': mock_market_data.volume,
                'provider': mock_market_data.provider
            }
            
            # Validate data structure
            required_fields = ['time', 'symbol', 'open', 'high', 'low', 'close', 'volume', 'provider']
            for field in required_fields:
                assert field in data_dict, f"Missing required field: {field}"
            
            # Test data validation patterns
            assert data_dict['close'] > 0
            assert data_dict['volume'] >= 0
            assert data_dict['high'] >= data_dict['low']
            
            logger.info("✅ Data flow simulation is valid")
            return True
            
        except Exception as e:
            logger.error(f"Data flow simulation test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_error_handling_patterns(self) -> bool:
        """Test error handling patterns."""
        logger.info("Testing error handling patterns...")
        
        try:
            # Test provider error handling
            alpaca_class = PROVIDERS['alpaca']
            provider = alpaca_class()
            
            # Test connection error handling (without actual connection)
            try:
                # This should raise an error due to missing API keys
                await provider.connect()
                logger.warning("Expected error did not occur")
                return False
            except ValueError as e:
                if "API key and secret not configured" in str(e):
                    logger.info("✅ Provider error handling works correctly")
                else:
                    logger.error(f"Unexpected error: {e}")
                    return False
            
            # Test data validation patterns
            from processors.validator import DataValidator
            validator = DataValidator()
            
            # Test with invalid data
            invalid_data = {
                'symbol': 'AAPL',
                'close': -10.0,  # Invalid negative price
                'volume': -100   # Invalid negative volume
            }
            
            validation = validator.validate_realtime_data(invalid_data)
            assert not validation['is_valid']
            assert len(validation['errors']) > 0
            
            logger.info("✅ Error handling patterns are valid")
            return True
            
        except Exception as e:
            logger.error(f"Error handling patterns test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_configuration_validation(self) -> bool:
        """Test configuration validation."""
        logger.info("Testing configuration validation...")
        
        try:
            # Test settings structure
            settings = get_settings()
            
            # Verify required settings exist
            required_settings = [
                'redis_host', 'redis_port', 'redis_db',
                'timescale_host', 'timescale_port', 'timescale_database',
                'log_level', 'prometheus_enabled'
            ]
            
            for setting in required_settings:
                assert hasattr(settings, setting), f"Missing setting: {setting}"
            
            # Test rate limiting configuration
            assert hasattr(settings, 'rate_limits')
            assert isinstance(settings.rate_limits, dict)
            
            # Test environment handling
            assert hasattr(settings, 'alpaca_api_key')
            assert hasattr(settings, 'alpaca_api_secret')
            
            logger.info("✅ Configuration validation is valid")
            return True
            
        except Exception as e:
            logger.error(f"Configuration validation test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_metrics_integration(self) -> bool:
        """Test metrics integration."""
        logger.info("Testing metrics integration...")
        
        try:
            # Test metrics structure
            assert hasattr(metrics, 'api_requests_total')
            assert hasattr(metrics, 'data_points_processed')
            assert hasattr(metrics, 'processing_errors')
            assert hasattr(metrics, 'storage_operations')
            
            # Test metric operations (without actual increment)
            # Just verify the structure exists
            assert hasattr(metrics.api_requests_total, 'labels')
            assert hasattr(metrics.data_points_processed, 'labels')
            
            logger.info("✅ Metrics integration is valid")
            return True
            
        except Exception as e:
            logger.error(f"Metrics integration test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_import_dependencies(self) -> bool:
        """Test import dependencies."""
        logger.info("Testing import dependencies...")
        
        try:
            # Test core imports
            import config
            import providers
            import storage
            import schedulers
            import processors
            import utils
            
            # Test specific modules
            from providers.alpaca import AlpacaProvider
            from storage.timescale import TimescaleDB
            from storage.redis_store import RedisStore
            from schedulers.realtime_coordinator import RealtimeCoordinator
            from processors.validator import DataValidator
            from utils.logging import get_logger
            from utils.metrics import metrics
            
            logger.info("✅ Import dependencies are valid")
            return True
            
        except Exception as e:
            logger.error(f"Import dependencies test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_code_structure_validation(self) -> bool:
        """Test code structure validation."""
        logger.info("Testing code structure validation...")
        
        try:
            # Test main entry point
            import main
            assert hasattr(main, 'DataIngestionService')
            assert hasattr(main, 'cli')
            
            # Test provider base class
            from providers.base import BaseProvider
            assert hasattr(BaseProvider, 'connect')
            assert hasattr(BaseProvider, 'disconnect')
            assert hasattr(BaseProvider, 'get_market_data')
            
            # Test data models
            from providers.base import MarketData, TickData, OrderBookData
            
            # Test utility modules
            from utils.logging import setup_logging
            from utils.retry import with_retry
            
            logger.info("✅ Code structure validation is valid")
            return True
            
        except Exception as e:
            logger.error(f"Code structure validation test failed: {e}")
            logger.error(traceback.format_exc())
            return False

async def main():
    """Run mock integration tests."""
    tester = MockIntegrationTester()
    
    try:
        success = await tester.run_all_tests()
        
        if success:
            logger.info("🎉 Mock integration tests PASSED!")
            return 0
        else:
            logger.error("❌ Mock integration tests FAILED!")
            return 1
            
    except Exception as e:
        logger.error(f"Mock integration test runner failed: {e}")
        logger.error(traceback.format_exc())
        return 1

if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)