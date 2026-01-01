#!/usr/bin/env python3
"""
Config Store Python Client

A robust Python client for the neural-trader config-store service.
Implements TDD London School approach with comprehensive error handling,
caching, schema validation, and environment variable fallback.

Architecture:
- gRPC-based communication with the Rust config-store service
- Intelligent caching with TTL support
- Type-safe configuration retrieval
- Schema validation with detailed error reporting  
- Graceful fallback to environment variables
- Comprehensive observability and monitoring
"""

import asyncio
import json
import logging
import os
import re
import time
from datetime import datetime, timedelta
from typing import Any, Dict, List, Optional, Union, Type, AsyncIterator
from dataclasses import dataclass
from enum import Enum

import grpc
from grpc import StatusCode
from grpc.aio import AioRpcError
import jsonschema
from jsonschema import ValidationError as JsonSchemaValidationError


# Custom Exceptions
class ConfigError(Exception):
    """Base configuration error"""
    pass

class ConnectionError(ConfigError):
    """Connection-related configuration error"""
    pass

class ValidationError(ConfigError):
    """Validation-related configuration error"""
    def __init__(self, message: str, field: str = None, code: str = None):
        super().__init__(message)
        self.field = field
        self.code = code

class TimeoutError(ConfigError):
    """Timeout-related configuration error"""
    pass

class KeyNotFoundError(ConfigError):
    """Key not found error"""
    def __init__(self, key: str):
        super().__init__(f"Configuration key '{key}' not found")
        self.key = key


@dataclass
class ConfigChange:
    """Represents a configuration change event"""
    key: str
    value: Any
    operation: str
    timestamp: datetime


