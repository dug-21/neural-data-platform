"""Hybrid configuration loader that integrates config-store with environment variables."""

import os
import json
import asyncio
from typing import Optional, Dict, Any, Callable, Union, Set
from functools import lru_cache
from datetime import datetime, timedelta

from pydantic import BaseModel, Field, field_validator
from pydantic_settings import BaseSettings, SettingsConfigDict

try:
    # Try to import config-store components
    from config_store import ConfigStore, ConfigValue, ConfigError, InMemoryConfigStore
    CONFIG_STORE_AVAILABLE = True
except ImportError:
    CONFIG_STORE_AVAILABLE = False
    # Create dummy classes for type hints
    class ConfigStore:
        pass
    class ConfigValue:
        pass
    class ConfigError(Exception):
        pass

from .secure_settings import SecureSettings, RateLimitConfig


class ConfigMigrationUtils:
    """Utilities for migrating configuration between different storage systems."""
    
    @staticmethod
    def env_to_config_store_path(env_key: str, prefix: str = "") -> str:
        """Convert environment variable key to config-store path format."""
        key = env_key.lower()
        if prefix and key.startswith(prefix.lower() + '_'):
            key = key[len(prefix) + 1:]
        return key.replace('__', '.').replace('_', '.')
    
    @staticmethod
    def config_store_path_to_env(path: str, prefix: str = "") -> str:
        """Convert config-store path to environment variable format."""
        env_key = path.replace('.', '_').upper()
        if prefix:
            env_key = f"{prefix.upper()}_{env_key}"
        return env_key
    
    @staticmethod
    def migrate_env_to_config_store(config_store: 'ConfigStore', 
                                  env_prefix: str = "NEURAL_TRADER",
                                  exclude_secrets: bool = True,
                                  secret_fields: Optional[Set[str]] = None) -> Dict[str, Any]:
        """Migrate environment variables to config store format."""
        if not CONFIG_STORE_AVAILABLE:
            return {}
            
        migrated = {}
        secret_fields = secret_fields or set()
        
        for env_key, env_value in os.environ.items():
            if env_key.startswith(f"{env_prefix}_"):
                config_path = ConfigMigrationUtils.env_to_config_store_path(env_key, env_prefix)
                
                # Skip secrets if requested
                if exclude_secrets and any(secret in config_path for secret in secret_fields):
                    continue
                
                # Try to parse JSON values, fall back to string
                try:
                    value = json.loads(env_value)
                except (json.JSONDecodeError, ValueError):
                    value = env_value
                
                migrated[config_path] = value
                
        return migrated


class HybridConfigStore:
    """Wrapper around config-store with fallback mechanisms."""
    
    def __init__(self, config_store: Optional['ConfigStore'] = None):
        self.config_store = config_store
        self._fallback_data: Dict[str, Any] = {}
        self._last_error: Optional[Exception] = None
    
    async def get(self, path: str, fallback: Any = None) -> Any:
        """Get configuration value with fallback support."""
        if not CONFIG_STORE_AVAILABLE or not self.config_store:
            return self._fallback_data.get(path, fallback)
        
        try:
            result = await self.config_store.get(path)
            if hasattr(result, 'value'):
                return result.value
            return result
        except Exception as e:
            self._last_error = e
            # Try fallback data first, then provided fallback
            return self._fallback_data.get(path, fallback)
    
    async def set(self, path: str, value: Any) -> bool:
        """Set configuration value in store or fallback."""
        if not CONFIG_STORE_AVAILABLE or not self.config_store:
            self._fallback_data[path] = value
            return True
        
        try:
            await self.config_store.set(path, ConfigValue(value))
            return True
        except Exception as e:
            self._last_error = e
            self._fallback_data[path] = value
            return False
    
    def set_fallback_data(self, data: Dict[str, Any]):
        """Set fallback data for when config-store is unavailable."""
        self._fallback_data.update(data)
    
    def get_last_error(self) -> Optional[Exception]:
        """Get the last error that occurred."""
        return self._last_error


