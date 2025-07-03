"""Retry logic and error handling utilities."""
import asyncio
from typing import TypeVar, Callable, Optional, Type, Tuple
from functools import wraps
import backoff
from tenacity import (
    retry,
    stop_after_attempt,
    wait_exponential,
    retry_if_exception_type,
    before_log,
    after_log
)
from ..utils.logging import get_logger

logger = get_logger(__name__)

T = TypeVar('T')


def with_retry(
    max_attempts: int = 3,
    max_delay: int = 60,
    exceptions: Tuple[Type[Exception], ...] = (Exception,),
    backoff_factor: float = 2.0
):
    """
    Decorator for retrying async functions with exponential backoff.
    
    Args:
        max_attempts: Maximum number of retry attempts
        max_delay: Maximum delay between retries in seconds
        exceptions: Tuple of exceptions to retry on
        backoff_factor: Exponential backoff factor
    """
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        @retry(
            stop=stop_after_attempt(max_attempts),
            wait=wait_exponential(multiplier=1, max=max_delay),
            retry=retry_if_exception_type(exceptions),
            before=before_log(logger, "DEBUG"),
            after=after_log(logger, "DEBUG"),
            reraise=True
        )
        async def wrapper(*args, **kwargs) -> T:
            return await func(*args, **kwargs)
        
        return wrapper
    return decorator


def with_circuit_breaker(
    failure_threshold: int = 5,
    recovery_timeout: int = 60,
    expected_exception: Type[Exception] = Exception
):
    """
    Circuit breaker pattern implementation.
    
    Args:
        failure_threshold: Number of failures before opening circuit
        recovery_timeout: Time in seconds before attempting to close circuit
        expected_exception: Exception type to count as failure
    """
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        state = {
            "failure_count": 0,
            "last_failure_time": None,
            "is_open": False
        }
        
        @wraps(func)
        async def wrapper(*args, **kwargs) -> T:
            import time
            
            # Check if circuit is open
            if state["is_open"]:
                if state["last_failure_time"] and \
                   time.time() - state["last_failure_time"] < recovery_timeout:
                    raise Exception(f"Circuit breaker is open for {func.__name__}")
                else:
                    # Try to close circuit
                    state["is_open"] = False
                    state["failure_count"] = 0
            
            try:
                result = await func(*args, **kwargs)
                # Reset failure count on success
                state["failure_count"] = 0
                return result
            
            except expected_exception as e:
                state["failure_count"] += 1
                state["last_failure_time"] = time.time()
                
                if state["failure_count"] >= failure_threshold:
                    state["is_open"] = True
                    logger.error(
                        "Circuit breaker opened",
                        function=func.__name__,
                        failure_count=state["failure_count"]
                    )
                
                raise
        
        return wrapper
    return decorator


class RateLimiter:
    """Token bucket rate limiter for API calls."""
    
    def __init__(self, rate: int, per: float):
        """
        Initialize rate limiter.
        
        Args:
            rate: Number of requests allowed
            per: Time period in seconds
        """
        self.rate = rate
        self.per = per
        self.allowance = rate
        self.last_check = asyncio.get_event_loop().time()
    
    async def acquire(self):
        """Acquire permission to make a request."""
        current = asyncio.get_event_loop().time()
        time_passed = current - self.last_check
        self.last_check = current
        self.allowance += time_passed * (self.rate / self.per)
        
        if self.allowance > self.rate:
            self.allowance = self.rate
        
        if self.allowance < 1.0:
            sleep_time = (1.0 - self.allowance) * (self.per / self.rate)
            await asyncio.sleep(sleep_time)
            self.allowance = 0.0
        else:
            self.allowance -= 1.0


def rate_limited(rate: int, per: float = 60.0):
    """
    Decorator for rate limiting async functions.
    
    Args:
        rate: Number of calls allowed
        per: Time period in seconds (default: 60)
    """
    limiter = RateLimiter(rate, per)
    
    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        async def wrapper(*args, **kwargs) -> T:
            await limiter.acquire()
            return await func(*args, **kwargs)
        
        return wrapper
    return decorator