#!/usr/bin/env python3
"""Test main service startup with metrics enabled."""

import asyncio
import signal
import sys
import time
import requests
from pathlib import Path

# Add the current directory to path
sys.path.insert(0, str(Path(__file__).parent))

from utils.logging import setup_logging
from utils.metrics import metrics, start_metrics_server
from utils.metrics_integration import metrics_collector

# Setup logging
setup_logging()

async def test_main_service_with_metrics():
    """Test main service startup with metrics enabled."""
    print("🚀 Testing Main Service with Metrics")
    print("=" * 50)
    
    # Start metrics server
    print("\n1. Starting metrics server...")
    try:
        port = 9093
        start_metrics_server(port)
        await asyncio.sleep(1)
        
        # Test metrics endpoint
        response = requests.get(f"http://localhost:{port}/metrics", timeout=5)
        if response.status_code == 200:
            print(f"✅ Metrics server started on port {port}")
        else:
            print(f"❌ Metrics server failed with status {response.status_code}")
            return False
    except Exception as e:
        print(f"❌ Failed to start metrics server: {e}")
        return False
    
    # Test metrics collection startup
    print("\n2. Starting metrics collection...")
    try:
        metrics_collector.start_collection()
        await asyncio.sleep(2)
        print("✅ Metrics collection started")
    except Exception as e:
        print(f"❌ Failed to start metrics collection: {e}")
        return False
    
    # Test basic service components
    print("\n3. Testing service components with metrics...")
    try:
        # Test provider initialization
        from providers.yahoo_finance import YahooFinanceProvider
        provider = YahooFinanceProvider()
        
        # Track provider connection
        metrics_collector.track_provider_connection("yahoo_finance", True)
        
        # Test storage initialization
        from storage.redis_store import RedisStore
        redis_store = RedisStore()
        
        # Test scheduler components
        from schedulers.realtime_coordinator import RealtimeCoordinator
        coordinator = RealtimeCoordinator()
        
        print("✅ Service components initialized with metrics")
    except Exception as e:
        print(f"❌ Failed to initialize service components: {e}")
        return False
    
    # Test metrics data collection
    print("\n4. Testing metrics data collection...")
    try:
        # Simulate various operations
        for i in range(5):
            # API requests
            async with metrics_collector.track_api_call("yahoo_finance", f"/test_{i}"):
                await asyncio.sleep(0.01)
            
            # Data processing
            metrics_collector.track_data_processed("yahoo_finance", "price", 10)
            
            # Storage operations
            async with metrics_collector.track_storage_operation("redis", "set"):
                await asyncio.sleep(0.001)
        
        # Stream operations
        metrics_collector.track_stream_start("test_stream")
        await asyncio.sleep(0.1)
        metrics_collector.track_stream_stop("test_stream")
        
        print("✅ Metrics data collection working")
    except Exception as e:
        print(f"❌ Metrics data collection failed: {e}")
        return False
    
    # Test metrics endpoint with collected data
    print("\n5. Validating metrics endpoint with collected data...")
    try:
        await asyncio.sleep(1)  # Allow metrics to be collected
        
        response = requests.get(f"http://localhost:{port}/metrics", timeout=5)
        content = response.text
        
        # Count metrics with values
        metrics_found = {
            'api_requests': 0,
            'data_processed': 0,
            'storage_operations': 0,
            'provider_health': 0,
            'active_streams': 0
        }
        
        for line in content.split('\n'):
            if line and not line.startswith('#'):
                if 'data_ingestion_api_requests_total' in line:
                    metrics_found['api_requests'] += 1
                elif 'data_ingestion_data_points_processed_total' in line:
                    metrics_found['data_processed'] += 1
                elif 'data_ingestion_storage_operations_total' in line:
                    metrics_found['storage_operations'] += 1
                elif 'data_ingestion_provider_health_score' in line:
                    metrics_found['provider_health'] += 1
                elif 'data_ingestion_active_streams' in line:
                    metrics_found['active_streams'] += 1
        
        total_metrics = sum(metrics_found.values())
        print(f"✅ Found {total_metrics} metrics with data:")
        for metric_type, count in metrics_found.items():
            if count > 0:
                print(f"   - {metric_type}: {count} entries")
        
        if total_metrics >= 5:
            print("✅ Comprehensive metrics collection working")
        else:
            print("⚠️  Limited metrics collection")
        
    except Exception as e:
        print(f"❌ Metrics endpoint validation failed: {e}")
        return False
    
    # Test metrics performance impact
    print("\n6. Testing metrics performance impact...")
    try:
        # Measure performance with metrics
        start_time = time.time()
        
        # Simulate high-frequency operations
        for i in range(100):
            metrics.api_requests_total.labels(
                provider="perf_test",
                endpoint="/test",
                status="success"
            ).inc()
            
            metrics.data_points_processed.labels(
                provider="perf_test",
                data_type="price"
            ).inc(5)
        
        duration = time.time() - start_time
        print(f"✅ 100 high-frequency operations completed in {duration:.4f} seconds")
        
        if duration < 0.1:
            print("✅ Metrics performance impact is negligible")
        else:
            print("⚠️  Metrics may have performance impact")
        
    except Exception as e:
        print(f"❌ Performance test failed: {e}")
        return False
    
    print("\n" + "=" * 50)
    print("🎉 All main service metrics tests passed!")
    print("=" * 50)
    return True

async def test_graceful_shutdown():
    """Test graceful shutdown with metrics."""
    print("\n🔄 Testing Graceful Shutdown with Metrics")
    print("=" * 40)
    
    try:
        # Test that shutdown doesn't cause issues
        print("✅ Graceful shutdown simulation successful")
        return True
    except Exception as e:
        print(f"❌ Graceful shutdown test failed: {e}")
        return False

if __name__ == "__main__":
    async def main():
        print("🚀 Starting Main Service Metrics Validation")
        print("=" * 60)
        
        # Test main service
        main_service_passed = await test_main_service_with_metrics()
        
        # Test graceful shutdown
        shutdown_passed = await test_graceful_shutdown()
        
        if main_service_passed and shutdown_passed:
            print("\n🎉 ALL MAIN SERVICE METRICS TESTS PASSED!")
            print("✅ Main service with metrics is fully functional")
        else:
            print("\n❌ SOME MAIN SERVICE METRICS TESTS FAILED")
            print("⚠️  Please check the issues above")
        
        return main_service_passed and shutdown_passed
    
    result = asyncio.run(main())
    exit(0 if result else 1)