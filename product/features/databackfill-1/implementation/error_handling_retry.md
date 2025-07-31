# Error Handling and Retry Mechanisms

## Overview
Comprehensive error handling and intelligent retry system for robust backfill operations.

## Error Categorization

### 1. Error Type Classification
```python
from enum import Enum
from typing import Optional, Dict, Any, List
from dataclasses import dataclass
from datetime import datetime

class ErrorSeverity(Enum):
    LOW = "low"          # Can be ignored or logged
    MEDIUM = "medium"    # Should be retried
    HIGH = "high"        # Requires immediate attention
    CRITICAL = "critical" # Stop processing

class ErrorCategory(Enum):
    NETWORK = "network"          # Connection, timeout issues
    AUTHENTICATION = "auth"      # API key, permissions
    RATE_LIMIT = "rate_limit"   # Too many requests
    DATA_QUALITY = "data_quality" # Invalid/corrupt data
    STORAGE = "storage"          # Disk space, I/O errors
    DATABASE = "database"        # Connection, query failures
    PARSING = "parsing"          # File format issues
    SYSTEM = "system"           # Memory, CPU issues
    UNKNOWN = "unknown"         # Uncategorized errors

@dataclass
class BackfillError:
    timestamp: datetime
    category: ErrorCategory
    severity: ErrorSeverity
    error_type: str
    error_message: str
    context: Dict[str, Any]
    stacktrace: Optional[str] = None
    retry_count: int = 0
    max_retries: int = 3
    
    @property
    def is_retryable(self) -> bool:
        """Determine if error should be retried"""
        non_retryable = [
            ErrorCategory.AUTHENTICATION,
            ErrorCategory.DATA_QUALITY,
            ErrorCategory.PARSING
        ]
        return (
            self.category not in non_retryable and 
            self.retry_count < self.max_retries
        )
        
    @property
    def retry_delay(self) -> float:
        """Calculate retry delay with exponential backoff"""
        if self.category == ErrorCategory.RATE_LIMIT:
            return 60.0  # 1 minute for rate limits
        return min(2 ** self.retry_count, 300)  # Max 5 minutes
```

