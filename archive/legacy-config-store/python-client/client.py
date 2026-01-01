"""Config Store Client implementation with advanced features."""

import asyncio
import os
import json
import time
import logging
import hashlib
from typing import Any, Dict, Optional, List, Union, Callable, AsyncGenerator, Set
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from concurrent.futures import ThreadPoolExecutor
from functools import lru_cache
from urllib.parse import urljoin
import httpx
import redis
from .errors import (
    ConfigError, ConfigNotFoundError, ConfigValidationError, 
    ConfigSecurityError, ConfigTimeoutError, ConfigConnectionError
)

logger = logging.getLogger(__name__)

@dataclass
class CacheEntry:
    """Cache entry with TTL support."""
    value: Any
    expiry_time: datetime
    access_count: int = 0
    last_accessed: Optional[datetime] = None

    def is_expired(self) -> bool:
        return datetime.now() >= self.expiry_time

    def access(self) -> None:
        self.access_count += 1
        self.last_accessed = datetime.now()


@dataclass
class CircuitBreakerState:
    """Circuit breaker state management."""
    failure_count: int = 0
    success_count: int = 0
    last_failure_time: Optional[datetime] = None
    state: str = "closed"  # closed, open, half_open
    half_open_calls: int = 0


@dataclass 
class RetryConfig:
    """Retry configuration."""
    max_attempts: int = 3
    base_delay: float = 1.0
    backoff_multiplier: float = 2.0
    max_delay: float = 60.0
    jitter_enabled: bool = True


@dataclass
class ConfigStoreConfig:
    """Configuration for ConfigStoreClient."""
    # Connection settings
    service_url: str = "http://localhost:8080"
    timeout: float = 30.0
    
    # Connection pooling
    connection_pool_size: int = 10
    connection_timeout: float = 5.0
    
    # Caching settings
    cache_ttl: int = 300  # 5 minutes default
    cache_max_size: int = 10000
    cache_enabled: bool = True
    
    # Retry settings
    retry_config: RetryConfig = field(default_factory=RetryConfig)
    
    # Circuit breaker settings
    circuit_breaker_enabled: bool = True
    failure_threshold: int = 5
    recovery_timeout: int = 60
    half_open_max_calls: int = 3
    
    # Environment fallback settings
    env_prefix: str = "NEURAL_TRADER"
    enable_env_fallback: bool = True
    
    # Security settings
    enable_security_filtering: bool = True
    secret_patterns: List[str] = field(default_factory=lambda: [
        r'[Aa][Pp][Ii][-_]?[Kk][Ee][Yy]',
        r'[Pp][Aa][Ss][Ss][Ww][Oo][Rr][Dd]',
        r'[Tt][Oo][Kk][Ee][Nn]',
        r'-----BEGIN[\s\w]*PRIVATE'
    ])


