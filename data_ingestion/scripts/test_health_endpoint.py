#!/usr/bin/env python3
"""
Standalone health check endpoint testing script.

This script tests the enhanced health check system without requiring
environment variables or full system deployment.

Usage:
    python scripts/test_health_endpoint.py
"""
import asyncio
import aiohttp
import json
import sys
from datetime import datetime
from typing import Dict, Any


async def test_health_endpoint(base_url: str = "http://localhost:8080") -> Dict[str, Any]:
    """Test health check endpoint and display results."""
    results = {
        'timestamp': datetime.now().isoformat(),
        'endpoints_tested': [],
        'all_passed': True
    }
    
    async with aiohttp.ClientSession() as session:
        # Test 1: Basic health check
        print("\n🔍 Testing basic health endpoint...")
        try:
            async with session.get(f"{base_url}/health", timeout=aiohttp.ClientTimeout(total=5)) as response:
                data = await response.json()
                status_code = response.status
                
                print(f"   Status Code: {status_code}")
                print(f"   Response: {json.dumps(data, indent=2)}")
                
                results['endpoints_tested'].append({
                    'endpoint': '/health',
                    'status_code': status_code,
                    'passed': status_code in [200, 503],  # Both are valid
                    'data': data
                })
                
                if status_code not in [200, 503]:
                    results['all_passed'] = False
                
        except Exception as e:
            print(f"   ❌ Error: {str(e)}")
            results['endpoints_tested'].append({
                'endpoint': '/health',
                'error': str(e),
                'passed': False
            })
            results['all_passed'] = False
        
        # Test 2: Detailed health check
        print("\n🔍 Testing detailed health endpoint...")
        try:
            async with session.get(f"{base_url}/health/detailed", timeout=aiohttp.ClientTimeout(total=5)) as response:
                data = await response.json()
                status_code = response.status
                
                print(f"   Status Code: {status_code}")
                print(f"   Components:")
                
                if 'checks' in data:
                    for component, check in data['checks'].items():
                        status = "✅" if check.get('healthy', False) else "❌"
                        print(f"     {status} {component}: {check.get('message', check)}")
                
                results['endpoints_tested'].append({
                    'endpoint': '/health/detailed',
                    'status_code': status_code,
                    'passed': status_code in [200, 503],
                    'data': data
                })
                
        except Exception as e:
            print(f"   ❌ Error: {str(e)}")
            results['endpoints_tested'].append({
                'endpoint': '/health/detailed',
                'error': str(e),
                'passed': False
            })
            results['all_passed'] = False
        
        # Test 3: Liveness probe
        print("\n🔍 Testing liveness probe...")
        try:
            async with session.get(f"{base_url}/health/live", timeout=aiohttp.ClientTimeout(total=5)) as response:
                text = await response.text()
                status_code = response.status
                
                print(f"   Status Code: {status_code}")
                print(f"   Response: {text}")
                
                results['endpoints_tested'].append({
                    'endpoint': '/health/live',
                    'status_code': status_code,
                    'passed': status_code == 200,
                    'response': text
                })
                
                if status_code != 200:
                    results['all_passed'] = False
                
        except Exception as e:
            print(f"   ❌ Error: {str(e)}")
            results['endpoints_tested'].append({
                'endpoint': '/health/live',
                'error': str(e),
                'passed': False
            })
            results['all_passed'] = False
        
        # Test 4: Readiness probe
        print("\n🔍 Testing readiness probe...")
        try:
            async with session.get(f"{base_url}/health/ready", timeout=aiohttp.ClientTimeout(total=5)) as response:
                text = await response.text()
                status_code = response.status
                
                print(f"   Status Code: {status_code}")
                print(f"   Response: {text}")
                
                results['endpoints_tested'].append({
                    'endpoint': '/health/ready',
                    'status_code': status_code,
                    'passed': status_code in [200, 503],
                    'response': text
                })
                
        except Exception as e:
            print(f"   ❌ Error: {str(e)}")
            results['endpoints_tested'].append({
                'endpoint': '/health/ready',
                'error': str(e),
                'passed': False
            })
            results['all_passed'] = False
    
    return results


async def run_standalone_health_server():
    """Run a standalone health check server for testing."""
    from utils.health_check import HealthCheckHandler
    
    print("🚀 Starting standalone health check server on port 8080...")
    handler = HealthCheckHandler(port=8080)
    
    try:
        await handler.start()
        print("✅ Health check server started successfully!")
        print("   Access endpoints:")
        print("   - http://localhost:8080/health")
        print("   - http://localhost:8080/health/detailed")
        print("   - http://localhost:8080/health/live")
        print("   - http://localhost:8080/health/ready")
        print("\n   Press Ctrl+C to stop the server")
        
        # Keep server running
        await asyncio.Event().wait()
        
    except KeyboardInterrupt:
        print("\n🛑 Shutting down health check server...")
        await handler.stop()
        print("✅ Server stopped")


async def main():
    """Main test function."""
    print("=" * 60)
    print("Phase 2: Health Check Endpoint Testing")
    print("=" * 60)
    
    if len(sys.argv) > 1 and sys.argv[1] == "--server":
        # Run standalone server mode
        await run_standalone_health_server()
    else:
        # Run tests against existing server
        print("\n📋 Testing health check endpoints...")
        print("   (Make sure the data ingestion service is running)")
        
        # Allow custom URL
        base_url = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
        
        results = await test_health_endpoint(base_url)
        
        # Summary
        print("\n" + "=" * 60)
        print("📊 Test Summary:")
        print(f"   Total endpoints tested: {len(results['endpoints_tested'])}")
        passed = sum(1 for e in results['endpoints_tested'] if e.get('passed', False))
        print(f"   Passed: {passed}/{len(results['endpoints_tested'])}")
        
        if results['all_passed']:
            print("\n✅ All health check tests passed!")
        else:
            print("\n❌ Some tests failed. Check the output above.")
            sys.exit(1)


if __name__ == "__main__":
    print("\n🏥 Health Check Testing Tool")
    print("\nUsage:")
    print("  Test endpoints:     python scripts/test_health_endpoint.py [URL]")
    print("  Run test server:    python scripts/test_health_endpoint.py --server")
    print("")
    
    asyncio.run(main())