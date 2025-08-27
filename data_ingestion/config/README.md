# Hybrid Configuration System

This module provides a hybrid configuration loader that integrates the `config-store` system with environment variables, while maintaining full backward compatibility with existing code.

## Overview

The hybrid configuration system provides:

- **Non-sensitive configuration loading from config-store**: Application settings, rate limits, database hosts, etc.
- **Secret loading from environment variables only**: API keys, passwords, and other sensitive data
- **Fallback mechanisms**: Automatic fallback when config-store is unavailable
- **Configuration migration utilities**: Tools to migrate between different configuration systems
- **Full backward compatibility**: Existing code works without any changes

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Application   │────│  HybridSettings  │────│   Environment   │
│      Code       │    │                  │    │   Variables     │
└─────────────────┘    │  ┌─────────────┐ │    │   (Secrets)     │
                       │  │ Config-Store│ │    └─────────────────┘
                       │  │ (Non-secret)│ │
                       │  └─────────────┐ │
                       │       │        │ │    ┌─────────────────┐
                       │  ┌────▼──────┐ │ │    │   Fallback      │
                       │  │   Cache   │ │ │────│   Mechanisms    │
                       │  └───────────┘ │ │    └─────────────────┘
                       └──────────────────┘
```

## Usage

### Basic Usage (Backward Compatible)

```python
from config import get_settings

# This works exactly as before - no changes needed
settings = get_settings()

print(f"Database host: {settings.timescale_host}")
print(f"Redis host: {settings.redis_host}")
print(f"Batch size: {settings.batch_size}")
```

### Advanced Hybrid Configuration

```python
import asyncio
from config import HybridSettings, get_config_store_status

async def main():
    settings = HybridSettings()
    
    # Check config store status
    status = get_config_store_status()
    print(f"Config store available: {status['config_store_available']}")
    
    # Get configuration with hybrid fallback
    db_host = await settings.get_config_value('database.host', 'localhost')
    batch_size = await settings.get_config_value('processing.batch_size', 1000)
    
    # Use helper methods for common configurations
    db_config = await settings.get_database_config()
    redis_config = await settings.get_redis_config()
    
    # Set runtime configuration
    await settings.set_config_value('runtime.last_update', '2025-01-01T00:00:00Z')

asyncio.run(main())
```

### Configuration Migration

```python
import asyncio
from config import ConfigMigrationTool, HybridSettings

async def migrate_config():
    settings = HybridSettings()
    migration_tool = ConfigMigrationTool(settings)
    
    # Dry run to see what would be migrated
    result = await migration_tool.migrate_env_to_config_store(
        env_prefix="NEURAL_TRADER", 
        dry_run=True
    )
    
    print(f"Would migrate {result['migrated_count']} configuration keys")
    
    # Actual migration (remove dry_run=True)
    # result = await migration_tool.migrate_env_to_config_store(
    #     env_prefix="NEURAL_TRADER", 
    #     dry_run=False
    # )

asyncio.run(migrate_config())
```

## Configuration Sources

### 1. Environment Variables (Secrets Only)

The following sensitive configuration is loaded **ONLY** from environment variables:

```bash
# API Keys
export IEX_CLOUD_API_KEY="your_key"
export ALPHA_VANTAGE_API_KEY="your_key"
export POLYGON_API_KEY="your_key"
# ... other API keys

# Database Credentials
export TIMESCALE_PASSWORD="your_password"
export REDIS_PASSWORD="your_password"
```

### 2. Config-Store (Non-Sensitive Configuration)

Non-sensitive configuration can be stored in config-store:

```python
# Application settings
await settings.set_config_value('processing.batch_size', 2000)
await settings.set_config_value('database.host', 'production-db')
await settings.set_config_value('redis.host', 'production-redis')

# Rate limits
await settings.set_config_value('rate_limits.alpha_vantage.calls_per_minute', 10)
```

### 3. Environment Variable Patterns

The system automatically converts between environment variables and config-store paths:

```
Environment Variable    →    Config-Store Path
NEURAL_TRADER_DB_HOST   →    db.host
NEURAL_TRADER_BATCH__SIZE →  batch.size
LOG_LEVEL              →    log.level
```

## Configuration Classes

### HybridSettings

Main configuration class that extends `SecureSettings` with config-store integration:

```python
class HybridSettings(SecureSettings):
    # Config-store settings
    config_store_enabled: bool = True
    config_store_backend: str = "in_memory"
    config_store_fallback_enabled: bool = True
    
    # Cache settings
    config_cache_ttl_seconds: int = 300  # 5 minutes
    config_cache_enabled: bool = True
```

**Key Methods:**
- `get_config_value(path, fallback)` - Get configuration with hybrid fallback
- `set_config_value(path, value)` - Set configuration in config-store
- `get_database_config()` - Get database configuration
- `get_redis_config()` - Get Redis configuration
- `get_config_store_status()` - Get config-store status

### ConfigMigrationUtils

Utilities for converting between different configuration formats:

```python
# Convert environment variable to config path
ConfigMigrationUtils.env_to_config_store_path("NEURAL_TRADER_DB_HOST", "NEURAL_TRADER")
# Returns: "db.host"

