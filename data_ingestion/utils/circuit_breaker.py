"""Circuit breaker implementation for WebSocket resilience."""
import asyncio
import time
from enum import Enum
from typing import Optional, Callable
from dataclasses import dataclass, field
from datetime import datetime, timedelta
import logging

logger = logging.getLogger(__name__)


class CircuitState(Enum):
    """Circuit breaker states."""
    CLOSED = "closed"  # Normal operation
    OPEN = "open"      # Failures exceeded, blocking requests
    HALF_OPEN = "half_open"  # Testing if service recovered


@dataclass
class CircuitBreakerConfig:
    """Configuration for circuit breaker."""
    failure_threshold: int = 5
    success_threshold: int = 2
    timeout: float = 60.0  # seconds
    half_open_requests: int = 3
    
    # Callbacks
    on_open: Optional[Callable] = None
    on_close: Optional[Callable] = None
    on_half_open: Optional[Callable] = None


@dataclass
class CircuitBreakerStats:
    """Statistics tracking for circuit breaker."""
    failure_count: int = 0
    success_count: int = 0
    last_failure_time: Optional[float] = None
    last_success_time: Optional[float] = None
    consecutive_failures: int = 0
    consecutive_successes: int = 0
    total_requests: int = 0
    total_failures: int = 0
    total_successes: int = 0
    
    def reset(self):
        """Reset transient counters."""
        self.failure_count = 0
        self.success_count = 0
        self.consecutive_failures = 0
        self.consecutive_successes = 0