class HybridSettings(SecureSettings):
    """
    Hybrid configuration loader that integrates config-store with environment variables.
    
    This class extends SecureSettings to provide:
    - Non-sensitive configuration loading from config-store
    - Secret loading from environment variables only
    - Fallback mechanisms when config-store is unavailable
    - Configuration migration utilities
    - Backward compatibility with existing SecureSettings
    """
    
    model_config = SettingsConfigDict(
        env_file='.env',
        env_file_encoding='utf-8',
        case_sensitive=False,
        env_nested_delimiter='__',
        extra='allow',
    )
    
    # Configuration store settings
    config_store_enabled: bool = Field(True, alias="CONFIG_STORE_ENABLED")
    config_store_backend: str = Field("in_memory", alias="CONFIG_STORE_BACKEND")  # in_memory, redis, etc.
    config_store_connection_timeout: int = Field(5, alias="CONFIG_STORE_CONNECTION_TIMEOUT")
    config_store_retry_attempts: int = Field(3, alias="CONFIG_STORE_RETRY_ATTEMPTS")
    config_store_fallback_enabled: bool = Field(True, alias="CONFIG_STORE_FALLBACK_ENABLED")
    
    # Migration settings
    enable_config_migration: bool = Field(False, alias="ENABLE_CONFIG_MIGRATION")
    migration_env_prefix: str = Field("NEURAL_TRADER", alias="MIGRATION_ENV_PREFIX")
    migration_dry_run: bool = Field(True, alias="MIGRATION_DRY_RUN")
    
    # Cache settings for config-store
    config_cache_ttl_seconds: int = Field(300, alias="CONFIG_CACHE_TTL_SECONDS")  # 5 minutes
    config_cache_enabled: bool = Field(True, alias="CONFIG_CACHE_ENABLED")
    
    def __init__(self, **kwargs):
        super().__init__(**kwargs)
        self._config_store: Optional[HybridConfigStore] = None
        self._config_cache: Dict[str, Dict[str, Any]] = {}  # path -> {value, expires_at}
        self._initialization_error: Optional[Exception] = None
        
        # Initialize config store if available and enabled
        if CONFIG_STORE_AVAILABLE and self.config_store_enabled:
            try:
                asyncio.create_task(self._initialize_config_store())
            except Exception as e:
                self._initialization_error = e
                if not self.config_store_fallback_enabled:
                    raise
    
    async def _initialize_config_store(self):
        """Initialize the config store asynchronously."""
        try:
            if self.config_store_backend == "in_memory":
                store = InMemoryConfigStore()
            else:
                # Future: Add other backend support (Redis, etc.)
                raise ValueError(f"Unsupported config store backend: {self.config_store_backend}")
            
            self._config_store = HybridConfigStore(store)
            
            # Migrate environment variables to config store if enabled
            if self.enable_config_migration:
                await self._migrate_env_to_config_store()
                
        except Exception as e:
            self._initialization_error = e
            if not self.config_store_fallback_enabled:
                raise
            # Create fallback config store
            self._config_store = HybridConfigStore(None)
    
    async def _migrate_env_to_config_store(self):
        """Migrate environment variables to config store."""
        if not self._config_store:
            return
        
        migration_data = ConfigMigrationUtils.migrate_env_to_config_store(
            self._config_store.config_store,
            env_prefix=self.migration_env_prefix,
            exclude_secrets=True,
            secret_fields=self._secret_fields
        )
        
        if self.migration_dry_run:
            print(f"Migration dry run - would migrate {len(migration_data)} keys:")
            for path, value in migration_data.items():
                print(f"  {path}: {type(value).__name__}")
        else:
            for path, value in migration_data.items():
                await self._config_store.set(path, value)
            print(f"Migrated {len(migration_data)} configuration keys to config store")
    
    def _is_cached(self, path: str) -> bool:
        """Check if configuration value is cached and not expired."""
        if not self.config_cache_enabled or path not in self._config_cache:
            return False
        
        cache_entry = self._config_cache[path]
        expires_at = cache_entry.get('expires_at')
        
        if expires_at and datetime.utcnow() > expires_at:
            del self._config_cache[path]
            return False
        
        return True
    
    def _cache_value(self, path: str, value: Any):
        """Cache a configuration value with TTL."""
        if not self.config_cache_enabled:
            return
        
        expires_at = datetime.utcnow() + timedelta(seconds=self.config_cache_ttl_seconds)
        self._config_cache[path] = {
            'value': value,
            'expires_at': expires_at
        }
    
    def _get_cached_value(self, path: str) -> Any:
        """Get cached configuration value."""
        return self._config_cache.get(path, {}).get('value')
    
    async def get_config_value(self, path: str, fallback: Any = None) -> Any:
        """
        Get configuration value from config-store with fallback to environment variables.
        
        This method provides the core hybrid functionality:
        1. Check cache first
        2. Try config-store 
        3. Fall back to environment variables
        4. Use provided fallback
        """
        # Check cache first
        if self._is_cached(path):
            return self._get_cached_value(path)
        
        # Try config store
        if self._config_store:
            try:
                value = await self._config_store.get(path, fallback)
                if value is not None:
                    self._cache_value(path, value)
                    return value
            except Exception as e:
                # Config store failed, continue to fallback
                pass
        
        # Fall back to environment variable
        env_key = ConfigMigrationUtils.config_store_path_to_env(path, self.migration_env_prefix)
        env_value = os.getenv(env_key)
        
        if env_value is not None:
            try:
                # Try to parse as JSON
                value = json.loads(env_value)
            except (json.JSONDecodeError, ValueError):
                value = env_value
            
            self._cache_value(path, value)
            return value
        
        # Return fallback
        return fallback
    
    async def set_config_value(self, path: str, value: Any) -> bool:
        """Set configuration value in config-store."""
        if not self._config_store:
            return False
        
        success = await self._config_store.set(path, value)
        if success:
            self._cache_value(path, value)
        
        return success
    
    def get_config_store_status(self) -> Dict[str, Any]:
        """Get status information about the config store."""
        return {
            'config_store_available': CONFIG_STORE_AVAILABLE,
            'config_store_enabled': self.config_store_enabled,
            'config_store_initialized': self._config_store is not None,
            'backend': self.config_store_backend,
            'fallback_enabled': self.config_store_fallback_enabled,
            'cache_enabled': self.config_cache_enabled,
            'cache_size': len(self._config_cache),
            'initialization_error': str(self._initialization_error) if self._initialization_error else None,
            'last_store_error': str(self._config_store.get_last_error()) if self._config_store and self._config_store.get_last_error() else None,
        }
    
    # Convenience methods for common configuration patterns
    async def get_rate_limit_config(self, api_name: str) -> Optional[RateLimitConfig]:
        """Get rate limit configuration for a specific API."""
        config_path = f"rate_limits.{api_name}"
        rate_limit_data = await self.get_config_value(config_path)
        
        if rate_limit_data:
            try:
                if isinstance(rate_limit_data, dict):
                    return RateLimitConfig(**rate_limit_data)
                elif hasattr(rate_limit_data, 'calls_per_minute'):
                    return rate_limit_data
            except Exception:
                pass
        
        # Fall back to environment-based rate limits
        return self.rate_limits.get(api_name)
    
    async def get_database_config(self) -> Dict[str, Any]:
        """Get database configuration from config-store or environment."""
        return {
            'host': await self.get_config_value('database.host', self.timescale_host),
            'port': await self.get_config_value('database.port', self.timescale_port),
            'database': await self.get_config_value('database.name', self.timescale_database),
            'user': await self.get_config_value('database.user', self.timescale_user),
            # Password always comes from environment for security
            'password': self.timescale_password,
        }
    
    async def get_redis_config(self) -> Dict[str, Any]:
        """Get Redis configuration from config-store or environment."""
        return {
            'host': await self.get_config_value('redis.host', self.redis_host),
            'port': await self.get_config_value('redis.port', self.redis_port),
            'db': await self.get_config_value('redis.db', self.redis_db),
            # Password always comes from environment for security
            'password': self.redis_password,
            'max_connections': await self.get_config_value('redis.max_connections', self.redis_max_connections),
            'decode_responses': await self.get_config_value('redis.decode_responses', self.redis_decode_responses),
        }
    
    async def update_rate_limits_from_config_store(self) -> bool:
        """Update rate limits from config store."""
        try:
            rate_limits_data = await self.get_config_value('rate_limits')
            if rate_limits_data and isinstance(rate_limits_data, dict):
                updated_rate_limits = {}
                for api_name, config_data in rate_limits_data.items():
                    if isinstance(config_data, dict):
                        updated_rate_limits[api_name] = RateLimitConfig(**config_data)
                    else:
                        updated_rate_limits[api_name] = config_data
                
                # Update the rate_limits field
                self.rate_limits.update(updated_rate_limits)
                return True
        except Exception:
            pass
        
        return False
    
    def clear_config_cache(self):
        """Clear the configuration cache."""
        self._config_cache.clear()
    
    async def refresh_config(self) -> Dict[str, bool]:
        """Refresh configuration from config-store."""
        results = {}
        
        # Clear cache first
        self.clear_config_cache()
        
        # Refresh rate limits
        results['rate_limits'] = await self.update_rate_limits_from_config_store()
        
        # Refresh other common configurations
        # (Add more as needed)
        
        return results


