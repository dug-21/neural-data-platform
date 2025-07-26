#!/usr/bin/env python3
"""Test script for Prometheus metrics integration."""
import asyncio
import aiohttp
import sys
from datetime import datetime

# Test metrics endpoint
async def test_metrics_endpoint(port=8001):
    """Test the Prometheus metrics endpoint."""
    metrics_url = f"http://localhost:{port}/metrics"
    
    print(f"Testing metrics endpoint at {metrics_url}")
    print("=" * 60)
    
    async with aiohttp.ClientSession() as session:
        try:
            async with session.get(metrics_url) as response:
                if response.status == 200:
                    text = await response.text()
                    
                    # Check for Phase 4 metrics
                    phase4_metrics = [
                        'data_ingestion_websocket_connections',
                        'data_ingestion_websocket_messages_total',
                        'data_ingestion_websocket_reconnections_total',
                        'data_ingestion_health_status',
                        'data_ingestion_health_check_duration_seconds',
                        'data_ingestion_circuit_breaker_state',
                        'data_ingestion_circuit_breaker_failures_total',
                        'data_ingestion_file_backfill_progress',
                        'data_ingestion_file_backfill_rows_total',
                        'data_ingestion_file_backfill_duration_seconds',
                        'data_ingestion_data_flow_age_seconds',
                        'data_ingestion_data_flow_rate'
                    ]
                    
                    print("✅ Metrics endpoint is accessible")
                    print(f"Response length: {len(text)} bytes")
                    print("\nPhase 4 Metrics Check:")
                    print("-" * 40)
                    
                    found_metrics = []
                    missing_metrics = []
                    
                    for metric in phase4_metrics:
                        if metric in text:
                            found_metrics.append(metric)
                            print(f"✅ {metric}")
                        else:
                            missing_metrics.append(metric)
                            print(f"❌ {metric}")
                    
                    print(f"\nSummary: {len(found_metrics)}/{len(phase4_metrics)} metrics found")
                    
                    # Show sample of actual metrics
                    print("\nSample metrics output:")
                    print("-" * 40)
                    lines = text.split('\n')
                    for line in lines[:50]:  # First 50 lines
                        if line and not line.startswith('#'):
                            print(line)
                    
                    return True
                    
                else:
                    print(f"❌ Metrics endpoint returned status {response.status}")
                    return False
                    
        except aiohttp.ClientError as e:
            print(f"❌ Connection error: {e}")
            print("\nMake sure the data-ingestion service is running on port {port}")
            return False
        except Exception as e:
            print(f"❌ Unexpected error: {e}")
            return False


async def test_health_endpoint(port=8001):
    """Test the health endpoint with circuit breaker info."""
    health_url = f"http://localhost:{port}/health/detailed"
    
    print(f"\n\nTesting health endpoint at {health_url}")
    print("=" * 60)
    
    async with aiohttp.ClientSession() as session:
        try:
            async with session.get(health_url) as response:
                if response.status in [200, 503]:
                    data = await response.json()
                    
                    print(f"Health Status: {data.get('status', 'unknown')}")
                    
                    # Check circuit breakers
                    checks = data.get('checks', {})
                    for component, details in checks.items():
                        if isinstance(details, dict) and 'circuit_breaker' in details.get('details', {}):
                            cb_state = details['details'].get('circuit_breaker', 'unknown')
                            print(f"  - {component}: circuit_breaker={cb_state}")
                    
                    return True
                else:
                    print(f"❌ Health endpoint returned status {response.status}")
                    return False
                    
        except Exception as e:
            print(f"❌ Error checking health: {e}")
            return False


async def main():
    """Run all tests."""
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8001
    
    print(f"Starting Prometheus metrics tests on port {port}")
    print(f"Time: {datetime.now()}")
    print("=" * 60)
    
    # Test metrics endpoint
    metrics_ok = await test_metrics_endpoint(port)
    
    # Test health endpoint
    health_ok = await test_health_endpoint(port)
    
    print("\n" + "=" * 60)
    if metrics_ok and health_ok:
        print("✅ All tests passed!")
        return 0
    else:
        print("❌ Some tests failed")
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))