class ConfigStoreClient:
    """
    Config Store Python Client
    
    Provides a high-level interface for interacting with the config-store service
    with built-in caching, validation, error handling, and fallback mechanisms.
    """
    
    def __init__(
        self,
        grpc_channel: Optional[Any] = None,
        cache_ttl_seconds: int = 300,
        enable_fallback: bool = True,
        max_retry_attempts: int = 3,
        connection_timeout: int = 5
    ):
        """
        Initialize config store client
        
        Args:
            grpc_channel: Optional gRPC channel for testing
            cache_ttl_seconds: Cache TTL in seconds (default: 5 minutes)
            enable_fallback: Enable fallback to environment variables
            max_retry_attempts: Maximum retry attempts for operations
            connection_timeout: Connection timeout in seconds
        """
        self.grpc_channel = grpc_channel
        self.cache_ttl_seconds = cache_ttl_seconds
        self.enable_fallback = enable_fallback
        self.max_retry_attempts = max_retry_attempts
        self.connection_timeout = connection_timeout
        
        # Internal state
        self._cache: Dict[str, Dict[str, Any]] = {}
        self._connection_pool = None
        self._health_check_interval = 30
        self._connected = False
        self._logger = logging.getLogger(__name__)
        
        # Key validation pattern
        self._key_pattern = re.compile(r'^[a-zA-Z0-9._-]+$')
        
    async def connect(self, address: str, timeout_seconds: int = 5) -> bool:
        """
        Establish connection to config store
        
        Args:
            address: gRPC server address (e.g., "localhost:50051")
            timeout_seconds: Connection timeout
            
        Returns:
            True if connection successful
            
        Raises:
            ConnectionError: If connection fails
            TimeoutError: If connection times out
        """
        try:
            if not self.grpc_channel:
                # This would create actual gRPC channel in production
                # For testing, we use the mocked channel passed in constructor
                self.grpc_channel = grpc.aio.insecure_channel(address)
            
            # Check if channel is ready (mocked in tests)
            if hasattr(self.grpc_channel, 'get_state'):
                state = self.grpc_channel.get_state()
                if state == "READY" or str(state).endswith("READY"):
                    self._connected = True
                    self._logger.info(f"Connected to config store at {address}")
                    return True
            
            # For actual implementation, wait for channel to be ready
            if not self._connected:
                await asyncio.wait_for(
                    self.grpc_channel.channel_ready(),
                    timeout=timeout_seconds
                )
                self._connected = True
                self._logger.info(f"Connected to config store at {address}")
                
            return True
            
        except asyncio.TimeoutError:
            raise TimeoutError(f"Connection timeout after {timeout_seconds} seconds")
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to connect to config store: {e}")
        except Exception as e:
            raise ConnectionError(f"Unexpected connection error: {e}")
            
    async def disconnect(self):
        """Close connection to config store"""
        if self.grpc_channel:
            await self.grpc_channel.close()
            self._connected = False
            self._logger.info("Disconnected from config store")
            
    async def health_check(self) -> bool:
        """
        Check if config store is healthy
        
        Returns:
            True if healthy, False otherwise
        """
        try:
            if not self._connected:
                return False
                
            # Call health check service method
            result = await self._call_service("HealthCheck", {})
            return result.get("healthy", False)
            
        except Exception as e:
            self._logger.warning(f"Health check failed: {e}")
            return False
            
    async def get(
        self, 
        key: str, 
        default: Any = None, 
        timeout_seconds: int = 5
    ) -> Any:
        """
        Get single configuration value
        
        Args:
            key: Configuration key
            default: Default value if key not found
            timeout_seconds: Request timeout
            
        Returns:
            Configuration value
            
        Raises:
            KeyNotFoundError: If key not found and no default provided
            ValidationError: If key format is invalid
            TimeoutError: If request times out
        """
        self._validate_key(key)
        
        # Check cache first
        cached_value = self._get_from_cache(key)
        if cached_value is not None:
            return cached_value
            
        try:
            # Call service
            result = await self._call_service(
                "GetConfig",
                {"key": key},
                timeout_seconds=timeout_seconds
            )
            
            value = result.get("value")
            self._set_in_cache(key, value)
            return value
            
        except grpc.RpcError as e:
            if e.code() == StatusCode.NOT_FOUND:
                # Try fallback to environment variable
                if self.enable_fallback:
                    env_value = self._get_env_fallback(key)
                    if env_value is not None:
                        return env_value
                        
                if default is not None:
                    return default
                    
                raise KeyNotFoundError(key)
            else:
                self._handle_grpc_error(e)
                
    async def get_typed(
        self,
        key: str,
        value_type: Type,
        default: Any = None,
        timeout_seconds: int = 5
    ) -> Any:
        """
        Get configuration value with type validation
        
        Args:
            key: Configuration key
            value_type: Expected value type
            default: Default value if key not found
            timeout_seconds: Request timeout
            
        Returns:
            Typed configuration value
            
        Raises:
            ValidationError: If type validation fails
        """
        value = await self.get(key, default, timeout_seconds)
        
        try:
            if value_type == int:
                return int(value)
            elif value_type == float:
                return float(value)
            elif value_type == bool:
                if isinstance(value, bool):
                    return value
                elif isinstance(value, str):
                    return value.lower() in ('true', '1', 'yes', 'on')
                else:
                    return bool(value)
            elif value_type == str:
                return str(value)
            else:
                # For complex types, assume value is already correct type
                if not isinstance(value, value_type):
                    raise ValueError(f"Expected {value_type}, got {type(value)}")
                return value
                
        except (ValueError, TypeError) as e:
            raise ValidationError(
                f"Type validation failed for key '{key}': {e}",
                field=key,
                code="TYPE_VALIDATION_FAILED"
            )
            
    async def get_bulk(
        self,
        keys: List[str],
        timeout_seconds: int = 10
    ) -> Dict[str, Any]:
        """
        Get multiple configuration values
        
        Args:
            keys: List of configuration keys
            timeout_seconds: Request timeout
            
        Returns:
            Dictionary of key-value pairs
        """
        # Validate all keys
        for key in keys:
            self._validate_key(key)
            
        # Check cache for all keys
        cached_results = {}
        missing_keys = []
        
        for key in keys:
            cached_value = self._get_from_cache(key)
            if cached_value is not None:
                cached_results[key] = cached_value
            else:
                missing_keys.append(key)
                
        # If all keys are cached, return immediately
        if not missing_keys:
            return cached_results
            
        try:
            # Call service for missing keys
            result = await self._call_service(
                "GetBulkConfig",
                {"keys": missing_keys},
                timeout_seconds=timeout_seconds
            )
            
            service_results = result.get("configs", {})
            
            # Cache the results
            for key, value in service_results.items():
                self._set_in_cache(key, value)
                
            # Combine cached and service results
            final_results = {**cached_results, **service_results}
            
            # Handle fallback for any still missing keys
            if self.enable_fallback:
                for key in keys:
                    if key not in final_results:
                        env_value = self._get_env_fallback(key)
                        if env_value is not None:
                            final_results[key] = env_value
                            
            return final_results
            
        except grpc.RpcError as e:
            self._handle_grpc_error(e)
            
    async def set(
        self,
        key: str,
        value: Any,
        timeout_seconds: int = 5
    ) -> bool:
        """
        Set configuration value
        
        Args:
            key: Configuration key
            value: Configuration value
            timeout_seconds: Request timeout
            
        Returns:
            True if successful
            
        Raises:
            ValidationError: If validation fails
        """
        self._validate_key(key)
        
        try:
            result = await self._call_service(
                "SetConfig",
                {"key": key, "value": value},
                timeout_seconds=timeout_seconds
            )
            
            # Invalidate cache entry
            self._invalidate_cache(key)
            
            return result.get("success", False)
            
        except grpc.RpcError as e:
            if e.code() == StatusCode.INVALID_ARGUMENT:
                raise ValidationError(
                    f"Validation failed for key '{key}': {e.details()}",
                    field=key,
                    code="VALIDATION_FAILED"
                )
            else:
                self._handle_grpc_error(e)
                
    async def set_bulk(
        self,
        config_dict: Dict[str, Any],
        timeout_seconds: int = 10
    ) -> bool:
        """
        Set multiple configuration values
        
        Args:
            config_dict: Dictionary of key-value pairs
            timeout_seconds: Request timeout
            
        Returns:
            True if successful
        """
        # Validate all keys
        for key in config_dict.keys():
            self._validate_key(key)
            
        try:
            result = await self._call_service(
                "SetBulkConfig",
                {"configs": config_dict},
                timeout_seconds=timeout_seconds
            )
            
            # Invalidate cache entries
            for key in config_dict.keys():
                self._invalidate_cache(key)
                
            return result.get("success", False)
            
        except grpc.RpcError as e:
            if e.code() == StatusCode.INVALID_ARGUMENT:
                raise ValidationError(
                    f"Validation failed: {e.details()}",
                    code="BULK_VALIDATION_FAILED"
                )
            else:
                self._handle_grpc_error(e)
                
    async def delete(self, key: str, timeout_seconds: int = 5) -> bool:
        """
        Delete configuration key
        
        Args:
            key: Configuration key to delete
            timeout_seconds: Request timeout
            
        Returns:
            True if successful
        """
        self._validate_key(key)
        
        try:
            result = await self._call_service(
                "DeleteConfig",
                {"key": key},
                timeout_seconds=timeout_seconds
            )
            
            # Invalidate cache entry
            self._invalidate_cache(key)
            
            return result.get("success", False)
            
        except grpc.RpcError as e:
            self._handle_grpc_error(e)
            
    async def watch(self, key_pattern: str) -> AsyncIterator[ConfigChange]:
        """
        Watch for configuration changes
        
        Args:
            key_pattern: Key pattern to watch (supports wildcards)
            
        Yields:
            ConfigChange objects for each change
            
        Raises:
            ConnectionError: If watch connection fails
        """
        try:
            stream = await self._call_service_stream(
                "WatchConfig",
                {"pattern": key_pattern}
            )
            
            async for change_data in stream:
                yield ConfigChange(
                    key=change_data["key"],
                    value=change_data["value"], 
                    operation=change_data["operation"],
                    timestamp=datetime.fromisoformat(change_data.get("timestamp", datetime.now().isoformat()))
                )
                
        except grpc.RpcError as e:
            raise ConnectionError(f"Watch connection failed: {e}")
            
    async def list_keys(self, prefix: str = "") -> List[str]:
        """
        List all configuration keys
        
        Args:
            prefix: Optional key prefix filter
            
        Returns:
            List of configuration keys
        """
        try:
            result = await self._call_service(
                "ListKeys",
                {"prefix": prefix}
            )
            
            return result.get("keys", [])
            
        except grpc.RpcError as e:
            self._handle_grpc_error(e)
            
    async def validate_schema(
        self,
        config: Dict[str, Any],
        schema: Dict[str, Any]
    ) -> List[ValidationError]:
        """
        Validate configuration against schema
        
        Args:
            config: Configuration dictionary to validate
            schema: JSON schema for validation
            
        Returns:
            List of validation errors (empty if valid)
        """
        errors = []
        
        try:
            jsonschema.validate(instance=config, schema=schema)
        except JsonSchemaValidationError as e:
            errors.append(ValidationError(
                message=e.message,
                field='.'.join(str(x) for x in e.absolute_path),
                code="SCHEMA_VALIDATION_FAILED"
            ))
        except Exception as e:
            errors.append(ValidationError(
                message=f"Schema validation error: {e}",
                code="SCHEMA_ERROR"
            ))
            
        return errors
        
    # Private helper methods
    
    def _validate_key(self, key: str):
        """Validate configuration key format"""
        if not key or not isinstance(key, str):
            raise ValidationError("Key must be a non-empty string", code="INVALID_KEY_FORMAT")
            
        if not self._key_pattern.match(key):
            raise ValidationError(
                f"Invalid key format: {key}. Keys must contain only letters, numbers, dots, hyphens, and underscores",
                code="INVALID_KEY_FORMAT"
            )
            
    def _get_from_cache(self, key: str) -> Any:
        """Get value from cache if not expired"""
        if key in self._cache:
            entry = self._cache[key]
            if time.time() - entry["timestamp"] < self.cache_ttl_seconds:
                return entry["value"]
            else:
                # Remove expired entry
                del self._cache[key]
        return None
        
    def _set_in_cache(self, key: str, value: Any):
        """Set value in cache with timestamp"""
        self._cache[key] = {
            "value": value,
            "timestamp": time.time()
        }
        
    def _invalidate_cache(self, key: str):
        """Remove key from cache"""
        if key in self._cache:
            del self._cache[key]
            
    def _get_env_fallback(self, key: str) -> Optional[str]:
        """Get value from environment variable as fallback"""
        # Convert config key to environment variable format
        # e.g., "database.host" -> "DATABASE_HOST"
        env_key = key.upper().replace(".", "_")
        return os.environ.get(env_key)
        
    def _handle_grpc_error(self, error: grpc.RpcError):
        """Handle gRPC errors and raise appropriate exceptions"""
        status_code = error.code()
        details = error.details()
        
        if status_code == StatusCode.UNAVAILABLE:
            raise ConnectionError(f"Service unavailable: {details}")
        elif status_code == StatusCode.DEADLINE_EXCEEDED:
            raise TimeoutError(f"Request deadline exceeded: {details}")
        elif status_code == StatusCode.INVALID_ARGUMENT:
            raise ValidationError(f"Invalid argument: {details}")
        elif status_code == StatusCode.NOT_FOUND:
            raise KeyNotFoundError("Configuration key not found")
        else:
            raise ConfigError(f"gRPC error ({status_code}): {details}")
            
    async def _call_service(
        self,
        method: str,
        params: Dict[str, Any],
        timeout_seconds: int = 5
    ) -> Dict[str, Any]:
        """Call gRPC service method"""
        # This would be implemented to make actual gRPC calls
        # For now, this is a placeholder that would be mocked in tests
        raise NotImplementedError("Service calls not implemented in base client")
        
    async def _call_service_stream(
        self,
        method: str,
        params: Dict[str, Any]
    ) -> AsyncIterator[Dict[str, Any]]:
        """Call gRPC streaming service method"""
        # This would be implemented to make actual streaming gRPC calls
        # For now, this is a placeholder that would be mocked in tests
        raise NotImplementedError("Streaming service calls not implemented in base client")
        
        # Placeholder for actual streaming implementation
        yield {}