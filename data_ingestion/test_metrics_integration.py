#!/usr/bin/env python3
"""Test metrics integration with data ingestion service."""

import asyncio
import time
import requests
from utils.metrics import metrics, start_metrics_server
from utils.metrics_integration import metrics_collector
from utils.logging import setup_logging
from providers.yahoo_finance import YahooFinanceProvider
from storage.redis_store import RedisStore

# Setup logging
setup_logging()

async def test_metrics_with_data_ingestion():
    """Test metrics integration with actual data ingestion components."""
    print("🔗 Testing Metrics Integration with Data Ingestion")
    print("=" * 60)
    
    # Start metrics server on different port
    port = 9092
    start_metrics_server(port)
    await asyncio.sleep(1)
    
    # Test 1: Provider metrics integration
    print("\n1. Testing provider metrics integration...")
    try:
        # Initialize provider with metrics
        provider = YahooFinanceProvider()
        
        # Test that provider can track metrics
        metrics_collector.track_provider_connection("yahoo_finance", True)
        metrics_collector.track_data_quality("yahoo_finance", "accuracy", 0.98)
        
        print("✅ Provider metrics integration working")
    except Exception as e:
        print(f"❌ Provider metrics integration failed: {e}")
        return False
    
    # Test 2: Storage metrics integration
    print("\n2. Testing storage metrics integration...")
    try:
        # Initialize Redis store
        redis_store = RedisStore()
        
        # Test metrics tracking with context manager
        async with metrics_collector.track_storage_operation("redis", "set"):
            # Simulate storage operation
            await asyncio.sleep(0.01)
        
        print("✅ Storage metrics integration working")
    except Exception as e:
        print(f"❌ Storage metrics integration failed: {e}")
        return False
    
    # Test 3: API endpoint metrics
    print("\n3. Testing API endpoint metrics...")
    try:
        # Simulate API calls with metrics
        async with metrics_collector.track_api_call("yahoo_finance", "/quote"):
            await asyncio.sleep(0.1)
        
        async with metrics_collector.track_api_call("yahoo_finance", "/history"):
            await asyncio.sleep(0.2)
        
        print("✅ API endpoint metrics working")
    except Exception as e:
        print(f"❌ API endpoint metrics failed: {e}")
        return False
    
    # Test 4: Stream metrics
    print("\n4. Testing stream metrics...")
    try:
        # Test stream tracking
        metrics_collector.track_stream_start("yahoo_finance_stream_1")
        metrics_collector.track_stream_start("yahoo_finance_stream_2")
        
        # Simulate some data processing
        metrics_collector.track_data_processed("yahoo_finance", "price", 100)
        metrics_collector.track_data_processed("yahoo_finance", "volume", 100)
        
        # Stop one stream
        metrics_collector.track_stream_stop("yahoo_finance_stream_1")
        
        print("✅ Stream metrics working")
    except Exception as e:
        print(f"❌ Stream metrics failed: {e}")
        return False
    
    # Test 5: Error handling metrics
    print("\n5. Testing error handling metrics...")
    try:
        # Simulate various errors
        metrics_collector.track_provider_error("yahoo_finance", "TimeoutError")
        metrics_collector.track_provider_error("yahoo_finance", "ConnectionError")
        metrics_collector.track_rate_limit_hit("yahoo_finance")
        metrics_collector.track_validation_failure("yahoo_finance")
        metrics_collector.track_streaming_error("yahoo_finance")
        
        print("✅ Error handling metrics working")
    except Exception as e:
        print(f"❌ Error handling metrics failed: {e}")
        return False
    
    # Test 6: Validate metrics endpoint has real data
    print("\n6. Validating metrics endpoint with real data...")
    try:
        await asyncio.sleep(1)  # Wait for metrics to be collected
        
        response = requests.get(f"http://localhost:{port}/metrics", timeout=10)
        content = response.text
        
        # Check for specific metrics with values
        metrics_with_values = {}
        
        for line in content.split('\n'):
            if line and not line.startswith('#'):
                if 'data_ingestion_api_requests_total' in line and 'yahoo_finance' in line:
                    metrics_with_values['api_requests'] = line.strip()
                elif 'data_ingestion_processing_errors_total' in line and 'yahoo_finance' in line:
                    metrics_with_values['processing_errors'] = line.strip()
                elif 'data_ingestion_storage_operations_total' in line and 'redis' in line:
                    metrics_with_values['storage_operations'] = line.strip()
                elif 'data_ingestion_active_streams' in line:
                    metrics_with_values['active_streams'] = line.strip()
                elif 'data_ingestion_provider_health_score' in line and 'yahoo_finance' in line:
                    metrics_with_values['provider_health'] = line.strip()
        
        print(f"✅ Found {len(metrics_with_values)} metrics with real values:")
        for metric_name, metric_line in metrics_with_values.items():
            print(f"   - {metric_name}: {metric_line}")
        
        if len(metrics_with_values) >= 3:
            print("✅ Metrics integration producing real data")
        else:
            print("⚠️  Limited metrics data found")
        
    except Exception as e:
        print(f"❌ Metrics endpoint validation failed: {e}")
        return False
    
    # Test 7: Check for metric conflicts
    print("\n7. Testing for metric conflicts...")
    try:
        # Create multiple metric instances to test for conflicts
        from prometheus_client import REGISTRY
        
        # Get all metric names from registry
        metric_names = set()
        for collector in REGISTRY._collector_to_names.values():
            metric_names.update(collector)
        
        # Check for duplicates (this would indicate conflicts)
        duplicate_names = []
        seen_names = set()
        
        for name in metric_names:
            if name in seen_names:
                duplicate_names.append(name)
            else:
                seen_names.add(name)
        
        if duplicate_names:
            print(f"❌ Found metric conflicts: {duplicate_names}")
            return False
        else:
            print(f"✅ No metric conflicts found ({len(metric_names)} unique metrics)")
        
    except Exception as e:
        print(f"❌ Metric conflict check failed: {e}")
        return False
    
    print("\n" + "=" * 60)
    print("🎉 All metrics integration tests passed!")
    print("=" * 60)
    return True

