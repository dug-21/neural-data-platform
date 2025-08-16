"""Channel validation utilities for Redis publishing."""
import re
from typing import Optional
from utils.logging import get_logger

logger = get_logger(__name__)


class ChannelValidator:
    """Validates Redis channel naming per INTERFACE_CONTRACT requirements."""
    
    # Channel pattern: market:{SYMBOL} where SYMBOL is 1-5 uppercase letters
    CHANNEL_PATTERN = re.compile(r"^market:[A-Z]{1,5}$")
    
    @staticmethod
    def validate_channel_name(channel: str) -> bool:
        """
        Validate channel name against INTERFACE_CONTRACT requirements.
        
        Args:
            channel: Channel name to validate
            
        Returns:
            bool: True if valid, False otherwise
        """
        return bool(ChannelValidator.CHANNEL_PATTERN.match(channel))
    
    @staticmethod
    def validate_symbol(symbol: str) -> Optional[str]:
        """
        Validate and normalize symbol for channel creation.
        
        Args:
            symbol: Raw symbol string
            
        Returns:
            Optional[str]: Normalized symbol if valid, None otherwise
        """
        if not symbol:
            return None
            
        # Normalize to uppercase and strip whitespace
        normalized = symbol.upper().strip()
        
        # Validate format: 1-5 uppercase letters only
        symbol_pattern = re.compile(r"^[A-Z]{1,5}$")
        if not symbol_pattern.match(normalized):
            logger.warning(f"Invalid symbol format: {symbol} -> {normalized}")
            return None
            
        return normalized
    
    @staticmethod
    def create_symbol_channel(symbol: str) -> Optional[str]:
        """
        Create a validated market channel for a symbol.
        
        Args:
            symbol: Symbol to create channel for
            
        Returns:
            Optional[str]: Valid channel name or None if invalid
        """
        validated_symbol = ChannelValidator.validate_symbol(symbol)
        if not validated_symbol:
            return None
            
        channel = f"market:{validated_symbol}"
        
        # Double-check validation
        if not ChannelValidator.validate_channel_name(channel):
            logger.error(f"Generated invalid channel: {channel}")
            return None
            
        return channel


class CircuitBreaker:
    """Circuit breaker for Redis publishing operations."""
    
    def __init__(self, failure_threshold: int = 5, recovery_timeout: int = 30):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.failure_count = 0
        self.last_failure_time = None
        self.state = "CLOSED"  # CLOSED, OPEN, HALF_OPEN
        
    def allow_request(self, channel: str) -> bool:
        """Check if request should be allowed through circuit breaker."""
        if self.state == "CLOSED":
            return True
        elif self.state == "OPEN":
            if self._should_attempt_reset():
                self.state = "HALF_OPEN"
                return True
            return False
        elif self.state == "HALF_OPEN":
            return True
        return False
    
    def record_success(self, channel: str):
        """Record successful operation."""
        if self.state == "HALF_OPEN":
            self.state = "CLOSED"
        self.failure_count = 0
        self.last_failure_time = None
    
    def record_failure(self, channel: str):
        """Record failed operation."""
        import time
        self.failure_count += 1
        self.last_failure_time = time.time()
        
        if self.failure_count >= self.failure_threshold:
            self.state = "OPEN"
            logger.warning(f"Circuit breaker opened for channel {channel}")
    
    def _should_attempt_reset(self) -> bool:
        """Check if enough time has passed to attempt reset."""
        if self.last_failure_time is None:
            return True
        
        import time
        return (time.time() - self.last_failure_time) >= self.recovery_timeout


class CircuitBreakerOpenError(Exception):
    """Exception raised when circuit breaker is open."""
    pass