class ConfigStoreClient:
    """Advanced configuration store client with caching, retry logic, and fallbacks."""
    
    def __init__(self, config: Optional[ConfigStoreConfig] = None):
        """Initialize the config store client.
        
        Args:
            config: Configuration options for the client
        """
        self.config = config or ConfigStoreConfig()
        self._cache: Dict[str, CacheEntry] = {}
        self._circuit_breaker = CircuitBreakerState()
        self._http_client: Optional[httpx.AsyncClient] = None
        self._redis_client: Optional[redis.Redis] = None
        self._executor = ThreadPoolExecutor(max_workers=self.config.connection_pool_size)
        self._shutdown = False
        
        # Initialize logging
        self._setup_logging()
        
    def _setup_logging(self):
        """Setup logging configuration."""
        if not logger.handlers:
            handler = logging.StreamHandler()
            formatter = logging.Formatter(
                '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
            )
            handler.setFormatter(formatter)
            logger.addHandler(handler)
            logger.setLevel(logging.INFO)
    
    async def __aenter__(self):
        """Async context manager entry."""
        await self._initialize_connections()
        return self
        
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit."""
        await self.close()
        
    async def _initialize_connections(self):
        """Initialize HTTP client and other connections."""
        if self._http_client is None:
            limits = httpx.Limits(
                max_keepalive_connections=self.config.connection_pool_size,
                max_connections=self.config.connection_pool_size * 2
            )
            timeout = httpx.Timeout(
                connect=self.config.connection_timeout,
                read=self.config.timeout,
                write=self.config.timeout,
                pool=self.config.timeout
            )
            self._http_client = httpx.AsyncClient(
                limits=limits,
                timeout=timeout
            )
            
    async def close(self):
        """Close connections and cleanup resources."""
        self._shutdown = True
        if self._http_client:
            await self._http_client.aclose()
        if self._redis_client:
            await self._redis_client.aclose()
        self._executor.shutdown(wait=True)

    def _is_circuit_breaker_open(self) -> bool:
        """Check if circuit breaker is open."""
        if not self.config.circuit_breaker_enabled:
            return False
            
        if self._circuit_breaker.state == "open":
            # Check if recovery timeout has passed
            if (self._circuit_breaker.last_failure_time and 
                datetime.now() - self._circuit_breaker.last_failure_time > 
                timedelta(seconds=self.config.recovery_timeout)):
                self._circuit_breaker.state = "half_open"
                self._circuit_breaker.half_open_calls = 0
                return False
            return True
        return False

    def _record_success(self):
        """Record a successful operation for circuit breaker."""
        if not self.config.circuit_breaker_enabled:
            return
            
        if self._circuit_breaker.state == "half_open":
            self._circuit_breaker.success_count += 1
            if self._circuit_breaker.success_count >= self.config.half_open_max_calls:
                self._circuit_breaker.state = "closed"
                self._circuit_breaker.failure_count = 0
        elif self._circuit_breaker.state == "closed":
            self._circuit_breaker.failure_count = 0

    def _record_failure(self):
        """Record a failed operation for circuit breaker."""
        if not self.config.circuit_breaker_enabled:
            return
            
        self._circuit_breaker.failure_count += 1
        self._circuit_breaker.last_failure_time = datetime.now()
        
        if self._circuit_breaker.state == "closed":
            if self._circuit_breaker.failure_count >= self.config.failure_threshold:
                self._circuit_breaker.state = "open"
        elif self._circuit_breaker.state == "half_open":
            self._circuit_breaker.state = "open"

    def _get_cache_key(self, path: str, context: Optional[Dict] = None) -> str:
        """Generate cache key for configuration path."""
        if context:
            context_hash = hashlib.md5(json.dumps(context, sort_keys=True).encode()).hexdigest()
            return f"{path}:{context_hash}"
        return path

    def _get_from_cache(self, cache_key: str) -> Optional[Any]:
        """Retrieve value from cache if not expired."""
        if not self.config.cache_enabled or cache_key not in self._cache:
            return None
            
        entry = self._cache[cache_key]
        if entry.is_expired():
            del self._cache[cache_key]
            return None
            
        entry.access()
        return entry.value

    def _set_cache(self, cache_key: str, value: Any, ttl: Optional[int] = None):
        """Store value in cache with TTL."""
        if not self.config.cache_enabled:
            return
            
        # Evict if cache is full
        if len(self._cache) >= self.config.cache_max_size:
            self._evict_cache_entries()
            
        ttl = ttl or self.config.cache_ttl
        expiry = datetime.now() + timedelta(seconds=ttl)
        self._cache[cache_key] = CacheEntry(
            value=value,
            expiry_time=expiry
        )

    def _evict_cache_entries(self):
        """Evict expired and least recently used cache entries."""
        now = datetime.now()
        
        # First, remove expired entries
        expired_keys = [key for key, entry in self._cache.items() if entry.is_expired()]
        for key in expired_keys:
            del self._cache[key]
            
        # If still too many entries, remove LRU entries
        if len(self._cache) >= self.config.cache_max_size:
            # Sort by last accessed time (oldest first)
            sorted_entries = sorted(
                self._cache.items(),
                key=lambda x: x[1].last_accessed or datetime.min
            )
            
            # Remove oldest 10% of entries
            remove_count = max(1, len(sorted_entries) // 10)
            for key, _ in sorted_entries[:remove_count]:
                del self._cache[key]

    def _convert_env_var_to_path(self, env_var: str) -> str:
        """Convert environment variable name to configuration path."""
        # Remove prefix: "NEURAL_TRADER_SYSTEM_TRADING_SYMBOLS" → "SYSTEM_TRADING_SYMBOLS"
        if env_var.startswith(f"{self.config.env_prefix}_"):
            without_prefix = env_var[len(f"{self.config.env_prefix}_"):]
        else:
            without_prefix = env_var
            
        # Convert to lowercase and replace underscores with dots
        parts = without_prefix.split("_")
        return ".".join(part.lower() for part in parts)

    def _get_from_environment(self, path: str) -> Optional[Any]:
        """Get configuration from environment variables."""
        if not self.config.enable_env_fallback:
            return None
            
        # Convert path to environment variable name
        env_var = f"{self.config.env_prefix}_" + path.replace(".", "_").upper()
        
        env_value = os.getenv(env_var)
        if env_value is None:
            return None
            
        # Try to parse as JSON first, then as string
        try:
            return json.loads(env_value)
        except (json.JSONDecodeError, TypeError):
            return env_value

    def _filter_sensitive_data(self, path: str, value: Any) -> Any:
        """Filter sensitive data based on security patterns."""
        if not self.config.enable_security_filtering:
            return value
            
        if isinstance(value, str):
            # Check if value contains sensitive patterns
            import re
            for pattern in self.config.secret_patterns:
                if re.search(pattern, value, re.IGNORECASE):
                    logger.warning(f"Sensitive data detected in configuration path: {path}")
                    return "***FILTERED***"
        
        return value

    async def _make_request(self, method: str, endpoint: str, **kwargs) -> httpx.Response:
        """Make HTTP request with retry logic and circuit breaker."""
        if self._is_circuit_breaker_open():
            raise ConfigConnectionError("Circuit breaker is open", endpoint)
            
        await self._initialize_connections()
        
        url = urljoin(self.config.service_url, endpoint)
        retry_config = self.config.retry_config
        
        for attempt in range(retry_config.max_attempts):
            try:
                response = await self._http_client.request(method, url, **kwargs)
                
                if response.status_code == 200:
                    self._record_success()
                    return response
                elif response.status_code == 404:
                    raise ConfigNotFoundError(endpoint)
                elif response.status_code >= 500:
                    # Server error - retry
                    raise ConfigConnectionError(f"Server error: {response.status_code}")
                else:
                    # Client error - don't retry
                    raise ConfigConnectionError(f"Client error: {response.status_code}")
                    
            except (httpx.ConnectError, httpx.TimeoutException, ConfigConnectionError) as e:
                self._record_failure()
                
                if attempt == retry_config.max_attempts - 1:
                    raise ConfigConnectionError(str(e), endpoint)
                    
                # Calculate delay with exponential backoff and jitter
                delay = min(
                    retry_config.base_delay * (retry_config.backoff_multiplier ** attempt),
                    retry_config.max_delay
                )
                
                if retry_config.jitter_enabled:
                    import random
                    delay *= (0.5 + random.random() * 0.5)  # Add ±50% jitter
                    
                logger.info(f"Retrying request to {endpoint} after {delay:.2f}s (attempt {attempt + 1})")
                await asyncio.sleep(delay)
                
        raise ConfigConnectionError("Max retries exceeded", endpoint)

    async def get(self, path: str, default: Any = None, ttl: Optional[int] = None) -> Any:
        """Get configuration value with caching and fallback logic.
        
        Args:
            path: Configuration path (e.g., "trading.api.binance.key")
            default: Default value if not found
            ttl: Cache TTL override
            
        Returns:
            Configuration value
            
        Raises:
            ConfigNotFoundError: If path not found and no default provided
            ConfigError: On other errors
        """
        cache_key = self._get_cache_key(path)
        
        # Check cache first
        cached_value = self._get_from_cache(cache_key)
        if cached_value is not None:
            logger.debug(f"Cache hit for path: {path}")
            return cached_value
            
        try:
            # Try config store service
            response = await self._make_request("GET", f"/config/{path}")
            data = response.json()
            
            value = data.get("value")
            if value is not None:
                # Apply security filtering
                filtered_value = self._filter_sensitive_data(path, value)
                
                # Cache the result
                self._set_cache(cache_key, filtered_value, ttl)
                
                logger.debug(f"Config store hit for path: {path}")
                return filtered_value
                
        except ConfigConnectionError as e:
            logger.warning(f"Config store unavailable: {e}")
            
        # Fallback to environment variables
        env_value = self._get_from_environment(path)
        if env_value is not None:
            logger.info(f"Environment fallback used for path: {path}")
            filtered_value = self._filter_sensitive_data(path, env_value)
            self._set_cache(cache_key, filtered_value, ttl)
            return filtered_value
            
        # Return default if provided
        if default is not None:
            logger.info(f"Default value used for path: {path}")
            return default
            
        raise ConfigNotFoundError(path)

    async def set(self, path: str, value: Any, ttl: Optional[int] = None) -> None:
        """Set configuration value.
        
        Args:
            path: Configuration path
            value: Value to set
            ttl: Cache TTL override
            
        Raises:
            ConfigError: On errors
        """
        try:
            payload = {"value": value}
            if ttl:
                payload["ttl"] = ttl
                
            response = await self._make_request("POST", f"/config/{path}", json=payload)
            
            # Update cache
            cache_key = self._get_cache_key(path)
            self._set_cache(cache_key, value, ttl)
            
            logger.info(f"Configuration set for path: {path}")
            
        except ConfigConnectionError as e:
            raise ConfigError(f"Failed to set configuration: {e}")

    async def delete(self, path: str) -> None:
        """Delete configuration value.
        
        Args:
            path: Configuration path
            
        Raises:
            ConfigError: On errors
        """
        try:
            await self._make_request("DELETE", f"/config/{path}")
            
            # Remove from cache
            cache_key = self._get_cache_key(path)
            self._cache.pop(cache_key, None)
            
            logger.info(f"Configuration deleted for path: {path}")
            
        except ConfigConnectionError as e:
            raise ConfigError(f"Failed to delete configuration: {e}")

    async def exists(self, path: str) -> bool:
        """Check if configuration path exists.
        
        Args:
            path: Configuration path
            
        Returns:
            True if path exists, False otherwise
        """
        try:
            await self.get(path)
            return True
        except ConfigNotFoundError:
            return False

    async def list_keys(self, prefix: str = "") -> List[str]:
        """List configuration keys with optional prefix.
        
        Args:
            prefix: Key prefix to filter by
            
        Returns:
            List of configuration keys
        """
        try:
            params = {"prefix": prefix} if prefix else {}
            response = await self._make_request("GET", "/config", params=params)
            data = response.json()
            return data.get("keys", [])
            
        except ConfigConnectionError as e:
            logger.warning(f"Failed to list keys: {e}")
            return []

    async def get_string(self, path: str, default: Optional[str] = None) -> str:
        """Get configuration as string."""
        value = await self.get(path, default)
        if not isinstance(value, str):
            raise ConfigValidationError(path, f"Expected string, got {type(value).__name__}")
        return value

    async def get_int(self, path: str, default: Optional[int] = None) -> int:
        """Get configuration as integer."""
        value = await self.get(path, default)
        if not isinstance(value, int):
            try:
                return int(value)
            except (ValueError, TypeError):
                raise ConfigValidationError(path, f"Cannot convert {value} to int")
        return value

    async def get_float(self, path: str, default: Optional[float] = None) -> float:
        """Get configuration as float."""
        value = await self.get(path, default)
        if not isinstance(value, (int, float)):
            try:
                return float(value)
            except (ValueError, TypeError):
                raise ConfigValidationError(path, f"Cannot convert {value} to float")
        return float(value)

    async def get_bool(self, path: str, default: Optional[bool] = None) -> bool:
        """Get configuration as boolean."""
        value = await self.get(path, default)
        if isinstance(value, bool):
            return value
        if isinstance(value, str):
            return value.lower() in ("true", "1", "yes", "on")
        if isinstance(value, (int, float)):
            return bool(value)
        raise ConfigValidationError(path, f"Cannot convert {value} to bool")

    async def get_list(self, path: str, default: Optional[List] = None) -> List:
        """Get configuration as list."""
        value = await self.get(path, default)
        if not isinstance(value, list):
            raise ConfigValidationError(path, f"Expected list, got {type(value).__name__}")
        return value

    async def get_dict(self, path: str, default: Optional[Dict] = None) -> Dict:
        """Get configuration as dictionary."""
        value = await self.get(path, default)
        if not isinstance(value, dict):
            raise ConfigValidationError(path, f"Expected dict, got {type(value).__name__}")
        return value

    async def health_check(self) -> Dict[str, Any]:
        """Perform health check on config store service.
        
        Returns:
            Health status information
        """
        try:
            response = await self._make_request("GET", "/health")
            data = response.json()
            
            return {
                "status": "healthy",
                "service_available": True,
                "circuit_breaker_state": self._circuit_breaker.state,
                "cache_size": len(self._cache),
                "response_time": data.get("response_time"),
                "service_data": data
            }
            
        except ConfigConnectionError as e:
            return {
                "status": "unhealthy",
                "service_available": False,
                "circuit_breaker_state": self._circuit_breaker.state,
                "cache_size": len(self._cache),
                "error": str(e)
            }

    async def clear_cache(self, prefix: Optional[str] = None) -> None:
        """Clear cache entries.
        
        Args:
            prefix: Only clear entries with this prefix (optional)
        """
        if prefix:
            keys_to_remove = [key for key in self._cache.keys() if key.startswith(prefix)]
            for key in keys_to_remove:
                del self._cache[key]
        else:
            self._cache.clear()
            
        logger.info(f"Cache cleared (prefix: {prefix or 'all'})")

    def get_cache_stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        now = datetime.now()
        total_entries = len(self._cache)
        expired_entries = sum(1 for entry in self._cache.values() if entry.is_expired())
        
        return {
            "total_entries": total_entries,
            "expired_entries": expired_entries,
            "active_entries": total_entries - expired_entries,
            "cache_hit_ratio": getattr(self, "_cache_hits", 0) / max(getattr(self, "_cache_requests", 1), 1)
        }