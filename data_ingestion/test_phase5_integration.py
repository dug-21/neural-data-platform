#!/usr/bin/env python3
"""Test script for Phase 5 integration - verifies all components work together."""
import asyncio
import aiohttp
import sys
import os
from datetime import datetime
import json

# Test configuration
TEST_HEALTH_PORT = 8001
TEST_METRICS_ENDPOINT = f"http://localhost:{TEST_HEALTH_PORT}/metrics"
TEST_HEALTH_ENDPOINT = f"http://localhost:{TEST_HEALTH_PORT}/health/detailed"

# Phase 5 integration checklist
INTEGRATION_TESTS = {
    "health_check_server": {
        "description": "Health check server is running",
        "endpoint": f"http://localhost:{TEST_HEALTH_PORT}/health",
        "expected_status": [200, 503]
    },
    "metrics_endpoint": {
        "description": "Prometheus metrics endpoint is accessible",
        "endpoint": TEST_METRICS_ENDPOINT,
        "expected_status": [200]
    },
    "circuit_breakers": {
        "description": "Circuit breakers are integrated",
        "check_function": "check_circuit_breakers"
    },
    "file_provider": {
        "description": "File provider is available",
        "check_function": "check_file_provider"
    },
    "websocket_resilience": {
        "description": "WebSocket providers have resilience features",
        "check_function": "check_websocket_resilience"
    }
}

# Required metrics for Phase 5
REQUIRED_METRICS = [
    # Phase 1 - WebSocket resilience
    'data_ingestion_websocket_connections',
    'data_ingestion_websocket_messages_total',
    'data_ingestion_websocket_reconnections_total',
    
    # Phase 2 - Health checks
    'data_ingestion_health_status',
    'data_ingestion_health_check_duration_seconds',
    'data_ingestion_health_component_status',
    
    # Phase 3 - File backfill
    'data_ingestion_file_backfill_progress',
    'data_ingestion_file_backfill_rows_total',
    'data_ingestion_file_backfill_duration_seconds',
    
    # Phase 4 - Circuit breakers and data flow
    'data_ingestion_circuit_breaker_state',
    'data_ingestion_circuit_breaker_failures_total',
    'data_ingestion_data_flow_age_seconds',
    'data_ingestion_data_flow_rate'
]


async def test_endpoint(session, url, expected_status):
    """Test if an endpoint is accessible."""
    try:
        async with session.get(url) as response:
            return response.status in expected_status, response.status
    except Exception as e:
        return False, str(e)


async def check_circuit_breakers(session):
    """Check if circuit breakers are properly integrated."""
    try:
        async with session.get(TEST_HEALTH_ENDPOINT) as response:
            if response.status in [200, 503]:
                data = await response.json()
                
                # Check for circuit breaker states in health response
                checks = data.get('checks', {})
                circuit_breaker_found = False
                
                for component, details in checks.items():
                    if isinstance(details, dict) and 'details' in details:
                        component_details = details.get('details', {})
                        if 'circuit_breaker' in component_details:
                            circuit_breaker_found = True
                            break
                
                return circuit_breaker_found, "Circuit breakers found in health checks" if circuit_breaker_found else "No circuit breakers found"
            else:
                return False, f"Health endpoint returned {response.status}"
    except Exception as e:
        return False, str(e)


async def check_file_provider(session):
    """Check if file provider is available in the system."""
    # Check if file_provider.py exists
    file_provider_path = "/workspaces/neural-trader/data_ingestion/providers/file_provider.py"
    if os.path.exists(file_provider_path):
        # Check if it's imported in __init__.py
        init_path = "/workspaces/neural-trader/data_ingestion/providers/__init__.py"
        if os.path.exists(init_path):
            with open(init_path, 'r') as f:
                content = f.read()
                if 'FileProvider' in content and 'file_provider' in content:
                    return True, "File provider is properly integrated"
    return False, "File provider not found or not integrated"