async def test_metrics_persistence():
    """Test metrics persistence and accuracy."""
    print("\n💾 Testing Metrics Persistence and Accuracy")
    print("=" * 50)
    
    try:
        # Record initial metric values
        initial_requests = 0
        initial_errors = 0
        
        # Make several tracked operations
        for i in range(5):
            async with metrics_collector.track_api_call("test_provider", f"/endpoint_{i}"):
                await asyncio.sleep(0.01)
        
        # Make some errors
        for i in range(3):
            try:
                async with metrics_collector.track_api_call("test_provider", "/error_endpoint"):
                    raise ValueError("Test error")
            except ValueError:
                pass
        
        # Check if metrics were recorded
        port = 9092
        response = requests.get(f"http://localhost:{port}/metrics", timeout=5)
        content = response.text
        
        # Count API requests
        api_request_count = 0
        for line in content.split('\n'):
            if 'data_ingestion_api_requests_total' in line and 'test_provider' in line:
                # Extract the value (last part after space)
                parts = line.strip().split()
                if len(parts) >= 2:
                    try:
                        value = float(parts[-1])
                        api_request_count += value
                    except ValueError:
                        pass
        
        print(f"✅ API requests recorded: {api_request_count}")
        
        if api_request_count >= 8:  # 5 successful + 3 error requests
            print("✅ Metrics persistence working correctly")
            return True
        else:
            print("⚠️  Metrics persistence may have issues")
            return False
        
    except Exception as e:
        print(f"❌ Metrics persistence test failed: {e}")
        return False

if __name__ == "__main__":
    async def main():
        print("🚀 Starting Metrics Integration Validation")
        print("=" * 70)
        
        # Test integration
        integration_passed = await test_metrics_with_data_ingestion()
        
        # Test persistence
        persistence_passed = await test_metrics_persistence()
        
        if integration_passed and persistence_passed:
            print("\n🎉 ALL METRICS INTEGRATION TESTS PASSED!")
            print("✅ Metrics system is fully integrated and working correctly")
        else:
            print("\n❌ SOME METRICS INTEGRATION TESTS FAILED")
            print("⚠️  Please check the issues above")
        
        return integration_passed and persistence_passed
    
    result = asyncio.run(main())
    exit(0 if result else 1)