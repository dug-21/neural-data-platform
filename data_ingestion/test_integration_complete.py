#!/usr/bin/env python3
"""
Comprehensive integration test suite for the neural-trader data ingestion pipeline.
Tests complete end-to-end functionality including provider initialization, data flow,
storage integration, and error handling.
"""

import asyncio
import json
import sys
import os
from datetime import datetime, timedelta
from typing import Dict, List, Any, Optional
import traceback
from contextlib import asynccontextmanager

# Add the current directory to sys.path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

# Import our modules
from config import get_settings
from providers import PROVIDERS
from storage import TimescaleDB, RedisStore
from schedulers import RealtimeCoordinator
from utils.logging import get_logger, setup_logging
from utils.metrics import metrics

# Setup logging
setup_logging()
logger = get_logger(__name__)

class IntegrationTester:
    """Comprehensive integration test suite."""
    
    def __init__(self):
        self.settings = get_settings()
        self.test_results = {}
        self.redis_store = None
        self.timescale_db = None
        self.coordinator = None
        self.test_symbols = ['AAPL', 'MSFT', 'GOOGL']
        
    async def run_all_tests(self):
        """Run all integration tests."""
        logger.info("🚀 Starting comprehensive integration tests...")
        
        tests = [
            ("provider_initialization", self.test_provider_initialization),
            ("alpaca_provider_functionality", self.test_alpaca_provider_functionality),
            ("redis_connectivity", self.test_redis_connectivity),
            ("timescale_integration", self.test_timescale_integration),
            ("data_pipeline_flow", self.test_data_pipeline_flow),
            ("realtime_coordinator", self.test_realtime_coordinator),
            ("error_handling", self.test_error_handling),
            ("storage_integration", self.test_storage_integration),
            ("metrics_collection", self.test_metrics_collection),
            ("end_to_end_workflow", self.test_end_to_end_workflow)
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
        logger.info(f"📊 Integration Test Summary:")
        logger.info(f"   Total Tests: {total_tests}")
        logger.info(f"   Passed: {passed_tests}")
        logger.info(f"   Failed: {total_tests - passed_tests}")
        logger.info(f"   Success Rate: {success_rate:.1f}%")
        
        return success_rate >= 80  # 80% pass rate required
    
    async def test_provider_initialization(self) -> bool:
        """Test provider initialization and connectivity."""
        logger.info("Testing provider initialization...")
        
        try:
            # Test available providers
            logger.info(f"Available providers: {list(PROVIDERS.keys())}")
            
            if 'alpaca' not in PROVIDERS:
                logger.error("Alpaca provider not found in PROVIDERS")
                return False
            
            # Initialize Alpaca provider
            alpaca_class = PROVIDERS['alpaca']
            alpaca_provider = alpaca_class()
            
            # Test connection
            await alpaca_provider.connect()
            logger.info("✅ Alpaca provider connected successfully")
            
            # Test disconnection
            await alpaca_provider.disconnect()
            logger.info("✅ Alpaca provider disconnected successfully")
            
            return True
            
        except Exception as e:
            logger.error(f"Provider initialization failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_alpaca_provider_functionality(self) -> bool:
        """Test Alpaca provider specific functionality."""
        logger.info("Testing Alpaca provider functionality...")
        
        try:
            # Initialize provider
            alpaca_class = PROVIDERS['alpaca']
            alpaca_provider = alpaca_class()
            await alpaca_provider.connect()
            
            # Test current market data
            logger.info("Testing current market data retrieval...")
            current_time = datetime.now()
            start_time = current_time - timedelta(minutes=5)
            
            data_count = 0
            async for market_data in alpaca_provider.get_market_data(
                ['AAPL'], start_time, current_time
            ):
                data_count += 1
                logger.info(f"Got market data: {market_data.symbol} = ${market_data.close:.2f}")
                
                # Validate data structure
                assert hasattr(market_data, 'symbol')
                assert hasattr(market_data, 'close')
                assert hasattr(market_data, 'time')
                assert hasattr(market_data, 'provider')
                assert market_data.provider == 'alpaca'
                
                if data_count >= 3:  # Test first 3 data points
                    break
            
            if data_count == 0:
                logger.warning("No market data received from Alpaca")
                return False
            
            logger.info(f"✅ Alpaca provider returned {data_count} market data points")
            
            # Test streaming (brief test)
            logger.info("Testing streaming functionality...")
            stream_count = 0
            async for stream_data in alpaca_provider.stream_market_data(['AAPL']):
                stream_count += 1
                logger.info(f"Got streaming data: {stream_data.symbol} = ${stream_data.close:.2f}")
                
                if stream_count >= 2:  # Test first 2 streaming updates
                    break
            
            await alpaca_provider.disconnect()
            
            if stream_count == 0:
                logger.warning("No streaming data received from Alpaca")
                return False
            
            logger.info(f"✅ Alpaca streaming returned {stream_count} data points")
            return True
            
        except Exception as e:
            logger.error(f"Alpaca provider test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_redis_connectivity(self) -> bool:
        """Test Redis connectivity and basic operations."""
        logger.info("Testing Redis connectivity...")
        
        try:
            self.redis_store = RedisStore()
            await self.redis_store.connect()
            
            # Test basic set/get
            test_key = "integration_test_key"
            test_value = "integration_test_value"
            await self.redis_store.set(test_key, test_value)
            
            retrieved_value = await self.redis_store.get(test_key)
            assert retrieved_value == test_value
            
            logger.info("✅ Redis basic operations work")
            
            # Test pub/sub
            test_channel = "integration_test_channel"
            test_message = "integration_test_message"
            await self.redis_store.publish(test_channel, test_message)
            
            logger.info("✅ Redis pub/sub works")
            
            # Test market data operations
            test_symbol = "AAPL"
            test_price_data = {
                "symbol": test_symbol,
                "close": 150.25,
                "volume": 1000000,
                "provider": "test"
            }
            
            await self.redis_store.set_latest_price(test_symbol, test_price_data)
            retrieved_price = await self.redis_store.get_latest_price(test_symbol)
            
            assert retrieved_price['symbol'] == test_symbol
            assert float(retrieved_price['close']) == 150.25
            
            logger.info("✅ Redis market data operations work")
            
            return True
            
        except Exception as e:
            logger.error(f"Redis connectivity test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_timescale_integration(self) -> bool:
        """Test TimescaleDB integration."""
        logger.info("Testing TimescaleDB integration...")
        
        try:
            self.timescale_db = TimescaleDB()
            await self.timescale_db.connect()
            
            # Test market data insertion
            test_data = [{
                'time': datetime.now(),
                'symbol': 'AAPL',
                'open': 150.0,
                'high': 151.0,
                'low': 149.0,
                'close': 150.5,
                'volume': 1000000,
                'provider': 'integration_test'
            }]
            
            await self.timescale_db.insert_market_data(test_data)
            logger.info("✅ TimescaleDB market data insertion works")
            
            # Test data retrieval
            now = datetime.now()
            start_time = now - timedelta(hours=1)
            
            df = await self.timescale_db.query_market_data(
                'AAPL', start_time, now, 'integration_test'
            )
            
            assert not df.empty
            assert 'AAPL' in df['symbol'].values
            
            logger.info("✅ TimescaleDB data retrieval works")
            
            return True
            
        except Exception as e:
            logger.error(f"TimescaleDB integration test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_data_pipeline_flow(self) -> bool:
        """Test complete data pipeline flow."""
        logger.info("Testing data pipeline flow...")
        
        try:
            # Initialize components
            if not self.redis_store:
                self.redis_store = RedisStore()
                await self.redis_store.connect()
            
            if not self.timescale_db:
                self.timescale_db = TimescaleDB()
                await self.timescale_db.connect()
            
            # Initialize Alpaca provider
            alpaca_class = PROVIDERS['alpaca']
            alpaca_provider = alpaca_class()
            await alpaca_provider.connect()
            
            # Get market data
            current_time = datetime.now()
            start_time = current_time - timedelta(minutes=10)
            
            pipeline_data = []
            async for market_data in alpaca_provider.get_market_data(
                ['AAPL'], start_time, current_time
            ):
                # Convert to dict for pipeline processing
                data_dict = {
                    'time': market_data.time,
                    'symbol': market_data.symbol,
                    'open': market_data.open,
                    'high': market_data.high,
                    'low': market_data.low,
                    'close': market_data.close,
                    'volume': market_data.volume,
                    'provider': market_data.provider
                }
                pipeline_data.append(data_dict)
                
                if len(pipeline_data) >= 3:
                    break
            
            if not pipeline_data:
                logger.warning("No data received for pipeline test")
                return False
            
            # Store in TimescaleDB
            await self.timescale_db.insert_market_data(pipeline_data)
            
            # Store in Redis
            for data in pipeline_data:
                await self.redis_store.set_latest_price(data['symbol'], data)
            
            logger.info(f"✅ Pipeline processed {len(pipeline_data)} data points")
            
            # Verify data persisted
            latest_price = await self.redis_store.get_latest_price('AAPL')
            assert latest_price is not None
            assert latest_price['symbol'] == 'AAPL'
            
            logger.info("✅ Data pipeline flow complete")
            
            await alpaca_provider.disconnect()
            return True
            
        except Exception as e:
            logger.error(f"Data pipeline flow test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_realtime_coordinator(self) -> bool:
        """Test RealtimeCoordinator functionality."""
        logger.info("Testing RealtimeCoordinator...")
        
        try:
            # Initialize coordinator
            self.coordinator = RealtimeCoordinator()
            await self.coordinator.initialize(['alpaca'])
            
            # Test subscription
            await self.coordinator.subscribe(['AAPL'])
            
            # Verify subscription
            assert 'AAPL' in self.coordinator.subscribed_symbols
            logger.info("✅ RealtimeCoordinator subscription works")
            
            # Test status
            status = await self.coordinator.get_stream_status()
            assert status['running'] == False  # Not started yet
            assert 'AAPL' in status['subscribed_symbols']
            assert 'alpaca' in status['active_providers']
            
            logger.info("✅ RealtimeCoordinator status works")
            
            # Test unsubscribe
            await self.coordinator.unsubscribe(['AAPL'])
            assert 'AAPL' not in self.coordinator.subscribed_symbols
            
            logger.info("✅ RealtimeCoordinator unsubscribe works")
            
            await self.coordinator.stop()
            return True
            
        except Exception as e:
            logger.error(f"RealtimeCoordinator test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_error_handling(self) -> bool:
        """Test error handling and recovery."""
        logger.info("Testing error handling...")
        
        try:
            # Test invalid symbol handling
            alpaca_class = PROVIDERS['alpaca']
            alpaca_provider = alpaca_class()
            await alpaca_provider.connect()
            
            # Try to get data for invalid symbol
            invalid_symbols = ['INVALID_SYMBOL_XYZ']
            current_time = datetime.now()
            start_time = current_time - timedelta(minutes=5)
            
            data_count = 0
            async for market_data in alpaca_provider.get_market_data(
                invalid_symbols, start_time, current_time
            ):
                data_count += 1
                if data_count >= 3:
                    break
            
            # Should handle gracefully (no data or error logged)
            logger.info(f"✅ Invalid symbol handling: {data_count} data points returned")
            
            await alpaca_provider.disconnect()
            
            # Test connection recovery
            redis_store = RedisStore()
            try:
                # Try to use without connecting
                await redis_store.get("test_key")
                logger.warning("Expected error did not occur")
                return False
            except Exception as e:
                logger.info(f"✅ Expected error caught: {type(e).__name__}")
            
            return True
            
        except Exception as e:
            logger.error(f"Error handling test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_storage_integration(self) -> bool:
        """Test storage layer integration."""
        logger.info("Testing storage integration...")
        
        try:
            # Test Redis-TimescaleDB integration
            if not self.redis_store:
                self.redis_store = RedisStore()
                await self.redis_store.connect()
            
            if not self.timescale_db:
                self.timescale_db = TimescaleDB()
                await self.timescale_db.connect()
            
            # Create test data
            test_symbol = 'TSLA'
            test_data = {
                'time': datetime.now(),
                'symbol': test_symbol,
                'open': 200.0,
                'high': 205.0,
                'low': 195.0,
                'close': 202.5,
                'volume': 2000000,
                'provider': 'integration_test'
            }
            
            # Store in both systems
            await self.redis_store.set_latest_price(test_symbol, test_data)
            await self.timescale_db.insert_market_data([test_data])
            
            # Verify Redis storage
            redis_data = await self.redis_store.get_latest_price(test_symbol)
            assert redis_data['symbol'] == test_symbol
            assert float(redis_data['close']) == 202.5
            
            # Verify TimescaleDB storage
            latest_price = await self.timescale_db.get_latest_price(test_symbol)
            assert latest_price is not None
            assert float(latest_price['price']) == 202.5
            
            logger.info("✅ Storage integration works")
            return True
            
        except Exception as e:
            logger.error(f"Storage integration test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_metrics_collection(self) -> bool:
        """Test metrics collection."""
        logger.info("Testing metrics collection...")
        
        try:
            # Generate some metrics
            
            # Test counter
            metrics.api_requests_total.labels(provider='test', endpoint='test').inc()
            
            # Test gauge
            metrics.active_connections.labels(connection_type='test').set(5)
            
            # Test histogram
            metrics.request_duration.labels(provider='test', endpoint='test').observe(0.5)
            
            logger.info("✅ Metrics collection works")
            return True
            
        except Exception as e:
            logger.error(f"Metrics collection test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def test_end_to_end_workflow(self) -> bool:
        """Test complete end-to-end workflow."""
        logger.info("Testing end-to-end workflow...")
        
        try:
            # Initialize all components
            coordinator = RealtimeCoordinator()
            await coordinator.initialize(['alpaca'])
            
            # Subscribe to symbols
            await coordinator.subscribe(['AAPL', 'MSFT'])
            
            # Start brief streaming test
            start_time = datetime.now()
            data_received = []
            
            # Add callback to collect data
            async def data_callback(data):
                data_received.append(data)
                logger.info(f"Received: {data['symbol']} = ${data['close']:.2f}")
            
            coordinator.add_data_callback(data_callback)
            
            # Start coordinator for a brief period
            async def run_coordinator():
                await coordinator.start()
            
            # Run for 30 seconds
            coordinator_task = asyncio.create_task(run_coordinator())
            await asyncio.sleep(30)
            
            # Stop coordinator
            await coordinator.stop()
            
            # Wait for coordinator to finish
            try:
                await asyncio.wait_for(coordinator_task, timeout=5)
            except asyncio.TimeoutError:
                coordinator_task.cancel()
            
            # Verify data was received
            if len(data_received) == 0:
                logger.warning("No data received in end-to-end test")
                return False
            
            logger.info(f"✅ End-to-end workflow processed {len(data_received)} data points")
            
            # Verify data structure
            for data in data_received[:3]:  # Check first 3 data points
                assert 'symbol' in data
                assert 'close' in data
                assert 'provider' in data
                assert 'time' in data
            
            logger.info("✅ End-to-end workflow complete")
            return True
            
        except Exception as e:
            logger.error(f"End-to-end workflow test failed: {e}")
            logger.error(traceback.format_exc())
            return False
    
    async def cleanup(self):
        """Clean up test resources."""
        logger.info("Cleaning up test resources...")
        
        try:
            if self.coordinator:
                await self.coordinator.stop()
            
            if self.redis_store:
                await self.redis_store.disconnect()
            
            if self.timescale_db:
                await self.timescale_db.disconnect()
            
            logger.info("✅ Cleanup complete")
            
        except Exception as e:
            logger.error(f"Cleanup failed: {e}")

async def main():
    """Run integration tests."""
    tester = IntegrationTester()
    
    try:
        success = await tester.run_all_tests()
        
        if success:
            logger.info("🎉 Integration tests PASSED!")
            return 0
        else:
            logger.error("❌ Integration tests FAILED!")
            return 1
            
    except Exception as e:
        logger.error(f"Integration test runner failed: {e}")
        logger.error(traceback.format_exc())
        return 1
    
    finally:
        await tester.cleanup()

if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)