"""Example demonstrating hybrid configuration loader usage."""

import asyncio
import os
import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(__file__)))

from config import (
    get_settings, 
    HybridSettings, 
    get_config_store_status,
    ConfigMigrationTool
)


async def main():
    """Demonstrate hybrid configuration functionality."""
    
    print("=== Hybrid Configuration Loader Example ===\n")
    
    # 1. Get settings instance (will use hybrid if available)
    settings = get_settings()
    print(f"Settings type: {type(settings).__name__}")
    print(f"Hybrid available: {hasattr(settings, 'get_config_value')}")
    
    # 2. Show config store status
    if hasattr(settings, 'get_config_store_status'):
        status = settings.get_config_store_status()
        print(f"\nConfig Store Status:")
        for key, value in status.items():
            print(f"  {key}: {value}")
    
    # 3. Demonstrate configuration access
    print(f"\n=== Configuration Access ===")
    print(f"TimescaleDB Host: {settings.timescale_host}")
    print(f"Redis Host: {settings.redis_host}")
    print(f"Log Level: {settings.log_level}")
    print(f"Batch Size: {settings.batch_size}")
    
    # 4. Test hybrid configuration methods if available
    if isinstance(settings, HybridSettings):
        print(f"\n=== Hybrid Configuration Methods ===")
        
        # Test getting configuration from hybrid source
        db_host = await settings.get_config_value('database.host', 'localhost')
        print(f"Database host (hybrid): {db_host}")
        
        # Test setting a configuration value
        success = await settings.set_config_value('test.example', 'hybrid_value')
        print(f"Set test config: {success}")
        
        # Retrieve the value we just set
        test_value = await settings.get_config_value('test.example')
        print(f"Retrieved test config: {test_value}")
        
        # Test database configuration helper
        db_config = await settings.get_database_config()
        print(f"Database config: {db_config}")
        
        # Test Redis configuration helper  
        redis_config = await settings.get_redis_config()
        print(f"Redis config: {redis_config}")
    
    # 5. Environment variable examples
    print(f"\n=== Environment Variable Integration ===")
    
    # Set some test environment variables
    os.environ['NEURAL_TRADER_TEST_VALUE'] = 'from_env'
    os.environ['NEURAL_TRADER_BATCH_SIZE'] = '2000'
    
    if isinstance(settings, HybridSettings):
        # Test retrieving from environment via hybrid method
        test_env = await settings.get_config_value('test.value', 'default')
        batch_size_env = await settings.get_config_value('batch.size', 1000)
        
        print(f"Test value from env: {test_env}")
        print(f"Batch size from env: {batch_size_env}")
    
    # 6. Migration example
    if isinstance(settings, HybridSettings):
        print(f"\n=== Configuration Migration Example ===")
        
        migration_tool = ConfigMigrationTool(settings)
        
        # Perform dry run migration
        try:
            result = await migration_tool.migrate_env_to_config_store(
                env_prefix="NEURAL_TRADER", 
                dry_run=True
            )
            print(f"Migration dry run result: {result}")
        except Exception as e:
            print(f"Migration failed: {e}")
    
    # 7. Rate limit configuration
    print(f"\n=== Rate Limit Configuration ===")
    print("Default rate limits:")
    for api, config in settings.rate_limits.items():
        print(f"  {api}: {config.calls_per_minute} calls/min, {config.calls_per_day} calls/day")
    
    if isinstance(settings, HybridSettings):
        # Try to get rate limit from hybrid source
        polygon_rate_limit = await settings.get_rate_limit_config('polygon')
        print(f"\nPolygon rate limit (hybrid): {polygon_rate_limit}")
    
    print(f"\n=== Example Complete ===")


def test_backward_compatibility():
    """Test that existing code still works without changes."""
    print("\n=== Backward Compatibility Test ===")
    
    # This should work exactly as before
    settings = get_settings()
    
    # Test basic properties
    print(f"TimescaleDB URL: {settings.timescale_url}")
    print(f"Redis URL: {settings.redis_url}")
    print(f"Max requests per minute: {settings.max_requests_per_minute}")
    
    # Test rate limits
    alpha_vantage_config = settings.rate_limits.get('alpha_vantage')
    if alpha_vantage_config:
        print(f"Alpha Vantage rate limit: {alpha_vantage_config.calls_per_minute}/min")
    
    print("✅ Backward compatibility maintained")


if __name__ == "__main__":
    # Run backward compatibility test first
    test_backward_compatibility()
    
    # Run main async example
    try:
        asyncio.run(main())
    except Exception as e:
        print(f"Error running example: {e}")
        import traceback
        traceback.print_exc()