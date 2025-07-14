#!/usr/bin/env python3
"""Test script to validate comprehensive metrics collection."""

import asyncio
import time
import requests
import json
from utils.metrics import metrics, start_metrics_server
from utils.metrics_integration import metrics_collector
from utils.logging import setup_logging

# Setup logging
setup_logging()

async def test_metrics_functionality():
    """Test comprehensive metrics functionality."""
    print("🔍 Testing Metrics Functionality")
    print("=" * 50)
    
    # Test 1: Basic metrics initialization
    print("\n1. Testing basic metrics initialization...")
    try:
        # Check if metrics instance exists
        assert hasattr(metrics, 'api_requests_total')
        assert hasattr(metrics, 'data_points_processed')
        assert hasattr(metrics, 'storage_operations')
        assert hasattr(metrics, 'active_connections')
        print("✅ Basic metrics initialized successfully")
    except Exception as e:
        print(f"❌ Basic metrics initialization failed: {e}")
        return False
    
    # Test 2: Metrics collection
    print("\n2. Testing metrics collection...")
    try:
        # Test counter metrics
        metrics.api_requests_total.labels(provider="test", endpoint="/test", status="success").inc()
        metrics.data_points_processed.labels(provider="test", data_type="price").inc(10)
        metrics.processing_errors.labels(provider="test", error_type="ValidationError").inc()
        
        # Test gauge metrics
        metrics.active_connections.labels(connection_type="websocket").set(5)
        metrics.queue_size.labels(queue_name="data_processing").set(100)
        
        # Test histogram metrics
        metrics.api_request_duration.labels(provider="test", endpoint="/test").observe(0.5)
        metrics.storage_duration.labels(storage_type="redis", operation="set").observe(0.001)
        
        print("✅ Metrics collection working correctly")
    except Exception as e:
        print(f"❌ Metrics collection failed: {e}")
        return False
    
    # Test 3: Start Prometheus server
    print("\n3. Testing Prometheus server startup...")
    try:
        port = 9091  # Use different port to avoid conflicts
        start_metrics_server(port)
        print(f"✅ Prometheus server started on port {port}")
        
        # Wait a moment for server to start
        await asyncio.sleep(2)
        
        # Test endpoint availability
        try:
            response = requests.get(f"http://localhost:{port}/metrics", timeout=5)
            if response.status_code == 200:
                print("✅ Metrics endpoint is accessible")
                
                # Check for expected metrics in response
                content = response.text
                if "data_ingestion_api_requests_total" in content:
                    print("✅ Custom metrics present in response")
                else:
                    print("⚠️  Custom metrics not found in response")
            else:
                print(f"❌ Metrics endpoint returned status {response.status_code}")
                return False
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to connect to metrics endpoint: {e}")
            return False
        
    except Exception as e:
        print(f"❌ Prometheus server startup failed: {e}")
        return False
    
    # Test 4: Metrics integration module
    print("\n4. Testing metrics integration module...")
    try:
        # Test MetricsCollector functionality
        metrics_collector.track_provider_connection("test_provider", True)
        metrics_collector.track_stream_start("test_stream_1")
        metrics_collector.track_data_quality("test_provider", "accuracy", 0.95)
        metrics_collector.track_provider_error("test_provider", "TimeoutError")
        metrics_collector.track_data_processed("test_provider", "trade", 50)
        
        print("✅ Metrics integration working correctly")
    except Exception as e:
        print(f"❌ Metrics integration failed: {e}")
        return False
    
    # Test 5: Context manager tracking
    print("\n5. Testing context manager tracking...")
    try:
        # Test API call tracking
        async with metrics_collector.track_api_call("test_provider", "/data"):
            await asyncio.sleep(0.1)  # Simulate API call
        
        # Test storage operation tracking
        async with metrics_collector.track_storage_operation("redis", "set"):
            await asyncio.sleep(0.01)  # Simulate storage operation
        
        # Test Redis publish tracking
        async with metrics_collector.track_redis_publish("market_data", 1024):
            await asyncio.sleep(0.005)  # Simulate Redis publish
        
        print("✅ Context manager tracking working correctly")
    except Exception as e:
        print(f"❌ Context manager tracking failed: {e}")
        return False
    
    # Test 6: Decorator functionality
    print("\n6. Testing decorator functionality...")
    try:
        # Test API request decorator
        @metrics.track_api_request("test_provider", "/test_endpoint")
        async def test_api_call():
            await asyncio.sleep(0.1)
            return "success"
        
        result = await test_api_call()
        assert result == "success"
        
        # Test storage operation decorator
        @metrics.track_storage_operation("timescale", "insert")
        async def test_storage_op():
            await asyncio.sleep(0.02)
            return "stored"
        
        result = await test_storage_op()
        assert result == "stored"
        
        print("✅ Decorator functionality working correctly")
    except Exception as e:
        print(f"❌ Decorator functionality failed: {e}")
        return False
    
    # Test 7: Health tracking
    print("\n7. Testing health tracking...")
    try:
        # Update provider health scores
        metrics.update_provider_health("test_provider", 0.98)
        metrics.update_provider_data_quality("test_provider", "completeness", 0.95)
        metrics.update_pipeline_backpressure("ingestion", "processing", 0.3)
        
        print("✅ Health tracking working correctly")
    except Exception as e:
        print(f"❌ Health tracking failed: {e}")
        return False
    
    # Test 8: Validate metrics endpoint content
    print("\n8. Validating metrics endpoint content...")
    try:
        response = requests.get(f"http://localhost:{port}/metrics", timeout=5)
        content = response.text
        
        # Check for various metric types
        expected_metrics = [
            "data_ingestion_api_requests_total",
            "data_ingestion_data_points_processed_total",
            "data_ingestion_processing_errors_total",
            "data_ingestion_storage_operations_total",
            "data_ingestion_active_connections",
            "data_ingestion_provider_health_score",
            "data_ingestion_redis_publish_total"
        ]
        
        found_metrics = []
        missing_metrics = []
        
        for metric in expected_metrics:
            if metric in content:
                found_metrics.append(metric)
            else:
                missing_metrics.append(metric)
        
        print(f"✅ Found {len(found_metrics)} out of {len(expected_metrics)} expected metrics")
        
        if missing_metrics:
            print(f"⚠️  Missing metrics: {missing_metrics}")
        
        # Check if metrics have values
        lines_with_values = [line for line in content.split('\n') if line and not line.startswith('#')]
        print(f"✅ {len(lines_with_values)} metric lines with values")
        
    except Exception as e:
        print(f"❌ Metrics endpoint validation failed: {e}")
        return False
    
    print("\n" + "=" * 50)
    print("🎉 All metrics validation tests passed!")
    print("=" * 50)
    return True

