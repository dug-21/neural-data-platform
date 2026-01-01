"""Exception classes for config store client."""

from typing import Optional, Dict, Any


class ConfigError(Exception):
    """Base exception for configuration errors."""
    
    def __init__(self, message: str, details: Optional[Dict[str, Any]] = None):
        super().__init__(message)
        self.details = details or {}
        
    def is_retriable(self) -> bool:
        """Check if this error is retriable."""
        return False


class ConfigNotFoundError(ConfigError):
    """Raised when a configuration key is not found."""
    
    def __init__(self, key: str, message: Optional[str] = None):
        self.key = key
        message = message or f"Configuration key '{key}' not found"
        super().__init__(message, {"key": key})


class ConfigValidationError(ConfigError):
    """Raised when configuration validation fails."""
    
    def __init__(self, key: str, reason: str, value: Any = None):
        self.key = key
        self.reason = reason
        self.value = value
        message = f"Configuration key '{key}' failed validation: {reason}"
        super().__init__(message, {"key": key, "reason": reason, "value": value})


class ConfigSecurityError(ConfigError):
    """Raised when security constraints are violated."""
    
    def __init__(self, key: str, reason: str):
        self.key = key
        self.reason = reason
        message = f"Security violation for key '{key}': {reason}"
        super().__init__(message, {"key": key, "reason": reason})


class ConfigTimeoutError(ConfigError):
    """Raised when configuration operations timeout."""
    
    def __init__(self, operation: str, timeout: float):
        self.operation = operation
        self.timeout = timeout
        message = f"Operation '{operation}' timed out after {timeout}s"
        super().__init__(message, {"operation": operation, "timeout": timeout})
        
    def is_retriable(self) -> bool:
        return True


class ConfigConnectionError(ConfigError):
    """Raised when connection to config store fails."""
    
    def __init__(self, reason: str, endpoint: Optional[str] = None):
        self.reason = reason
        self.endpoint = endpoint
        message = f"Connection error: {reason}"
        if endpoint:
            message += f" (endpoint: {endpoint})"
        super().__init__(message, {"reason": reason, "endpoint": endpoint})
        
    def is_retriable(self) -> bool:
        return True