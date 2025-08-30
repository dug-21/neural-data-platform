"""
Rate Limiter for Trading APIs
Ensures compliance with API rate limits to prevent blocking
"""

import time
import asyncio
from datetime import datetime, timedelta
from collections import deque
from typing import Dict, Optional, Callable, Any
from functools import wraps
import logging

logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class RateLimiter:
    """
    Generic rate limiter for API calls
    Supports both per-minute and per-day limits
    """
    
    def __init__(self, 
                 calls_per_minute: Optional[int] = None,
                 calls_per_day: Optional[int] = None,
                 name: str = "API"):
        self.name = name
        self.calls_per_minute = calls_per_minute
        self.calls_per_day = calls_per_day
        
        # Track call timestamps
        self.minute_calls = deque()
        self.day_calls = deque()
        
        # Lock for thread safety
        self._lock = asyncio.Lock() if asyncio.get_event_loop().is_running() else None
    
    def _clean_old_calls(self):
        """Remove calls older than the tracking window"""
        now = datetime.now()
        
        # Clean minute window
        if self.calls_per_minute:
            minute_ago = now - timedelta(minutes=1)
            while self.minute_calls and self.minute_calls[0] < minute_ago:
                self.minute_calls.popleft()
        
        # Clean day window
        if self.calls_per_day:
            day_ago = now - timedelta(days=1)
            while self.day_calls and self.day_calls[0] < day_ago:
                self.day_calls.popleft()
    
    def can_make_request(self) -> tuple[bool, Optional[float]]:
        """
        Check if a request can be made
        Returns: (can_request, wait_time_seconds)
        """
        self._clean_old_calls()
        now = datetime.now()
        
        # Check minute limit
        if self.calls_per_minute and len(self.minute_calls) >= self.calls_per_minute:
            # Calculate wait time until oldest call expires
            wait_time = (self.minute_calls[0] + timedelta(minutes=1) - now).total_seconds()
            return False, max(0, wait_time)
        
        # Check daily limit
        if self.calls_per_day and len(self.day_calls) >= self.calls_per_day:
            # Calculate wait time until oldest call expires
            wait_time = (self.day_calls[0] + timedelta(days=1) - now).total_seconds()
            return False, max(0, wait_time)
        
        return True, None
    
    def record_request(self):
        """Record that a request was made"""
        now = datetime.now()
        
        if self.calls_per_minute:
            self.minute_calls.append(now)
        if self.calls_per_day:
            self.day_calls.append(now)
    
    async def wait_if_needed(self):
        """Wait if rate limit would be exceeded"""
        can_request, wait_time = self.can_make_request()
        
        if not can_request and wait_time:
            logger.info(f"{self.name}: Rate limit reached. Waiting {wait_time:.2f} seconds...")
            await asyncio.sleep(wait_time)
    
    def get_remaining_calls(self) -> Dict[str, int]:
        """Get remaining calls for each time window"""
        self._clean_old_calls()
        
        remaining = {}
        if self.calls_per_minute:
            remaining['per_minute'] = max(0, self.calls_per_minute - len(self.minute_calls))
        if self.calls_per_day:
            remaining['per_day'] = max(0, self.calls_per_day - len(self.day_calls))
        
        return remaining


class APIRateLimiters:
    """Pre-configured rate limiters for common trading APIs"""
    
    @staticmethod
    def alpha_vantage_free():
        """Alpha Vantage free tier: 5 requests/minute, 500 requests/day"""
        return RateLimiter(
            calls_per_minute=5,
            calls_per_day=500,
            name="AlphaVantage"
        )
    
    @staticmethod
    def alpha_vantage_conservative():
        """Conservative limits for Alpha Vantage (25/day as per some docs)"""
        return RateLimiter(
            calls_per_minute=5,
            calls_per_day=25,
            name="AlphaVantage-Conservative"
        )
    
    @staticmethod
    def polygon_free():
        """Polygon.io free tier: 5 requests/minute"""
        return RateLimiter(
            calls_per_minute=5,
            name="Polygon"
        )
    
    @staticmethod
    def polygon_unlimited():
        """Polygon.io paid tier: unlimited, but let's be reasonable"""
        return RateLimiter(
            calls_per_minute=300,  # 5 per second
            name="Polygon-Premium"
        )


