# SPARC Implementation Plan: API Rate Limiting & Secure Environment Variables

## Situation

### Current State
1. **Rate Limiting**: Already well-implemented in Python (`src/utils/rate_limiter.py`) with:
   - Per-API rate limits (Alpha Vantage: 5/min, 500/day)
   - Token bucket algorithm
   - Decorators for easy application
   - Configuration in settings but not per-API config files

2. **Secrets Management**: Currently allows loading from `.env` files:
   - All settings including secrets can be loaded from disk
   - Need to prevent secrets/passwords from `.env` while allowing other configs
   - Identified secrets: API keys, passwords, client secrets

### Requirements
1. Add per-API rate limit configuration in settings files
2. Prevent secrets from being loaded from `.env` files
3. Maintain backward compatibility for non-secret configurations
4. Use TDD approach for implementation

## Problem

### Problem 1: Rate Limit Configuration
- Rate limits are hardcoded in `APIRateLimiters` class
- No way to override limits via configuration
- Different environments may need different limits

### Problem 2: Secrets on Disk
- Current `BaseSettings` loads all values from `.env` 
- Security risk: API keys and passwords stored on disk
- Need selective loading: configs from `.env`, secrets from environment only

## Approach

### Solution Architecture

#### 1. Enhanced Rate Limiting Configuration
```python
# In settings.py
class RateLimitConfig(BaseModel):
    """Rate limit configuration for each API"""
    calls_per_minute: Optional[int] = None
    calls_per_day: Optional[int] = None
    burst_size: Optional[int] = None
    
class Settings(BaseSettings):
    # Rate limits per API
    rate_limits: Dict[str, RateLimitConfig] = Field(
        default_factory=lambda: {
            "alpha_vantage": {"calls_per_minute": 5, "calls_per_day": 500},
            "polygon": {"calls_per_minute": 5},
            "finnhub": {"calls_per_minute": 60},
            # ... other APIs
        }
    )
```

#### 2. Secure Settings with Custom Settings Source
```python
from pydantic_settings import BaseSettings, SettingsConfigDict
from pydantic import Field, field_validator
from typing import Any, Dict, Optional, Tuple

class SecureSettings(BaseSettings):
    """Settings that prevent secrets from being loaded from files"""
    
    model_config = SettingsConfigDict(
        env_file='.env',
        env_file_encoding='utf-8',
        case_sensitive=False,
        # Custom settings source priority
        secrets_dir=None,  # Disable secrets directory
    )
    
    # Mark fields as secrets
    _secret_fields = {
        'iex_cloud_api_key', 'alpha_vantage_api_key', 'polygon_api_key',
        'finnhub_api_key', 'fred_api_key', 'reddit_client_secret',
        'quandl_api_key', 'newsapi_key', 'yahoo_api_key',
        'timescale_password', 'redis_password'
    }
    
    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: type[BaseSettings],
        init_settings,
        env_settings,
        dotenv_settings,
        file_secret_settings,
    ) -> Tuple[Any, ...]:
        """Customize settings sources to filter secrets from dotenv"""
        
        # Create a filtered dotenv settings source
        class FilteredDotEnvSettings(dotenv_settings.__class__):
            def __call__(self) -> Dict[str, Any]:
                # Get all dotenv values
                dotenv_values = super().__call__()
                
                # Remove secret fields
                for field in cls._secret_fields:
                    env_var = field.upper()
                    if env_var in dotenv_values:
                        # Log warning but don't load
                        print(f"WARNING: Secret '{env_var}' found in .env file - ignoring for security")
                        dotenv_values.pop(env_var)
                
                return dotenv_values
        
        return (
            init_settings,
            env_settings,
            FilteredDotEnvSettings(dotenv_settings._env_file),
            file_secret_settings,
        )
```

### Implementation Steps

1. **Create Test Suite (TDD)**
   - Test rate limit configuration loading
   - Test secret filtering from .env
   - Test environment variable priority
   - Test backward compatibility

2. **Implement Rate Limit Configuration**
   - Add RateLimitConfig model
   - Update Settings with rate_limits field
   - Modify rate_limiter.py to use configuration

3. **Implement Secure Settings**
   - Create SecureSettings base class
   - Override settings source priority
   - Filter secrets from dotenv loading

4. **Update Providers**
   - Update each provider to use configured rate limits
   - Ensure graceful fallbacks

## Response

### Testing Strategy (TDD)

1. **Rate Limit Tests** (`tests/test_rate_limit_config.py`)
   - Test loading rate limits from settings
   - Test environment variable overrides
   - Test default values
   - Test per-API configuration

2. **Security Tests** (`tests/test_secure_settings.py`)
   - Test secrets blocked from .env
   - Test secrets loaded from environment
   - Test non-secrets still load from .env
   - Test warning messages

3. **Integration Tests**
   - Test providers use configured limits
   - Test full settings loading
   - Test backward compatibility

### Implementation Timeline

1. **Phase 1: Tests** (30 minutes)
   - Write all test cases
   - Ensure tests fail initially

2. **Phase 2: Rate Limiting** (45 minutes)
   - Implement configuration structure
   - Update rate limiter
   - Make tests pass

3. **Phase 3: Secure Settings** (45 minutes)
   - Implement secure settings
   - Filter secrets from .env
   - Make security tests pass

4. **Phase 4: Integration** (30 minutes)
   - Update all providers
   - Run integration tests
   - Documentation

## Consequences

### Positive
- Enhanced security: secrets never on disk
- Flexible rate limiting per environment
- Maintains backward compatibility
- Clear separation of concerns

### Negative
- Slightly more complex settings
- Requires environment variables for secrets
- May break existing deployments using .env for secrets

### Mitigations
- Clear migration guide
- Warning messages for secrets in .env
- Graceful fallbacks for rate limits
- Comprehensive documentation