# Convert config path to environment variable
ConfigMigrationUtils.config_store_path_to_env("db.host", "NEURAL_TRADER") 
# Returns: "NEURAL_TRADER_DB_HOST"
```

### ConfigMigrationTool

Tool for migrating configurations between systems:

```python
tool = ConfigMigrationTool(settings)

# Migrate environment variables to config-store
result = await tool.migrate_env_to_config_store(dry_run=True)

# Export configuration to file
success = await tool.export_config_to_file("config_backup.json")
```

## Fallback Mechanisms

The system provides multiple layers of fallback:

1. **Cache** - Check cached values first (with TTL)
2. **Config-Store** - Try to load from config-store
3. **Environment Variables** - Fall back to environment variables
4. **Provided Default** - Use the default value passed to the method
5. **Field Default** - Use the field's default value from the class

## Environment Variables

### Configuration Control

```bash
# Enable/disable config-store
export CONFIG_STORE_ENABLED=true

# Config-store backend (currently only in_memory supported)
export CONFIG_STORE_BACKEND=in_memory

# Enable fallback when config-store fails
export CONFIG_STORE_FALLBACK_ENABLED=true

# Cache settings
export CONFIG_CACHE_ENABLED=true
export CONFIG_CACHE_TTL_SECONDS=300

# Migration settings
export ENABLE_CONFIG_MIGRATION=false
export MIGRATION_ENV_PREFIX=NEURAL_TRADER
export MIGRATION_DRY_RUN=true
```

### Existing Environment Variables

All existing environment variables continue to work exactly as before:

```bash
# Database
export TIMESCALE_HOST=localhost
export TIMESCALE_PORT=5432
export TIMESCALE_DATABASE=neural_trader
export TIMESCALE_USER=trader
export TIMESCALE_PASSWORD=secret

# Redis
export REDIS_HOST=localhost
export REDIS_PORT=6379
export REDIS_PASSWORD=secret

# API Keys (secrets - always from environment)
export ALPHA_VANTAGE_API_KEY=your_key
export POLYGON_API_KEY=your_key
# ... etc
```

## Security Model

### Secrets Handling

**Secrets are NEVER loaded from config-store or .env files**. They must be provided via environment variables:

- API keys
- Database passwords  
- Redis passwords
- Client secrets

The system explicitly filters out these fields when loading from config-store or .env files.

### Secret Fields

The following fields are treated as secrets:

```python
_secret_fields = {
    'iex_cloud_api_key', 'alpha_vantage_api_key', 'polygon_api_key',
    'finnhub_api_key', 'fred_api_key', 'reddit_client_id',
    'reddit_client_secret', 'quandl_api_key', 'newsapi_key',
    'yahoo_api_key', 'alpaca_api_key', 'alpaca_api_secret',
    'timescale_password', 'redis_password'
}
```

## Testing

### Running Tests

```bash
# Run hybrid configuration tests
python -m pytest tests/test_hybrid_settings.py -v

# Test basic functionality
python -c "from config import get_settings; print('✅ Works')"

# Run example
python examples/hybrid_config_example.py
```

### Mocking for Tests

```python
import pytest
from unittest.mock import patch
from config import HybridSettings

def test_with_mocked_env():
    test_env = {
        'TIMESCALE_HOST': 'test-db',
        'REDIS_HOST': 'test-redis'
    }
    
    with patch.dict(os.environ, test_env):
        settings = HybridSettings()
        assert settings.timescale_host == 'test-db'
```

## Troubleshooting

### Common Issues

**1. Config-store not available:**
```
Config store available: False
```
Solution: This is normal if config-store Rust library is not compiled. The system will fall back to environment variables.

**2. Migration fails:**
```
Migration failed: Config store not initialized
```
Solution: Ensure `CONFIG_STORE_ENABLED=true` and the config-store backend is available.

**3. Cache issues:**
```python
# Clear cache if needed
settings.clear_config_cache()
```

### Debug Information

```python
from config import get_config_store_status

status = get_config_store_status()
print(f"Status: {status}")

# Check for errors
if status.get('initialization_error'):
    print(f"Init error: {status['initialization_error']}")
if status.get('last_store_error'): 
    print(f"Store error: {status['last_store_error']}")
```

## Backward Compatibility

**100% backward compatibility is maintained.** All existing code continues to work without any changes:

```python
# This works exactly as before
from config import Settings, get_settings

settings = get_settings()
db_url = settings.timescale_url  # Still works
```

The `Settings` class now points to `HybridSettings`, but provides all the same functionality as `SecureSettings`.

## Future Enhancements

- Redis backend for config-store
- Configuration versioning
- Real-time configuration updates
- Configuration validation schemas
- Audit logging for configuration changes