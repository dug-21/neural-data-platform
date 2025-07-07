#!/usr/bin/env python3
"""Integration test for secure settings and configurable rate limiting"""

import os
import tempfile
from data_ingestion.config.settings import get_settings, Settings
from data_ingestion.utils.configurable_rate_limiter import ConfigurableRateLimiter

def test_secure_settings():
    """Test that secrets are properly secured"""
    print("=== Testing Secure Settings ===")
    
    # Save existing environment variables
    env_backup = {}
    secret_vars = ['ALPHA_VANTAGE_API_KEY', 'REDIS_PASSWORD', 'IEX_CLOUD_API_KEY', 
                   'POLYGON_API_KEY', 'FINNHUB_API_KEY']
    for var in secret_vars:
        if var in os.environ:
            env_backup[var] = os.environ[var]
            del os.environ[var]
    
    # Create a temp .env file with secrets
    with tempfile.NamedTemporaryFile(mode='w', suffix='.env', delete=False) as f:
        f.write("# Non-secrets (should load)\n")
        f.write("LOG_LEVEL=DEBUG\n")
        f.write("MAX_REQUESTS_PER_MINUTE=100\n")
        f.write("BATCH_SIZE=2000\n")
        f.write("\n# Secrets (should NOT load)\n")
        f.write("ALPHA_VANTAGE_API_KEY=secret_from_file\n")
        f.write("REDIS_PASSWORD=redis_secret_from_file\n")
        temp_path = f.name
    
    try:
        # Test with env file
        settings = Settings(_env_file=temp_path)
        
        # Check non-secrets loaded
        print(f"✓ Non-secret LOG_LEVEL loaded from .env: {settings.log_level}")
        assert settings.log_level == "DEBUG"
        
        # Check secrets NOT loaded from file
        print(f"✓ Secret ALPHA_VANTAGE_API_KEY not loaded from .env: {settings.alpha_vantage_api_key}")
        assert settings.alpha_vantage_api_key is None
        
        # Test with environment variable
        os.environ['ALPHA_VANTAGE_API_KEY'] = 'secret_from_env'
        settings2 = Settings(_env_file=temp_path)
        
        print(f"✓ Secret ALPHA_VANTAGE_API_KEY loaded from environment: {settings2.alpha_vantage_api_key}")
        assert settings2.alpha_vantage_api_key == 'secret_from_env'
        
        # Clean up
        del os.environ['ALPHA_VANTAGE_API_KEY']
        
    finally:
        os.unlink(temp_path)
        # Restore environment
        for var, value in env_backup.items():
            os.environ[var] = value
    
    print("✅ Secure settings test passed!\n")

def test_configurable_rate_limiting():
    """Test configurable rate limiting"""
    print("=== Testing Configurable Rate Limiting ===")
    
    # Test default rate limits
    settings = get_settings()
    
    print("Default rate limits:")
    for api_name, config in settings.rate_limits.items():
        print(f"  {api_name}: {config.calls_per_minute} calls/min, {config.calls_per_day} calls/day")
    
    # Test environment override
    os.environ['RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE'] = '10'
    os.environ['RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY'] = '1000'
    
    settings2 = Settings()
    alpha_config = settings2.rate_limits['alpha_vantage']
    print(f"\n✓ Alpha Vantage rate limit overridden: {alpha_config.calls_per_minute} calls/min, {alpha_config.calls_per_day} calls/day")
    assert alpha_config.calls_per_minute == 10
    assert alpha_config.calls_per_day == 1000
    
    # Test creating rate limiter from settings
    limiter = ConfigurableRateLimiter.from_settings('alpha_vantage', settings2)
    print(f"✓ Created rate limiter: {limiter.name} with {limiter.calls_per_minute} calls/min")
    assert limiter.calls_per_minute == 10
    
    # Test burst limiting
    burst_limiter = ConfigurableRateLimiter(
        name='test_burst',
        calls_per_minute=60,
        burst_size=5
    )
    
    # Should allow 5 requests in burst
    allowed_count = 0
    for _ in range(10):
        can_request, _ = burst_limiter.can_make_request()
        if can_request:
            burst_limiter.record_request()
            allowed_count += 1
    
    print(f"✓ Burst limiting working: allowed {allowed_count}/10 requests (expected 5)")
    assert allowed_count == 5
    
    # Clean up
    del os.environ['RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE']
    del os.environ['RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY']
    
    print("✅ Configurable rate limiting test passed!\n")

def main():
    """Run integration tests"""
    print("Running Integration Tests for Neural Trader Security Features\n")
    
    test_secure_settings()
    test_configurable_rate_limiting()
    
    print("🎉 All integration tests passed!")
    print("\nSummary:")
    print("1. ✅ Secrets are blocked from loading from .env files")
    print("2. ✅ Non-secret configurations still load from .env files")
    print("3. ✅ Secrets can be loaded from environment variables")
    print("4. ✅ Rate limits are configurable per API")
    print("5. ✅ Rate limits can be overridden via environment variables")
    print("6. ✅ Burst limiting is supported")

if __name__ == "__main__":
    main()