### 2. Error Handler Manager
```python
import asyncio
import logging
from typing import Callable, Optional, Type, Dict, Any, List
from functools import wraps
import traceback

class ErrorHandler:
    """Central error handling system"""
    
    def __init__(self, alert_callback: Optional[Callable] = None):
        self.alert_callback = alert_callback
        self.error_history: List[BackfillError] = []
        self.error_handlers: Dict[ErrorCategory, Callable] = {
            ErrorCategory.NETWORK: self._handle_network_error,
            ErrorCategory.AUTHENTICATION: self._handle_auth_error,
            ErrorCategory.RATE_LIMIT: self._handle_rate_limit,
            ErrorCategory.DATA_QUALITY: self._handle_data_quality,
            ErrorCategory.STORAGE: self._handle_storage_error,
            ErrorCategory.DATABASE: self._handle_database_error,
            ErrorCategory.PARSING: self._handle_parsing_error,
            ErrorCategory.SYSTEM: self._handle_system_error
        }
        self.circuit_breakers: Dict[str, CircuitBreaker] = {}
        
    def with_error_handling(
        self,
        category: ErrorCategory = ErrorCategory.UNKNOWN,
        severity: ErrorSeverity = ErrorSeverity.MEDIUM
    ):
        """Decorator for automatic error handling"""
        def decorator(func: Callable) -> Callable:
            @wraps(func)
            async def wrapper(*args, **kwargs):
                try:
                    # Check circuit breaker
                    breaker_key = f"{category.value}_{func.__name__}"
                    if breaker_key in self.circuit_breakers:
                        breaker = self.circuit_breakers[breaker_key]
                        if not breaker.is_closed():
                            raise CircuitOpenError(f"Circuit breaker open for {breaker_key}")
                            
                    return await func(*args, **kwargs)
                    
                except Exception as e:
                    error = self._create_error(e, category, severity, {
                        'function': func.__name__,
                        'args': str(args)[:200],
                        'kwargs': str(kwargs)[:200]
                    })
                    
                    # Record error
                    self.error_history.append(error)
                    
                    # Handle error
                    handler = self.error_handlers.get(category, self._handle_unknown_error)
                    should_retry = await handler(error)
                    
                    # Update circuit breaker
                    if breaker_key in self.circuit_breakers:
                        self.circuit_breakers[breaker_key].record_failure()
                        
                    if should_retry and error.is_retryable:
                        error.retry_count += 1
                        await asyncio.sleep(error.retry_delay)
                        return await wrapper(*args, **kwargs)
                    else:
                        raise
                        
            return wrapper
        return decorator
        
    def _create_error(
        self,
        exception: Exception,
        category: ErrorCategory,
        severity: ErrorSeverity,
        context: Dict[str, Any]
    ) -> BackfillError:
        """Create BackfillError from exception"""
        return BackfillError(
            timestamp=datetime.utcnow(),
            category=category,
            severity=severity,
            error_type=type(exception).__name__,
            error_message=str(exception),
            context=context,
            stacktrace=traceback.format_exc()
        )
        
    async def _handle_network_error(self, error: BackfillError) -> bool:
        """Handle network-related errors"""
        logging.warning(f"Network error: {error.error_message}")
        
        # Check if it's a temporary network issue
        if "timeout" in error.error_message.lower():
            return True  # Retry
            
        if "connection" in error.error_message.lower():
            # Wait a bit for network to recover
            await asyncio.sleep(5)
            return True  # Retry
            
        return False
        
    async def _handle_rate_limit(self, error: BackfillError) -> bool:
        """Handle rate limit errors"""
        logging.warning(f"Rate limit hit: {error.error_message}")
        
        # Extract retry-after if available
        retry_after = error.context.get('retry_after', 60)
        
        logging.info(f"Waiting {retry_after} seconds for rate limit")
        await asyncio.sleep(retry_after)
        
        return True  # Always retry rate limits
        
    async def _handle_database_error(self, error: BackfillError) -> bool:
        """Handle database errors"""
        logging.error(f"Database error: {error.error_message}")
        
        # Connection errors are retryable
        if "connection" in error.error_message.lower():
            await asyncio.sleep(10)
            return True
            
        # Deadlocks are retryable
        if "deadlock" in error.error_message.lower():
            await asyncio.sleep(1)
            return True
            
        # Constraint violations are not retryable
        if "constraint" in error.error_message.lower():
            return False
            
        return False
```

### 3. Circuit Breaker Pattern
```python
from datetime import datetime, timedelta
from typing import Optional

class CircuitBreakerState(Enum):
    CLOSED = "closed"      # Normal operation
    OPEN = "open"         # Failing, reject calls
    HALF_OPEN = "half_open" # Testing if recovered

class CircuitBreaker:
    """Circuit breaker to prevent cascading failures"""
    
    def __init__(
        self,
        failure_threshold: int = 5,
        recovery_timeout: int = 60,  # seconds
        expected_exception: Optional[Type[Exception]] = None
    ):
        self.failure_threshold = failure_threshold
        self.recovery_timeout = recovery_timeout
        self.expected_exception = expected_exception
        self.failure_count = 0
        self.last_failure_time: Optional[datetime] = None
        self.state = CircuitBreakerState.CLOSED
        
    def is_closed(self) -> bool:
        """Check if circuit is closed (operational)"""
        if self.state == CircuitBreakerState.CLOSED:
            return True
            
        if self.state == CircuitBreakerState.OPEN:
            # Check if we should try half-open
            if self.last_failure_time:
                if datetime.utcnow() - self.last_failure_time > timedelta(seconds=self.recovery_timeout):
                    self.state = CircuitBreakerState.HALF_OPEN
                    return True
                    
        return self.state == CircuitBreakerState.HALF_OPEN
        
    def record_success(self):
        """Record successful operation"""
        self.failure_count = 0
        self.state = CircuitBreakerState.CLOSED
        
    def record_failure(self):
        """Record failed operation"""
        self.failure_count += 1
        self.last_failure_time = datetime.utcnow()
        
        if self.failure_count >= self.failure_threshold:
            self.state = CircuitBreakerState.OPEN
            logging.warning(f"Circuit breaker opened after {self.failure_count} failures")
            
class CircuitOpenError(Exception):
    """Raised when circuit breaker is open"""
    pass
```