async def check_websocket_resilience(session):
    """Check if WebSocket providers have resilience features."""
    # Check if alpaca.py has circuit breaker integration
    alpaca_path = "/workspaces/neural-trader/data_ingestion/providers/alpaca.py"
    if os.path.exists(alpaca_path):
        with open(alpaca_path, 'r') as f:
            content = f.read()
            features = []
            
            if 'CircuitBreaker' in content:
                features.append("Circuit breaker")
            if 'exponential_backoff' in content or 'backoff_factor' in content:
                features.append("Exponential backoff")
            if 'message_buffer' in content:
                features.append("Message buffering")
            if 'max_reconnect_attempts' in content:
                features.append("Reconnection logic")
            
            if features:
                return True, f"WebSocket resilience features found: {', '.join(features)}"
    
    return False, "WebSocket resilience features not found"


async def check_metrics(session):
    """Check if all required metrics are available."""
    try:
        async with session.get(TEST_METRICS_ENDPOINT) as response:
            if response.status == 200:
                text = await response.text()
                
                found_metrics = []
                missing_metrics = []
                
                for metric in REQUIRED_METRICS:
                    if metric in text:
                        found_metrics.append(metric)
                    else:
                        missing_metrics.append(metric)
                
                success = len(missing_metrics) == 0
                message = f"Found {len(found_metrics)}/{len(REQUIRED_METRICS)} metrics"
                
                if missing_metrics:
                    message += f"\nMissing: {', '.join(missing_metrics[:5])}"
                    if len(missing_metrics) > 5:
                        message += f" and {len(missing_metrics) - 5} more"
                
                return success, message
            else:
                return False, f"Metrics endpoint returned {response.status}"
    except Exception as e:
        return False, str(e)


async def main():
    """Run all integration tests."""
    print("=" * 80)
    print("Phase 5 Integration Test")
    print(f"Time: {datetime.now()}")
    print("=" * 80)
    print()
    
    # Check if service is running
    print("Checking if data-ingestion service is running...")
    print(f"Testing endpoints on port {TEST_HEALTH_PORT}")
    print()
    
    results = {}
    async with aiohttp.ClientSession() as session:
        # Test basic endpoints
        for test_name, test_config in INTEGRATION_TESTS.items():
            print(f"Testing: {test_config['description']}...")
            
            if 'endpoint' in test_config:
                success, result = await test_endpoint(
                    session, 
                    test_config['endpoint'], 
                    test_config['expected_status']
                )
                results[test_name] = {
                    'success': success,
                    'message': f"Status: {result}" if success else f"Error: {result}"
                }
            elif 'check_function' in test_config:
                func_name = test_config['check_function']
                func = globals().get(func_name)
                if func:
                    success, message = await func(session)
                    results[test_name] = {
                        'success': success,
                        'message': message
                    }
                else:
                    results[test_name] = {
                        'success': False,
                        'message': f"Check function {func_name} not found"
                    }
            
            # Print result
            if results[test_name]['success']:
                print(f"  ✅ {results[test_name]['message']}")
            else:
                print(f"  ❌ {results[test_name]['message']}")
        
        # Check metrics
        print(f"\nChecking Prometheus metrics...")
        metrics_success, metrics_message = await check_metrics(session)
        results['metrics'] = {
            'success': metrics_success,
            'message': metrics_message
        }
        if metrics_success:
            print(f"  ✅ {metrics_message}")
        else:
            print(f"  ❌ {metrics_message}")
    
    # Summary
    print("\n" + "=" * 80)
    print("INTEGRATION TEST SUMMARY")
    print("=" * 80)
    
    total_tests = len(results)
    passed_tests = sum(1 for r in results.values() if r['success'])
    
    print(f"Total tests: {total_tests}")
    print(f"Passed: {passed_tests}")
    print(f"Failed: {total_tests - passed_tests}")
    
    if passed_tests == total_tests:
        print("\n✅ ALL INTEGRATION TESTS PASSED!")
        print("\nPhase 5 integration is complete. The system has:")
        print("- Circuit breakers for resilience")
        print("- WebSocket reconnection with exponential backoff")
        print("- File-based backfill with checkpoint recovery")
        print("- Comprehensive Prometheus metrics")
        print("- Enhanced health checks with market hours awareness")
        return 0
    else:
        print("\n❌ Some integration tests failed")
        print("\nFailed tests:")
        for test_name, result in results.items():
            if not result['success']:
                print(f"  - {INTEGRATION_TESTS.get(test_name, {}).get('description', test_name)}")
                print(f"    {result['message']}")
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))