def rate_limited(limiter: RateLimiter):
    """Decorator to apply rate limiting to functions"""
    def decorator(func: Callable) -> Callable:
        @wraps(func)
        async def async_wrapper(*args, **kwargs):
            await limiter.wait_if_needed()
            limiter.record_request()
            return await func(*args, **kwargs)
        
        @wraps(func)
        def sync_wrapper(*args, **kwargs):
            can_request, wait_time = limiter.can_make_request()
            if not can_request and wait_time:
                logger.info(f"{limiter.name}: Rate limit reached. Waiting {wait_time:.2f} seconds...")
                time.sleep(wait_time)
            limiter.record_request()
            return func(*args, **kwargs)
        
        # Return appropriate wrapper based on function type
        if asyncio.iscoroutinefunction(func):
            return async_wrapper
        else:
            return sync_wrapper
    
    return decorator


# Example usage
class TradingDataClient:
    """Example client with rate limiting"""
    
    def __init__(self, api_type: str = "polygon_free"):
        # Get appropriate rate limiter
        if api_type == "alpha_vantage_free":
            self.limiter = APIRateLimiters.alpha_vantage_free()
        elif api_type == "polygon_free":
            self.limiter = APIRateLimiters.polygon_free()
        else:
            self.limiter = APIRateLimiters.polygon_unlimited()
    
    @rate_limited(APIRateLimiters.polygon_free())
    async def get_quote(self, symbol: str) -> Dict[str, Any]:
        """Get quote with automatic rate limiting"""
        # Simulate API call
        logger.info(f"Fetching quote for {symbol}")
        await asyncio.sleep(0.1)  # Simulate network delay
        return {"symbol": symbol, "price": 150.00}
    
    @rate_limited(APIRateLimiters.alpha_vantage_free())
    def get_historical_data(self, symbol: str) -> Dict[str, Any]:
        """Get historical data with rate limiting"""
        logger.info(f"Fetching historical data for {symbol}")
        time.sleep(0.1)  # Simulate network delay
        return {"symbol": symbol, "data": "historical"}


# Bulk request handler with rate limiting
class BulkRequestHandler:
    """Handle bulk requests with rate limiting and retries"""
    
    def __init__(self, rate_limiter: RateLimiter, max_retries: int = 3):
        self.limiter = rate_limiter
        self.max_retries = max_retries
    
    async def process_requests(self, 
                             requests: list,
                             request_func: Callable,
                             batch_size: Optional[int] = None) -> list:
        """
        Process multiple requests with rate limiting
        
        Args:
            requests: List of request parameters
            request_func: Async function to call for each request
            batch_size: Optional batch size for grouping requests
        
        Returns:
            List of results
        """
        results = []
        errors = []
        
        # Process in batches if specified
        if batch_size:
            for i in range(0, len(requests), batch_size):
                batch = requests[i:i + batch_size]
                batch_results = await asyncio.gather(
                    *[self._process_single_request(req, request_func) for req in batch],
                    return_exceptions=True
                )
                results.extend(batch_results)
        else:
            # Process one by one with rate limiting
            for req in requests:
                result = await self._process_single_request(req, request_func)
                results.append(result)
        
        return results
    
    async def _process_single_request(self, request_params: Any, request_func: Callable) -> Any:
        """Process a single request with retries"""
        for attempt in range(self.max_retries):
            try:
                await self.limiter.wait_if_needed()
                self.limiter.record_request()
                
                # Make the actual request
                result = await request_func(request_params)
                return result
                
            except Exception as e:
                logger.warning(f"Request failed (attempt {attempt + 1}/{self.max_retries}): {e}")
                if attempt == self.max_retries - 1:
                    return {"error": str(e), "request": request_params}
                
                # Exponential backoff
                await asyncio.sleep(2 ** attempt)


# Usage example
async def example_usage():
    """Demonstrate rate limiter usage"""
    
    # Create a client with rate limiting
    client = TradingDataClient("polygon_free")
    
    # Single requests with automatic rate limiting
    symbols = ['AAPL', 'MSFT', 'GOOGL', 'TSLA', 'NVDA', 'AMD', 'META']
    
    print("Fetching quotes with rate limiting...")
    start_time = time.time()
    
    for symbol in symbols:
        quote = await client.get_quote(symbol)
        remaining = client.limiter.get_remaining_calls()
        print(f"Got {quote}, Remaining calls: {remaining}")
    
    elapsed = time.time() - start_time
    print(f"\nProcessed {len(symbols)} requests in {elapsed:.2f} seconds")
    
    # Bulk request example
    print("\n\nBulk request example:")
    bulk_handler = BulkRequestHandler(APIRateLimiters.polygon_free())
    
    async def fetch_data(symbol):
        # Simulate API call
        await asyncio.sleep(0.1)
        return {"symbol": symbol, "price": 100 + len(symbol)}
    
    results = await bulk_handler.process_requests(
        symbols,
        fetch_data,
        batch_size=2  # Process 2 at a time
    )
    
    print(f"Bulk results: {results}")


if __name__ == "__main__":
    # Run example
    asyncio.run(example_usage())