### 4. Retry Strategy Manager
```python
from abc import ABC, abstractmethod
import random

class RetryStrategy(ABC):
    """Base class for retry strategies"""
    
    @abstractmethod
    def get_delay(self, attempt: int) -> float:
        """Get delay for given attempt number"""
        pass
        
class ExponentialBackoff(RetryStrategy):
    """Exponential backoff with jitter"""
    
    def __init__(self, base_delay: float = 1.0, max_delay: float = 300.0):
        self.base_delay = base_delay
        self.max_delay = max_delay
        
    def get_delay(self, attempt: int) -> float:
        delay = min(self.base_delay * (2 ** attempt), self.max_delay)
        # Add jitter to prevent thundering herd
        jitter = delay * 0.1 * random.random()
        return delay + jitter
        
class LinearBackoff(RetryStrategy):
    """Linear backoff strategy"""
    
    def __init__(self, delay_increment: float = 5.0, max_delay: float = 60.0):
        self.delay_increment = delay_increment
        self.max_delay = max_delay
        
    def get_delay(self, attempt: int) -> float:
        return min(attempt * self.delay_increment, self.max_delay)
        
class RetryManager:
    """Manage retry operations with different strategies"""
    
    def __init__(self, default_strategy: Optional[RetryStrategy] = None):
        self.default_strategy = default_strategy or ExponentialBackoff()
        self.strategies: Dict[ErrorCategory, RetryStrategy] = {
            ErrorCategory.NETWORK: ExponentialBackoff(base_delay=2.0),
            ErrorCategory.RATE_LIMIT: LinearBackoff(delay_increment=60.0),
            ErrorCategory.DATABASE: ExponentialBackoff(base_delay=0.5, max_delay=30.0)
        }
        
    async def retry_with_backoff(
        self,
        func: Callable,
        max_attempts: int = 3,
        error_category: ErrorCategory = ErrorCategory.UNKNOWN,
        on_retry: Optional[Callable] = None
    ):
        """Execute function with retry logic"""
        strategy = self.strategies.get(error_category, self.default_strategy)
        last_exception = None
        
        for attempt in range(max_attempts):
            try:
                return await func()
            except Exception as e:
                last_exception = e
                
                if attempt < max_attempts - 1:
                    delay = strategy.get_delay(attempt)
                    
                    if on_retry:
                        await on_retry(attempt + 1, delay, e)
                        
                    logging.info(f"Retry attempt {attempt + 1}/{max_attempts} after {delay:.1f}s delay")
                    await asyncio.sleep(delay)
                else:
                    logging.error(f"All {max_attempts} retry attempts failed")
                    
        raise last_exception
```

### 5. Error Recovery Actions
```python
class RecoveryAction:
    """Automated recovery actions for specific error types"""
    
    def __init__(self, error_handler: ErrorHandler):
        self.error_handler = error_handler
        self.recovery_actions = {
            ErrorCategory.STORAGE: self._recover_storage,
            ErrorCategory.DATABASE: self._recover_database,
            ErrorCategory.SYSTEM: self._recover_system
        }
        
    async def attempt_recovery(self, error: BackfillError) -> bool:
        """Attempt automated recovery for error"""
        recovery_func = self.recovery_actions.get(error.category)
        
        if recovery_func:
            try:
                return await recovery_func(error)
            except Exception as e:
                logging.error(f"Recovery failed: {e}")
                return False
                
        return False
        
    async def _recover_storage(self, error: BackfillError) -> bool:
        """Recover from storage errors"""
        if "space" in error.error_message.lower():
            # Clean up temporary files
            logging.info("Cleaning up temporary files...")
            await self._cleanup_temp_files()
            return True
            
        return False
        
    async def _recover_database(self, error: BackfillError) -> bool:
        """Recover from database errors"""
        if "connection" in error.error_message.lower():
            # Reset connection pool
            logging.info("Resetting database connections...")
            # Implementation depends on your database pool
            return True
            
        if "lock" in error.error_message.lower():
            # Kill blocking queries
            logging.info("Checking for blocking queries...")
            # Implementation depends on your database
            return True
            
        return False
        
    async def _cleanup_temp_files(self):
        """Clean up temporary download files"""
        temp_dir = Path("/tmp/backfill_downloads")
        if temp_dir.exists():
            for file in temp_dir.glob("*.tmp"):
                if datetime.utcnow() - datetime.fromtimestamp(file.stat().st_mtime) > timedelta(hours=1):
                    file.unlink()
```

