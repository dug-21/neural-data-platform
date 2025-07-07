# Security Implementation Documentation

## Overview

This document describes the security enhancements implemented for the Neural Trader data ingestion system, specifically focusing on:

1. **Secure Secrets Management** - Preventing secrets from being loaded from `.env` files
2. **Configurable Rate Limiting** - Per-API rate limit configuration with environment variable overrides

## 1. Secure Secrets Management

### Problem
API keys and passwords were previously loadable from `.env` files, which could lead to accidental commits of sensitive data to version control.

### Solution
Implemented a `SecureSettings` class that filters out secret fields when loading from `.env` files while still allowing them to be loaded from environment variables.

### Implementation Details

**File:** `data_ingestion/config/secure_settings.py`

Key features:
- Identifies secret fields in a `_secret_fields` set
- Custom settings source that filters secrets from dotenv files
- Warnings when secrets are found in `.env` files
- Non-secret configurations still load normally from `.env`

**Secret fields protected:**
- API Keys: `iex_cloud_api_key`, `alpha_vantage_api_key`, `polygon_api_key`, `finnhub_api_key`, `fred_api_key`, `reddit_client_id`, `reddit_client_secret`, `quandl_api_key`, `newsapi_key`, `yahoo_api_key`
- Passwords: `timescale_password`, `redis_password`

### Usage

```python
from data_ingestion.config.settings import get_settings

# Secrets will only load from environment variables
settings = get_settings()

# Non-secrets still load from .env
print(settings.log_level)  # Works from .env

# Secrets require environment variables
print(settings.alpha_vantage_api_key)  # Only from env var
```

### Environment Variable Setup

```bash
# Set secrets as environment variables
export ALPHA_VANTAGE_API_KEY="your-secret-key"
export REDIS_PASSWORD="your-redis-password"

# Non-secrets can still be in .env file
echo "LOG_LEVEL=DEBUG" >> .env
echo "BATCH_SIZE=2000" >> .env
```

## 2. Configurable Rate Limiting

### Problem
Rate limits were hardcoded in the `RateLimiter` class, making it difficult to adjust for different environments or API plan changes.

### Solution
Created a `ConfigurableRateLimiter` that reads rate limit configuration from settings and supports environment variable overrides.

### Implementation Details

**Files:**
- `data_ingestion/config/secure_settings.py` - Added `RateLimitConfig` model and rate limits configuration
- `data_ingestion/utils/configurable_rate_limiter.py` - Extended RateLimiter with configuration support

**Features:**
- Per-API rate limit configuration in settings
- Environment variable overrides for each API
- JSON configuration support
- Burst limiting capability
- Backward compatible with existing code

### Default Rate Limits

```python
rate_limits = {
    "alpha_vantage": RateLimitConfig(calls_per_minute=5, calls_per_day=500),
    "polygon": RateLimitConfig(calls_per_minute=5),
    "finnhub": RateLimitConfig(calls_per_minute=60),
    "newsapi": RateLimitConfig(calls_per_day=100),
    "fred": RateLimitConfig(calls_per_minute=120),
    "reddit": RateLimitConfig(calls_per_minute=60),
    "nasdaq": RateLimitConfig(calls_per_day=50000),
    "yahoo_finance": RateLimitConfig(calls_per_day=200),
}
```

### Environment Variable Overrides

Individual API rate limits can be overridden:

```bash
# Override Alpha Vantage rate limits
export RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE=10
export RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_DAY=1000
export RATE_LIMIT_ALPHA_VANTAGE_BURST_SIZE=5

# Override Polygon rate limits
export RATE_LIMIT_POLYGON_CALLS_PER_MINUTE=20
```

JSON configuration for multiple APIs:

```bash
export RATE_LIMITS_JSON='{
    "custom_api": {
        "calls_per_minute": 100,
        "calls_per_day": 10000,
        "burst_size": 50
    }
}'
```

### Usage in Providers

```python
from data_ingestion.config.settings import get_settings
from data_ingestion.utils.configurable_rate_limiter import ConfigurableRateLimiter

settings = get_settings()

# Create rate limiter from settings
limiter = ConfigurableRateLimiter.from_settings('alpha_vantage', settings)

# Use in API calls
can_request, wait_time = limiter.can_make_request()
if can_request:
    limiter.record_request()
    # Make API call
else:
    # Wait or handle rate limit
    time.sleep(wait_time)
```

## Testing

### Test Files
- `data_ingestion/tests/test_secure_settings.py` - 17 tests for secure settings
- `data_ingestion/tests/test_rate_limit_config.py` - 9 tests for rate limiting
- `data_ingestion/test_integration.py` - Integration tests

### Running Tests

```bash
# Run all security tests
python -m pytest data_ingestion/tests/test_secure_settings.py data_ingestion/tests/test_rate_limit_config.py -v

# Run integration test
python -m data_ingestion.test_integration
```

## Migration Guide

### For Developers

1. **Remove secrets from .env files**
   - Delete any API keys or passwords from `.env` files
   - Move them to environment variables or secret management systems

2. **Update deployment scripts**
   - Ensure environment variables are set before starting the application
   - Consider using tools like `direnv`, `dotenv-vault`, or cloud secret managers

3. **Rate limit adjustments**
   - Review current API usage and adjust rate limits as needed
   - Set environment variables for any API-specific overrides

### For DevOps

1. **Environment Setup**
   ```bash
   # Example systemd environment file
   Environment="ALPHA_VANTAGE_API_KEY=xxx"
   Environment="REDIS_PASSWORD=xxx"
   Environment="RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE=10"
   ```

2. **Docker Compose**
   ```yaml
   services:
     data_ingestion:
       environment:
         - ALPHA_VANTAGE_API_KEY
         - REDIS_PASSWORD
         - RATE_LIMIT_ALPHA_VANTAGE_CALLS_PER_MINUTE=10
   ```

3. **Kubernetes Secrets**
   ```yaml
   apiVersion: v1
   kind: Secret
   metadata:
     name: api-secrets
   stringData:
     ALPHA_VANTAGE_API_KEY: "xxx"
     REDIS_PASSWORD: "xxx"
   ```

## Security Best Practices

1. **Never commit secrets** - Use `.gitignore` to exclude `.env` files
2. **Use secret management** - Consider HashiCorp Vault, AWS Secrets Manager, etc.
3. **Rotate keys regularly** - Update API keys periodically
4. **Monitor rate limits** - Track API usage to avoid hitting limits
5. **Principle of least privilege** - Only provide necessary API keys to each service

## Troubleshooting

### Secrets not loading
- Check environment variables are properly set
- Look for WARNING messages about ignored secrets
- Verify variable names match exactly (case-sensitive)

### Rate limits not working
- Check environment variable format: `RATE_LIMIT_{API_NAME}_{METRIC}`
- Verify JSON format if using `RATE_LIMITS_JSON`
- Check for validation errors in logs

### Performance issues
- Rate limiters use token bucket algorithm (O(1) complexity)
- Burst limiting allows temporary spikes
- Consider adjusting `burst_size` for APIs with variable load