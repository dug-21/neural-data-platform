#!/usr/bin/env python3
"""
Example: Running health check server standalone without environment variables.

This demonstrates the code-first approach where the health check system
works without any environment configuration.
"""
import asyncio
import sys
import os

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from utils.health_check import HealthCheckHandler


async def main():
    """Run standalone health check server."""
    print("🏥 Starting Health Check Server (No Environment Variables Required)")
    print("=" * 60)
    
    # Create health check handler with default configuration
    handler = HealthCheckHandler(port=8080)
    
    print("Configuration:")
    print(f"  Port: {handler.port}")
    print(f"  Max Data Age: {handler.max_data_age_seconds} seconds")
    print(f"  Min Success Rate: {handler.min_success_rate * 100}%")
    print(f"  Min Active Streams: {handler.min_active_streams}")
    print()
    print("Circuit Breakers:")
    for name, breaker in handler.circuit_breakers.items():
        print(f"  {name}: {breaker.state.value}")
    print()
    
    try:
        # Start the server
        await handler.start()
        
        print("✅ Health check server started successfully!")
        print()
        print("Available endpoints:")
        print("  http://localhost:8080/health          - Basic health status")
        print("  http://localhost:8080/health/detailed - Detailed component status")
        print("  http://localhost:8080/health/live     - Kubernetes liveness probe")
        print("  http://localhost:8080/health/ready    - Kubernetes readiness probe")
        print()
        print("Test with:")
        print("  curl http://localhost:8080/health")
        print()
        print("Press Ctrl+C to stop the server...")
        
        # Keep running
        await asyncio.Event().wait()
        
    except KeyboardInterrupt:
        print("\n🛑 Shutting down...")
        await handler.stop()
        print("✅ Server stopped")
    except Exception as e:
        print(f"❌ Error: {e}")
        await handler.stop()
        raise


if __name__ == "__main__":
    asyncio.run(main())