# Maintain backward compatibility
Settings = HybridSettings


@lru_cache()
def get_settings() -> HybridSettings:
    """Get cached settings instance with hybrid configuration support."""
    return HybridSettings()


# Convenience functions for common operations
async def get_hybrid_config_value(path: str, fallback: Any = None) -> Any:
    """Get configuration value using hybrid approach."""
    settings = get_settings()
    return await settings.get_config_value(path, fallback)


async def set_hybrid_config_value(path: str, value: Any) -> bool:
    """Set configuration value using hybrid approach."""
    settings = get_settings()
    return await settings.set_config_value(path, value)


def get_config_store_status() -> Dict[str, Any]:
    """Get status of the config store system."""
    settings = get_settings()
    return settings.get_config_store_status()


# Migration utilities
class ConfigMigrationTool:
    """Tool for migrating configurations between different systems."""
    
    def __init__(self, settings: Optional[HybridSettings] = None):
        self.settings = settings or get_settings()
    
    async def migrate_env_to_config_store(self, env_prefix: str = "NEURAL_TRADER", 
                                        dry_run: bool = True) -> Dict[str, Any]:
        """Migrate environment variables to config store."""
        if not self.settings._config_store:
            raise RuntimeError("Config store not initialized")
        
        migration_data = ConfigMigrationUtils.migrate_env_to_config_store(
            self.settings._config_store.config_store,
            env_prefix=env_prefix,
            exclude_secrets=True,
            secret_fields=self.settings._secret_fields
        )
        
        if not dry_run:
            for path, value in migration_data.items():
                await self.settings._config_store.set(path, value)
        
        return {
            'migrated_count': len(migration_data),
            'dry_run': dry_run,
            'data': migration_data if dry_run else {},
        }
    
    async def export_config_to_file(self, file_path: str, format: str = 'json') -> bool:
        """Export current configuration to file."""
        try:
            config_data = {}
            
            # Export non-secret configuration
            for field_name, field_info in self.settings.model_fields.items():
                if field_name not in self.settings._secret_fields:
                    value = getattr(self.settings, field_name, None)
                    if value is not None:
                        config_data[field_name] = value
            
            # Write to file
            with open(file_path, 'w') as f:
                if format.lower() == 'json':
                    json.dump(config_data, f, indent=2, default=str)
                else:
                    raise ValueError(f"Unsupported format: {format}")
            
            return True
        except Exception:
            return False