# Re-export rate limiter components
import sys
import os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'src'))

from utils.rate_limiter import RateLimiter, APIRateLimiters, rate_limited, BulkRequestHandler

# Import configurable rate limiter separately to avoid circular import
def get_configurable_rate_limiter():
    from .configurable_rate_limiter import ConfigurableRateLimiter
    return ConfigurableRateLimiter

# For convenience, make ConfigurableRateLimiter available at module level
ConfigurableRateLimiter = None

def __getattr__(name):
    global ConfigurableRateLimiter
    if name == 'ConfigurableRateLimiter':
        if ConfigurableRateLimiter is None:
            from .configurable_rate_limiter import ConfigurableRateLimiter as CRL
            ConfigurableRateLimiter = CRL
        return ConfigurableRateLimiter
    raise AttributeError(f"module {__name__} has no attribute {name}")

__all__ = [
    'RateLimiter', 
    'APIRateLimiters', 
    'rate_limited', 
    'BulkRequestHandler',
    'ConfigurableRateLimiter'
]