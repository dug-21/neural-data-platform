"""Tests for configurable rate limiting"""
import pytest
import os
from unittest.mock import patch, MagicMock
from pydantic import ValidationError

# These imports will fail initially (TDD)
from data_ingestion.config.settings import Settings, RateLimitConfig
from data_ingestion.utils.configurable_rate_limiter import ConfigurableRateLimiter


class TestRateLimitConfiguration:
    """Test rate limit configuration in settings"""
    
    def test_default_rate_limits_loaded(self):
        """Test that default rate limits are loaded from settings"""
        settings = Settings()
        
        # Should have rate limits for major APIs
        assert "alpha_vantage" in settings.rate_limits
        assert "polygon" in settings.rate_limits
        assert "finnhub" in settings.rate_limits
        assert "newsapi" in settings.rate_limits
        
        # Check specific limits
        alpha_config = settings.rate_limits["alpha_vantage"]
        assert alpha_config.calls_per_minute == 5
        assert alpha_config.calls_per_day == 500
        
    def test_rate_limit_env_override(self):
        """Test that rate limits can be overridden via environment variables"""
        # Set environment variables
        env_vars = {
            "RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE": "10",
            "RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY": "1000",
            "RATE_LIMIT_POLYGON_CALLS_PER_MINUTE": "20",
        }
        
        with patch.dict(os.environ, env_vars):
            settings = Settings()
            
            # Check overrides applied
            assert settings.rate_limits["alpha_vantage"].calls_per_minute == 10
            assert settings.rate_limits["alpha_vantage"].calls_per_day == 1000
            assert settings.rate_limits["polygon"].calls_per_minute == 20
    
    def test_rate_limit_json_config(self):
        """Test loading rate limits from JSON string in environment"""
        rate_limits_json = """
        {
            "custom_api": {
                "calls_per_minute": 100,
                "calls_per_day": 10000,
                "burst_size": 50
            }
        }
        """
        
        with patch.dict(os.environ, {"RATE_LIMITS_JSON": rate_limits_json}):
            settings = Settings()
            
            assert "custom_api" in settings.rate_limits
            custom_config = settings.rate_limits["custom_api"]
            assert custom_config.calls_per_minute == 100
            assert custom_config.calls_per_day == 10000
            assert custom_config.burst_size == 50
    
    def test_rate_limit_config_validation(self):
        """Test that invalid rate limit configurations are rejected"""
        # Test negative values
        with pytest.raises(ValidationError):
            RateLimitConfig(calls_per_minute=-1)
        
        # Test invalid types
        with pytest.raises(ValidationError):
            RateLimitConfig(calls_per_minute="not a number")
    
    def test_rate_limit_partial_config(self):
        """Test that partial rate limit configs work (some fields None)"""
        config = RateLimitConfig(calls_per_minute=10)
        assert config.calls_per_minute == 10
        assert config.calls_per_day is None
        assert config.burst_size is None


class TestConfigurableRateLimiter:
    """Test the configurable rate limiter"""
    
    def test_rate_limiter_from_config(self):
        """Test creating rate limiter from configuration"""
        config = RateLimitConfig(
            calls_per_minute=10,
            calls_per_day=1000,
            burst_size=5
        )
        
        limiter = ConfigurableRateLimiter.from_config("test_api", config)
        
        assert limiter.name == "test_api"
        assert limiter.calls_per_minute == 10
        assert limiter.calls_per_day == 1000
        assert limiter.burst_size == 5
    
    def test_rate_limiter_from_settings(self):
        """Test creating rate limiters from settings"""
        settings = Settings()
        
        # Should be able to get limiter for configured APIs
        alpha_limiter = ConfigurableRateLimiter.from_settings("alpha_vantage", settings)
        assert alpha_limiter.calls_per_minute == settings.rate_limits["alpha_vantage"].calls_per_minute
        
    def test_rate_limiter_fallback(self):
        """Test fallback when API not in configuration"""
        settings = Settings()
        
        # Non-configured API should get default limiter
        unknown_limiter = ConfigurableRateLimiter.from_settings("unknown_api", settings)
        assert unknown_limiter is not None
        # Should have some reasonable defaults
        assert unknown_limiter.calls_per_minute == 60  # Default
    
    @pytest.mark.asyncio
    async def test_burst_limiting(self):
        """Test burst size limiting"""
        config = RateLimitConfig(
            calls_per_minute=60,
            burst_size=5  # Allow burst of 5
        )
        
        limiter = ConfigurableRateLimiter.from_config("burst_test", config)
        
        # Should allow burst
        for _ in range(5):
            can_request, wait_time = limiter.can_make_request()
            assert can_request
            limiter.record_request()
        
        # 6th request should be limited
        can_request, wait_time = limiter.can_make_request()
        assert not can_request
        assert wait_time > 0
    
    def test_rate_limiter_with_custom_provider_config(self):
        """Test that providers can specify custom rate limit needs"""
        # Provider-specific configuration
        provider_config = {
            "yahoo_finance": {
                "rate_limiter": "conservative",  # Special handling
                "calls_per_hour": 200,  # Custom time window
            }
        }
        
        with patch.dict(os.environ, {"PROVIDER_RATE_CONFIG": str(provider_config)}):
            settings = Settings()
            
            # Should handle provider-specific config
            yahoo_limiter = ConfigurableRateLimiter.from_settings("yahoo_finance", settings)
            # Convert per-hour to per-minute for comparison
            expected_per_minute = 200 / 60  # ~3.33 calls per minute
            assert yahoo_limiter.calls_per_minute <= 4  # Conservative rounding