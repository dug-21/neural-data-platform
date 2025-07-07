"""Configurable rate limiter that integrates with settings"""
from typing import Optional, Dict
import asyncio
from datetime import datetime, timedelta
from collections import deque

from data_ingestion.config.settings import Settings, RateLimitConfig
import sys
import os
# Add src to path to import RateLimiter
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', '..', 'src'))
from utils.rate_limiter import RateLimiter


class ConfigurableRateLimiter(RateLimiter):
    """Rate limiter that can be configured from settings"""
    
    def __init__(
        self, 
        name: str,
        calls_per_minute: Optional[int] = None,
        calls_per_day: Optional[int] = None,
        burst_size: Optional[int] = None
    ):
        super().__init__(
            calls_per_minute=calls_per_minute,
            calls_per_day=calls_per_day,
            name=name
        )
        self.burst_size = burst_size
        self._burst_tokens = burst_size if burst_size else float('inf')
        self._last_refill = datetime.now()
        
    def can_make_request(self) -> tuple[bool, Optional[float]]:
        """Check if request can be made with burst support"""
        # First check burst limit
        if self.burst_size:
            self._refill_burst_tokens()
            if self._burst_tokens < 1:
                # Calculate wait time for next token
                time_since_refill = (datetime.now() - self._last_refill).total_seconds()
                time_per_token = 60.0 / self.calls_per_minute if self.calls_per_minute else 1.0
                wait_time = time_per_token - time_since_refill
                return False, max(0, wait_time)
        
        # Then check regular rate limits
        return super().can_make_request()
    
    def record_request(self):
        """Record request and consume burst token"""
        super().record_request()
        if self.burst_size:
            self._burst_tokens -= 1
    
    def _refill_burst_tokens(self):
        """Refill burst tokens based on time elapsed"""
        if not self.calls_per_minute:
            return
            
        now = datetime.now()
        time_elapsed = (now - self._last_refill).total_seconds()
        
        # Refill tokens based on rate
        tokens_to_add = (time_elapsed / 60.0) * self.calls_per_minute
        self._burst_tokens = min(self.burst_size, self._burst_tokens + tokens_to_add)
        
        if tokens_to_add > 0:
            self._last_refill = now
    
    @classmethod
    def from_config(cls, name: str, config: RateLimitConfig) -> 'ConfigurableRateLimiter':
        """Create rate limiter from configuration"""
        return cls(
            name=name,
            calls_per_minute=config.calls_per_minute,
            calls_per_day=config.calls_per_day,
            burst_size=config.burst_size
        )
    
    @classmethod
    def from_settings(cls, api_name: str, settings: Settings) -> 'ConfigurableRateLimiter':
        """Create rate limiter from settings for specific API"""
        # Get rate limit config for API
        rate_config = settings.rate_limits.get(api_name)
        
        if rate_config:
            return cls.from_config(api_name, rate_config)
        
        # Default rate limiter if not configured
        return cls(
            name=api_name,
            calls_per_minute=settings.max_requests_per_minute,
            calls_per_day=None,
            burst_size=None
        )
    
    @classmethod
    def get_all_from_settings(cls, settings: Settings) -> Dict[str, 'ConfigurableRateLimiter']:
        """Get all configured rate limiters from settings"""
        limiters = {}
        
        for api_name, config in settings.rate_limits.items():
            limiters[api_name] = cls.from_config(api_name, config)
        
        return limiters