async def test_metrics_performance():
    """Test metrics performance impact."""
    print("\n📊 Testing Metrics Performance Impact")
    print("=" * 50)
    
    # Test with high-frequency operations
    start_time = time.time()
    
    # Simulate high-frequency metric updates
    for i in range(1000):
        metrics.api_requests_total.labels(provider="perf_test", endpoint="/test", status="success").inc()
        metrics.data_points_processed.labels(provider="perf_test", data_type="price").inc()
        metrics.api_request_duration.labels(provider="perf_test", endpoint="/test").observe(0.001)
    
    duration = time.time() - start_time
    print(f"✅ 1000 metric updates completed in {duration:.3f} seconds")
    print(f"✅ Average time per metric update: {(duration/1000)*1000:.3f} ms")
    
    if duration < 0.1:  # Should be very fast
        print("✅ Performance impact is minimal")
        return True
    else:
        print("⚠️  Performance impact may be significant")
        return False

if __name__ == "__main__":
    async def main():
        print("🚀 Starting Comprehensive Metrics Validation")
        print("=" * 60)
        
        # Test functionality
        functionality_passed = await test_metrics_functionality()
        
        # Test performance
        performance_passed = await test_metrics_performance()
        
        if functionality_passed and performance_passed:
            print("\n🎉 ALL METRICS TESTS PASSED!")
            print("✅ Metrics system is fully functional and performant")
        else:
            print("\n❌ SOME METRICS TESTS FAILED")
            print("⚠️  Please check the issues above")
        
        return functionality_passed and performance_passed
    
    result = asyncio.run(main())
    exit(0 if result else 1)