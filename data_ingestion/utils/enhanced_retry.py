"""
Enhanced Retry Logic for Phase 2 INTERFACE_CONTRACT Compliance
"""
import asyncio
import time
import logging
from typing import Any, Callable, Type, Tuple, Optional
from functools import wraps

logger = logging.getLogger(__name__)


class RetryConfig:
    """Retry configuration per INTERFACE_CONTRACT specification."""
    
    def __init__(
        self,
        max_attempts: int = 3,
        base_delay_ms: int = 100,
        max_delay_ms: int = 5000,
        backoff_multiplier: float = 2.0,
        jitter: bool = True
    ):
        self.max_attempts = max_attempts
        self.base_delay_ms = base_delay_ms
        self.max_delay_ms = max_delay_ms
        self.backoff_multiplier = backoff_multiplier
        self.jitter = jitter
    
    def calculate_delay(self, attempt: int) -> float:
        """Calculate delay for given attempt with exponential backoff."""
        delay_ms = min(
            self.base_delay_ms * (self.backoff_multiplier ** attempt),
            self.max_delay_ms
        )
        
        if self.jitter:
            import random
            # Add ±25% jitter to prevent thundering herd
            jitter_range = delay_ms * 0.25
            delay_ms += random.uniform(-jitter_range, jitter_range)
        
        return max(delay_ms / 1000, 0)  # Convert to seconds


class RetryableError(Exception):
    """Base class for retryable errors."""
    pass


class NonRetryableError(Exception):
    """Base class for non-retryable errors."""
    pass


def async_retry(
    config: Optional[RetryConfig] = None,
    retryable_exceptions: Tuple[Type[Exception], ...] = (Exception,),
    non_retryable_exceptions: Tuple[Type[Exception], ...] = ()
):
    """
    Async retry decorator with exponential backoff.
    
    Args:
        config: Retry configuration
        retryable_exceptions: Exceptions that should trigger retry
        non_retryable_exceptions: Exceptions that should not be retried
    """
    if config is None:
        config = RetryConfig()
    
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        async def wrapper(*args, **kwargs) -> Any:
            last_exception = None
            
            for attempt in range(config.max_attempts):
                try:
                    result = await func(*args, **kwargs)
                    
                    # Log successful retry if not first attempt
                    if attempt > 0:
                        logger.info(
                            f"Function {func.__name__} succeeded on attempt {attempt + 1}"
                        )
                    
                    return result
                    
                except non_retryable_exceptions as e:
                    logger.error(
                        f"Non-retryable error in {func.__name__}: {e}"
                    )
                    raise
                    
                except retryable_exceptions as e:
                    last_exception = e
                    
                    if attempt == config.max_attempts - 1:
                        # Last attempt failed
                        logger.error(
                            f"Function {func.__name__} failed after {config.max_attempts} attempts: {e}"
                        )
                        break
                    
                    # Calculate delay and wait
                    delay = config.calculate_delay(attempt)
                    logger.warning(
                        f"Function {func.__name__} failed on attempt {attempt + 1}, "
                        f"retrying in {delay:.3f}s: {e}"
                    )
                    await asyncio.sleep(delay)
            
            # All attempts failed
            raise last_exception
            
        return wrapper
    return decorator


class CircuitBreakerRetryIntegration:
    """Integrates circuit breaker with retry logic."""
    
    def __init__(self, circuit_breaker, retry_config: RetryConfig):
        self.circuit_breaker = circuit_breaker
        self.retry_config = retry_config
    
    async def execute_with_retry_and_circuit_breaker(
        self,
        func: Callable,
        channel: str,
        *args,
        **kwargs
    ) -> Any:
        """
        Execute function with both retry logic and circuit breaker protection.
        
        Args:
            func: Async function to execute
            channel: Channel name for circuit breaker
            *args: Function arguments
            **kwargs: Function keyword arguments
            
        Returns:
            Function result
            
        Raises:
            Exception: If all retries fail or circuit is open
        """
        # Check circuit breaker first
        if not self.circuit_breaker.allow_request(channel):
            raise RetryableError(f"Circuit breaker open for channel: {channel}")
        
        last_exception = None
        
        for attempt in range(self.retry_config.max_attempts):
            try:
                result = await func(*args, **kwargs)
                
                # Record success in circuit breaker
                self.circuit_breaker.record_success(channel)
                
                if attempt > 0:
                    logger.info(
                        f"Function {func.__name__} succeeded on attempt {attempt + 1} for channel {channel}"
                    )
                
                return result
                
            except Exception as e:
                last_exception = e
                
                # Record failure in circuit breaker
                self.circuit_breaker.record_failure(channel)
                
                # Check if this is a retryable error
                if isinstance(e, NonRetryableError):
                    logger.error(f"Non-retryable error for {channel}: {e}")
                    raise
                
                if attempt == self.retry_config.max_attempts - 1:
                    logger.error(
                        f"Function {func.__name__} failed after {self.retry_config.max_attempts} attempts for channel {channel}: {e}"
                    )
                    break
                
                # Check circuit breaker before retrying
                if not self.circuit_breaker.allow_request(channel):
                    logger.warning(f"Circuit breaker opened for channel {channel}, stopping retries")
                    break
                
                # Calculate delay and wait
                delay = self.retry_config.calculate_delay(attempt)
                logger.warning(
                    f"Function {func.__name__} failed on attempt {attempt + 1} for channel {channel}, "
                    f"retrying in {delay:.3f}s: {e}"
                )
                await asyncio.sleep(delay)
        
        # All attempts failed or circuit opened
        raise last_exception


# Redis-specific error classifications
class RedisConnectionError(RetryableError):
    """Redis connection errors that should be retried."""
    pass


class RedisTimeoutError(RetryableError):
    """Redis timeout errors that should be retried."""
    pass


class RedisAuthError(NonRetryableError):
    """Redis authentication errors that should not be retried."""
    pass


class RedisInvalidChannelError(NonRetryableError):
    """Redis invalid channel errors that should not be retried."""
    pass


def create_redis_retry_config() -> RetryConfig:
    """Create retry configuration optimized for Redis operations."""
    return RetryConfig(
        max_attempts=3,
        base_delay_ms=100,
        max_delay_ms=5000,
        backoff_multiplier=2.0,
        jitter=True
    )


def redis_retry(func: Callable) -> Callable:
    """Convenience decorator for Redis operations with appropriate retry config."""
    config = create_redis_retry_config()
    
    return async_retry(
        config=config,
        retryable_exceptions=(RedisConnectionError, RedisTimeoutError, ConnectionError, TimeoutError),
        non_retryable_exceptions=(RedisAuthError, RedisInvalidChannelError, ValueError, TypeError)
    )(func)