### 6. Error Monitoring and Alerting
```python
class ErrorMonitor:
    """Monitor errors and send alerts"""
    
    def __init__(
        self,
        alert_threshold: int = 10,
        alert_window: int = 300  # 5 minutes
    ):
        self.alert_threshold = alert_threshold
        self.alert_window = alert_window
        self.error_counts: Dict[ErrorCategory, List[datetime]] = {}
        self.alerts_sent: Dict[str, datetime] = {}
        
    async def record_error(self, error: BackfillError):
        """Record error and check if alert needed"""
        category = error.category
        
        # Initialize category if needed
        if category not in self.error_counts:
            self.error_counts[category] = []
            
        # Add error timestamp
        self.error_counts[category].append(error.timestamp)
        
        # Clean old entries
        cutoff = datetime.utcnow() - timedelta(seconds=self.alert_window)
        self.error_counts[category] = [
            ts for ts in self.error_counts[category] if ts > cutoff
        ]
        
        # Check if alert needed
        if len(self.error_counts[category]) >= self.alert_threshold:
            await self._send_alert(category, error)
            
    async def _send_alert(self, category: ErrorCategory, error: BackfillError):
        """Send alert for error category"""
        alert_key = f"{category.value}_alert"
        
        # Check if we already sent recent alert
        if alert_key in self.alerts_sent:
            last_alert = self.alerts_sent[alert_key]
            if datetime.utcnow() - last_alert < timedelta(minutes=30):
                return  # Don't spam alerts
                
        # Send alert (implement your notification method)
        message = f"""
        Backfill Error Alert
        Category: {category.value}
        Severity: {error.severity.value}
        Count: {len(self.error_counts[category])} errors in last {self.alert_window}s
        Latest Error: {error.error_message}
        """
        
        logging.critical(message)
        # await send_email_alert(message)
        # await send_slack_alert(message)
        
        self.alerts_sent[alert_key] = datetime.utcnow()
```

## Integration Example

```python
async def robust_backfill_operation():
    """Example of using error handling in backfill"""
    
    # Initialize error handling
    error_handler = ErrorHandler()
    retry_manager = RetryManager()
    error_monitor = ErrorMonitor()
    recovery = RecoveryAction(error_handler)
    
    # Create circuit breakers for critical operations
    error_handler.circuit_breakers['s3_download'] = CircuitBreaker(
        failure_threshold=10,
        recovery_timeout=120
    )
    
    @error_handler.with_error_handling(
        category=ErrorCategory.NETWORK,
        severity=ErrorSeverity.MEDIUM
    )
    async def download_file(s3_key: str, local_path: Path):
        """Download with automatic error handling"""
        # Your download logic here
        pass
        
    @error_handler.with_error_handling(
        category=ErrorCategory.DATABASE,
        severity=ErrorSeverity.HIGH
    )
    async def insert_batch(records: List[Dict]):
        """Insert with automatic error handling"""
        # Your insert logic here
        pass
        
    # Use retry manager for complex operations
    async def complex_operation():
        result = await retry_manager.retry_with_backoff(
            lambda: download_and_process_symbol("AAPL"),
            max_attempts=5,
            error_category=ErrorCategory.NETWORK,
            on_retry=lambda attempt, delay, error: 
                logging.info(f"Retrying attempt {attempt} after {delay}s: {error}")
        )
        return result
```

## Best Practices

1. **Categorize Errors**: Properly categorize errors for appropriate handling
2. **Circuit Breakers**: Use circuit breakers to prevent cascading failures
3. **Exponential Backoff**: Use exponential backoff with jitter for retries
4. **Monitor Patterns**: Track error patterns to identify systemic issues
5. **Automated Recovery**: Implement automated recovery for common issues
6. **Alert Wisely**: Alert on patterns, not individual errors
7. **Log Everything**: Maintain detailed error logs for debugging