class CircuitBreaker:
    """
    Circuit breaker pattern implementation for WebSocket connections.
    
    States:
    - CLOSED: Normal operation, requests pass through
    - OPEN: Too many failures, requests are blocked
    - HALF_OPEN: Testing recovery, limited requests allowed
    """
    
    def __init__(self, config: Optional[CircuitBreakerConfig] = None):
        self.config = config or CircuitBreakerConfig()
        self.state = CircuitState.CLOSED
        self.stats = CircuitBreakerStats()
        self._state_changed_at = time.time()
        self._half_open_requests = 0
        self._lock = asyncio.Lock()
        
    @property
    def is_open(self) -> bool:
        """Check if circuit is open."""
        return self.state == CircuitState.OPEN
        
    @property
    def is_closed(self) -> bool:
        """Check if circuit is closed."""
        return self.state == CircuitState.CLOSED
        
    @property
    def is_half_open(self) -> bool:
        """Check if circuit is half open."""
        return self.state == CircuitState.HALF_OPEN
        
    def should_allow_request(self) -> bool:
        """
        Determine if a request should be allowed.
        
        Returns:
            bool: True if request should proceed, False otherwise
        """
        if self.state == CircuitState.CLOSED:
            return True
            
        if self.state == CircuitState.OPEN:
            # Check if timeout has passed
            if self._should_attempt_reset():
                self._transition_to_half_open()
                return True
            return False
            
        if self.state == CircuitState.HALF_OPEN:
            # Allow limited requests in half-open state
            return self._half_open_requests < self.config.half_open_requests
            
        return False
        
    def _should_attempt_reset(self) -> bool:
        """Check if enough time has passed to attempt reset."""
        return time.time() - self._state_changed_at >= self.config.timeout
        
    async def record_success(self):
        """Record a successful request."""
        async with self._lock:
            self.stats.success_count += 1
            self.stats.consecutive_successes += 1
            self.stats.consecutive_failures = 0
            self.stats.last_success_time = time.time()
            self.stats.total_successes += 1
            self.stats.total_requests += 1
            
            if self.state == CircuitState.HALF_OPEN:
                if self.stats.consecutive_successes >= self.config.success_threshold:
                    self._transition_to_closed()
                    
            logger.debug(f"Circuit breaker recorded success. State: {self.state}, "
                        f"Consecutive successes: {self.stats.consecutive_successes}")
                    
    async def record_failure(self, error: Optional[Exception] = None):
        """Record a failed request."""
        async with self._lock:
            self.stats.failure_count += 1
            self.stats.consecutive_failures += 1
            self.stats.consecutive_successes = 0
            self.stats.last_failure_time = time.time()
            self.stats.total_failures += 1
            self.stats.total_requests += 1
            
            if error:
                logger.warning(f"Circuit breaker recorded failure: {error}")
            
            if self.state == CircuitState.CLOSED:
                if self.stats.consecutive_failures >= self.config.failure_threshold:
                    self._transition_to_open()
                    
            elif self.state == CircuitState.HALF_OPEN:
                # Any failure in half-open state reopens the circuit
                self._transition_to_open()
                
            logger.debug(f"Circuit breaker recorded failure. State: {self.state}, "
                        f"Consecutive failures: {self.stats.consecutive_failures}")
                    
    def _transition_to_open(self):
        """Transition to OPEN state."""
        self.state = CircuitState.OPEN
        self._state_changed_at = time.time()
        self._half_open_requests = 0
        
        logger.warning(f"Circuit breaker opened after {self.stats.consecutive_failures} failures")
        
        if self.config.on_open:
            try:
                self.config.on_open()
            except Exception as e:
                logger.error(f"Error in on_open callback: {e}")
                
    def _transition_to_closed(self):
        """Transition to CLOSED state."""
        self.state = CircuitState.CLOSED
        self._state_changed_at = time.time()
        self._half_open_requests = 0
        self.stats.reset()
        
        logger.info("Circuit breaker closed - service recovered")
        
        if self.config.on_close:
            try:
                self.config.on_close()
            except Exception as e:
                logger.error(f"Error in on_close callback: {e}")
                
    def _transition_to_half_open(self):
        """Transition to HALF_OPEN state."""
        self.state = CircuitState.HALF_OPEN
        self._state_changed_at = time.time()
        self._half_open_requests = 0
        self.stats.reset()
        
        logger.info("Circuit breaker half-open - testing service recovery")
        
        if self.config.on_half_open:
            try:
                self.config.on_half_open()
            except Exception as e:
                logger.error(f"Error in on_half_open callback: {e}")
                
    def increment_half_open_requests(self):
        """Increment the count of requests made in half-open state."""
        if self.state == CircuitState.HALF_OPEN:
            self._half_open_requests += 1
            
    def get_stats(self) -> dict:
        """Get circuit breaker statistics."""
        return {
            "state": self.state.value,
            "failure_count": self.stats.failure_count,
            "success_count": self.stats.success_count,
            "consecutive_failures": self.stats.consecutive_failures,
            "consecutive_successes": self.stats.consecutive_successes,
            "total_requests": self.stats.total_requests,
            "total_failures": self.stats.total_failures,
            "total_successes": self.stats.total_successes,
            "success_rate": (
                self.stats.total_successes / self.stats.total_requests
                if self.stats.total_requests > 0 else 0.0
            ),
            "last_failure_time": (
                datetime.fromtimestamp(self.stats.last_failure_time).isoformat()
                if self.stats.last_failure_time else None
            ),
            "last_success_time": (
                datetime.fromtimestamp(self.stats.last_success_time).isoformat()
                if self.stats.last_success_time else None
            ),
            "time_in_current_state": time.time() - self._state_changed_at
        }
        
    def reset(self):
        """Reset the circuit breaker to closed state."""
        self.state = CircuitState.CLOSED
        self._state_changed_at = time.time()
        self._half_open_requests = 0
        self.stats = CircuitBreakerStats()
        logger.info("Circuit breaker manually reset")


class AsyncCircuitBreaker(CircuitBreaker):
    """
    Async decorator version of circuit breaker for wrapping async functions.
    """
    
    def __call__(self, func: Callable):
        """Decorator for async functions."""
        async def wrapper(*args, **kwargs):
            if not self.should_allow_request():
                raise Exception(f"Circuit breaker is {self.state.value}")
                
            if self.is_half_open:
                self.increment_half_open_requests()
                
            try:
                result = await func(*args, **kwargs)
                await self.record_success()
                return result
            except Exception as e:
                await self.record_failure(e)
                raise
                
        return wrapper