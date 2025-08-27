"""Application settings and configuration."""
# Import from secure_settings for backward compatibility
from .secure_settings import (
    Settings as SecureSettingsAlias, 
    SecureSettings, 
    RateLimitConfig, 
    get_settings as get_secure_settings
)

# Import hybrid settings for enhanced functionality
try:
    from .hybrid_settings import HybridSettings, get_settings as get_hybrid_settings
    HYBRID_AVAILABLE = True
except ImportError:
    HYBRID_AVAILABLE = False
    HybridSettings = None
    get_hybrid_settings = None

# Maintain backward compatibility - Settings points to SecureSettings by default
# but can be switched to HybridSettings if available and desired
Settings = SecureSettingsAlias

def get_settings():
    """
    Get settings instance with hybrid support if available.
    
    This function maintains backward compatibility while providing
    enhanced functionality when config-store is available.
    """
    if HYBRID_AVAILABLE and get_hybrid_settings:
        # Use hybrid settings for enhanced functionality
        return get_hybrid_settings()
    else:
        # Fall back to secure settings
        return get_secure_settings()

# Re-export everything for backward compatibility
__all__ = [
    'Settings', 
    'SecureSettings', 
    'RateLimitConfig', 
    'get_settings'
]

# Add hybrid settings to exports if available
if HYBRID_AVAILABLE:
    __all__.extend(['HybridSettings'])