"""Python Config Store Client

This module provides a Python client for the config-store system, implementing 
advanced features like hybrid configuration loading, fallback mechanisms, 
security filtering, and deep integration with the Rust config-store service.
"""

from .client import ConfigStoreClient
from .errors import (
    ConfigError,
    ConfigNotFoundError, 
    ConfigValidationError,
    ConfigSecurityError,
    ConfigTimeoutError,
    ConfigConnectionError
)

__version__ = "0.1.0"
__all__ = [
    "ConfigStoreClient",
    "ConfigError", 
    "ConfigNotFoundError",
    "ConfigValidationError", 
    "ConfigSecurityError",
    "ConfigTimeoutError",
    "ConfigConnectionError"
]