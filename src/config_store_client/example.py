"""Example usage of the ConfigStoreClient.

This example demonstrates the key features of the config-store client:
- Configuration retrieval with caching
- Environment variable fallback
- Type conversion methods
- Error handling
- Health checks
"""

import asyncio
import os
from .client import ConfigStoreClient, ConfigStoreConfig, RetryConfig


async def main():
    """Demonstrate ConfigStoreClient usage."""
    print("🚀 ConfigStoreClient Example")
    print("=" * 50)
    
    # Configure the client
    retry_config = RetryConfig(
        max_attempts=3,
        base_delay=1.0,
        backoff_multiplier=2.0,
        jitter_enabled=True
    )
    
    config = ConfigStoreConfig(
        service_url="http://localhost:8080",  # Config store service URL
        timeout=30.0,
        cache_ttl=300,  # 5 minutes
        retry_config=retry_config,
        enable_env_fallback=True,
        env_prefix="NEURAL_TRADER"
    )
    
    # Use client as async context manager
    async with ConfigStoreClient(config) as client:
        
        # 1. Health Check
        print("1. Health Check")
        print("-" * 20)
        health = await client.health_check()
        print(f"Service Status: {health['status']}")
        print(f"Service Available: {health['service_available']}")
        print(f"Circuit Breaker: {health['circuit_breaker_state']}")
        print(f"Cache Size: {health['cache_size']}")
        print()
        
        # 2. Environment Variable Fallback Demo
        print("2. Environment Variable Fallback")
        print("-" * 35)
        
        # Set some test environment variables
        os.environ["NEURAL_TRADER_DEMO_API_KEY"] = "demo-api-key-12345"
        os.environ["NEURAL_TRADER_DEMO_TIMEOUT"] = "30"
        os.environ["NEURAL_TRADER_DEMO_ENABLED"] = "true"
        os.environ["NEURAL_TRADER_DEMO_SYMBOLS"] = '["BTC", "ETH", "AAPL"]'
        
        try:
            # These will fallback to environment variables if service is unavailable
            api_key = await client.get_string("demo.api.key")
            timeout = await client.get_int("demo.timeout") 
            enabled = await client.get_bool("demo.enabled")
            symbols = await client.get_list("demo.symbols")
            
            print(f"API Key: {api_key}")
            print(f"Timeout: {timeout}")
            print(f"Enabled: {enabled}")
            print(f"Symbols: {symbols}")
            
        except Exception as e:
            print(f"Configuration retrieval failed: {e}")
        
        print()
        
        # 3. Type Conversion Demo
        print("3. Type Conversion Examples")
        print("-" * 30)
        
        test_configs = {
            "app.name": "neural-trader",
            "app.version": "2.0.0",
            "app.port": 8080,
            "app.debug": True,
            "app.rate_limit": 100.5,
            "app.features": ["trading", "analytics", "ml"],
            "app.database": {"host": "localhost", "port": 5432}
        }
        
        # Set environment variables for the demo
        for key, value in test_configs.items():
            env_key = f"NEURAL_TRADER_{key.replace('.', '_').upper()}"
            if isinstance(value, (list, dict)):
                os.environ[env_key] = str(value).replace("'", '"')
            else:
                os.environ[env_key] = str(value)
        
        try:
            app_name = await client.get_string("app.name")
            app_port = await client.get_int("app.port") 
            app_debug = await client.get_bool("app.debug")
            app_rate_limit = await client.get_float("app.rate_limit")
            
            print(f"App Name: {app_name}")
            print(f"App Port: {app_port}")
            print(f"Debug Mode: {app_debug}")
            print(f"Rate Limit: {app_rate_limit}")
            
        except Exception as e:
            print(f"Type conversion failed: {e}")
        
        print()
        
        # 4. Cache Statistics
        print("4. Cache Statistics")
        print("-" * 20)
        
        cache_stats = client.get_cache_stats()
        print(f"Total Entries: {cache_stats['total_entries']}")
        print(f"Active Entries: {cache_stats['active_entries']}")
        print(f"Expired Entries: {cache_stats['expired_entries']}")
        print(f"Cache Hit Ratio: {cache_stats['cache_hit_ratio']:.2%}")
        print()
        
        # 5. Error Handling Demo
        print("5. Error Handling")
        print("-" * 18)
        
        try:
            # This should raise ConfigNotFoundError (no env var or service)
            missing_value = await client.get("non.existent.key")
            print(f"Unexpected success: {missing_value}")
        except Exception as e:
            print(f"✅ Properly handled missing config: {type(e).__name__}: {e}")
        
        # Test with default value
        default_value = await client.get("non.existent.key", default="fallback-value")
        print(f"✅ Default value used: {default_value}")
        
        print()
        
        # 6. Configuration Operations (if service is available)
        if health['service_available']:
            print("6. Configuration Operations (Service Available)")
            print("-" * 50)
            
            try:
                # Set a configuration value
                await client.set("demo.test.key", "test-value")
                print("✅ Configuration set successfully")
                
                # Retrieve it
                retrieved_value = await client.get("demo.test.key")
                print(f"✅ Retrieved value: {retrieved_value}")
                
                # Check if it exists
                exists = await client.exists("demo.test.key")
                print(f"✅ Key exists: {exists}")
                
                # List keys
                keys = await client.list_keys("demo")
                print(f"✅ Keys with 'demo' prefix: {keys}")
                
                # Delete it
                await client.delete("demo.test.key")
                print("✅ Configuration deleted successfully")
                
                # Verify deletion
                exists_after_delete = await client.exists("demo.test.key")
                print(f"✅ Key exists after deletion: {exists_after_delete}")
                
            except Exception as e:
                print(f"❌ Configuration operation failed: {e}")
        else:
            print("6. Configuration Operations (Service Unavailable)")
            print("-" * 50)
            print("⚠️  Config store service is not available")
            print("   All operations are using environment variable fallback")
        
        print()
        print("🎉 Example completed successfully!")
        print("=" * 50)


if __name__ == "__main__":
    # Set up basic logging
    import logging
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
    )
    
    # Run the example
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n👋 Example interrupted by user")
    except Exception as e:
        print(f"\n❌ Example failed: {e}")
        import traceback
        traceback.print_exc()